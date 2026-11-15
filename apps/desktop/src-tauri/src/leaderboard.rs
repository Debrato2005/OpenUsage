//! One owned worker keeps credentials and HTTP outside workspace and GUI locks.
use crate::{WorkspaceService, arguments, workspace::Workspace};
use openusage_board::{
    ApprovedSyncBody, BoardClient, ClientContext, DepartureError, DepartureSession, DeviceFlow,
    EnrollmentSession, EnrollmentStatus, LocalCredentialState, PollOutcome, SessionError,
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tauri::State;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Issue {
    Unavailable,
    InvalidArguments,
    WorkspaceLocked,
    Busy,
    Unsupported,
    Stopped,
    Unconfigured,
    ConsentRequired,
    StaleOperation,
    CredentialUnavailable,
    InvalidCredential,
    AlreadyJoined,
    NotJoined,
    SaveFailed,
    AuthorizationDenied,
    AuthorizationExpired,
    RemoteUnavailable,
    RemoteRefused,
    RateLimited,
    TooManyDevices,
    IncompleteData,
    InvalidData,
    TooLarge,
    PreviewExpired,
    LocalCleanupFailed,
}
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum EnrollmentState {
    NotChecked,
    NotJoined,
    Pending,
    Joined,
    Invalid,
    Unavailable,
}
#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    Join,
    Reconnect,
    Sync,
    Leave,
    RecoverLeave,
    Forget,
}
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum RemoteEffect {
    None,
    Confirmed,
    Unknown,
}
#[derive(Clone, Serialize)]
#[serde(
    tag = "state",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum Outcome {
    Joined {
        login: String,
    },
    Synced {
        accepted: String,
    },
    Left {
        login: Option<String>,
        credentials: LocalCredentialState,
    },
    Forgotten,
    Cancelled,
    Failed {
        issue: Issue,
        remote_effect: RemoteEffect,
    },
}
#[derive(Clone, Serialize)]
#[serde(
    tag = "phase",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum Progress {
    Preparing,
    Authorizing {
        user_code: String,
        verification_uri: &'static str,
        expires_in_seconds: u32,
    },
    Preview {
        body: String,
    },
    Dispatching,
    Settling,
    Cancelling,
    Finished {
        outcome: Outcome,
    },
}
#[derive(Clone, Serialize)]
struct Operation {
    id: String,
    kind: Kind,
    #[serde(flatten)]
    progress: Progress,
}
#[derive(Clone, Serialize)]
pub(crate) struct View {
    configured: bool,
    enrollment: EnrollmentState,
    login: Option<String>,
    busy: bool,
    operation: Option<Operation>,
    #[serde(rename = "statusIssue", skip_serializing_if = "Option::is_none")]
    status_issue: Option<Issue>,
}
impl Default for View {
    fn default() -> Self {
        Self {
            configured: false,
            enrollment: EnrollmentState::NotChecked,
            login: None,
            busy: false,
            operation: None,
            status_issue: None,
        }
    }
}
struct Action {
    id: String,
    kind: Kind,
    days: u16,
}
struct Control {
    stopping: bool,
    closed: bool,
    deleting_local_data: bool,
    requested: u64,
    completed: u64,
    result: Result<View, Issue>,
    queued: Option<Action>,
    working: bool,
    cancel: bool,
    dispatched: bool,
    confirmed: bool,
}
struct Shared {
    control: Mutex<Control>,
    signal: Condvar,
}
pub(crate) struct Worker {
    shared: Arc<Shared>,
    thread: Mutex<Option<JoinHandle<()>>>,
}
pub(crate) struct LocalDeletionLease {
    shared: Arc<Shared>,
    invalidate_cache: bool,
}
impl LocalDeletionLease {
    pub(crate) fn invalidate_cache(&mut self) {
        self.invalidate_cache = true;
    }
}
impl Drop for LocalDeletionLease {
    fn drop(&mut self) {
        let mut control = self
            .shared
            .control
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if self.invalidate_cache {
            // Admission requires idle work, so no live outcome can be lost here.
            control.result = Ok(View::default());
        }
        control.deleting_local_data = false;
        self.shared.signal.notify_all();
    }
}
enum Work {
    Inspect(u64),
    Action(Action),
}
impl Worker {
    pub(crate) fn start(workspace: Arc<Mutex<Option<Workspace>>>) -> Self {
        let shared = Arc::new(Shared {
            control: Mutex::new(Control {
                stopping: false,
                closed: false,
                deleting_local_data: false,
                requested: 0,
                completed: 0,
                result: Ok(View::default()),
                queued: None,
                working: false,
                cancel: false,
                dispatched: false,
                confirmed: false,
            }),
            signal: Condvar::new(),
        });
        let owned = shared.clone();
        let thread = std::thread::spawn(move || {
            loop {
                let work = {
                    let guard = owned.control.lock().unwrap_or_else(|p| p.into_inner());
                    let mut guard = owned
                        .signal
                        .wait_while(guard, |state| {
                            !state.stopping
                                && state.queued.is_none()
                                && state.requested == state.completed
                        })
                        .unwrap_or_else(|p| p.into_inner());
                    if guard.stopping {
                        break;
                    }
                    if let Some(action) = guard.queued.take() {
                        Work::Action(action)
                    } else {
                        Work::Inspect(guard.requested)
                    }
                };
                match work {
                    Work::Inspect(request) => {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            inspect(&workspace)
                        }))
                        .unwrap_or(Err(Issue::Unavailable));
                        let mut control = owned.control.lock().unwrap_or_else(|p| p.into_inner());
                        apply_inspection(&mut control, result);
                        control.completed = request;
                        owned.signal.notify_all();
                    }
                    Work::Action(action) => {
                        let outcome =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                run_action(&workspace, &owned, &action)
                            }))
                            .unwrap_or_else(|_| {
                                let mut control =
                                    owned.control.lock().unwrap_or_else(|p| p.into_inner());
                                let remote_effect =
                                    if control.dispatched && !matches!(action.kind, Kind::Forget) {
                                        RemoteEffect::Unknown
                                    } else {
                                        RemoteEffect::None
                                    };
                                let view = view_mut(&mut control);
                                view.enrollment = EnrollmentState::Unavailable;
                                view.login = None;
                                Err(Failure::Issue(Issue::Unavailable, remote_effect))
                            });
                        let outcome = match outcome {
                            Ok(outcome) => outcome,
                            Err(Failure::Cancelled) => Outcome::Cancelled,
                            Err(Failure::Issue(issue, remote_effect)) => Outcome::Failed {
                                issue,
                                remote_effect,
                            },
                        };
                        // Action locals, credentials, and the guard drop before this receipt.
                        let mut control = owned.control.lock().unwrap_or_else(|p| p.into_inner());
                        let view = view_mut(&mut control);
                        view.busy = false;
                        view.operation = Some(Operation {
                            id: action.id,
                            kind: action.kind,
                            progress: Progress::Finished { outcome },
                        });
                        control.working = false;
                        control.cancel = false;
                        control.dispatched = false;
                        control.confirmed = false;
                        owned.signal.notify_all();
                    }
                }
            }
            let mut control = owned.control.lock().unwrap_or_else(|p| p.into_inner());
            control.closed = true;
            owned.signal.notify_all();
        });
        Self {
            shared,
            thread: Mutex::new(Some(thread)),
        }
    }
    pub(crate) fn local_deletion(&self) -> Result<LocalDeletionLease, Issue> {
        let mut control = self.shared.control.lock().map_err(|_| Issue::Unavailable)?;
        if control.stopping || control.closed {
            return Err(Issue::Stopped);
        }
        if control.deleting_local_data
            || control.working
            || control.queued.is_some()
            || control.requested != control.completed
        {
            return Err(Issue::Busy);
        }
        control.deleting_local_data = true;
        Ok(LocalDeletionLease {
            shared: self.shared.clone(),
            invalidate_cache: false,
        })
    }
    pub(crate) fn load(&self) -> Result<View, Issue> {
        self.read(false)
    }
    pub(crate) fn refresh(&self) -> Result<View, Issue> {
        self.read(true)
    }
    fn read(&self, refresh: bool) -> Result<View, Issue> {
        let mut control = self.shared.control.lock().map_err(|_| Issue::Unavailable)?;
        if control.stopping || control.closed {
            return Err(Issue::Stopped);
        }
        if control.deleting_local_data {
            return Err(Issue::Busy);
        }
        if control.working
            || (!refresh
                && control
                    .result
                    .as_ref()
                    .is_ok_and(|view| view.operation.is_some()))
        {
            return control.result.clone();
        }
        if control.requested == control.completed {
            control.requested = control.requested.checked_add(1).ok_or(Issue::Unavailable)?;
        }
        let request = control.requested;
        self.shared.signal.notify_all();
        let control = self
            .shared
            .signal
            .wait_while(control, |state| {
                !state.stopping && !state.closed && state.completed < request
            })
            .map_err(|_| Issue::Unavailable)?;
        if control.stopping || control.closed {
            return Err(Issue::Stopped);
        }
        if control.deleting_local_data {
            return Err(Issue::Busy);
        }
        control.result.clone()
    }
    pub(crate) fn begin(
        &self,
        kind: Kind,
        days: Option<u16>,
        confirmed: bool,
    ) -> Result<View, Issue> {
        if !confirmed {
            return Err(Issue::ConsentRequired);
        }
        if match kind {
            Kind::Sync => days.is_some_and(|days| !(1..=366).contains(&days)),
            _ => days.is_some(),
        } {
            return Err(Issue::InvalidArguments);
        }
        let mut control = self.shared.control.lock().map_err(|_| Issue::Unavailable)?;
        if control.stopping || control.closed {
            return Err(Issue::Stopped);
        }
        if control.deleting_local_data || control.working || control.requested != control.completed
        {
            return Err(Issue::Busy);
        }
        let id = uuid::Uuid::new_v4().to_string();
        let mut view = control.result.clone().unwrap_or_default();
        view.busy = true;
        view.status_issue = None;
        view.operation = Some(Operation {
            id: id.clone(),
            kind,
            progress: Progress::Preparing,
        });
        control.result = Ok(view.clone());
        control.queued = Some(Action {
            id,
            kind,
            days: days.unwrap_or(30),
        });
        control.working = true;
        control.cancel = false;
        control.dispatched = false;
        control.confirmed = false;
        self.shared.signal.notify_all();
        Ok(view)
    }
    pub(crate) fn cancel(&self, id: &str) -> Result<View, Issue> {
        let mut control = self.shared.control.lock().map_err(|_| Issue::Unavailable)?;
        if control.stopping || control.closed {
            return Err(Issue::Stopped);
        }
        if control.deleting_local_data {
            return Err(Issue::Busy);
        }
        let matched = control
            .result
            .as_ref()
            .ok()
            .and_then(|v| v.operation.as_ref())
            .is_some_and(|op| op.id == id);
        if !matched {
            return Err(Issue::StaleOperation);
        }
        if !control.working {
            return control.result.clone();
        }
        control.cancel = true;
        let progress = if control.dispatched {
            Progress::Settling
        } else {
            Progress::Cancelling
        };
        if let Ok(view) = &mut control.result
            && let Some(op) = &mut view.operation
        {
            op.progress = progress;
        }
        self.shared.signal.notify_all();
        control.result.clone()
    }
    pub(crate) fn confirm_sync(&self, id: &str, confirmed: bool) -> Result<View, Issue> {
        if !confirmed {
            return Err(Issue::ConsentRequired);
        }
        let mut control = self.shared.control.lock().map_err(|_| Issue::Unavailable)?;
        if control.stopping || control.closed {
            return Err(Issue::Stopped);
        }
        if control.deleting_local_data {
            return Err(Issue::Busy);
        }
        if control.cancel || control.confirmed || !control.working {
            return Err(Issue::StaleOperation);
        }
        let valid = control
            .result
            .as_ref()
            .ok()
            .and_then(|v| v.operation.as_ref())
            .is_some_and(|op| {
                op.id == id
                    && matches!(op.kind, Kind::Sync)
                    && matches!(op.progress, Progress::Preview { .. })
            });
        if !valid {
            return Err(Issue::StaleOperation);
        }
        control.confirmed = true;
        if let Some(op) = &mut view_mut(&mut control).operation {
            op.progress = Progress::Preparing;
        }
        self.shared.signal.notify_all();
        control.result.clone()
    }
    pub(crate) fn open_login_page(&self, id: &str) -> Result<(), Issue> {
        let uri = {
            let control = self.shared.control.lock().map_err(|_| Issue::Unavailable)?;
            if control.stopping || control.closed || control.cancel {
                return Err(Issue::Stopped);
            }
            if control.deleting_local_data {
                return Err(Issue::Busy);
            }
            match control
                .result
                .as_ref()
                .ok()
                .and_then(|view| view.operation.as_ref())
            {
                Some(Operation {
                    id: current,
                    progress:
                        Progress::Authorizing {
                            verification_uri, ..
                        },
                    ..
                }) if current == id => *verification_uri,
                _ => return Err(Issue::StaleOperation),
            }
        };
        open::that(uri).map_err(|_| Issue::Unavailable)
    }
    pub(crate) fn request_stop(&self) {
        let mut control = self
            .shared
            .control
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        control.stopping = true;
        control.cancel = true;
        self.shared.signal.notify_all();
    }
    pub(crate) fn close(&self) -> Result<(), Issue> {
        self.request_stop();
        let thread = self.thread.lock().map_err(|_| Issue::Unavailable)?.take();
        thread.map_or(Ok(()), |thread| {
            thread.join().map_err(|_| Issue::Unavailable)
        })
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
fn view_mut(control: &mut Control) -> &mut View {
    if control.result.is_err() {
        control.result = Ok(View::default());
    }
    match &mut control.result {
        Ok(view) => view,
        Err(_) => unreachable!(),
    }
}
fn apply_inspection(control: &mut Control, result: Result<View, Issue>) {
    let previous = control.result.as_ref().ok().cloned();
    control.result = match result {
        Ok(mut view) => {
            view.operation = previous.and_then(|view| view.operation);
            Ok(view)
        }
        Err(issue) => match previous {
            Some(mut view) if view.operation.is_some() => {
                view.enrollment = EnrollmentState::Unavailable;
                view.login = None;
                view.status_issue = Some(issue);
                Ok(view)
            }
            _ => Err(issue),
        },
    };
}
enum Failure {
    Cancelled,
    Issue(Issue, RemoteEffect),
}
impl Failure {
    fn before(issue: Issue) -> Self {
        Self::Issue(issue, RemoteEffect::None)
    }
}
fn check_cancel(shared: &Shared) -> Result<(), Failure> {
    let control = shared
        .control
        .lock()
        .map_err(|_| Failure::before(Issue::Unavailable))?;
    if control.cancel || control.stopping {
        Err(Failure::Cancelled)
    } else {
        Ok(())
    }
}
fn publish(shared: &Shared, action: &Action, progress: Progress) -> Result<(), Failure> {
    let mut control = shared
        .control
        .lock()
        .map_err(|_| Failure::before(Issue::Unavailable))?;
    if control.cancel || control.stopping {
        return Err(Failure::Cancelled);
    }
    let view = view_mut(&mut control);
    view.operation = Some(Operation {
        id: action.id.clone(),
        kind: action.kind,
        progress,
    });
    Ok(())
}
fn reserve(shared: &Shared, action: &Action) -> Result<(), Failure> {
    let mut control = shared
        .control
        .lock()
        .map_err(|_| Failure::before(Issue::Unavailable))?;
    if control.cancel || control.stopping {
        return Err(Failure::Cancelled);
    }
    control.dispatched = true;
    view_mut(&mut control).operation = Some(Operation {
        id: action.id.clone(),
        kind: action.kind,
        progress: Progress::Dispatching,
    });
    Ok(())
}
fn update_status(shared: &Shared, session: &EnrollmentSession) {
    let mut control = shared.control.lock().unwrap_or_else(|p| p.into_inner());
    let view = view_mut(&mut control);
    view.configured = true;
    view.status_issue = None;
    view.login = None;
    match session.status() {
        EnrollmentStatus::NotJoined => view.enrollment = EnrollmentState::NotJoined,
        EnrollmentStatus::Pending => view.enrollment = EnrollmentState::Pending,
        EnrollmentStatus::Joined { login } => {
            view.enrollment = EnrollmentState::Joined;
            view.login = Some(login.into());
        }
    }
}
fn session_issue(error: SessionError) -> Issue {
    match error {
        SessionError::Busy => Issue::Busy,
        SessionError::Unsupported => Issue::Unsupported,
        SessionError::InvalidCredential => Issue::InvalidCredential,
        SessionError::AlreadyJoined => Issue::AlreadyJoined,
        SessionError::NotJoined => Issue::NotJoined,
        SessionError::SaveFailed => Issue::SaveFailed,
        SessionError::StoreUnavailable | SessionError::UnsafePath => Issue::CredentialUnavailable,
        SessionError::Configuration => Issue::Unconfigured,
    }
}
fn remote_issue(error: openusage_board::Error) -> Issue {
    match error {
        openusage_board::Error::Unconfigured | openusage_board::Error::InvalidConfiguration => {
            Issue::Unconfigured
        }
        openusage_board::Error::RateLimited => Issue::RateLimited,
        openusage_board::Error::TooManyDevices => Issue::TooManyDevices,
        openusage_board::Error::NotJoined => Issue::NotJoined,
        openusage_board::Error::GithubRefused | openusage_board::Error::Refused => {
            Issue::RemoteRefused
        }
        _ => Issue::RemoteUnavailable,
    }
}
fn authorize(
    workspace: &Arc<Mutex<Option<Workspace>>>,
    shared: &Shared,
    action: &Action,
    client: &BoardClient,
) -> Result<openusage_board::GithubToken, Failure> {
    check_cancel(shared)?;
    ready(workspace).map_err(Failure::before)?;
    let clock = Instant::now();
    let mut flow = client
        .device_start(clock.elapsed())
        .map_err(|e| Failure::before(remote_issue(e)))?;
    publish_challenge(shared, action, &flow, &clock)?;
    loop {
        let control = shared
            .control
            .lock()
            .map_err(|_| Failure::before(Issue::Unavailable))?;
        let due = flow.retry_at().min(flow.expires_at());
        let delay = due.saturating_sub(clock.elapsed());
        let (control, _) = shared
            .signal
            .wait_timeout_while(control, delay, |state| {
                !state.cancel && !state.stopping && clock.elapsed() < due
            })
            .map_err(|_| Failure::before(Issue::Unavailable))?;
        if control.cancel || control.stopping {
            return Err(Failure::Cancelled);
        }
        drop(control);
        ready(workspace).map_err(Failure::before)?;
        let result = client.device_poll(&mut flow, clock.elapsed());
        check_cancel(shared)?;
        match result.map_err(|e| Failure::before(remote_issue(e)))? {
            PollOutcome::Authorized(token) => return Ok(token),
            PollOutcome::Pending { .. } | PollOutcome::SlowDown { .. } => {
                publish_challenge(shared, action, &flow, &clock)?
            }
            PollOutcome::Denied => return Err(Failure::before(Issue::AuthorizationDenied)),
            PollOutcome::Expired => return Err(Failure::before(Issue::AuthorizationExpired)),
        }
    }
}
fn publish_challenge(
    shared: &Shared,
    action: &Action,
    flow: &DeviceFlow,
    clock: &Instant,
) -> Result<(), Failure> {
    publish(
        shared,
        action,
        Progress::Authorizing {
            user_code: flow.user_code().into(),
            verification_uri: flow.verification_uri(),
            expires_in_seconds: flow
                .expires_at()
                .saturating_sub(clock.elapsed())
                .as_secs()
                .clamp(1, 900) as u32,
        },
    )
}
fn upload_issue(error: openusage_storage::BoardUploadError) -> Issue {
    use openusage_storage::BoardUploadError;
    match error {
        BoardUploadError::InvalidSelection | BoardUploadError::InvalidData(_) => Issue::InvalidData,
        BoardUploadError::Incomplete(_) => Issue::IncompleteData,
        BoardUploadError::TooLarge => Issue::TooLarge,
        BoardUploadError::ReadFailed => Issue::Unavailable,
    }
}
fn await_confirmation(shared: &Shared) -> Result<(), Failure> {
    let deadline = Instant::now() + Duration::from_secs(300);
    let control = shared
        .control
        .lock()
        .map_err(|_| Failure::before(Issue::Unavailable))?;
    let (control, _) = shared
        .signal
        .wait_timeout_while(
            control,
            deadline.saturating_duration_since(Instant::now()),
            |state| {
                !state.stopping && !state.cancel && !state.confirmed && Instant::now() < deadline
            },
        )
        .map_err(|_| Failure::before(Issue::Unavailable))?;
    if control.stopping || control.cancel {
        return Err(Failure::Cancelled);
    }
    if !control.confirmed || Instant::now() >= deadline {
        return Err(Failure::before(Issue::PreviewExpired));
    }
    Ok(())
}
fn local_guard() -> Result<openusage_keys::BoardGuard, Issue> {
    #[cfg(all(feature = "native-test", target_os = "linux"))]
    let guard = {
        let root =
            openusage_keys::private_keyring_fixture().map_err(|_| Issue::CredentialUnavailable)?;
        openusage_keys::BoardGuard::acquire(&root.join("leaderboard"))
    };
    #[cfg(not(all(feature = "native-test", target_os = "linux")))]
    let guard = openusage_keys::BoardGuard::for_current_user();
    guard.map_err(|error| match error {
        openusage_keys::BoardGuardError::Busy => Issue::Busy,
        openusage_keys::BoardGuardError::Unsupported => Issue::Unsupported,
        _ => Issue::CredentialUnavailable,
    })
}
fn set_absent(shared: &Shared) {
    let mut control = shared.control.lock().unwrap_or_else(|p| p.into_inner());
    let view = view_mut(&mut control);
    view.enrollment = EnrollmentState::NotJoined;
    view.login = None;
}
fn set_credential_unknown(shared: &Shared) {
    let mut control = shared.control.lock().unwrap_or_else(|p| p.into_inner());
    let view = view_mut(&mut control);
    view.enrollment = EnrollmentState::Unavailable;
    view.login = None;
}
fn run_action(
    workspace: &Arc<Mutex<Option<Workspace>>>,
    shared: &Shared,
    action: &Action,
) -> Result<Outcome, Failure> {
    check_cancel(shared)?;
    ready(workspace).map_err(Failure::before)?;
    if matches!(action.kind, Kind::Forget) {
        let guard = local_guard().map_err(Failure::before)?;
        ready(workspace).map_err(Failure::before)?;
        reserve(shared, action)?;
        let erased = openusage_keys::BoardVault::new().and_then(|vault| vault.erase_local(&guard));
        if erased.is_err() {
            set_credential_unknown(shared);
            return Err(Failure::before(Issue::CredentialUnavailable));
        }
        set_absent(shared);
        return Ok(Outcome::Forgotten);
    }
    let context =
        ClientContext::from_current_build().map_err(|e| Failure::before(remote_issue(e)))?;
    let client = context.client();
    let origin = client
        .service_origin()
        .ok_or_else(|| Failure::before(Issue::Unconfigured))?;
    {
        let mut control = shared.control.lock().unwrap_or_else(|p| p.into_inner());
        view_mut(&mut control).configured = true;
    }
    if matches!(action.kind, Kind::RecoverLeave) {
        let session = DepartureSession::from_guard(origin, local_guard().map_err(Failure::before)?)
            .map_err(|e| Failure::before(session_issue(e)))?;
        ready(workspace).map_err(Failure::before)?;
        let token = authorize(workspace, shared, action, client)?;
        ready(workspace).map_err(Failure::before)?;
        reserve(shared, action)?;
        let receipt = match session.leave(client, token) {
            Ok(receipt) => receipt,
            Err(DepartureError::Remote(error)) => {
                return Err(Failure::Issue(remote_issue(error), RemoteEffect::Unknown));
            }
            Err(DepartureError::CleanupFailed) => {
                set_credential_unknown(shared);
                return Err(Failure::Issue(
                    Issue::LocalCleanupFailed,
                    RemoteEffect::Confirmed,
                ));
            }
        };
        match receipt.credential_state {
            LocalCredentialState::Removed | LocalCredentialState::NotPresent => set_absent(shared),
            LocalCredentialState::Unavailable | LocalCredentialState::Retained => {
                set_credential_unknown(shared)
            }
        }
        return Ok(Outcome::Left {
            login: Some(receipt.login),
            credentials: receipt.credential_state,
        });
    }
    let mut session = context
        .open_session()
        .map_err(|e| Failure::before(session_issue(e)))?;
    ready(workspace).map_err(Failure::before)?;
    update_status(shared, &session);
    match action.kind {
        Kind::Join | Kind::Reconnect => {
            let id = session
                .prepare_join(matches!(action.kind, Kind::Reconnect))
                .map_err(|e| Failure::before(session_issue(e)))?;
            update_status(shared, &session);
            let token = authorize(workspace, shared, action, client)?;
            ready(workspace).map_err(Failure::before)?;
            reserve(shared, action)?;
            let joined = client
                .join(token, &id)
                .map_err(|e| Failure::Issue(remote_issue(e), RemoteEffect::Unknown))?;
            let saved = session.finish_join(joined);
            update_status(shared, &session);
            saved.map_err(|_| Failure::Issue(Issue::SaveFailed, RemoteEffect::Confirmed))?;
            let login = session
                .enrollment()
                .map_err(|e| Failure::Issue(session_issue(e), RemoteEffect::Confirmed))?
                .login()
                .into();
            Ok(Outcome::Joined { login })
        }
        Kind::Sync => {
            let enrollment = session
                .enrollment()
                .map_err(|e| Failure::before(session_issue(e)))?;
            let now = jiff::Timestamp::now();
            let body = {
                let guard = workspace
                    .lock()
                    .map_err(|_| Failure::before(Issue::Unavailable))?;
                let workspace = guard
                    .as_ref()
                    .filter(|w| w.leaderboard_ready())
                    .ok_or_else(|| Failure::before(Issue::WorkspaceLocked))?;
                workspace
                    .leaderboard_upload(
                        enrollment.device_id().as_str(),
                        i32::from(action.days),
                        now,
                    )
                    .map_err(|e| Failure::before(upload_issue(e)))?
            };
            let approved = ApprovedSyncBody::from_json(body.as_bytes().to_vec(), now)
                .map_err(|_| Failure::before(Issue::InvalidData))?;
            publish(shared, action, Progress::Preview { body })?;
            await_confirmation(shared)?;
            ready(workspace).map_err(Failure::before)?;
            reserve(shared, action)?;
            let accepted = client
                .sync(enrollment.token(), &approved)
                .map_err(|e| Failure::Issue(remote_issue(e), RemoteEffect::Unknown))?;
            Ok(Outcome::Synced {
                accepted: accepted.to_string(),
            })
        }
        Kind::Leave => {
            let enrollment = session
                .enrollment()
                .map_err(|e| Failure::before(session_issue(e)))?;
            let login = enrollment.login().to_owned();
            ready(workspace).map_err(Failure::before)?;
            reserve(shared, action)?;
            client
                .leave(enrollment.token())
                .map_err(|e| Failure::Issue(remote_issue(e), RemoteEffect::Unknown))?;
            if session.clear_after_leave().is_err() {
                set_credential_unknown(shared);
                return Err(Failure::Issue(
                    Issue::LocalCleanupFailed,
                    RemoteEffect::Confirmed,
                ));
            }
            update_status(shared, &session);
            Ok(Outcome::Left {
                login: Some(login),
                credentials: LocalCredentialState::Removed,
            })
        }
        Kind::RecoverLeave | Kind::Forget => unreachable!(),
    }
}

fn ready(workspace: &Arc<Mutex<Option<Workspace>>>) -> Result<(), Issue> {
    let guard = workspace.lock().map_err(|_| Issue::Unavailable)?;
    if !guard.as_ref().is_some_and(Workspace::leaderboard_ready) {
        return Err(Issue::WorkspaceLocked);
    }
    Ok(())
}

fn inspect(workspace: &Arc<Mutex<Option<Workspace>>>) -> Result<View, Issue> {
    ready(workspace)?;
    let context = ClientContext::from_current_build().map_err(|_| Issue::Unavailable)?;
    let client = context.client();
    if !client.is_configured() {
        return Ok(View::default());
    }
    let mut view = View {
        configured: true,
        ..View::default()
    };
    let session = match context.open_session() {
        Ok(session) => session,
        Err(SessionError::Busy) => return Err(Issue::Busy),
        Err(SessionError::Unsupported) => return Err(Issue::Unsupported),
        Err(SessionError::InvalidCredential) => {
            view.enrollment = EnrollmentState::Invalid;
            return Ok(view);
        }
        Err(error) => {
            view.enrollment = EnrollmentState::Unavailable;
            view.status_issue = Some(session_issue(error));
            return Ok(view);
        }
    };
    ready(workspace)?;
    match session.status() {
        EnrollmentStatus::NotJoined => view.enrollment = EnrollmentState::NotJoined,
        EnrollmentStatus::Pending => view.enrollment = EnrollmentState::Pending,
        EnrollmentStatus::Joined { login } => {
            view.enrollment = EnrollmentState::Joined;
            view.login = Some(login.into());
        }
    }
    Ok(view)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BeginArgs {
    kind: Kind,
    days: Option<u16>,
    confirmed: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OperationArgs {
    operation_id: String,
}

#[tauri::command]
pub(crate) async fn load_leaderboard(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<View, Issue> {
    let _: crate::ListProfilesArgs = arguments(&request).map_err(|_| Issue::InvalidArguments)?;
    let worker = service.5.clone();
    tauri::async_runtime::spawn_blocking(move || worker.load())
        .await
        .map_err(|_| Issue::Unavailable)?
}
#[tauri::command]
pub(crate) async fn refresh_leaderboard(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<View, Issue> {
    let _: crate::ListProfilesArgs = arguments(&request).map_err(|_| Issue::InvalidArguments)?;
    let worker = service.5.clone();
    tauri::async_runtime::spawn_blocking(move || worker.refresh())
        .await
        .map_err(|_| Issue::Unavailable)?
}
#[tauri::command]
pub(crate) fn begin_leaderboard(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<View, Issue> {
    let args: BeginArgs = arguments(&request).map_err(|_| Issue::InvalidArguments)?;
    service.5.begin(args.kind, args.days, args.confirmed)
}
#[tauri::command]
pub(crate) fn cancel_leaderboard(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<View, Issue> {
    let args: OperationArgs = arguments(&request).map_err(|_| Issue::InvalidArguments)?;
    service.5.cancel(&args.operation_id)
}
#[tauri::command]
pub(crate) async fn open_leaderboard_login_page(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<(), Issue> {
    let args: OperationArgs = arguments(&request).map_err(|_| Issue::InvalidArguments)?;
    let worker = service.5.clone();
    tauri::async_runtime::spawn_blocking(move || worker.open_login_page(&args.operation_id))
        .await
        .map_err(|_| Issue::Unavailable)?
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConfirmArgs {
    operation_id: String,
    confirmed: bool,
}
#[tauri::command]
pub(crate) fn confirm_leaderboard_sync(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<View, Issue> {
    let args: ConfirmArgs = arguments(&request).map_err(|_| Issue::InvalidArguments)?;
    service.5.confirm_sync(&args.operation_id, args.confirmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker_with_finished_operation() -> (Worker, serde_json::Value) {
        let worker = Worker::start(Arc::new(Mutex::new(None)));
        let started = worker.begin(Kind::Join, None, true).ok().unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let view = worker.load().ok().unwrap();
            if !view.busy {
                assert!(matches!(
                    view.operation.as_ref().map(|operation| &operation.progress),
                    Some(Progress::Finished {
                        outcome: Outcome::Failed {
                            issue: Issue::WorkspaceLocked,
                            remote_effect: RemoteEffect::None,
                        },
                    })
                ));
                assert_eq!(view.operation.unwrap().id, started.operation.unwrap().id);
                break;
            }
            assert!(Instant::now() < deadline, "worker did not finish");
            std::thread::sleep(Duration::from_millis(1));
        }
        let receipt = serde_json::to_value(worker.refresh().ok().unwrap()).unwrap();
        assert_eq!(receipt["statusIssue"], "workspace_locked");
        (worker, receipt)
    }

    #[test]
    fn local_deletion_preflight_refusal_preserves_the_finished_operation() {
        let (worker, receipt) = worker_with_finished_operation();
        let lease = worker.local_deletion().ok().unwrap();
        assert!(matches!(worker.load(), Err(Issue::Busy)));
        assert!(matches!(worker.refresh(), Err(Issue::Busy)));
        assert!(matches!(
            worker.begin(Kind::Join, None, true),
            Err(Issue::Busy)
        ));
        assert!(matches!(worker.local_deletion(), Err(Issue::Busy)));
        drop(lease);
        assert_eq!(
            serde_json::to_value(worker.load().ok().unwrap()).unwrap(),
            receipt
        );
        assert_eq!(
            serde_json::to_value(worker.refresh().ok().unwrap()).unwrap(),
            receipt
        );
    }

    #[test]
    fn local_deletion_after_preflight_clears_the_finished_operation_on_drop() {
        for unwinding in [false, true] {
            let (worker, receipt) = worker_with_finished_operation();
            let completed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut lease = worker.local_deletion().ok().unwrap();
                lease.invalidate_cache();
                assert!(matches!(worker.load(), Err(Issue::Busy)));
                if unwinding {
                    panic!("synthetic deletion failure");
                }
            }));
            assert_eq!(completed.is_err(), unwinding);
            assert!(matches!(
                worker.cancel(receipt["operation"]["id"].as_str().unwrap()),
                Err(Issue::StaleOperation)
            ));
            assert!(matches!(worker.load(), Err(Issue::WorkspaceLocked)));
            assert!(worker.local_deletion().is_ok());
        }
    }

    #[test]
    fn begin_rejects_invalid_days_without_queueing_work() {
        let worker = Worker::start(Arc::new(Mutex::new(None)));
        for days in [0, 367, u16::MAX] {
            assert!(matches!(
                worker.begin(Kind::Sync, Some(days), true),
                Err(Issue::InvalidArguments)
            ));
        }
        for kind in [
            Kind::Join,
            Kind::Reconnect,
            Kind::Leave,
            Kind::RecoverLeave,
            Kind::Forget,
        ] {
            for days in [0, 1, 366] {
                assert!(matches!(
                    worker.begin(kind, Some(days), true),
                    Err(Issue::InvalidArguments)
                ));
            }
        }
        let control = worker.shared.control.lock().unwrap();
        assert!(control.queued.is_none());
        assert!(!control.working);
        let view = control.result.as_ref().ok().unwrap();
        assert!(!view.busy);
        assert!(view.operation.is_none());
    }

    #[test]
    fn begin_requires_consent_without_queueing_work() {
        let worker = Worker::start(Arc::new(Mutex::new(None)));
        for kind in [
            Kind::Join,
            Kind::Reconnect,
            Kind::Sync,
            Kind::Leave,
            Kind::RecoverLeave,
            Kind::Forget,
        ] {
            assert!(matches!(
                worker.begin(kind, None, false),
                Err(Issue::ConsentRequired)
            ));
        }
        let control = worker.shared.control.lock().unwrap();
        assert!(control.queued.is_none());
        assert!(!control.working);
        assert!(control.result.as_ref().ok().unwrap().operation.is_none());
    }

    #[test]
    fn begin_accepts_sync_day_boundaries_and_default() {
        for days in [None, Some(1), Some(30), Some(366)] {
            let worker = Worker::start(Arc::new(Mutex::new(None)));
            let view = worker.begin(Kind::Sync, days, true).ok().unwrap();
            assert!(view.busy);
            assert!(matches!(
                view.operation,
                Some(Operation {
                    kind: Kind::Sync,
                    progress: Progress::Preparing,
                    ..
                })
            ));
        }
    }
}
