//! Operational incidents use typed provider evidence, never synthetic quota windows.
//! The current incident journal survives restore; immutable outcomes explain events.
use crate::{
    ActivitySourceKind, AlertDeliveryState, NotificationOutcome, ResetAlertOverride,
    ResetNotification, Storage, StorageError,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{collections::BTreeSet, fmt::Write};
type Result<T> = std::result::Result<T, StorageError>;
pub(crate) const OUTCOME_COLUMNS: &str = "id,profile_id,home_reference,binding_id,source_kind,provider_version,observed_at_utc,accepted_at_utc,outcome,incident_id,accepted_run_id,transient_failures,accepted_order";
pub(crate) const STATE_COLUMNS: &str = "scope_key,profile_id,home_reference,binding_id,source_kind,authentication_incident_id,collection_incident_id,transient_failures,last_request_id,last_evidence_at_utc";
pub(crate) const EVENT_COLUMNS: &str = "id,event_key,profile_id,scope_key,incident_id,kind,evidence_outcome_id,evidence_at_utc,source_kind";
pub(crate) const PREFERENCE_COLUMNS: &str = "kind,enabled";
pub(crate) const OVERRIDE_COLUMNS: &str = "profile_id,kind,enabled";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationalAlertKind {
    AuthenticationRequired,
    CollectionFailure,
}
impl OperationalAlertKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuthenticationRequired => "authentication_required",
            Self::CollectionFailure => "collection_failure",
        }
    }
    pub(crate) fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "authentication_required" => Self::AuthenticationRequired,
            "collection_failure" => Self::CollectionFailure,
            _ => return None,
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationalFailureKind {
    AuthenticationRequired,
    TransientCollectionFailure,
}
impl OperationalFailureKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::AuthenticationRequired => "authentication_required",
            Self::TransientCollectionFailure => "transient_collection_failure",
        }
    }
    fn alert_kind(self) -> OperationalAlertKind {
        match self {
            Self::AuthenticationRequired => OperationalAlertKind::AuthenticationRequired,
            Self::TransientCollectionFailure => OperationalAlertKind::CollectionFailure,
        }
    }
}
#[derive(Clone)]
pub struct OperationalFailure {
    pub request_id: String,
    pub profile_id: String,
    pub expected_home_reference: String,
    pub expected_binding_id: Option<String>,
    pub provider_version: String,
    pub source_kind: ActivitySourceKind,
    pub observed_at_utc: String,
    pub kind: OperationalFailureKind,
}
impl std::fmt::Debug for OperationalFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OperationalFailure { .. }")
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OperationalAlertPreferences {
    pub enabled: bool,
    pub profile_overrides: Vec<ResetAlertOverride>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct OperationalAlertEvent {
    pub id: String,
    pub profile_id: String,
    pub profile_label: String,
    pub kind: OperationalAlertKind,
    pub evidence_at_utc: String,
    pub source_kind: ActivitySourceKind,
    pub delivery_state: AlertDeliveryState,
}
impl std::fmt::Debug for OperationalAlertEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OperationalAlertEvent { .. }")
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationalAlertHistory {
    pub events: Vec<OperationalAlertEvent>,
    pub truncated: bool,
}
#[derive(Clone, PartialEq, Eq)]
pub struct OperationalNotification {
    pub event_id: String,
    pub attempt_id: String,
    pub profile_id: String,
    pub profile_label: String,
    pub kind: OperationalAlertKind,
    pub evidence_at_utc: String,
}
impl std::fmt::Debug for OperationalNotification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OperationalNotification { .. }")
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum PendingNotification {
    Quota(ResetNotification),
    Operational(OperationalNotification),
}
fn read(_: rusqlite::Error) -> StorageError {
    StorageError::ReadFailed
}
fn write(_: rusqlite::Error) -> StorageError {
    StorageError::WriteFailed
}
fn invalid() -> StorageError {
    StorageError::InvalidAlertEvidence
}
fn uuid(value: &str) -> bool {
    crate::backup_validation::canonical_uuid(value)
}
fn source(value: &str) -> Option<ActivitySourceKind> {
    Some(match value {
        "official_artifact" => ActivitySourceKind::OfficialArtifact,
        "test_fixture" => ActivitySourceKind::TestFixture,
        _ => return None,
    })
}
pub(crate) fn register_id(connection: &Connection, id: &str) -> rusqlite::Result<()> {
    connection.execute("INSERT INTO notification_event_ids VALUES(?1)", [id])?;
    Ok(())
}
fn scope_key(profile: &str, home: &str, binding: Option<&str>, source: &str) -> String {
    let mut value = String::from("operational_scope:1");
    for field in [profile, home, binding.unwrap_or(""), source] {
        write!(value, ":{}:{field}", field.len()).expect("String write");
    }
    value
}
fn event_key(kind: OperationalAlertKind, scope: &str, incident: &str) -> String {
    format!(
        "operational_alert:1:{}:{}:{scope}:{}:{incident}",
        kind.as_str().len(),
        kind.as_str(),
        incident.len()
    )
}
fn valid_preferences(value: &OperationalAlertPreferences) -> bool {
    let mut ids = BTreeSet::new();
    value.profile_overrides.len() <= 100
        && value
            .profile_overrides
            .iter()
            .all(|item| uuid(&item.profile_id) && ids.insert(&item.profile_id))
}
pub(crate) fn load_preferences(
    connection: &Connection,
    kind: OperationalAlertKind,
) -> Result<OperationalAlertPreferences> {
    let enabled = connection
        .query_row(
            "SELECT enabled FROM operational_alert_preferences WHERE kind=?1",
            [kind.as_str()],
            |r| r.get(0),
        )
        .optional()
        .map_err(read)?
        .unwrap_or(false);
    let mut query=connection.prepare("SELECT profile_id,enabled FROM operational_alert_overrides WHERE kind=?1 ORDER BY profile_id").map_err(read)?;
    let profile_overrides = query
        .query_map([kind.as_str()], |r| {
            Ok(ResetAlertOverride {
                profile_id: r.get(0)?,
                enabled: r.get(1)?,
            })
        })
        .map_err(read)?
        .collect::<rusqlite::Result<_>>()
        .map_err(read)?;
    let value = OperationalAlertPreferences {
        enabled,
        profile_overrides,
    };
    if !valid_preferences(&value) {
        return Err(StorageError::ReadFailed);
    }
    Ok(value)
}
fn enabled(value: &OperationalAlertPreferences, profile: &str) -> bool {
    value
        .profile_overrides
        .iter()
        .find(|item| item.profile_id == profile)
        .map_or(value.enabled, |item| item.enabled)
}
pub(crate) fn save_preferences(
    connection: &Connection,
    kind: OperationalAlertKind,
    value: &OperationalAlertPreferences,
) -> Result<()> {
    if !valid_preferences(value) {
        return Err(StorageError::InvalidAlertPreferences);
    }
    for item in &value.profile_overrides {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1)",
                [&item.profile_id],
                |r| r.get(0),
            )
            .map_err(read)?;
        if !exists {
            return Err(StorageError::InvalidAlertPreferences);
        }
    }
    connection.execute("INSERT INTO operational_alert_preferences VALUES(?1,?2) ON CONFLICT(kind) DO UPDATE SET enabled=excluded.enabled",params![kind.as_str(),value.enabled]).map_err(write)?;
    connection
        .execute(
            "DELETE FROM operational_alert_overrides WHERE kind=?1",
            [kind.as_str()],
        )
        .map_err(write)?;
    for item in &value.profile_overrides {
        connection
            .execute(
                "INSERT INTO operational_alert_overrides VALUES(?1,?2,?3)",
                params![item.profile_id, kind.as_str(), item.enabled],
            )
            .map_err(write)?;
    }
    let mut query=connection.prepare("SELECT e.id,e.profile_id FROM operational_alert_events e JOIN reset_notification_outbox o ON o.event_id=e.id WHERE e.kind=?1 AND o.state='queued'").map_err(read)?;
    let queued = query
        .query_map([kind.as_str()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(read)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(read)?;
    drop(query);
    for (id, profile) in queued {
        if !enabled(value, &profile) {
            connection.execute("UPDATE reset_notification_outbox SET state='suppressed' WHERE event_id=?1 AND state='queued'",[id]).map_err(write)?;
        }
    }
    Ok(())
}
#[derive(Default)]
struct IncidentState {
    authentication: Option<String>,
    collection: Option<String>,
    failures: u8,
    last_evidence: Option<String>,
}
fn load_state(connection: &Connection, scope: &str) -> Result<IncidentState> {
    connection.query_row("SELECT authentication_incident_id,collection_incident_id,transient_failures,last_evidence_at_utc FROM operational_alert_state WHERE scope_key=?1",[scope],|r|Ok(IncidentState{authentication:r.get(0)?,collection:r.get(1)?,failures:r.get(2)?,last_evidence:r.get(3)?})).optional().map_err(read).map(|v|v.unwrap_or_default())
}
fn require_target(connection: &Connection, failure: &OperationalFailure) -> Result<bool> {
    let current:Option<Option<String>>=connection.query_row("SELECT b.binding_id FROM profiles p LEFT JOIN profile_bindings b ON b.profile_id=p.id WHERE p.id=?1 AND p.approved_home_reference=?2 AND p.archived=0 AND p.environment='native'",params![failure.profile_id,failure.expected_home_reference],|r|r.get(0)).optional().map_err(read)?;
    Ok(current.is_some_and(|binding| binding == failure.expected_binding_id))
}
type RecordedFailure = (
    String,
    String,
    Option<String>,
    String,
    String,
    String,
    String,
);

impl Storage {
    pub fn load_authentication_alert_preferences(&self) -> Result<OperationalAlertPreferences> {
        load_preferences(
            &self.connection,
            OperationalAlertKind::AuthenticationRequired,
        )
    }
    pub fn load_collection_failure_alert_preferences(&self) -> Result<OperationalAlertPreferences> {
        load_preferences(&self.connection, OperationalAlertKind::CollectionFailure)
    }
    /// The native supervisor retains approval, request, target and monotonic-age
    /// fences until this transaction commits. A typed failure contains no raw error.
    pub fn record_operational_failure(
        &mut self,
        failure: &OperationalFailure,
        now_utc: &str,
    ) -> Result<()> {
        if !uuid(&failure.request_id)
            || !uuid(&failure.profile_id)
            || !uuid(&failure.expected_home_reference)
            || failure
                .expected_binding_id
                .as_deref()
                .is_some_and(|id| !uuid(id))
            || !crate::activity::safe_text(&failure.provider_version, 128)
            || !crate::activity::activity_utc_timestamp(&failure.observed_at_utc)
            || !crate::activity::activity_utc_timestamp(now_utc)
        {
            return Err(invalid());
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write)?;
        let duplicate:Option<RecordedFailure>=tx.query_row("SELECT profile_id,home_reference,binding_id,source_kind,provider_version,observed_at_utc,outcome FROM operational_alert_outcomes WHERE id=?1",[&failure.request_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).optional().map_err(read)?;
        if let Some(existing) = duplicate {
            if existing
                != (
                    failure.profile_id.clone(),
                    failure.expected_home_reference.clone(),
                    failure.expected_binding_id.clone(),
                    failure.source_kind.as_str().into(),
                    failure.provider_version.clone(),
                    failure.observed_at_utc.clone(),
                    failure.kind.as_str().into(),
                )
            {
                return Err(invalid());
            }
            return Ok(());
        }
        if !crate::threshold_alerts::fresh(
            &failure.observed_at_utc,
            &failure.observed_at_utc,
            now_utc,
        ) || !require_target(&tx, failure)?
        {
            return Err(invalid());
        }
        let newest:Option<String>=tx.query_row("SELECT max(observed_at_utc) FROM collection_runs WHERE profile_id=?1 AND binding_id IS ?2 AND source_kind=?3",params![failure.profile_id,failure.expected_binding_id,failure.source_kind.as_str()],|r|r.get(0)).map_err(read)?;
        if newest
            .as_deref()
            .is_some_and(|newest| newest > failure.observed_at_utc.as_str())
        {
            return Err(invalid());
        }
        if failure.kind == OperationalFailureKind::AuthenticationRequired {
            tx.execute("UPDATE profiles SET connection_state='authentication_required',updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND connection_state!='authentication_required'",[&failure.profile_id]).map_err(write)?;
            let authorized:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM collection_runs r JOIN profile_identity_events i ON i.id=r.binding_id AND i.profile_id=r.profile_id WHERE r.profile_id=?1 AND r.binding_id=?2 AND i.home_reference=?3 AND r.source_kind=?4 AND r.outcome='accepted' AND r.observed_at_utc<=?5)",params![failure.profile_id,failure.expected_binding_id,failure.expected_home_reference,failure.source_kind.as_str(),failure.observed_at_utc],|r|r.get(0)).map_err(read)?;
            if !authorized {
                return tx.commit().map_err(write);
            }
        }
        let scope = scope_key(
            &failure.profile_id,
            &failure.expected_home_reference,
            failure.expected_binding_id.as_deref(),
            failure.source_kind.as_str(),
        );
        let mut state = load_state(&tx, &scope)?;
        if state
            .last_evidence
            .as_deref()
            .is_some_and(|time| time > failure.observed_at_utc.as_str())
        {
            return Err(invalid());
        }
        let emit = match failure.kind {
            OperationalFailureKind::AuthenticationRequired => {
                let first = state.authentication.is_none();
                if first {
                    state.authentication = Some(failure.request_id.clone());
                }
                first
            }
            OperationalFailureKind::TransientCollectionFailure => {
                if state.collection.is_none() {
                    state.collection = Some(failure.request_id.clone());
                }
                let before = state.failures;
                state.failures = state.failures.saturating_add(1).min(3);
                before == 2
            }
        };
        let incident = match failure.kind {
            OperationalFailureKind::AuthenticationRequired => state.authentication.as_deref(),
            OperationalFailureKind::TransientCollectionFailure => state.collection.as_deref(),
        }
        .ok_or_else(invalid)?;
        tx.execute("INSERT INTO operational_alert_outcomes VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,NULL,?11,(SELECT coalesce(max(accepted_order),0)+1 FROM operational_alert_outcomes))",params![failure.request_id,failure.profile_id,failure.expected_home_reference,failure.expected_binding_id,failure.source_kind.as_str(),failure.provider_version,failure.observed_at_utc,now_utc,failure.kind.as_str(),incident,state.failures]).map_err(write)?;
        tx.execute("INSERT INTO operational_alert_state VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT(scope_key) DO UPDATE SET authentication_incident_id=excluded.authentication_incident_id,collection_incident_id=excluded.collection_incident_id,transient_failures=excluded.transient_failures,last_request_id=excluded.last_request_id,last_evidence_at_utc=excluded.last_evidence_at_utc",params![scope,failure.profile_id,failure.expected_home_reference,failure.expected_binding_id,failure.source_kind.as_str(),state.authentication,state.collection,state.failures,failure.request_id,failure.observed_at_utc]).map_err(write)?;
        if emit {
            record_event(&tx, failure, &scope, incident)?;
        }
        tx.commit().map_err(write)
    }
}
fn record_event(
    connection: &Connection,
    failure: &OperationalFailure,
    scope: &str,
    incident: &str,
) -> Result<()> {
    let kind = failure.kind.alert_kind();
    let key = event_key(kind, scope, incident);
    let id = crate::activity::new_uuid().map_err(|_| StorageError::WriteFailed)?;
    register_id(connection, &id).map_err(write)?;
    connection
        .execute(
            "INSERT INTO operational_alert_events VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                id,
                key,
                failure.profile_id,
                scope,
                incident,
                kind.as_str(),
                failure.request_id,
                failure.observed_at_utc,
                failure.source_kind.as_str()
            ],
        )
        .map_err(write)?;
    if crate::reset_notifications::source_eligible(failure.source_kind.as_str()) {
        let prefs = load_preferences(connection, kind)?;
        connection
            .execute(
                "INSERT INTO reset_notification_outbox VALUES(?1,?2,NULL,NULL,NULL)",
                params![
                    id,
                    if enabled(&prefs, &failure.profile_id) {
                        "queued"
                    } else {
                        "suppressed"
                    }
                ],
            )
            .map_err(write)?;
    }
    Ok(())
}

