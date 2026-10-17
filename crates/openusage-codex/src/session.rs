//! An isolated, one-shot probe. This module never opens an existing provider home.

use crate::{AccountSnapshot, ProtocolError, ServerInfo};
use std::{fmt, path::Path};

pub const TESTED_PROVIDER_VERSION: &str = "0.163.0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionError {
    UnsupportedPlatform,
    InvalidExecutable,
    InvalidManagedDirectory,
    UnsupportedVersion,
    StartFailed,
    TransportFailed,
    TimedOut,
    UnexpectedExit,
    NotificationLimit,
    TerminationFailed,
    CleanupFailed,
    Protocol(ProtocolError),
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedPlatform => {
                "The isolated provider probe is unavailable on this platform."
            }
            Self::InvalidExecutable => "Approve a regular absolute provider executable.",
            Self::InvalidManagedDirectory => {
                "Use a private managed directory for the provider probe."
            }
            Self::UnsupportedVersion => {
                "The provider executable version does not match the tested version."
            }
            Self::StartFailed => "The isolated provider process could not start.",
            Self::TransportFailed => "The provider transport failed.",
            Self::TimedOut => "The provider operation exceeded its time limit.",
            Self::UnexpectedExit => "The provider exited before its response completed.",
            Self::NotificationLimit => "The provider exceeded the notification limit.",
            Self::TerminationFailed => "The owned provider process could not be reaped.",
            Self::CleanupFailed => "The temporary provider home could not be removed.",
            Self::Protocol(_) => "The provider response failed the protocol check.",
        })
    }
}

impl std::error::Error for SessionError {}

impl From<ProtocolError> for SessionError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

pub struct ProviderProbe {
    pub server: ServerInfo,
    pub account: AccountSnapshot,
    pub version: &'static str,
    /// The owned child is terminated and reaped before this result returns.
    pub process_id: u32,
}

/// The Rust caller must approve the executable's source. A version string alone is not provenance.
/// This blocking probe belongs on a worker thread. It performs no login or inference.
pub fn probe_account(
    approved_executable: &Path,
    private_managed_parent: &Path,
) -> Result<ProviderProbe, SessionError> {
    #[cfg(unix)]
    {
        unix::probe(approved_executable, private_managed_parent)
    }
    #[cfg(not(unix))]
    {
        let _ = (approved_executable, private_managed_parent);
        Err(SessionError::UnsupportedPlatform)
    }
}

#[cfg(unix)]
mod unix {
    use super::*;
    use crate::{
        MAX_MESSAGE_BYTES, Request, initialized_notification, parse_account, parse_initialize,
    };
    use serde::Deserialize;
    use std::{
        fs,
        io::{BufRead, BufReader, Read, Write},
        os::unix::{fs::MetadataExt, process::CommandExt},
        path::PathBuf,
        process::{Child, ChildStdin, Command, Stdio},
        sync::mpsc::{self, Receiver},
        thread::JoinHandle,
        time::{Duration, Instant},
    };
    use zeroize::Zeroizing;

    const READY_TIMEOUT: Duration = Duration::from_secs(15);
    const READ_TIMEOUT: Duration = Duration::from_secs(30);
    const MAX_NOTIFICATIONS: usize = 32;
    const MAX_RESPONSE_BYTES: usize = 4 * MAX_MESSAGE_BYTES;
    const CONFIG_OVERRIDES: &[&str] = &[
        "cli_auth_credentials_store=\"file\"",
        "analytics.enabled=false",
        "feedback.enabled=false",
        "otel.exporter=\"none\"",
        "otel.metrics_exporter=\"none\"",
        "check_for_update_on_startup=false",
    ];

    struct Sandbox {
        root: tempfile::TempDir,
        codex_home: PathBuf,
        environment: Vec<(&'static str, PathBuf)>,
    }

