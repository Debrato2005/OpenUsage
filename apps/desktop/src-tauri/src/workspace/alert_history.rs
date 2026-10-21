//! Saved quota alert evidence is distinct from desktop notification delivery.
use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SavedAlertHistory {
    events: Vec<SavedAlertEvent>,
    truncated: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedQuotaAlert {
    id: String,
    profile_id: String,
    profile_label: String,
    kind: &'static str,
    threshold_percent: Option<u8>,
    evidence_run_id: String,
    limit_id: String,
    window_role: &'static str,
    window_duration_mins: u64,
    evidence_at_utc: String,
    source_kind: &'static str,
    delivery_state: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedOperationalAlert {
    id: String,
    profile_id: String,
    profile_label: String,
    kind: &'static str,
    evidence_at_utc: String,
    source_kind: &'static str,
    delivery_state: &'static str,
}
#[derive(Serialize)]
#[serde(untagged)]
enum SavedAlertEvent {
    Quota(SavedQuotaAlert),
    Operational(SavedOperationalAlert),
}
impl SavedAlertEvent {
    fn order(&self) -> (&str, &str) {
        match self {
            Self::Quota(event) => (&event.evidence_at_utc, &event.id),
            Self::Operational(event) => (&event.evidence_at_utc, &event.id),
        }
    }
}

impl Workspace {
    pub fn load_alert_history(&self) -> Result<SavedAlertHistory, CommandError> {
        let history = self
            .storage
            .as_ref()
            .ok_or(CommandError::WorkspaceLocked)?
            .alert_history()
            .map_err(|_| CommandError::StorageUnavailable)?;
        let operational = self
            .storage
            .as_ref()
            .ok_or(CommandError::WorkspaceLocked)?
            .operational_alert_history()
            .map_err(|_| CommandError::StorageUnavailable)?;
        let mut events: Vec<SavedAlertEvent> = history
            .events
            .into_iter()
            .map(|event| {
                SavedAlertEvent::Quota(SavedQuotaAlert {
                    id: event.id,
                    profile_id: event.profile_id,
                    profile_label: event.profile_label,
                    kind: if event.threshold_percent.is_some() {
                        "quota_threshold"
                    } else {
                        "quota_reset"
                    },
                    threshold_percent: event.threshold_percent,
                    evidence_run_id: event.evidence_run_id,
                    limit_id: event.limit_id,
                    window_role: event.window_role.as_str(),
                    window_duration_mins: event.window_duration_mins,
                    evidence_at_utc: event.evidence_at_utc,
                    source_kind: event.source_kind.as_str(),
                    delivery_state: event.delivery_state.as_str(),
                })
            })
            .collect();
        events.extend(operational.events.into_iter().map(|event| {
            SavedAlertEvent::Operational(SavedOperationalAlert {
                id: event.id,
                profile_id: event.profile_id,
                profile_label: event.profile_label,
                kind: event.kind.as_str(),
                evidence_at_utc: event.evidence_at_utc,
                source_kind: event.source_kind.as_str(),
                delivery_state: event.delivery_state.as_str(),
            })
        }));
        events.sort_by(|a, b| b.order().cmp(&a.order()));
        let truncated = history.truncated || operational.truncated || events.len() > 100;
        events.truncate(100);
        Ok(SavedAlertHistory { events, truncated })
    }
}