pub(crate) fn accepted_success(connection: &Connection, run: &str, now: &str) -> Result<()> {
    let (profile,home,binding,source,version,observed,completed):(String,String,String,String,String,String,String)=connection.query_row("SELECT r.profile_id,i.home_reference,r.binding_id,r.source_kind,r.provider_version,r.observed_at_utc,r.completed_at_utc FROM collection_runs r JOIN profile_identity_events i ON i.id=r.binding_id AND i.profile_id=r.profile_id WHERE r.id=?1",[run],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).map_err(read)?;
    if !crate::threshold_alerts::fresh(&observed, &completed, now) {
        return Ok(());
    }
    let active:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM operational_alert_state WHERE profile_id=?1 AND home_reference=?2 AND (binding_id=?3 OR binding_id IS NULL) AND source_kind=?4 AND last_evidence_at_utc<=?5 AND (authentication_incident_id IS NOT NULL OR transient_failures>0))",params![profile,home,binding,source,observed],|r|r.get(0)).map_err(read)?;
    if !active {
        return Ok(());
    }
    connection.execute("INSERT INTO operational_alert_outcomes VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'accepted_success',NULL,?1,0,(SELECT coalesce(max(accepted_order),0)+1 FROM operational_alert_outcomes))",params![run,profile,home,binding,source,version,observed,now]).map_err(write)?;
    connection.execute("UPDATE operational_alert_state SET authentication_incident_id=NULL,collection_incident_id=NULL,transient_failures=0,last_request_id=?6,last_evidence_at_utc=?5 WHERE profile_id=?1 AND home_reference=?2 AND (binding_id=?3 OR binding_id IS NULL) AND source_kind=?4 AND last_evidence_at_utc<=?5",params![profile,home,binding,source,observed,run]).map_err(write)?;
    connection.execute("UPDATE reset_notification_outbox SET state='suppressed' WHERE state='queued' AND event_id IN(SELECT e.id FROM operational_alert_events e JOIN operational_alert_state s ON s.scope_key=e.scope_key WHERE s.profile_id=?1 AND s.home_reference=?2 AND s.source_kind=?3 AND ((e.kind='authentication_required' AND s.authentication_incident_id IS NULL) OR (e.kind='collection_failure' AND s.collection_incident_id IS NULL)))",params![profile,home,source]).map_err(write)?;
    Ok(())
}
impl Storage {
    pub fn save_notification_preferences(
        &mut self,
        reset: &crate::ResetAlertPreferences,
        usage: &crate::ThresholdAlertPreferences,
        authentication: &OperationalAlertPreferences,
        collection: &OperationalAlertPreferences,
    ) -> Result<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write)?;
        crate::reset_notifications::save_reset_preferences(&tx, reset)?;
        crate::threshold_alerts::save(&tx, usage)?;
        save_preferences(
            &tx,
            OperationalAlertKind::AuthenticationRequired,
            authentication,
        )?;
        save_preferences(&tx, OperationalAlertKind::CollectionFailure, collection)?;
        tx.commit().map_err(write)
    }
    pub fn operational_alert_history(&self) -> Result<OperationalAlertHistory> {
        let mut query=self.connection.prepare("SELECT e.id,e.profile_id,p.name,e.kind,e.evidence_at_utc,e.source_kind,COALESCE(o.state,'recorded') FROM operational_alert_events e JOIN profiles p ON p.id=e.profile_id LEFT JOIN reset_notification_outbox o ON o.event_id=e.id ORDER BY e.evidence_at_utc DESC,e.id DESC LIMIT 101").map_err(read)?;
        let mut rows = query.query([]).map_err(read)?;
        let mut events = Vec::new();
        while let Some(row) = rows.next().map_err(read)? {
            let event = OperationalAlertEvent {
                id: row.get(0).map_err(read)?,
                profile_id: row.get(1).map_err(read)?,
                profile_label: row.get(2).map_err(read)?,
                kind: OperationalAlertKind::parse(&row.get::<_, String>(3).map_err(read)?)
                    .ok_or(StorageError::ReadFailed)?,
                evidence_at_utc: row.get(4).map_err(read)?,
                source_kind: source(&row.get::<_, String>(5).map_err(read)?)
                    .ok_or(StorageError::ReadFailed)?,
                delivery_state: crate::reset_notifications::delivery_state(
                    &row.get::<_, String>(6).map_err(read)?,
                )
                .ok_or(StorageError::ReadFailed)?,
            };
            if !uuid(&event.id)
                || !uuid(&event.profile_id)
                || !crate::activity::activity_utc_timestamp(&event.evidence_at_utc)
                || event.delivery_state == AlertDeliveryState::Covered
            {
                return Err(StorageError::ReadFailed);
            }
            events.push(event);
        }
        let truncated = events.len() > 100;
        events.truncate(100);
        Ok(OperationalAlertHistory { events, truncated })
    }
    pub fn claim_notification(&mut self, now_utc: &str) -> Result<Option<PendingNotification>> {
        if let Some(quota) = self.claim_usage_notification(now_utc)? {
            return Ok(Some(PendingNotification::Quota(quota)));
        }
        self.claim_operational_notification(now_utc)
            .map(|value| value.map(PendingNotification::Operational))
    }
    pub fn complete_notification(
        &mut self,
        event_id: &str,
        attempt_id: &str,
        outcome: NotificationOutcome,
    ) -> Result<()> {
        self.complete_reset_notification(event_id, attempt_id, outcome)
    }
    fn claim_operational_notification(
        &mut self,
        now_utc: &str,
    ) -> Result<Option<OperationalNotification>> {
        if !crate::activity::activity_utc_timestamp(now_utc) {
            return Err(invalid());
        }
        let now = now_utc.parse::<jiff::Timestamp>().map_err(|_| invalid())?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write)?;
        let quiet =
            crate::reset_notifications::quiet(&crate::reset_notifications::load(&tx)?, now)?;
        let auth_prefs = load_preferences(&tx, OperationalAlertKind::AuthenticationRequired)?;
        let failure_prefs = load_preferences(&tx, OperationalAlertKind::CollectionFailure)?;
        let mut query=tx.prepare("SELECT e.id,e.event_key,e.profile_id,p.name,e.kind,e.evidence_at_utc,e.source_kind,e.incident_id,f.home_reference,f.binding_id,p.approved_home_reference,p.archived,b.binding_id,s.authentication_incident_id,s.collection_incident_id,s.last_evidence_at_utc FROM reset_notification_outbox o JOIN operational_alert_events e ON e.id=o.event_id JOIN operational_alert_outcomes f ON f.id=e.evidence_outcome_id JOIN profiles p ON p.id=e.profile_id LEFT JOIN profile_bindings b ON b.profile_id=p.id LEFT JOIN operational_alert_state s ON s.scope_key=e.scope_key WHERE o.state='queued' ORDER BY e.evidence_at_utc,e.id LIMIT 256").map_err(read)?;
        let candidates = query
            .query_map([], |r| {
                Ok(OperationalCandidate {
                    id: r.get(0)?,
                    key: r.get(1)?,
                    profile: r.get(2)?,
                    label: r.get(3)?,
                    kind: r.get(4)?,
                    evidence: r.get(5)?,
                    source: r.get(6)?,
                    incident: r.get(7)?,
                    home: r.get(8)?,
                    binding: r.get(9)?,
                    current_home: r.get(10)?,
                    archived: r.get(11)?,
                    current_binding: r.get(12)?,
                    auth_incident: r.get(13)?,
                    collection_incident: r.get(14)?,
                    latest_evidence: r.get(15)?,
                })
            })
            .map_err(read)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(read)?;
        drop(query);
        for candidate in candidates {
            let kind =
                OperationalAlertKind::parse(&candidate.kind).ok_or(StorageError::ReadFailed)?;
            let prefs = match kind {
                OperationalAlertKind::AuthenticationRequired => &auth_prefs,
                OperationalAlertKind::CollectionFailure => &failure_prefs,
            };
            let active = match kind {
                OperationalAlertKind::AuthenticationRequired => candidate.auth_incident.as_deref(),
                OperationalAlertKind::CollectionFailure => candidate.collection_incident.as_deref(),
            };
            if candidate.archived
                || candidate.home != candidate.current_home
                || candidate.binding != candidate.current_binding
                || !crate::reset_notifications::source_eligible(&candidate.source)
                || !enabled(prefs, &candidate.profile)
                || active != Some(candidate.incident.as_str())
            {
                tx.execute("UPDATE reset_notification_outbox SET state='suppressed' WHERE event_id=?1 AND state='queued'",[&candidate.id]).map_err(write)?;
                continue;
            }
            if candidate.evidence.as_str() > now_utc
                || candidate
                    .latest_evidence
                    .as_deref()
                    .is_none_or(|latest| latest > now_utc)
                || quiet
            {
                continue;
            }
            let consumed: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM reset_notification_receipts WHERE event_key=?1)",
                    [&candidate.key],
                    |r| r.get(0),
                )
                .map_err(read)?;
            if consumed {
                tx.execute("UPDATE reset_notification_outbox SET state='suppressed' WHERE event_id=?1 AND state='queued'",[&candidate.id]).map_err(write)?;
                continue;
            }
            let attempt = crate::activity::new_uuid().map_err(|_| StorageError::WriteFailed)?;
            tx.execute(
                "INSERT INTO reset_notification_receipts VALUES(?1,?2,'uncertain',?3,NULL,NULL)",
                params![candidate.key, attempt, now_utc],
            )
            .map_err(write)?;
            tx.execute("UPDATE reset_notification_outbox SET state='uncertain',attempt_id=?2,attempted_at_utc=?3 WHERE event_id=?1 AND state='queued'",params![candidate.id,attempt,now_utc]).map_err(write)?;
            let notification = OperationalNotification {
                event_id: candidate.id,
                attempt_id: attempt,
                profile_id: candidate.profile,
                profile_label: candidate.label,
                kind,
                evidence_at_utc: candidate.evidence,
            };
            tx.commit().map_err(write)?;
            return Ok(Some(notification));
        }
        tx.commit().map_err(write)?;
        Ok(None)
    }
}
struct OperationalCandidate {
    id: String,
    key: String,
    profile: String,
    label: String,
    kind: String,
    evidence: String,
    source: String,
    incident: String,
    home: String,
    binding: Option<String>,
    current_home: String,
    archived: bool,
    current_binding: Option<String>,
    auth_incident: Option<String>,
    collection_incident: Option<String>,
    latest_evidence: Option<String>,
}

