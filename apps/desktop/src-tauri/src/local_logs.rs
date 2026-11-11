//! Local log consent and scans (SRS Section 5C and Section 18.3).
//! Detection checks only whether known folders exist. A worker scans each granted folder outside
//! the workspace lock, one run at a time, and a failure stays inside local logs (OU-LOG-008).
use crate::{WorkspaceService, arguments, workspace::CommandError, workspace::Workspace};
use openusage_locallogs::{self as locallogs, Detected, detect};
use openusage_storage::{LogConsent, LogSource};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::Duration;
use tauri::State;

const SCAN_INTERVAL: Duration = Duration::from_secs(600);
const LOCKED_RETRY: Duration = Duration::from_secs(2);

#[cfg(all(feature = "pi-native-qualification", target_os = "linux"))]
mod pi_qualification;

pub(crate) fn tool_allowed(id: &str) -> bool {
    locallogs::tool(id).is_some_and(|tool| {
        tool.enabled
            || (cfg!(all(
                feature = "pi-native-qualification",
                target_os = "linux"
            )) && id == "pi")
    })
}

fn empty_scan_home() -> std::io::Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o700));
    }
    builder.tempdir()
}

fn detect_user_home() -> Vec<Detected> {
    // A native-test build never looks at the real home: a journey names a synthetic one.
    #[cfg(feature = "native-test")]
    let home = std::env::var_os("OPENUSAGE_NATIVE_LOG_HOME");
    #[cfg(not(feature = "native-test"))]
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    let home = home.map(PathBuf::from).filter(|home| home.is_absolute());
    home.map(|home| {
        let detected = detect(&home);
        #[cfg(all(feature = "pi-native-qualification", target_os = "linux"))]
        let detected = detected
            .into_iter()
            .chain(locallogs::detect::detect_pi_candidate(&home))
            .collect();
        detected
    })
    .unwrap_or_default()
}