    impl Sandbox {
        fn new(parent: &Path) -> Result<Self, SessionError> {
            if !absolute_normal_path(parent) {
                return Err(SessionError::InvalidManagedDirectory);
            }
            let metadata =
                fs::symlink_metadata(parent).map_err(|_| SessionError::InvalidManagedDirectory)?;
            // geteuid reads the current process identity without pointer arguments.
            if parent.canonicalize().ok().as_deref() != Some(parent)
                || !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || metadata.mode() & 0o077 != 0
                || metadata.uid() != unsafe { libc::geteuid() }
            {
                return Err(SessionError::InvalidManagedDirectory);
            }
            let root = tempfile::Builder::new()
                .prefix("openusage-provider-")
                .tempdir_in(parent)
                .map_err(|_| SessionError::InvalidManagedDirectory)?;
            let mut environment = Vec::new();
            for (name, leaf) in [
                ("HOME", "home"),
                ("CODEX_HOME", "codex"),
                ("XDG_CONFIG_HOME", "config"),
                ("XDG_DATA_HOME", "data"),
                ("XDG_CACHE_HOME", "cache"),
                ("XDG_STATE_HOME", "state"),
                ("XDG_RUNTIME_DIR", "runtime"),
                ("TMPDIR", "tmp"),
            ] {
                use std::os::unix::fs::DirBuilderExt;
                let path = root.path().join(leaf);
                fs::DirBuilder::new()
                    .mode(0o700)
                    .create(&path)
                    .map_err(|_| SessionError::InvalidManagedDirectory)?;
                environment.push((name, path));
            }
            Ok(Self {
                codex_home: root.path().join("codex"),
                root,
                environment,
            })
        }

        fn command(&self, executable: &Path) -> Command {
            let mut command = Command::new(executable);
            command
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("LANG", "C.UTF-8")
                .current_dir(self.root.path())
                .process_group(0);
            for (name, value) in &self.environment {
                command.env(name, value);
            }
            command
        }
    }

    type Line = Result<Zeroizing<Vec<u8>>, SessionError>;