fn take_field<'a>(body: &mut &'a str) -> Option<&'a str> {
    let (raw, rest) = body.split_once(':')?;
    let length = raw.parse::<usize>().ok()?;
    if length.to_string() != raw {
        return None;
    }
    let field = rest.get(..length)?;
    *body = rest.get(length..)?;
    Some(field)
}
fn scope_fields(mut body: &str) -> Option<Vec<&str>> {
    body = body.strip_prefix("operational_scope:1:")?;
    let mut fields = Vec::with_capacity(4);
    for index in 0..4 {
        fields.push(take_field(&mut body)?);
        if index < 3 {
            body = body.strip_prefix(':')?;
        }
    }
    if !body.is_empty() {
        return None;
    }
    Some(fields)
}
fn valid_scope_fields(fields: &[&str]) -> bool {
    uuid(fields[0])
        && uuid(fields[1])
        && (fields[2].is_empty() || uuid(fields[2]))
        && source(fields[3]).is_some()
}
pub(crate) fn valid_receipt_key(key: &str) -> bool {
    let Some(mut body) = key.strip_prefix("operational_alert:1:") else {
        return false;
    };
    let Some(kind) = take_field(&mut body) else {
        return false;
    };
    if OperationalAlertKind::parse(kind).is_none() {
        return false;
    }
    let Some(rest) = body.strip_prefix(":operational_scope:1:") else {
        return false;
    };
    body = rest;
    let mut fields = Vec::with_capacity(4);
    for _ in 0..4 {
        let Some(field) = take_field(&mut body) else {
            return false;
        };
        fields.push(field);
        let Some(rest) = body.strip_prefix(':') else {
            return false;
        };
        body = rest;
    }
    let Some(incident) = take_field(&mut body) else {
        return false;
    };
    body.is_empty() && valid_scope_fields(&fields) && uuid(incident)
}