fn now_utc() -> String {
    jiff::Timestamp::now()
        .strftime("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

/// The pinned binary and its digest. Rust checks the digest before each run (OU-LOG-005).
#[derive(Clone)]
struct PinnedScanner {
    binary: PathBuf,
    digest: String,
    version: String,
}

#[derive(Clone)]
pub(crate) struct Scanner {
    production: PinnedScanner,
    #[cfg(all(feature = "pi-native-qualification", target_os = "linux"))]
    pi: Option<PinnedScanner>,
}
impl Scanner {
    fn for_tool(&self, tool: &str) -> Option<&PinnedScanner> {
        if !tool_allowed(tool) {
            return None;
        }
        #[cfg(all(feature = "pi-native-qualification", target_os = "linux"))]
        if tool == "pi" {
            return self.pi.as_ref();
        }
        Some(&self.production)
    }

    fn bundled() -> Option<Self> {
        let digest = locallogs::pinned_sha256_here()?;
        let binary = std::env::current_exe()
            .ok()?
            .parent()?
            .join(locallogs::BINARY_NAME);
        // A native journey can name the pinned binary. The digest check still applies.
        #[cfg(feature = "native-test")]
        let binary = std::env::var_os("OPENUSAGE_NATIVE_CCUSAGE")
            .map(PathBuf::from)
            .unwrap_or(binary);
        Some(Self {
            production: PinnedScanner {
                binary,
                digest: digest.into(),
                version: locallogs::CCUSAGE_VERSION.into(),
            },
            #[cfg(all(feature = "pi-native-qualification", target_os = "linux"))]
            pi: pi_qualification::scanner(),
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Locked,
    Unavailable,
    Scanned { failed: bool, at: Option<String> },
}

pub(crate) struct ScanGroup {
    pub sources: Vec<LogSource>,
    pub zone: Option<String>,
}

/// No workspace lock is held while ccusage runs.
fn scan_all(workspace: &Mutex<Option<Workspace>>, scanner: &Scanner) -> Outcome {
    let plan = workspace
        .lock()
        .ok()
        .and_then(|mut state| state.as_mut().map(Workspace::local_log_scan_plan));
    let Some(Ok(groups)) = plan else {
        return Outcome::Locked;
    };
    let (mut failed, mut at) = (false, None);
    for group in groups {
        let result =
            if locallogs::tool(&group.sources[0].tool).is_some_and(|t| t.agent_report.is_some()) {
                scan_group(workspace, scanner, &group)
            } else {
                scan_source(
                    workspace,
                    scanner,
                    &group.sources[0],
                    group.zone.as_deref().unwrap_or("UTC"),
                )
            };
        match result {
            Some(scanned_at) => at = Some(scanned_at),
            None => failed = true,
        }
    }
    Outcome::Scanned { failed, at }
}

fn scan_group(
    workspace: &Mutex<Option<Workspace>>,
    scanner: &Scanner,
    group: &ScanGroup,
) -> Option<String> {
    let scanner = scanner.for_tool(&group.sources.first()?.tool)?;
    let empty_home = empty_scan_home().ok()?;
    let folders: Vec<_> = group.sources.iter().map(|s| Path::new(&s.folder)).collect();
    let report = collect_group(
        &locallogs::GroupedScanRequest {
            binary: &scanner.binary,
            expected_sha256: &scanner.digest,
            tool: &group.sources[0].tool,
            folders: &folders,
            empty_home: empty_home.path(),
            timezone: group.zone.as_deref().unwrap_or("UTC"),
            timeout: locallogs::DEFAULT_TIMEOUT,
            max_output: locallogs::DEFAULT_MAX_OUTPUT,
        },
        || sources_consented(workspace, &group.sources, true),
    )
    .ok()?;
    let scanned_at = now_utc();
    workspace
        .lock()
        .ok()?
        .as_mut()?
        .save_grouped_log_reports(group, &scanned_at, &scanner.version, &report)
        .ok()?;
    Some(scanned_at)
}

fn collect_group(
    request: &locallogs::GroupedScanRequest<'_>,
    check: impl FnMut() -> bool,
) -> Result<locallogs::GroupedScanResult, locallogs::ScanError> {
    #[cfg(all(feature = "pi-native-qualification", target_os = "linux"))]
    if request.tool == "pi" {
        return locallogs::grouped::qualify_pi_candidate(request, check);
    }
    locallogs::scan_grouped_with_check(request, check)
}

fn sources_consented(
    workspace: &Mutex<Option<Workspace>>,
    sources: &[LogSource],
    grouped: bool,
) -> bool {
    workspace
        .lock()
        .ok()
        .and_then(|mut state| {
            state
                .as_mut()
                .map(|w| w.check_local_scan_sources(sources, grouped).is_ok())
        })
        .unwrap_or(false)
}

/// Read all required reports before the single save. A failed report cannot publish part of a scan.
fn scan_source(
    workspace: &Mutex<Option<Workspace>>,
    scanner: &Scanner,
    source: &LogSource,
    zone: &str,
) -> Option<String> {
    let scanner = scanner.for_tool(&source.tool)?;
    let empty_home = empty_scan_home().ok()?;
    let request = locallogs::ScanRequest {
        binary: &scanner.binary,
        expected_sha256: &scanner.digest,
        tool: &source.tool,
        folder: Path::new(&source.folder),
        empty_home: empty_home.path(),
        timezone: zone,
        since: None,
        timeout: locallogs::DEFAULT_TIMEOUT,
        max_output: locallogs::DEFAULT_MAX_OUTPUT,
    };
    let check = || sources_consented(workspace, std::slice::from_ref(source), false);
    if !check() {
        return None;
    }
    let rows = locallogs::scan(&request).ok()?;
    if !check() {
        return None;
    }
    let sessions = locallogs::scan_sessions(&request).ok()?;
    let hours = if locallogs::tool(&source.tool).is_some_and(|t| t.enabled && t.hours) {
        if !check() {
            return None;
        }
        Some(locallogs::scan_hours(&request).ok()?)
    } else {
        None
    };
    let scanned_at = now_utc();
    workspace
        .lock()
        .ok()?
        .as_mut()?
        .save_complete_log_reports(
            source,
            zone,
            &scanned_at,
            &rows,
            &sessions,
            hours.as_deref(),
        )
        .ok()?;
    Some(scanned_at)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScanStatus {
    /// `idle`, `running`, `failed`, or `unavailable` when this platform has no pinned binary.
    state: &'static str,
    last_scan_at_utc: Option<String>,
}

struct Shared {
    stop: AtomicBool,
    state: Mutex<(bool, ScanStatus)>,
    signal: Condvar,
}

/// One owned thread. It scans after start, after consent, and every ten minutes.
pub(crate) struct LocalLogWorker {
    shared: Arc<Shared>,
    thread: Mutex<Option<JoinHandle<()>>>,
}
impl LocalLogWorker {
    pub(crate) fn start(workspace: Arc<Mutex<Option<Workspace>>>) -> Self {
        Self::start_with(workspace, Scanner::bundled())
    }

    fn start_with(workspace: Arc<Mutex<Option<Workspace>>>, scanner: Option<Scanner>) -> Self {
        let idle = ScanStatus {
            state: "idle",
            last_scan_at_utc: None,
        };
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            state: Mutex::new((true, idle)),
            signal: Condvar::new(),
        });
        let owned = shared.clone();
        let thread = std::thread::spawn(move || {
            let mut wait = Duration::ZERO;
            loop {
                {
                    let guard = owned.state.lock().unwrap_or_else(|p| p.into_inner());
                    let (mut guard, _) = owned
                        .signal
                        .wait_timeout_while(guard, wait, |(due, _)| {
                            !*due && !owned.stop.load(Ordering::Acquire)
                        })
                        .unwrap_or_else(|p| p.into_inner());
                    if owned.stop.load(Ordering::Acquire) {
                        return;
                    }
                    guard.0 = false;
                    if scanner.is_some() {
                        guard.1.state = "running";
                    }
                }
                let outcome = scanner.as_ref().map_or(Outcome::Unavailable, |scanner| {
                    scan_all(&workspace, scanner)
                });
                let mut guard = owned.state.lock().unwrap_or_else(|p| p.into_inner());
                wait = match outcome {
                    Outcome::Locked => {
                        guard.1.state = "idle";
                        LOCKED_RETRY
                    }
                    Outcome::Unavailable => {
                        guard.1.state = "unavailable";
                        SCAN_INTERVAL
                    }
                    Outcome::Scanned { failed, at } => {
                        guard.1.state = if failed { "failed" } else { "idle" };
                        if at.is_some() {
                            guard.1.last_scan_at_utc = at;
                        }
                        SCAN_INTERVAL
                    }
                };
            }
        });
        Self {
            shared,
            thread: Mutex::new(Some(thread)),
        }
    }

    pub(crate) fn wake(&self) {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .0 = true;
        self.shared.signal.notify_all();
    }

    pub(crate) fn status(&self) -> ScanStatus {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .1
            .clone()
    }

    pub(crate) fn close(&self) {
        self.shared.stop.store(true, Ordering::Release);
        self.shared.signal.notify_all();
        if let Some(thread) = self.thread.lock().unwrap_or_else(|p| p.into_inner()).take() {
            let _ = thread.join();
        }
    }
}
impl Drop for LocalLogWorker {
    fn drop(&mut self) {
        self.close();
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolView {
    tool: String,
    detected: bool,
    granted: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocalLogsView {
    /// True on first launch: tools exist and no choice is saved (OU-LOG-001).
    consent_required: bool,
    tools: Vec<ToolView>,
    scan: ScanStatus,
}

/// Before setup or unlock, no choice can exist yet, so every detected tool asks for consent.
fn view(
    workspace: &mut Workspace,
    detected: &[Detected],
    scan: ScanStatus,
) -> Result<LocalLogsView, CommandError> {
    let sources = match workspace.log_sources() {
        Err(CommandError::WorkspaceLocked) => Vec::new(),
        sources => sources?,
    };
    let tools = locallogs::tools()
        .iter()
        .filter(|tool| tool_allowed(&tool.id))
        .map(|tool| ToolView {
            tool: tool.id.clone(),
            detected: detected.iter().any(|d| d.tool == tool.id),
            granted: sources
                .iter()
                .any(|s| s.tool == tool.id && s.consent == LogConsent::Granted),
        })
        .filter(|view| view.detected || sources.iter().any(|s| s.tool == view.tool))
        .collect();
    Ok(LocalLogsView {
        consent_required: sources.is_empty() && !detected.is_empty(),
        tools,
        scan,
    })
}

fn consent(
    workspace: &mut Workspace,
    detected: &[Detected],
    tools: &[String],
) -> Result<(), CommandError> {
    let unique: BTreeSet<&String> = tools.iter().collect();
    let known = |tool: &String| tool_allowed(tool);
    if unique.len() != tools.len() || !tools.iter().all(known) {
        return Err(CommandError::InvalidArguments);
    }
    let detected: Vec<(&str, &str)> = detected
        .iter()
        .map(|d| (d.tool, d.folder.as_str()))
        .collect();
    workspace.set_local_log_consent(&detected, tools, &now_utc())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoadArgs {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsentArgs {
    tools: Vec<String>,
}

#[tauri::command]
pub async fn load_local_logs(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<LocalLogsView, CommandError> {
    let _: LoadArgs = arguments(&request)?;
    let scan = service.4.status();
    crate::subscriptions::access(service, move |w| view(w, &detect_user_home(), scan)).await
}

/// Saves the chosen tools, then starts a scan (OU-LOG-001, OU-LOG-002).
#[tauri::command]
pub async fn save_local_log_consent(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<LocalLogsView, CommandError> {
    let args: ConsentArgs = arguments(&request)?;
    let worker = Arc::clone(&service.4);
    let scan = worker.status();
    let result = crate::subscriptions::access(service, move |w| {
        let detected = detect_user_home();
        consent(w, &detected, &args.tools)?;
        view(w, &detected, scan)
    })
    .await;
    if result.is_ok() {
        worker.wake();
    }
    result
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::keyvault::{DatabaseKey, KeyVault, VaultError};
    use std::cell::RefCell;
    use std::os::unix::fs::PermissionsExt;
    use std::time::Instant;
    use zeroize::Zeroizing;

    #[derive(Default)]
    struct MemoryVault(RefCell<Option<DatabaseKey>>);
    impl KeyVault for MemoryVault {
        fn load(&self) -> Result<Option<DatabaseKey>, VaultError> {
            Ok(self.0.borrow().as_ref().map(|key| Zeroizing::new(**key)))
        }
        fn protect(&self, key: &[u8; 32]) -> Result<(), VaultError> {
            *self.0.borrow_mut() = Some(Zeroizing::new(*key));
            Ok(())
        }
        fn remove(&self, _: &[u8; 32]) -> Result<(), VaultError> {
            *self.0.borrow_mut() = None;
            Ok(())
        }
    }

    const CLAUDE_REPORT: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../crates/openusage-locallogs/tests/fixtures/ccusage-20.0.48-daily-by-agent-claude.json"
    );
    const CLAUDE_SESSIONS: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../crates/openusage-locallogs/tests/fixtures/ccusage-20.0.48-session-claude.json"
    );
    const CLAUDE_BLOCKS: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../crates/openusage-locallogs/tests/fixtures/ccusage-20.0.48-blocks-claude.json"
    );

    fn scanner(dir: &Path, name: &str, body: &str) -> Scanner {
        let binary = dir.join(name);
        std::fs::write(&binary, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let digest = locallogs::sha256_hex(&std::fs::read(&binary).unwrap());
        Scanner {
            production: PinnedScanner {
                binary,
                digest,
                version: locallogs::CCUSAGE_VERSION.into(),
            },
            #[cfg(all(feature = "pi-native-qualification", target_os = "linux"))]
            pi: None,
        }
    }

    fn totals(workspace: &mut Workspace) -> Vec<(String, String, u64)> {
        workspace
            .local_logs_days_for_test()
            .into_iter()
            .map(|d| (d.counts.local_date, d.counts.model, d.counts.total_tokens))
            .collect()
    }

    #[test]
    fn descriptors_keep_production_and_candidate_identity_separate() {
        let descriptor = PinnedScanner {
            binary: PathBuf::from("/synthetic/production"),
            digest: "a".repeat(64),
            version: locallogs::CCUSAGE_VERSION.into(),
        };
        let scanner = Scanner {
            production: descriptor,
            #[cfg(all(feature = "pi-native-qualification", target_os = "linux"))]
            pi: None,
        };
        assert!(scanner.for_tool("pi").is_none());
        assert!(scanner.for_tool("opencode").is_none());
        assert!(scanner.for_tool("unknown").is_none());
        assert_eq!(scanner.for_tool("claude").unwrap().version, "20.0.48");
        assert_eq!(scanner.for_tool("codex").unwrap().digest, "a".repeat(64));
        #[cfg(all(feature = "pi-native-qualification", target_os = "linux"))]
        {
            let mut scanner = scanner;
            scanner.pi = Some(PinnedScanner {
                binary: PathBuf::from("/synthetic/pi-candidate"),
                digest: "b".repeat(64),
                version: "20.0.48-openusage.pi-bounded-release.1".into(),
            });
            assert_eq!(scanner.for_tool("pi").unwrap().digest, "b".repeat(64));
            assert_eq!(scanner.for_tool("claude").unwrap().digest, "a".repeat(64));
            assert_eq!(scanner.for_tool("codex").unwrap().version, "20.0.48");
            assert!(scanner.for_tool("opencode").is_none());
        }
    }

    // One test, because ccusage runs share one process-wide lock.
    #[test]
    fn consent_gates_scans_and_a_failure_keeps_saved_totals() {
        let temporary = tempfile::tempdir().unwrap();
        let home = temporary.path().join("home");
        std::fs::create_dir_all(home.join(".claude/projects")).unwrap();
        std::fs::create_dir_all(home.join(".codex/sessions")).unwrap();
        let detected = detect(&home);
        let mut workspace = Workspace::new(temporary.path().join("workspace")).unwrap();
        let idle = ScanStatus {
            state: "idle",
            last_scan_at_utc: None,
        };
        // Before setup, the welcome screen lists every detected tool. Saving needs storage.
        let welcome = view(&mut workspace, &detected, idle.clone()).unwrap();
        assert!(welcome.consent_required);
        assert!(welcome.tools.iter().all(|t| t.detected && !t.granted));
        assert_eq!(
            consent(&mut workspace, &detected, &["claude".into()]),
            Err(CommandError::WorkspaceLocked)
        );
        let vault = MemoryVault::default();
        workspace.setup(Ok(&vault), None).unwrap();
        let first = view(&mut workspace, &detected, idle.clone()).unwrap();
        assert!(first.consent_required);
        assert_eq!(
            first
                .tools
                .iter()
                .map(|t| (t.tool.as_str(), t.detected, t.granted))
                .collect::<Vec<_>>(),
            [("claude", true, false), ("codex", true, false)]
        );
        for bad in [
            vec!["claude".to_string(), "claude".to_string()],
            vec!["opencode".to_string()],
        ] {
            assert_eq!(
                consent(&mut workspace, &detected, &bad),
                Err(CommandError::InvalidArguments)
            );
        }
        consent(&mut workspace, &detected, &["claude".into()]).unwrap();
        let chosen = view(&mut workspace, &detected, idle.clone()).unwrap();
        assert!(!chosen.consent_required);
        assert_eq!(
            chosen
                .tools
                .iter()
                .map(|t| (t.tool.as_str(), t.granted))
                .collect::<Vec<_>>(),
            [("claude", true), ("codex", false)]
        );

        let shared = Mutex::new(Some(workspace));
        let bin = temporary.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        // Only the granted Claude Code folder runs. A Codex run would leave this marker.
        let marker = bin.join("codex-ran");
        let ok = scanner(
            &bin,
            "ok",
            &format!(
                "[ -n \"$CODEX_HOME\" ] && touch {}; if [ \"$2\" = session ]; then cat {CLAUDE_SESSIONS}; elif [ \"$1\" = blocks ]; then cat {CLAUDE_BLOCKS}; else cat {CLAUDE_REPORT}; fi",
                marker.display()
            ),
        );
        let Outcome::Scanned {
            failed: false,
            at: Some(_),
        } = scan_all(&shared, &ok)
        else {
            panic!("the granted folder was not scanned");
        };
        assert!(!marker.exists());
        let mut state = shared.lock().unwrap();
        let saved = totals(state.as_mut().unwrap());
        assert_eq!(saved.len(), 3);
        // The same run saved the Claude Code sessions with their project (SC-021).
        let range = state
            .as_mut()
            .unwrap()
            .local_usage_range(
                ("2026-01-01", "2026-12-31"),
                Some(("2026-01-01T00:00:00.000Z", "2027-01-01T00:00:00.000Z")),
            )
            .unwrap();
        assert_eq!(range.sessions.len(), 2);
        assert!(
            range
                .sessions
                .iter()
                .all(|s| s.session.project_name.as_deref() == Some("-home-dev-alpha"))
        );
        // The same run saved the Claude Code hours (SC-021).
        assert_eq!(range.hours.len(), 3);
        drop(state);

        // A failed run keeps the saved totals (OU-LOG-008, OU-LOG-011).
        let failing = scanner(&bin, "fail", "exit 3");
        assert_eq!(
            scan_all(&shared, &failing),
            Outcome::Scanned {
                failed: true,
                at: None
            }
        );
        // A binary that does not match its digest never runs.
        let mut substituted = scanner(&bin, "substituted", &format!("touch {}", marker.display()));
        substituted.production.digest = "0".repeat(64);
        assert!(matches!(
            scan_all(&shared, &substituted),
            Outcome::Scanned { failed: true, .. }
        ));
        assert!(!marker.exists());
        assert_eq!(totals(shared.lock().unwrap().as_mut().unwrap()), saved);

        // Failure of a later required report must not update days or their scan time.
        let before = shared
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .log_sources()
            .unwrap();
        for argument in ["session", "blocks"] {
            let partial = scanner(
                &bin,
                "partial",
                &format!(
                    "if [ \"$1\" = {argument} ] || [ \"$2\" = {argument} ]; then echo malformed; elif [ \"$2\" = session ]; then cat {CLAUDE_SESSIONS}; elif [ \"$1\" = blocks ]; then cat {CLAUDE_BLOCKS}; else cat {CLAUDE_REPORT}; fi"
                ),
            );
            assert!(matches!(
                scan_all(&shared, &partial),
                Outcome::Scanned {
                    failed: true,
                    at: None
                }
            ));
            let mut state = shared.lock().unwrap();
            assert_eq!(state.as_mut().unwrap().log_sources().unwrap(), before);
            assert_eq!(totals(state.as_mut().unwrap()), saved);
        }

        // A locked workspace runs nothing.
        assert_eq!(scan_all(&Mutex::new(None), &ok), Outcome::Locked);

        // The worker scans on start and stops when closed.
        let workspace = Arc::new(shared);
        let worker = LocalLogWorker::start_with(workspace.clone(), Some(ok));
        let deadline = Instant::now() + Duration::from_secs(20);
        while worker.status().last_scan_at_utc.is_none() {
            assert!(Instant::now() < deadline, "the worker did not scan");
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(worker.status().state, "idle");
        worker.close();
        assert_eq!(totals(workspace.lock().unwrap().as_mut().unwrap()), saved);
        let unavailable = LocalLogWorker::start_with(Arc::new(Mutex::new(None)), None);
        let deadline = Instant::now() + Duration::from_secs(20);
        while unavailable.status().state != "unavailable" {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
