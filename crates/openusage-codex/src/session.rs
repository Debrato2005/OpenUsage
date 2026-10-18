//! Bounded provider operations in isolated, explicitly selected managed homes.

use crate::{AccountSnapshot, ProtocolError, ServerInfo};
use std::{fmt, path::Path};

pub const TESTED_PROVIDER_VERSION: &str = "0.163.0";
/// OpenUsage's local limit for a pending login. This is not provider-reported expiry.
pub const DEVICE_LOGIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15 * 60);

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
    Cancelled,
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
            Self::Cancelled => "The provider check was cancelled.",
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

/// Provisional full-read diagnostics only. No identity key, email, or account claim is exposed.
pub struct QuotaProbe {
    /// Local full-response observation time, retained across delayed completion/publication.
    pub observed_at: std::time::SystemTime,
    pub server: ServerInfo,
    pub quota: crate::QuotaSnapshot,
    pub version: &'static str,
    pub process_id: u32,
}

impl fmt::Debug for QuotaProbe {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuotaProbe")
            .field("version", &self.version)
            .field("quota", &self.quota)
            .finish()
    }
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
pub(crate) mod unix {
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
        process::{Child, ChildStdin, ChildStdout, Command, Stdio},
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc::{self, Receiver},
        },
        thread::JoinHandle,
        time::{Duration, Instant},
    };
    use zeroize::Zeroizing;

    const READY_TIMEOUT: Duration = Duration::from_secs(15);
    const READ_TIMEOUT: Duration = Duration::from_secs(30);
    const CANCEL_TIMEOUT: Duration = Duration::from_secs(1);

    #[derive(Clone, Copy)]
    pub(crate) struct Timeouts {
        pub ready: Duration,
        pub read: Duration,
        pub login: Duration,
    }

    impl Default for Timeouts {
        fn default() -> Self {
            Self {
                ready: READY_TIMEOUT,
                read: READ_TIMEOUT,
                login: DEVICE_LOGIN_TIMEOUT,
            }
        }
    }
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
            Self::with_home(parent, None)
        }

        fn with_home(parent: &Path, managed_home: Option<&Path>) -> Result<Self, SessionError> {
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
                if name == "CODEX_HOME"
                    && let Some(home) = managed_home
                {
                    environment.push((name, home.to_owned()));
                    continue;
                }
                let path = root.path().join(leaf);
                fs::DirBuilder::new()
                    .mode(0o700)
                    .create(&path)
                    .map_err(|_| SessionError::InvalidManagedDirectory)?;
                environment.push((name, path));
            }
            Ok(Self {
                codex_home: managed_home
                    .map(Path::to_owned)
                    .unwrap_or_else(|| root.path().join("codex")),
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

    #[derive(Default)]
    pub(crate) struct ProcessControl {
        child: Mutex<Option<Child>>,
        cancelled: AtomicBool,
    }

    impl ProcessControl {
        pub(crate) fn request_cancel(&self) {
            self.cancelled.store(true, Ordering::Release);
        }

        pub(crate) fn check_cancelled(&self) -> Result<(), SessionError> {
            if self.cancelled.load(Ordering::Acquire) {
                Err(SessionError::Cancelled)
            } else {
                Ok(())
            }
        }

        pub(crate) fn has_child(&self) -> bool {
            self.child
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .is_some()
        }

        pub(crate) fn stop_child(&self) -> Result<(), SessionError> {
            let mut owned = self
                .child
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(child) = owned.as_mut() else {
                return Ok(());
            };
            // The live Child reserves its PID until wait completes.
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
            owned.take();
            if group_failed {
                Err(SessionError::TerminationFailed)
            } else {
                Ok(())
            }
        }
    }

    pub(crate) fn absolute_normal_path(path: &Path) -> bool {
        path.is_absolute()
            && !path.components().any(|part| {
                matches!(
                    part,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
    }

    struct OwnedProcess {
        control: Arc<ProcessControl>,
        input: Option<ChildStdin>,
        lines: Option<Receiver<Line>>,
        reader: Option<JoinHandle<()>>,
        reader_stop: Arc<AtomicBool>,
    }

    impl OwnedProcess {
        #[cfg(test)]
        fn start(mut command: Command) -> Result<Self, SessionError> {
            Self::start_controlled(&mut command, Arc::default())
        }

        fn start_controlled(
            command: &mut Command,
            control: Arc<ProcessControl>,
        ) -> Result<Self, SessionError> {
            control.check_cancelled()?;
            let mut child = command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|_| SessionError::StartFailed)?;
            let input = child.stdin.take();
            let output = child.stdout.take();
            *control
                .child
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(child);
            let mut process = Self {
                control,
                input,
                lines: None,
                reader: None,
                reader_stop: Arc::default(),
            };
            process.control.check_cancelled()?;
            let output = output.ok_or(SessionError::TransportFailed)?;
            use std::os::fd::AsRawFd;
            // The pipe descriptor stays alive through both fcntl calls.
            let flags = unsafe { libc::fcntl(output.as_raw_fd(), libc::F_GETFL) };
            if flags < 0
                || unsafe {
                    libc::fcntl(output.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK)
                } < 0
            {
                return Err(SessionError::TransportFailed);
            }
            let (sender, receiver) = mpsc::sync_channel(1);
            process.lines = Some(receiver);
            let reader_stop = Arc::clone(&process.reader_stop);
            process.reader = Some(
                std::thread::Builder::new()
                    .name("openusage-provider-stdio".into())
                    .spawn(move || {
                        let mut reader = BufReader::new(output);
                        loop {
                            let line = read_live_line(&mut reader, &reader_stop);
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
            self.control.check_cancelled()?;
            self.send_during_cleanup(bytes)
        }

        fn send_during_cleanup(&mut self, bytes: &[u8]) -> Result<(), SessionError> {
            // Only fixed requests and the bounded provider login ID reach this writer.
            self.input
                .as_mut()
                .ok_or(SessionError::TransportFailed)?
                .write_all(bytes)
                .map_err(|_| SessionError::TransportFailed)
        }

        fn line(&self, deadline: Instant) -> Line {
            self.line_with_cancel(deadline, true)
        }

        fn line_with_cancel(&self, deadline: Instant, cancellable: bool) -> Line {
            loop {
                if cancellable {
                    self.control.check_cancelled()?;
                }
                let remaining = deadline
                    .checked_duration_since(Instant::now())
                    .ok_or(SessionError::TimedOut)?;
                let result = self
                    .lines
                    .as_ref()
                    .ok_or(SessionError::TransportFailed)?
                    .recv_timeout(remaining.min(Duration::from_millis(100)));
                match result {
                    Ok(result) => {
                        if cancellable {
                            self.control.check_cancelled()?;
                        }
                        return result;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return Err(SessionError::UnexpectedExit);
                    }
                }
            }
        }

        fn response(&self, id: u32, timeout: Duration) -> Line {
            self.response_routed(id, Instant::now() + timeout, true, |_| Ok(()))
        }

        fn response_routed(
            &self,
            id: u32,
            deadline: Instant,
            cancellable: bool,
            mut notification: impl FnMut(Zeroizing<Vec<u8>>) -> Result<(), SessionError>,
        ) -> Line {
            let mut received = 0;
            for _ in 0..=MAX_NOTIFICATIONS {
                let bytes = self.line_with_cancel(deadline, cancellable)?;
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
                        crate::validate_text(method, 128)?;
                        notification(bytes)?;
                    }
                    _ => return Err(ProtocolError::InvalidMessage.into()),
                }
            }
            Err(SessionError::NotificationLimit)
        }

        fn stop(&mut self) -> Result<(), SessionError> {
            self.reader_stop.store(true, Ordering::Release);
            self.input.take();
            self.lines.take();
            let stopped = self.control.stop_child();
            let joined = self
                .reader
                .take()
                .map(|reader| reader.join().map_err(|_| SessionError::TerminationFailed))
                .unwrap_or(Ok(()));
            stopped.and(joined)
        }

        fn id(&self) -> Result<u32, SessionError> {
            self.control
                .child
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_ref()
                .map(Child::id)
                .ok_or(SessionError::UnexpectedExit)
        }
    }

    impl Drop for OwnedProcess {
        fn drop(&mut self) {
            let _ = self.stop();
        }
    }

    #[cfg(test)]
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

    fn read_live_line(reader: &mut BufReader<ChildStdout>, stop: &AtomicBool) -> Line {
        use std::os::fd::AsRawFd;
        let mut bytes = Zeroizing::new(Vec::new());
        loop {
            if stop.load(Ordering::Acquire) {
                return Err(SessionError::Cancelled);
            }
            let remaining = MAX_MESSAGE_BYTES + 1 - bytes.len();
            let read = reader
                .by_ref()
                .take(remaining as u64)
                .read_until(b'\n', &mut bytes);
            if bytes.len() > MAX_MESSAGE_BYTES {
                return Err(ProtocolError::OversizedMessage.into());
            }
            if bytes.last() == Some(&b'\n') {
                return Ok(bytes);
            }
            match read {
                Ok(_) => {
                    return Err(if bytes.is_empty() {
                        SessionError::UnexpectedExit
                    } else {
                        ProtocolError::InvalidMessage.into()
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    let mut descriptor = libc::pollfd {
                        fd: reader.get_ref().as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    // poll borrows one valid descriptor and permits periodic stop checks.
                    if unsafe { libc::poll(&mut descriptor, 1, 100) } < 0
                        && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
                    {
                        return Err(SessionError::TransportFailed);
                    }
                }
                Err(_) => return Err(SessionError::TransportFailed),
            }
        }
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
        run_session(
            executable,
            sandbox,
            Arc::default(),
            Timeouts::default(),
            || Ok(()),
        )
    }

    pub(crate) fn check_managed(
        executable: &Path,
        home: &Path,
        control: Arc<ProcessControl>,
        timeouts: Timeouts,
        before_start: impl Fn() -> Result<(), SessionError>,
    ) -> Result<ProviderProbe, SessionError> {
        control.check_cancelled()?;
        let sandbox = Sandbox::with_home(home, Some(home))?;
        let result = run_session(executable, &sandbox, control, timeouts, before_start);
        sandbox
            .root
            .close()
            .map_err(|_| SessionError::CleanupFailed)?;
        result
    }

    pub(crate) fn quota_managed(
        executable: &Path,
        home: &Path,
        control: Arc<ProcessControl>,
        timeouts: Timeouts,
        before_start: impl Fn() -> Result<(), SessionError>,
    ) -> Result<QuotaProbe, SessionError> {
        control.check_cancelled()?;
        let sandbox = Sandbox::with_home(home, Some(home))?;
        let result = run_quota(executable, &sandbox, control, timeouts, before_start);
        sandbox
            .root
            .close()
            .map_err(|_| SessionError::CleanupFailed)?;
        result
    }

    pub(crate) fn login_managed(
        executable: &Path,
        home: &Path,
        control: Arc<ProcessControl>,
        timeouts: Timeouts,
        before_start: impl Fn() -> Result<(), SessionError>,
        prompt: impl FnOnce(crate::DeviceLoginPrompt) -> Result<(), SessionError>,
    ) -> Result<ProviderProbe, SessionError> {
        control.check_cancelled()?;
        let sandbox = Sandbox::with_home(home, Some(home))?;
        let result = run_device_login(
            executable,
            &sandbox,
            control,
            timeouts,
            before_start,
            prompt,
        );
        sandbox
            .root
            .close()
            .map_err(|_| SessionError::CleanupFailed)?;
        result
    }

    fn run_session(
        executable: &Path,
        sandbox: &Sandbox,
        control: Arc<ProcessControl>,
        timeouts: Timeouts,
        before_start: impl Fn() -> Result<(), SessionError>,
    ) -> Result<ProviderProbe, SessionError> {
        let (mut process, server) =
            start_initialized(executable, sandbox, control, timeouts, before_start)?;
        let process_id = process.id()?;
        let result = (|| {
            process.send(&Request::account_read(2).to_line()?)?;
            let account = parse_account(&process.response(2, timeouts.read)?, 2)?;
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

    fn run_quota(
        executable: &Path,
        sandbox: &Sandbox,
        control: Arc<ProcessControl>,
        timeouts: Timeouts,
        before_start: impl Fn() -> Result<(), SessionError>,
    ) -> Result<QuotaProbe, SessionError> {
        let (mut process, server) =
            start_initialized(executable, sandbox, control, timeouts, before_start)?;
        let process_id = process.id()?;
        let result = (|| {
            process.send(&Request::account_read(2).to_line()?)?;
            let before = parse_account(
                &process.response_routed(
                    2,
                    Instant::now() + timeouts.read,
                    true,
                    quota_notification,
                )?,
                2,
            )?;
            if before.authentication != crate::AccountAuth::Chatgpt || !before.requires_openai_auth
            {
                return Err(ProtocolError::UnsupportedAuthentication.into());
            }
            if before.identity != crate::IdentityConfidence::Provisional {
                return Err(ProtocolError::AccountIdentityUnavailable.into());
            }
            process.send(&Request::rate_limits_read(3).to_line()?)?;
            let quota = crate::parse_quota(
                &process.response_routed(
                    3,
                    Instant::now() + timeouts.read,
                    true,
                    quota_notification,
                )?,
                3,
            )?;
            let observed_at = std::time::SystemTime::now();
            process.send(&Request::account_read(4).to_line()?)?;
            let after = parse_account(
                &process.response_routed(
                    4,
                    Instant::now() + timeouts.read,
                    true,
                    quota_notification,
                )?,
                4,
            )?;
            // Descriptive equality detects changes; it never establishes a stable identity key.
            if before.authentication != after.authentication
                || before.identity != after.identity
                || before.requires_openai_auth != after.requires_openai_auth
                || before.email != after.email
                || before.plan != after.plan
            {
                return Err(ProtocolError::AccountChanged.into());
            }
            Ok(QuotaProbe {
                observed_at,
                server,
                quota,
                version: TESTED_PROVIDER_VERSION,
                process_id,
            })
        })();
        process.stop()?;
        result
    }

    fn quota_notification(bytes: Zeroizing<Vec<u8>>) -> Result<(), SessionError> {
        #[derive(Deserialize)]
        struct Notice {
            method: String,
        }
        let notice: Notice =
            serde_json::from_slice(&bytes).map_err(|_| ProtocolError::InvalidMessage)?;
        if notice.method == "account/updated" {
            return Err(ProtocolError::AccountChanged.into());
        }
        // Sparse quota events are never merged into this full-read diagnostic or credit/reset state.
        Ok(())
    }

    fn start_initialized(
        executable: &Path,
        sandbox: &Sandbox,
        control: Arc<ProcessControl>,
        timeouts: Timeouts,
        before_start: impl Fn() -> Result<(), SessionError>,
    ) -> Result<(OwnedProcess, ServerInfo), SessionError> {
        control.check_cancelled()?;
        before_start()?;
        let mut version = sandbox.command(executable);
        version.arg("--version");
        let mut version = OwnedProcess::start_controlled(&mut version, Arc::clone(&control))?;
        let version_result = version.line(Instant::now() + timeouts.ready);
        version.stop()?;
        // Its Drop shares ProcessControl. Finish it before that control owns the app-server child.
        drop(version);
        if version_result?.as_slice() != b"codex-cli 0.163.0\n" {
            return Err(SessionError::UnsupportedVersion);
        }
        control.check_cancelled()?;
        before_start()?;
        let mut command = sandbox.command(executable);
        command.args(["app-server", "--listen", "stdio://"]);
        for setting in CONFIG_OVERRIDES {
            command.args(["-c", setting]);
        }
        let mut process = OwnedProcess::start_controlled(&mut command, control)?;
        process.send(&Request::initialize(1).to_line()?)?;
        let server = parse_initialize(
            &process.response(1, timeouts.ready)?,
            1,
            &sandbox.codex_home,
        )?;
        process.send(initialized_notification())?;
        Ok((process, server))
    }

    #[derive(Default)]
    struct LoginNotifications {
        frames: std::collections::VecDeque<Zeroizing<Vec<u8>>>,
        count: usize,
        bytes: usize,
    }

    impl LoginNotifications {
        fn retain(&mut self, bytes: Zeroizing<Vec<u8>>) -> Result<(), SessionError> {
            self.count += 1;
            self.bytes += bytes.len();
            if self.count > MAX_NOTIFICATIONS || self.bytes > MAX_RESPONSE_BYTES {
                return Err(SessionError::NotificationLimit);
            }
            #[derive(Deserialize)]
            struct Notice {
                method: String,
            }
            let notice: Notice =
                serde_json::from_slice(&bytes).map_err(|_| ProtocolError::InvalidMessage)?;
            crate::validate_text(&notice.method, 128)?;
            if notice.method == "account/login/completed" {
                self.frames.push_back(bytes);
            }
            Ok(())
        }
    }

    fn run_device_login(
        executable: &Path,
        sandbox: &Sandbox,
        control: Arc<ProcessControl>,
        timeouts: Timeouts,
        before_start: impl Fn() -> Result<(), SessionError>,
        prompt: impl FnOnce(crate::DeviceLoginPrompt) -> Result<(), SessionError>,
    ) -> Result<ProviderProbe, SessionError> {
        use crate::{LoginFlow, LoginState, parse_login_start};
        let (mut process, server) =
            start_initialized(executable, sandbox, control, timeouts, before_start)?;
        let process_id = process.id()?;
        let mut attempt = None;
        let result = (|| {
            let mut notifications = LoginNotifications::default();
            process.send(&Request::login_start(3, LoginFlow::DeviceCode).to_line()?)?;
            let response =
                process.response_routed(3, Instant::now() + timeouts.read, true, |bytes| {
                    notifications.retain(bytes)
                })?;
            let (pending, metadata) =
                parse_login_start(&response, 3, LoginFlow::DeviceCode)?.into_device()?;
            attempt = Some(pending);
            prompt(metadata)?;
            let deadline = Instant::now() + timeouts.login;
            let login = attempt.as_mut().ok_or(ProtocolError::InvalidMessage)?;
            loop {
                process.control.check_cancelled()?;
                if Instant::now() >= deadline {
                    return Err(SessionError::TimedOut);
                }
                if let Some(bytes) = notifications.frames.pop_front() {
                    match login.apply_completion(&bytes)? {
                        LoginState::NeedsAccountRead => break,
                        _ => return Err(ProtocolError::ProviderFailure.into()),
                    }
                }
                let bytes = process.line(deadline)?;
                // Notifications must not impersonate replies or server requests.
                #[derive(Deserialize)]
                struct Route {
                    #[serde(default, deserialize_with = "has_id")]
                    id: bool,
                }
                fn has_id<'de, D: serde::Deserializer<'de>>(
                    deserializer: D,
                ) -> Result<bool, D::Error> {
                    serde::de::IgnoredAny::deserialize(deserializer).map(|_| true)
                }
                let route: Route =
                    serde_json::from_slice(&bytes).map_err(|_| ProtocolError::InvalidMessage)?;
                if route.id {
                    return Err(ProtocolError::MismatchedRequest.into());
                }
                notifications.retain(bytes)?;
            }
            process.send(&Request::account_read(4).to_line()?)?;
            let account = parse_account(&process.response(4, timeouts.read)?, 4)?;
            if account.authentication != crate::AccountAuth::Chatgpt {
                return Err(ProtocolError::UnsupportedAuthentication.into());
            }
            Ok(ProviderProbe {
                server,
                account,
                version: TESTED_PROVIDER_VERSION,
                process_id,
            })
        })();
        if result.is_err()
            && let Some(login) = &attempt
            && login.state() == LoginState::Pending
        {
            // Cancellation ends the attempt; it does not prove that provider credentials were removed.
            // Ignore provider text and always stop/reap the owned process after this bounded request.
            if let Ok(request) = login
                .cancel_request(5)
                .and_then(|request| request.to_line())
                && process.send_during_cleanup(&request).is_ok()
            {
                let _ =
                    process.response_routed(5, Instant::now() + CANCEL_TIMEOUT, false, |_| Ok(()));
            }
        }
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
                control: Arc::default(),
                input: None,
                lines: Some(receiver),
                reader: None,
                reader_stop: Arc::default(),
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
                control: Arc::default(),
                input: None,
                lines: Some(receiver),
                reader: None,
                reader_stop: Arc::default(),
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
            let id = process.id().unwrap() as libc::pid_t;
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