pub(crate) fn validate(
    connection: &Connection,
    schema: &str,
) -> std::result::Result<(), crate::BackupValidationError> {
    use crate::backup_validation::{canonical_uuid, optional_text, require, sql_error, text};
    require(matches!(schema, "main" | "openusage_checkpoint"))?;
    let invalid = || crate::BackupValidationError::InvalidDatabase;
    // The common root has exactly one typed evidence child, with no orphan IDs.
    let bad:i64=connection.query_row(&format!("SELECT count(*) FROM {schema}.notification_event_ids n LEFT JOIN {schema}.alert_events q ON q.id=n.id LEFT JOIN {schema}.operational_alert_events o ON o.id=n.id WHERE (q.id IS NULL AND o.id IS NULL) OR (q.id IS NOT NULL AND o.id IS NOT NULL)"),[],|r|r.get(0)).map_err(sql_error)?;
    require(bad == 0)?;
    let mut query=connection.prepare(&format!("SELECT {OUTCOME_COLUMNS} FROM {schema}.operational_alert_outcomes ORDER BY accepted_order")).map_err(sql_error)?;
    let mut rows = query.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        for index in [0, 1, 2] {
            require(canonical_uuid(text(row, index)?))?;
        }
        let binding = optional_text(row, 3)?;
        let outcome = text(row, 8)?;
        let incident = optional_text(row, 9)?;
        let run = optional_text(row, 10)?;
        let count: i64 = row.get(11).map_err(sql_error)?;
        require(
            binding.is_none_or(canonical_uuid)
                && source(text(row, 4)?).is_some()
                && crate::activity::safe_text(text(row, 5)?, 128)
                && crate::activity::activity_utc_timestamp(text(row, 6)?)
                && crate::activity::activity_utc_timestamp(text(row, 7)?)
                && crate::threshold_alerts::fresh(text(row, 6)?, text(row, 6)?, text(row, 7)?)
                && matches!(
                    outcome,
                    "authentication_required" | "transient_collection_failure" | "accepted_success"
                )
                && (0..=3).contains(&count)
                && row.get::<_, i64>(12).map_err(sql_error)? > 0,
        )?;
        if let Some(binding) = binding {
            let home:String=connection.query_row(&format!("SELECT home_reference FROM {schema}.profile_identity_events WHERE id=?1 AND profile_id=?2"),params![binding,text(row,1)?],|r|r.get(0)).map_err(sql_error)?;
            require(home == text(row, 2)?)?;
        }
        if outcome == "accepted_success" {
            require(incident.is_none() && count == 0 && run == Some(text(row, 0)?))?;
            let matched:bool=connection.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {schema}.collection_runs r WHERE r.id=?1 AND r.profile_id=?2 AND r.binding_id=?3 AND r.source_kind=?4 AND r.provider_version=?5 AND r.observed_at_utc=?6 AND r.completed_at_utc<=?7 AND r.outcome='accepted')"),params![run,text(row,1)?,binding,text(row,4)?,text(row,5)?,text(row,6)?,text(row,7)?],|r|r.get(0)).map_err(sql_error)?;
            require(matched)?;
        } else {
            require(run.is_none() && incident.is_some_and(canonical_uuid))?;
            if outcome == "authentication_required" {
                require(binding.is_some())?;
                let authorized:bool=connection.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {schema}.collection_runs r WHERE r.profile_id=?1 AND r.binding_id=?2 AND r.source_kind=?3 AND r.observed_at_utc<=?4 AND r.outcome='accepted')"),params![text(row,1)?,binding,text(row,4)?,text(row,6)?],|r|r.get(0)).map_err(sql_error)?;
                require(authorized)?;
            } else {
                require((1..=3).contains(&count))?;
            }
        }
    }
    let mut query = connection
        .prepare(&format!(
            "SELECT {STATE_COLUMNS} FROM {schema}.operational_alert_state"
        ))
        .map_err(sql_error)?;
    let mut rows = query.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        let scope = text(row, 0)?;
        let fields = scope_fields(scope).ok_or_else(invalid)?;
        let binding = optional_text(row, 3)?;
        let auth = optional_text(row, 5)?;
        let collection = optional_text(row, 6)?;
        let count: i64 = row.get(7).map_err(sql_error)?;
        require(
            valid_scope_fields(&fields)
                && canonical_uuid(text(row, 1)?)
                && canonical_uuid(text(row, 2)?)
                && binding.is_none_or(canonical_uuid)
                && source(text(row, 4)?).is_some()
                && scope == scope_key(text(row, 1)?, text(row, 2)?, binding, text(row, 4)?)
                && auth.is_none_or(canonical_uuid)
                && collection.is_none_or(canonical_uuid)
                && (0..=3).contains(&count)
                && (count == 0) == collection.is_none()
                && canonical_uuid(text(row, 8)?)
                && crate::activity::activity_utc_timestamp(text(row, 9)?),
        )?;
        // A preserved journal row may refer to history removed by restore. Its
        // typed scope remains authoritative; it cannot manufacture quota proof.
        let last:Option<(String,String,Option<String>,String,String)>=connection.query_row(&format!("SELECT profile_id,home_reference,binding_id,source_kind,outcome FROM {schema}.operational_alert_outcomes WHERE id=?1"),[text(row,8)?],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(sql_error)?;
        if let Some((profile, home, last_binding, source, outcome)) = last {
            require(
                profile == text(row, 1)?
                    && home == text(row, 2)?
                    && source == text(row, 4)?
                    && (last_binding.as_deref() == binding
                        || (binding.is_none() && outcome == "accepted_success")),
            )?;
        }
    }
    let mut query=connection.prepare(&format!("SELECT e.id,e.event_key,e.profile_id,e.scope_key,e.incident_id,e.kind,e.evidence_outcome_id,e.evidence_at_utc,e.source_kind,f.profile_id,f.home_reference,f.binding_id,f.source_kind,f.observed_at_utc,f.outcome,f.incident_id,f.transient_failures,f.accepted_order FROM {schema}.operational_alert_events e JOIN {schema}.operational_alert_outcomes f ON f.id=e.evidence_outcome_id")).map_err(sql_error)?;
    let mut rows = query.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        let kind = OperationalAlertKind::parse(text(row, 5)?).ok_or_else(invalid)?;
        let scope = text(row, 3)?;
        let incident = text(row, 4)?;
        require(
            canonical_uuid(text(row, 0)?)
                && canonical_uuid(text(row, 2)?)
                && canonical_uuid(incident)
                && canonical_uuid(text(row, 6)?)
                && text(row, 1)? == event_key(kind, scope, incident)
                && valid_receipt_key(text(row, 1)?)
                && text(row, 2)? == text(row, 9)?
                && scope
                    == scope_key(
                        text(row, 9)?,
                        text(row, 10)?,
                        optional_text(row, 11)?,
                        text(row, 12)?,
                    )
                && text(row, 7)? == text(row, 13)?
                && text(row, 8)? == text(row, 12)?
                && optional_text(row, 15)? == Some(incident),
        )?;
        let expected = match kind {
            OperationalAlertKind::AuthenticationRequired => "authentication_required",
            OperationalAlertKind::CollectionFailure => "transient_collection_failure",
        };
        require(text(row, 14)? == expected)?;
        if kind == OperationalAlertKind::CollectionFailure {
            require(row.get::<_, i64>(16).map_err(sql_error)? == 3)?;
        }
        let first:Option<i64>=connection.query_row(&format!("SELECT min(accepted_order) FROM {schema}.operational_alert_outcomes WHERE profile_id=?1 AND home_reference=?2 AND binding_id IS ?3 AND source_kind=?4 AND incident_id=?5 AND outcome=?6 AND (?7='authentication_required' OR transient_failures=3)"),params![text(row,2)?,text(row,10)?,optional_text(row,11)?,text(row,8)?,incident,expected,kind.as_str()],|r|r.get(0)).map_err(sql_error)?;
        require(first == Some(row.get(17).map_err(sql_error)?))?;
    }
    let mut query = connection
        .prepare(&format!(
            "SELECT kind,enabled FROM {schema}.operational_alert_preferences"
        ))
        .map_err(sql_error)?;
    let mut rows = query.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        require(
            OperationalAlertKind::parse(text(row, 0)?).is_some()
                && matches!(row.get::<_, i64>(1).map_err(sql_error)?, 0 | 1),
        )?;
    }
    let mut query = connection
        .prepare(&format!(
            "SELECT profile_id,kind,enabled FROM {schema}.operational_alert_overrides"
        ))
        .map_err(sql_error)?;
    let mut rows = query.query([]).map_err(sql_error)?;
    let mut auth = 0;
    let mut collection = 0;
    while let Some(row) = rows.next().map_err(sql_error)? {
        match OperationalAlertKind::parse(text(row, 1)?) {
            Some(OperationalAlertKind::AuthenticationRequired) => auth += 1,
            Some(OperationalAlertKind::CollectionFailure) => collection += 1,
            None => return Err(invalid()),
        };
        require(
            auth <= 100
                && collection <= 100
                && canonical_uuid(text(row, 0)?)
                && matches!(row.get::<_, i64>(2).map_err(sql_error)?, 0 | 1),
        )?;
    }
    Ok(())
}