    fn absolute_normal_path(path: &Path) -> bool {
        path.is_absolute()
            && !path.components().any(|part| {
                matches!(
                    part,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
    }

    struct OwnedProcess {
        child: Option<Child>,
        input: Option<ChildStdin>,
        lines: Option<Receiver<Line>>,
        reader: Option<JoinHandle<()>>,
    }

    impl OwnedProcess {
        fn start(mut command: Command) -> Result<Self, SessionError> {
            let child = command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|_| SessionError::StartFailed)?;
            let mut process = Self {
                child: Some(child),
                input: None,
                lines: None,
                reader: None,
            };
            let child = process.child.as_mut().ok_or(SessionError::StartFailed)?;
            process.input = child.stdin.take();
            let output = child.stdout.take().ok_or(SessionError::TransportFailed)?;
            let (sender, receiver) = mpsc::sync_channel(1);
            process.lines = Some(receiver);
            process.reader = Some(
                std::thread::Builder::new()
                    .name("openusage-provider-stdio".into())
                    .spawn(move || {
                        let mut reader = BufReader::new(output);
                        loop {
                            let line = read_line(&mut reader);
                            let finished = line.is_err();
                            if sender.send(line).is_err() || finished {
                                break;
                            }
                        }
                    })
                    .map_err(|_| SessionError::StartFailed)?,
            );
            Ok(process)
        }

        fn send(&mut self, bytes: &[u8]) -> Result<(), SessionError> {
            // Only the fixed, small initialize/initialized/account-read messages reach this writer.
            self.input
                .as_mut()
                .ok_or(SessionError::TransportFailed)?
                .write_all(bytes)
                .map_err(|_| SessionError::TransportFailed)
        }

        fn line(&self, deadline: Instant) -> Line {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(SessionError::TimedOut)?;
            self.lines
                .as_ref()
                .ok_or(SessionError::TransportFailed)?
                .recv_timeout(remaining)
                .map_err(|error| match error {
                    mpsc::RecvTimeoutError::Timeout => SessionError::TimedOut,
                    mpsc::RecvTimeoutError::Disconnected => SessionError::UnexpectedExit,
                })?
        }

        fn response(&self, id: u32, timeout: Duration) -> Line {
            let deadline = Instant::now() + timeout;
            let mut received = 0;
            for _ in 0..=MAX_NOTIFICATIONS {
                let bytes = self.line(deadline)?;
                received += bytes.len();
                if received > MAX_RESPONSE_BYTES {
                    return Err(SessionError::NotificationLimit);
                }
                #[derive(Deserialize)]
                struct Route {
                    #[serde(default, deserialize_with = "present_id")]
                    id: Option<u32>,
                    method: Option<String>,
                }
                fn present_id<'de, D: serde::Deserializer<'de>>(
                    deserializer: D,
                ) -> Result<Option<u32>, D::Error> {
                    u32::deserialize(deserializer).map(Some)
                }
                let route: Route =
                    serde_json::from_slice(&bytes).map_err(|_| ProtocolError::InvalidMessage)?;
                match (route.id, route.method.as_deref()) {
                    (Some(received), None) if received == id => return Ok(bytes),
                    (Some(_), _) => return Err(ProtocolError::MismatchedRequest.into()),
                    (None, Some(method)) => {
                        // Unrelated notifications carry no actions. Bound and discard them.
                        crate::validate_text(method, 128)?;
                    }
                    _ => return Err(ProtocolError::InvalidMessage.into()),
                }
            }
            Err(SessionError::NotificationLimit)
        }

        fn stop(&mut self) -> Result<(), SessionError> {
            self.input.take();
            self.lines.take();
            let mut child = match self.child.take() {
                Some(child) => child,
                None => return Ok(()),
            };
            // The child PID stays reserved until wait. Its dedicated group contains only owned descendants.
            let group_killed = unsafe { libc::killpg(child.id() as libc::pid_t, libc::SIGKILL) };
            let group_failed = group_killed != 0
                && std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH);
            let _ = child.kill();
            loop {
                match child.wait() {
                    Ok(_) => break,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => return Err(SessionError::TerminationFailed),
                }
            }
            if group_failed {
                return Err(SessionError::TerminationFailed);
            }
            if let Some(reader) = self.reader.take() {
                reader.join().map_err(|_| SessionError::TerminationFailed)?;
            }
            Ok(())
        }
    }

    impl Drop for OwnedProcess {
        fn drop(&mut self) {
            let _ = self.stop();
        }
    }

    fn read_line(reader: &mut impl BufRead) -> Line {
        let mut bytes = Zeroizing::new(Vec::new());
        reader
            .take((MAX_MESSAGE_BYTES + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .map_err(|_| SessionError::TransportFailed)?;
        if bytes.len() > MAX_MESSAGE_BYTES {
            return Err(ProtocolError::OversizedMessage.into());
        }
        if bytes.is_empty() {
            return Err(SessionError::UnexpectedExit);
        }
        if bytes.last() != Some(&b'\n') {
            return Err(ProtocolError::InvalidMessage.into());
        }
        Ok(bytes)
    }

    pub(super) fn probe(executable: &Path, parent: &Path) -> Result<ProviderProbe, SessionError> {
        if !absolute_normal_path(executable) {
            return Err(SessionError::InvalidExecutable);
        }
        if !absolute_normal_path(parent) {
            return Err(SessionError::InvalidManagedDirectory);
        }
        let metadata =
            fs::symlink_metadata(executable).map_err(|_| SessionError::InvalidExecutable)?;
        if executable.canonicalize().ok().as_deref() != Some(executable)
            || !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.mode() & 0o111 == 0
            || metadata.mode() & 0o022 != 0
        {
            return Err(SessionError::InvalidExecutable);
        }
        let sandbox = Sandbox::new(parent)?;
        let result = probe_in_sandbox(executable, &sandbox);
        sandbox
            .root
            .close()
            .map_err(|_| SessionError::CleanupFailed)?;
        result
    }

    fn probe_in_sandbox(
        executable: &Path,
        sandbox: &Sandbox,
    ) -> Result<ProviderProbe, SessionError> {
        let mut version = sandbox.command(executable);
        version.arg("--version");
        let mut version = OwnedProcess::start(version)?;
        let version_result = version.line(Instant::now() + READY_TIMEOUT);
        version.stop()?;
        if version_result?.as_slice() != b"codex-cli 0.163.0\n" {
            return Err(SessionError::UnsupportedVersion);
        }
        let mut command = sandbox.command(executable);
        command.args(["app-server", "--listen", "stdio://"]);
        for setting in CONFIG_OVERRIDES {
            command.args(["-c", setting]);
        }
        let mut process = OwnedProcess::start(command)?;
        let process_id = process
            .child
            .as_ref()
            .ok_or(SessionError::StartFailed)?
            .id();
        let result = (|| {
            process.send(&Request::initialize(1).to_line()?)?;
            let server =
                parse_initialize(&process.response(1, READY_TIMEOUT)?, 1, &sandbox.codex_home)?;
            process.send(initialized_notification())?;
            process.send(&Request::account_read(2).to_line()?)?;
            let account = parse_account(&process.response(2, READ_TIMEOUT)?, 2)?;
            Ok(ProviderProbe {
                server,
                account,
                version: TESTED_PROVIDER_VERSION,
                process_id,
            })
        })();
        process.stop()?;
        result
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::io::Cursor;

        #[test]
        fn synthetic_reader_rejects_oversized_and_unterminated_frames() {
            assert_eq!(
                read_line(&mut Cursor::new(vec![b'x'; MAX_MESSAGE_BYTES + 1])),
                Err(ProtocolError::OversizedMessage.into())
            );
            assert_eq!(
                read_line(&mut Cursor::new(b"{\"id\":1}")),
                Err(ProtocolError::InvalidMessage.into())
            );
        }

        #[test]
        fn synthetic_routes_reject_mismatched_ids_notification_floods_and_expired_deadlines() {
            let (sender, receiver) = mpsc::channel();
            sender
                .send(Ok(Zeroizing::new(b"{\"id\":9,\"result\":{}}\n".to_vec())))
                .unwrap();
            let mut process = OwnedProcess {
                child: None,
                input: None,
                lines: Some(receiver),
                reader: None,
            };
            assert_eq!(
                process.response(1, READY_TIMEOUT),
                Err(ProtocolError::MismatchedRequest.into())
            );
            assert_eq!(
                process.line(Instant::now() - Duration::from_secs(1)),
                Err(SessionError::TimedOut)
            );
            let (sender, receiver) = mpsc::channel();
            for _ in 0..=MAX_NOTIFICATIONS {
                sender
                    .send(Ok(Zeroizing::new(
                        b"{\"method\":\"account/updated\",\"params\":{}}\n".to_vec(),
                    )))
                    .unwrap();
            }
            process.lines = Some(receiver);
            assert_eq!(
                process.response(1, READY_TIMEOUT),
                Err(SessionError::NotificationLimit)
            );
        }

        #[test]
        fn synthetic_unrelated_notification_does_not_replace_the_pending_response() {
            let (sender, receiver) = mpsc::channel();
            sender
                .send(Ok(Zeroizing::new(
                    b"{\"method\":\"remoteControl/status/changed\",\"params\":{},\"emittedAtMs\":0}\n".to_vec(),
                )))
                .unwrap();
            let response =
                b"{\"id\":2,\"result\":{\"account\":null,\"requiresOpenaiAuth\":true}}\n";
            sender.send(Ok(Zeroizing::new(response.to_vec()))).unwrap();
            let process = OwnedProcess {
                child: None,
                input: None,
                lines: Some(receiver),
                reader: None,
            };
            assert_eq!(
                process.response(2, READY_TIMEOUT).unwrap().as_slice(),
                response
            );
        }

        #[test]
        fn synthetic_child_timeout_terminates_and_reaps_the_owned_process_group() {
            let mut command = Command::new("/bin/sleep");
            command.arg("60").env_clear().process_group(0);
            let process = OwnedProcess::start(command).unwrap();
            let id = process.child.as_ref().unwrap().id() as libc::pid_t;
            assert_eq!(
                process.line(Instant::now() + Duration::from_millis(20)),
                Err(SessionError::TimedOut)
            );
            drop(process);
            // kill with signal0 only checks process/group existence and sends no signal.
            assert_eq!(unsafe { libc::kill(id, 0) }, -1);
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::ESRCH)
            );
            assert_eq!(unsafe { libc::killpg(id, 0) }, -1);
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::ESRCH)
            );
        }

