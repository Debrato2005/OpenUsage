//! Runtime monitoring permission comes only from a committed, validated login.
use super::*;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

const CYCLE: Duration = Duration::from_secs(300);
const STAGGER: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MonitoringState {
    #[default]
    Stopped,
    Running,
    Paused,
}
impl MonitoringState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Running => "running",
            Self::Paused => "paused",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MonitoringIssue {
    SignInRequired,
    AccountChanged,
    SourceChanged,
    ProfileUnavailable,
    Unsupported,
    StorageUnavailable,
    ProviderError,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitoringStatus {
    pub state: MonitoringState,
    pub reading_revision: String,
    pub profiles: Vec<MonitoringProfileStatus>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitoringProfileStatus {
    profile_id: String,
    state: MonitoringState,
    eligible: bool,
    issue: Option<MonitoringIssue>,
    next_read_in_seconds: Option<u16>,
    reading_active: bool,
}
#[derive(Default)]
struct Entry {
    eligibility: Option<provider::ProfileBinding>,
    grant_id: Option<String>,
    read_grant: Option<(String, String)>,
    state: MonitoringState,
    issue: Option<MonitoringIssue>,
    due: Option<Instant>,
    previous_start: Option<Instant>,
    request: Option<String>,
}
pub(super) struct RuntimeMonitoring {
    entries: BTreeMap<String, Entry>,
    last_start: Option<Instant>,
    reading_revision: String,
}
impl RuntimeMonitoring {
    pub(super) fn new() -> Result<Self, CommandError> {
        Ok(Self {
            entries: BTreeMap::new(),
            last_start: None,
            reading_revision: provider::new_id()?,
        })
    }
    pub(super) fn reset_after_restore(&mut self) {
        self.entries.clear();
        self.last_start = None;
    }
}
pub(crate) struct ScheduledCollection {
    pub profile_id: String,
    pub request_id: String,
    pub pending: provider::CompletingCollection,
    pub(super) revision: String,
}

impl Workspace {
    pub(crate) fn monitoring_state(&self) -> MonitoringState {
        if self.monitoring.entries.values().any(|e| {
            e.state == MonitoringState::Running
                && e.eligibility
                    .as_ref()
                    .is_some_and(|b| self.provider.binding_is_current(b))
        }) {
            MonitoringState::Running
        } else if self
            .monitoring
            .entries
            .values()
            .any(|e| e.state != MonitoringState::Stopped)
        {
            MonitoringState::Paused
        } else {
            MonitoringState::Stopped
        }
    }
    pub(super) fn monitoring_login_committed(
        &mut self,
        binding: provider::ProfileBinding,
        grant_id: String,
    ) {
        let entry = self
            .monitoring
            .entries
            .entry(binding.profile_id.clone())
            .or_default();
        entry.eligibility = Some(binding);
        entry.grant_id = Some(grant_id);
        entry.read_grant = None;
        entry.state = MonitoringState::Stopped;
        entry.issue = None;
        entry.due = None;
    }
    pub(crate) fn revoke_monitoring_profile(&mut self, id: &str, issue: MonitoringIssue) {
        if let Some(entry) = self.monitoring.entries.get_mut(id) {
            entry.eligibility = None;
            entry.grant_id = None;
            entry.read_grant = None;
            entry.state = MonitoringState::Paused;
            entry.issue = Some(issue);
            entry.due = None;
        }
    }
    pub(crate) fn revoke_all_monitoring(&mut self, issue: MonitoringIssue) {
        for entry in self.monitoring.entries.values_mut() {
            entry.eligibility = None;
            entry.grant_id = None;
            entry.read_grant = None;
            entry.state = MonitoringState::Paused;
            entry.issue = Some(issue);
            entry.due = None;
        }
    }
    pub(super) fn record_monitoring_read(&mut self, id: &str, request: &str) {
        if let Some(entry) = self.monitoring.entries.get_mut(id) {
            entry.read_grant = entry.grant_id.clone().map(|grant| (request.into(), grant));
        }
    }
    pub(crate) fn note_monitoring_read_error(
        &mut self,
        id: &str,
        request: &str,
        error: CommandError,
    ) {
        if matches!(
            error,
            CommandError::ProviderCollectionCancelled
                | CommandError::ProviderQuotaCancelled
                | CommandError::ProviderActivityCancelled
        ) {
            return;
        }
        if self.monitoring.entries.get(id).is_some_and(|entry| {
            entry.read_grant.as_ref().is_some_and(|(reading, grant)| {
                reading == request && entry.grant_id.as_ref() == Some(grant)
            })
        }) {
            self.note_monitoring_error(id, error);
        }
    }
    pub(crate) fn note_monitoring_error(&mut self, id: &str, error: CommandError) {
        let issue = match error {
            CommandError::ProviderAccountChanged
            | CommandError::ProviderCollectionAssociationChanged
            | CommandError::ProviderActivityAssociationChanged
            | CommandError::ProviderQuotaAssociationChanged => {
                Some(MonitoringIssue::AccountChanged)
            }
            CommandError::ProviderAccountIdentityUnavailable
            | CommandError::ProviderAuthenticationRequired => Some(MonitoringIssue::SignInRequired),
            CommandError::ProviderUnsupported => Some(MonitoringIssue::Unsupported),
            CommandError::ProviderHomeUnavailable
            | CommandError::ProviderProfileArchived
            | CommandError::ProfileNotFound => Some(MonitoringIssue::ProfileUnavailable),
            CommandError::StaleProviderApproval
            | CommandError::ProviderExecutableNotApproved
            | CommandError::ProviderSourceUnverified => Some(MonitoringIssue::SourceChanged),
            CommandError::WorkspaceLocked
            | CommandError::StorageUnavailable
            | CommandError::ProviderCollectionSaveFailed => {
                Some(MonitoringIssue::StorageUnavailable)
            }
            _ => None,
        };
        if let Some(issue) = issue {
            self.revoke_monitoring_profile(id, issue);
        } else if let Some(entry) = self.monitoring.entries.get_mut(id) {
            entry.issue = Some(MonitoringIssue::ProviderError);
        }
    }
    fn monitoring_binding_issue(
        &mut self,
        id: &str,
        binding: &provider::ProfileBinding,
    ) -> Option<MonitoringIssue> {
        match self.selected_provider_home(id) {
            Ok((home, _, _)) if home == binding.home_reference => {}
            Err(CommandError::WorkspaceLocked | CommandError::StorageUnavailable) => {
                return Some(MonitoringIssue::StorageUnavailable);
            }
            _ => return Some(MonitoringIssue::ProfileUnavailable),
        }
        if !self.provider.binding_is_current(binding) {
            Some(MonitoringIssue::SourceChanged)
        } else {
            None
        }
    }
    pub(crate) fn load_monitoring(&mut self) -> Result<MonitoringStatus, CommandError> {
        let profiles = self.list_profiles()?.profiles;
        let unavailable = self
            .monitoring
            .entries
            .keys()
            .filter(|id| !profiles.iter().any(|p| !p.archived && &p.id == *id))
            .cloned()
            .collect::<Vec<_>>();
        for id in unavailable {
            self.revoke_monitoring_profile(&id, MonitoringIssue::ProfileUnavailable);
        }
        let now = Instant::now();
        let mut statuses = Vec::new();
        for profile in profiles.into_iter().filter(|p| !p.archived) {
            let binding = self
                .monitoring
                .entries
                .get(&profile.id)
                .and_then(|e| e.eligibility.clone());
            if let Some(binding) = binding
                && let Some(issue) = self.monitoring_binding_issue(&profile.id, &binding)
            {
                self.revoke_monitoring_profile(&profile.id, issue);
            }
            let entry = self.monitoring.entries.get(&profile.id);
            let due = entry
                .filter(|e| e.state == MonitoringState::Running && e.request.is_none())
                .and_then(|e| e.due);
            let wait = due.map(|due| {
                let stagger = self
                    .monitoring
                    .last_start
                    .map_or(due, |last| due.max(last + STAGGER));
                let duration = stagger.saturating_duration_since(now);
                let seconds =
                    (duration.as_secs() + u64::from(duration.subsec_nanos() > 0)).min(4320) as u16;
                seconds.max(
                    self.provider
                        .scheduled_retry_after_seconds(&profile.id)
                        .unwrap_or(0),
                )
            });
            statuses.push(MonitoringProfileStatus {
                profile_id: profile.id,
                state: entry.map_or(MonitoringState::Stopped, |e| e.state),
                eligible: entry.is_some_and(|e| e.eligibility.is_some()),
                issue: entry.map_or(Some(MonitoringIssue::SignInRequired), |e| e.issue),
                next_read_in_seconds: wait,
                reading_active: entry.is_some_and(|e| e.request.is_some()),
            });
        }
        Ok(MonitoringStatus {
            state: self.monitoring_state(),
            reading_revision: self.monitoring.reading_revision.clone(),
            profiles: statuses,
        })
    }
    pub(crate) fn start_profile_monitoring(
        &mut self,
        id: &str,
        approval: &str,
    ) -> Result<MonitoringStatus, CommandError> {
        validate_profile_id(id)?;
        restore::validate_preview_id(approval)?;
        self.load_monitoring()?;
        let entry = self
            .monitoring
            .entries
            .get_mut(id)
            .filter(|e| {
                e.eligibility
                    .as_ref()
                    .is_some_and(|b| b.approval_id == approval)
            })
            .ok_or(CommandError::MonitoringNotEligible)?;
        if entry.state != MonitoringState::Running {
            entry.state = MonitoringState::Running;
            entry.issue = None;
            let now = Instant::now();
            entry.due = Some(
                entry
                    .previous_start
                    .map_or(now, |last| now.max(last + CYCLE)),
            );
        }
        self.load_monitoring()
    }
    /// Mark Pause before cancellation; the exact request remains active until its worker has joined.
    pub(crate) fn pause_profile_monitoring(
        &mut self,
        id: &str,
    ) -> Result<Option<String>, CommandError> {
        validate_profile_id(id)?;
        if !self
            .list_profiles()?
            .profiles
            .iter()
            .any(|p| p.id == id && !p.archived)
        {
            return Err(CommandError::ProfileNotFound);
        }
        let entry = self.monitoring.entries.entry(id.into()).or_default();
        entry.state = MonitoringState::Paused;
        entry.due = None;
        if entry.eligibility.is_none() && entry.issue.is_none() {
            entry.issue = Some(MonitoringIssue::SignInRequired);
        }
        Ok(entry.request.clone())
    }
    pub(crate) fn cancelled_scheduled_read(&mut self, id: &str, request: &str) {
        if let Some(entry) = self.monitoring.entries.get_mut(id)
            && entry.request.as_deref() == Some(request)
        {
            entry.request = None;
        }
    }
    pub(super) fn monitoring_profile_cancelled(&mut self, id: &str) {
        if let Some(entry) = self.monitoring.entries.get_mut(id) {
            entry.request = None;
        }
    }
    pub(crate) fn scheduled_cancellations(&self) -> Vec<(String, String)> {
        self.monitoring
            .entries
            .iter()
            .filter(|(_, e)| e.state != MonitoringState::Running || e.eligibility.is_none())
            .filter_map(|(id, e)| e.request.as_ref().map(|r| (id.clone(), r.clone())))
            .collect()
    }
    pub(crate) fn next_scheduled_collection(&mut self) -> Option<ScheduledCollection> {
        self.next_scheduled_collection_at(Instant::now())
    }
    pub(super) fn next_scheduled_collection_at(
        &mut self,
        now: Instant,
    ) -> Option<ScheduledCollection> {
        if self
            .monitoring
            .last_start
            .is_some_and(|last| now.saturating_duration_since(last) < STAGGER)
        {
            return None;
        }
        let id = self
            .monitoring
            .entries
            .iter()
            .filter(|(_, e)| {
                e.state == MonitoringState::Running
                    && e.request.is_none()
                    && e.due.is_some_and(|due| due <= now)
            })
            .min_by_key(|(_, e)| e.due)
            .map(|(id, _)| id.clone())?;
        let binding = self.monitoring.entries[&id].eligibility.clone()?;
        if let Some(issue) = self.monitoring_binding_issue(&id, &binding) {
            self.revoke_monitoring_profile(&id, issue);
            return None;
        }
        let revision = provider::new_id().ok()?;
        let result = (|| {
            let (home, storage_revision, path) = self.selected_provider_home(&id)?;
            let expected_binding = self
                .storage
                .as_ref()
                .ok_or(CommandError::WorkspaceLocked)?
                .profile_binding(&id)
                .map_err(|_| CommandError::ProviderCollectionSaveFailed)?
                .map(|b| b.binding_id);
            self.provider.begin_scheduled_collection(
                &id,
                &binding.approval_id,
                &home,
                storage_revision,
                &path,
                provider::ReadAssociation {
                    expected_binding,
                    review: None,
                },
            )
        })();
        let receipt = match result {
            Ok(r) => r,
            Err(error) => {
                let wait = if error == CommandError::ProviderRefreshBackoff {
                    self.provider
                        .scheduled_retry_after_seconds(&id)
                        .unwrap_or(1)
                        .max(1)
                } else {
                    1
                };
                let entry = self.monitoring.entries.get_mut(&id)?;
                entry.due = Some(
                    (now + Duration::from_secs(u64::from(wait)))
                        .max(entry.previous_start.map_or(now, |last| last + CYCLE)),
                );
                if !matches!(
                    error,
                    CommandError::ProviderRefreshBackoff
                        | CommandError::ProviderCapacityReached
                        | CommandError::ProviderCheckInProgress
                        | CommandError::ProviderLoginInProgress
                        | CommandError::ProviderQuotaInProgress
                        | CommandError::ProviderActivityInProgress
                        | CommandError::ProviderCollectionInProgress
                ) {
                    self.note_monitoring_error(&id, error);
                }
                return None;
            }
        };
        // Validation and admission can take time. The next cycle starts no sooner than
        // 300 seconds after this successful admission has returned.
        let admitted = now.max(Instant::now());
        self.monitoring.last_start = Some(admitted);
        let entry = self.monitoring.entries.get_mut(&id)?;
        entry.previous_start = Some(admitted);
        entry.due = Some(admitted + CYCLE);
        entry.request = Some(receipt.request_id.clone());
        self.record_monitoring_read(&id, &receipt.request_id);
        self.quota_requests
            .insert(id.clone(), receipt.request_id.clone());
        self.activity_requests
            .insert(id.clone(), receipt.request_id.clone());
        match self.provider.complete_collection(&id, &receipt.request_id) {
            Ok(pending) => Some(ScheduledCollection {
                profile_id: id,
                request_id: receipt.request_id,
                pending,
                revision,
            }),
            Err(error) => {
                self.monitoring.entries.get_mut(&id)?.request = None;
                self.note_monitoring_error(&id, error);
                None
            }
        }
    }
    pub(crate) fn finish_scheduled_collection(
        &mut self,
        id: &str,
        request: &str,
        revision: String,
        result: Result<provider::CompletedCollection, CommandError>,
    ) {
        let current = self.monitoring.entries.get(id).is_some_and(|e| {
            e.state == MonitoringState::Running
                && e.eligibility.is_some()
                && e.request.as_deref() == Some(request)
        });
        if !current {
            if let Ok(completed) = result {
                completed.abort();
            }
            self.cancelled_scheduled_read(id, request);
            return;
        }
        let result = result.and_then(|completed| self.finish_profile_collection_read(completed));
        self.cancelled_scheduled_read(id, request);
        match result {
            Ok(value) => {
                if value.accepted {
                    self.monitoring.reading_revision = revision;
                }
                if let Some(entry) = self.monitoring.entries.get_mut(id) {
                    if value.accepted {
                        entry.issue = None;
                    } else {
                        entry.state = MonitoringState::Paused;
                        entry.due = None;
                        entry.issue = Some(MonitoringIssue::Unsupported);
                    }
                }
            }
            Err(error) => {
                self.note_collection_failure(id, request);
                self.note_monitoring_read_error(id, request, error);
            }
        }
    }
}

impl ScheduledCollection {
    pub(crate) fn run(self, workspace: &std::sync::Arc<std::sync::Mutex<Option<Workspace>>>) {
        let result = self.pending.wait();
        if let Ok(mut state) = workspace.lock()
            && let Some(workspace) = state.as_mut()
        {
            workspace.finish_scheduled_collection(
                &self.profile_id,
                &self.request_id,
                self.revision,
                result,
            );
        }
    }
}