pub(crate) fn copy_records(
    source: &Connection,
    target: &Connection,
    schema: &str,
    restore: bool,
) -> std::result::Result<(), crate::BackupValidationError> {
    use crate::backup_validation::{copy_fields, require, sql_error};
    require(matches!(schema, "main" | "openusage_checkpoint"))?;
    for (table, columns) in [
        ("operational_alert_outcomes", OUTCOME_COLUMNS),
        ("operational_alert_events", EVENT_COLUMNS),
        ("operational_alert_preferences", PREFERENCE_COLUMNS),
        ("operational_alert_overrides", OVERRIDE_COLUMNS),
    ] {
        copy_fields(source, target, schema, table, columns, columns)?;
    }
    if !restore {
        return copy_fields(
            source,
            target,
            schema,
            "operational_alert_state",
            STATE_COLUMNS,
            STATE_COLUMNS,
        );
    }
    let mut query = source
        .prepare(&format!(
            "SELECT {STATE_COLUMNS} FROM {schema}.operational_alert_state"
        ))
        .map_err(sql_error)?;
    let mut rows = query.query([]).map_err(sql_error)?;
    while let Some(row) = rows.next().map_err(sql_error)? {
        let values = (0..10)
            .map(|index| {
                row.get_ref(index)
                    .map(rusqlite::types::ToSqlOutput::Borrowed)
            })
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sql_error)?;
        target.execute("INSERT INTO operational_alert_state VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT(scope_key) DO NOTHING",rusqlite::params_from_iter(values)).map_err(sql_error)?;
    }
    Ok(())
}