        #[test]
        fn synthetic_cleanup_fault_is_reported_after_reaping_the_child() {
            use std::os::unix::fs::PermissionsExt;

            let temporary = tempfile::tempdir().unwrap();
            let parent = temporary.path().join("managed");
            fs::create_dir(&parent).unwrap();
            fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
            let parent = parent.canonicalize().unwrap();
            let executable = temporary.path().join("synthetic-provider");
            fs::write(&executable, r##"#!/bin/sh
if [ "$1" = "--version" ]; then
    printf 'codex-cli 0.163.0\n'
    exit 0
fi
IFS= read -r request
printf '{"id":1,"result":{"userAgent":"synthetic cleanup fixture","codexHome":"%s","platformFamily":"unix","platformOs":"linux"}}\n' "$CODEX_HOME"
IFS= read -r notification
IFS= read -r request
printf '%s' "$$" > "$CODEX_HOME/synthetic-child-pid"
owned_root=${CODEX_HOME%/codex}
mv "$owned_root" "$owned_root-moved"
printf '{"id":2,"result":{"account":null,"requiresOpenaiAuth":true}}\n'
exec /bin/sleep 60
"##).unwrap();
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
            let executable = executable.canonicalize().unwrap();
            assert!(matches!(
                probe(&executable, &parent),
                Err(SessionError::CleanupFailed)
            ));
            let entries: Vec<_> = fs::read_dir(&parent).unwrap().collect();
            assert_eq!(entries.len(), 1);
            let moved_home = entries[0].as_ref().unwrap().path().join("codex");
            let pid: libc::pid_t = fs::read_to_string(moved_home.join("synthetic-child-pid"))
                .unwrap()
                .parse()
                .unwrap();
            // Signal0 checks only this fixture's recorded child identity.
            assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::ESRCH)
            );
        }
    }
}
