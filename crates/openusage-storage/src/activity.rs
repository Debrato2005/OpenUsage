//! Exact, provisional profile activity. A local binding is never a verified provider identity.

use crate::Storage;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{collections::BTreeSet, fmt};

pub const ACTIVITY_SOURCE_METHOD: &str = "account/usage/read";
pub const ACTIVITY_SOURCE_SCOPE: &str = "account_activity_with_unknown_application_scope";
pub const MAX_ACTIVITY_ROWS_PER_READ: usize = 4096;

#[derive(Clone, PartialEq, Eq)]
pub struct ActivityIdentityEvidence {
    pub email: String,
    pub plan: Option<String>,
}
impl fmt::Debug for ActivityIdentityEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ActivityIdentityEvidence { .. }")
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityBinding {
    pub profile_id: String,
    pub home_reference: String,
    pub binding_id: String,
}
pub struct ActivityBindingInput {
    pub profile_id: String,
    pub expected_home_reference: String,
    /// None permits only the first association. A new association needs an exact previous binding.
    pub expected_previous_binding_id: Option<String>,
    pub new_binding_id: String,
    pub evidence: ActivityIdentityEvidence,
    pub observed_at_utc: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivitySourceKind {
    OfficialArtifact,
    TestFixture,
}
impl ActivitySourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OfficialArtifact => "official_artifact",
            Self::TestFixture => "test_fixture",
        }
    }
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ActivitySummaryValues {
    pub lifetime_tokens: Option<String>,
    pub peak_daily_tokens: Option<String>,
    pub longest_running_turn_sec: Option<String>,
    pub current_streak_days: Option<String>,
    pub longest_streak_days: Option<String>,
}
impl fmt::Debug for ActivitySummaryValues {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ActivitySummaryValues { .. }")
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct ActivityDayValue {
    pub provider_date: String,
    pub tokens: String,
}
impl fmt::Debug for ActivityDayValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ActivityDayValue { .. }")
    }
}
#[derive(Clone)]
pub struct ActivityReading {
    pub run_id: String,
    pub binding: ActivityBinding,
    pub identity_evidence: ActivityIdentityEvidence,
    pub provider_version: String,
    pub source_kind: ActivitySourceKind,
    pub started_at_utc: String,
    pub observed_at_utc: String,
    pub completed_at_utc: String,
    pub summary: ActivitySummaryValues,
    pub daily_usage: Option<Vec<ActivityDayValue>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityCommit {
    pub profile_id: String,
    pub binding_id: String,
    pub collection_run_id: String,
    pub snapshot_id: String,
    pub observed_at_utc: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityDailyState {
    Unavailable,
    Empty,
    Returned,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredActivity {
    pub binding: ActivityBinding,
    pub commit: ActivityCommit,
    pub source_kind: ActivitySourceKind,
    pub provider_version: String,
    pub summary: ActivitySummaryValues,
    pub daily_state: ActivityDailyState,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredActivityDay {
    pub value: ActivityDayValue,
    pub binding_id: String,
    pub collection_run_id: String,
    pub observed_at_utc: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityDayPage {
    pub rows: Vec<StoredActivityDay>,
    pub truncated: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityError {
    InvalidInput,
    ProfileUnavailable,
    BindingChanged,
    IdentityChanged,
    AlreadyAccepted,
    StaleObservation,
    ReadFailed,
    WriteFailed,
}
impl fmt::Display for ActivityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "The activity reading did not pass its checks.",
            Self::ProfileUnavailable => {
                "The selected profile is unavailable for activity collection."
            }
            Self::BindingChanged => "The profile association changed. Collect a new reading.",
            Self::IdentityChanged => "The account description changed. Confirm a new association.",
            Self::AlreadyAccepted => "This activity reading has already been saved.",
            Self::StaleObservation => "A newer activity reading is already saved.",
            Self::ReadFailed => "The saved activity could not be read.",
            Self::WriteFailed => {
                "The activity could not be saved. Previous records were preserved."
            }
        })
    }
}
impl std::error::Error for ActivityError {}
type Result<T> = std::result::Result<T, ActivityError>;

impl Storage {
    /// A local association reference only; it establishes no verified provider identity.
    pub fn activity_binding(&self, profile_id: &str) -> Result<Option<ActivityBinding>> {
        require_uuid(profile_id)?;
        self.connection.query_row("SELECT e.home_reference,c.binding_id FROM current_activity c JOIN profile_identity_events e ON e.id=c.binding_id AND e.profile_id=c.profile_id WHERE c.profile_id=?1",[profile_id],|r|Ok(ActivityBinding {profile_id:profile_id.into(),home_reference:r.get(0)?,binding_id:r.get(1)?})).optional().map_err(read_error)
    }
    /// Native code must authorize a new association. Old history keeps its original binding.
    pub fn associate_activity_binding(
        &mut self,
        input: ActivityBindingInput,
    ) -> Result<ActivityBinding> {
        require_uuid(&input.profile_id)?;
        require_uuid(&input.expected_home_reference)?;
        require_uuid(&input.new_binding_id)?;
        if let Some(id) = &input.expected_previous_binding_id {
            require_uuid(id)?;
        }
        validate_evidence(&input.evidence)?;
        require(activity_utc_timestamp(&input.observed_at_utc))?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write_error)?;
        require_profile(&tx, &input.profile_id, &input.expected_home_reference)?;
        let previous: Option<String> = tx
            .query_row(
                "SELECT binding_id FROM current_activity WHERE profile_id=?1",
                [&input.profile_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(read_error)?;
        if previous != input.expected_previous_binding_id {
            return Err(ActivityError::BindingChanged);
        }
        if tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM profile_identity_events WHERE id=?1)",
                [&input.new_binding_id],
                |r| r.get::<_, bool>(0),
            )
            .map_err(read_error)?
        {
            return Err(ActivityError::BindingChanged);
        }
        tx.execute("INSERT INTO profile_identity_events(id,profile_id,home_reference,previous_binding_id,returned_email,returned_plan,identity_confidence,verified_identity_key,observed_at_utc) VALUES (?1,?2,?3,?4,?5,?6,'provisional',NULL,?7)", params![input.new_binding_id,input.profile_id,input.expected_home_reference,previous,input.evidence.email,input.evidence.plan,input.observed_at_utc]).map_err(write_error)?;
        // Current rows are a replaceable cache. Their complete accepted observations stay in history.
        tx.execute(
            "DELETE FROM current_activity_days WHERE profile_id=?1",
            [&input.profile_id],
        )
        .map_err(write_error)?;
        tx.execute("INSERT INTO current_activity(profile_id,binding_id,summary_id,collection_run_id,created_at_utc,updated_at_utc) VALUES (?1,?2,NULL,NULL,?3,?3) ON CONFLICT(profile_id) DO UPDATE SET binding_id=excluded.binding_id,summary_id=NULL,collection_run_id=NULL,updated_at_utc=excluded.updated_at_utc", params![input.profile_id,input.new_binding_id,input.observed_at_utc]).map_err(write_error)?;
        tx.commit().map_err(write_error)?;
        Ok(ActivityBinding {
            profile_id: input.profile_id,
            home_reference: input.expected_home_reference,
            binding_id: input.new_binding_id,
        })
    }

    /// The native coordinator holds request/source/generation and physical-home fences through COMMIT.
    /// This transaction compares only the selected profile, so another profile's commit is harmless.
    pub fn accept_activity(&mut self, reading: ActivityReading) -> Result<ActivityCommit> {
        validate_reading(&reading)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(write_error)?;
        require_profile(
            &tx,
            &reading.binding.profile_id,
            &reading.binding.home_reference,
        )?;
        let current: Option<(String,String,Option<String>,String)> = tx.query_row("SELECT c.binding_id,e.returned_email,e.returned_plan,e.home_reference FROM current_activity c JOIN profile_identity_events e ON e.id=c.binding_id AND e.profile_id=c.profile_id WHERE c.profile_id=?1", [&reading.binding.profile_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(read_error)?;
        let Some((binding, email, plan, home)) = current else {
            return Err(ActivityError::BindingChanged);
        };
        if binding != reading.binding.binding_id || home != reading.binding.home_reference {
            return Err(ActivityError::BindingChanged);
        }
        if email != reading.identity_evidence.email || plan != reading.identity_evidence.plan {
            return Err(ActivityError::IdentityChanged);
        }
        if tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM collection_runs WHERE id=?1)",
                [&reading.run_id],
                |r| r.get::<_, bool>(0),
            )
            .map_err(read_error)?
        {
            return Err(ActivityError::AlreadyAccepted);
        }
        let latest: Option<String> = tx.query_row("SELECT r.observed_at_utc FROM current_activity c JOIN collection_runs r ON r.id=c.collection_run_id WHERE c.profile_id=?1", [&reading.binding.profile_id], |r| r.get(0)).optional().map_err(read_error)?;
        if latest.is_some_and(|value| value > reading.observed_at_utc) {
            return Err(ActivityError::StaleObservation);
        }
        tx.execute("INSERT INTO collection_runs(id,profile_id,binding_id,started_at_utc,completed_at_utc,observed_at_utc,source_method,provider_version,source_kind,source_scope,raw_scope,normalized_unit,coverage,identity_confidence,verified_identity_key,outcome) VALUES (?1,?2,?3,?4,?5,?6,'account/usage/read',?7,?8,'account_activity_with_unknown_application_scope',NULL,'tokens','unknown','provisional',NULL,'accepted')",params![reading.run_id,reading.binding.profile_id,reading.binding.binding_id,reading.started_at_utc,reading.completed_at_utc,reading.observed_at_utc,reading.provider_version,reading.source_kind.as_str()]).map_err(write_error)?;
        let daily_state = match &reading.daily_usage {
            None => "unavailable",
            Some(rows) if rows.is_empty() => "empty",
            Some(_) => "returned",
        };
        let s = &reading.summary;
        tx.execute("INSERT INTO activity_summaries(id,profile_id,binding_id,collection_run_id,lifetime_tokens,peak_daily_tokens,longest_running_turn_sec,current_streak_days,longest_streak_days,daily_state) VALUES (?1,?2,?3,?1,?4,?5,?6,?7,?8,?9)",params![reading.run_id,reading.binding.profile_id,reading.binding.binding_id,s.lifetime_tokens,s.peak_daily_tokens,s.longest_running_turn_sec,s.current_streak_days,s.longest_streak_days,daily_state]).map_err(write_error)?;
        for day in reading.daily_usage.iter().flatten() {
            let previous: Option<String> = tx.query_row("SELECT tokens FROM current_activity_days WHERE profile_id=?1 AND provider_date=?2 AND source_scope=?3",params![reading.binding.profile_id,day.provider_date,ACTIVITY_SOURCE_SCOPE], |r| r.get(0)).optional().map_err(read_error)?;
            tx.execute("INSERT INTO activity_day_history(id,profile_id,provider_date,source_scope,binding_id,collection_run_id,prior_tokens,accepted_tokens,observed_at_utc) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![new_uuid()?,reading.binding.profile_id,day.provider_date,ACTIVITY_SOURCE_SCOPE,reading.binding.binding_id,reading.run_id,previous,day.tokens,reading.observed_at_utc]).map_err(write_error)?;
            tx.execute("INSERT INTO current_activity_days(profile_id,provider_date,source_scope,binding_id,collection_run_id,tokens,observed_at_utc,created_at_utc,updated_at_utc) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8) ON CONFLICT(profile_id,provider_date,source_scope) DO UPDATE SET binding_id=excluded.binding_id,collection_run_id=excluded.collection_run_id,tokens=excluded.tokens,observed_at_utc=excluded.observed_at_utc,updated_at_utc=excluded.updated_at_utc",params![reading.binding.profile_id,day.provider_date,ACTIVITY_SOURCE_SCOPE,reading.binding.binding_id,reading.run_id,day.tokens,reading.observed_at_utc,reading.completed_at_utc]).map_err(write_error)?;
        }
        tx.execute("UPDATE current_activity SET summary_id=?1,collection_run_id=?1,updated_at_utc=?2 WHERE profile_id=?3",params![reading.run_id,reading.completed_at_utc,reading.binding.profile_id]).map_err(write_error)?;
        tx.commit().map_err(write_error)?;
        Ok(ActivityCommit {
            profile_id: reading.binding.profile_id,
            binding_id: reading.binding.binding_id,
            collection_run_id: reading.run_id.clone(),
            snapshot_id: reading.run_id,
            observed_at_utc: reading.observed_at_utc,
        })
    }

    pub fn current_activity(&self, profile_id: &str) -> Result<Option<StoredActivity>> {
        require_uuid(profile_id)?;
        self.connection.query_row("SELECT e.home_reference,c.binding_id,r.id,r.observed_at_utc,r.source_kind,r.provider_version,s.lifetime_tokens,s.peak_daily_tokens,s.longest_running_turn_sec,s.current_streak_days,s.longest_streak_days,s.daily_state FROM current_activity c JOIN profile_identity_events e ON e.id=c.binding_id JOIN collection_runs r ON r.id=c.collection_run_id JOIN activity_summaries s ON s.id=c.summary_id WHERE c.profile_id=?1",[profile_id],|r| {
            let binding = ActivityBinding {
                profile_id: profile_id.into(), home_reference: r.get(0)?, binding_id: r.get(1)?,
            };
            let source_kind = match r.get::<_, String>(4)?.as_str() {
                "official_artifact" => ActivitySourceKind::OfficialArtifact,
                "test_fixture" => ActivitySourceKind::TestFixture,
                _ => return Err(rusqlite::Error::InvalidQuery),
            };
            let daily_state = match r.get::<_, String>(11)?.as_str() {
                "unavailable" => ActivityDailyState::Unavailable,
                "empty" => ActivityDailyState::Empty,
                "returned" => ActivityDailyState::Returned,
                _ => return Err(rusqlite::Error::InvalidQuery),
            };
            Ok(StoredActivity {
                commit: ActivityCommit {
                    profile_id: profile_id.into(), binding_id: binding.binding_id.clone(),
                    collection_run_id: r.get(2)?, snapshot_id: r.get(2)?, observed_at_utc: r.get(3)?,
                }, binding, source_kind, provider_version: r.get(5)?,
                summary: ActivitySummaryValues {
                    lifetime_tokens: r.get(6)?, peak_daily_tokens: r.get(7)?,
                    longest_running_turn_sec: r.get(8)?, current_streak_days: r.get(9)?,
                    longest_streak_days: r.get(10)?,
                }, daily_state,
            })
        }).optional().map_err(read_error)
    }

    /// A date-range page is bounded explicitly. A truncated page never claims complete coverage.
    pub fn activity_days(
        &self,
        profile_id: &str,
        start_date: &str,
        end_date: &str,
        maximum_rows: usize,
    ) -> Result<ActivityDayPage> {
        require_uuid(profile_id)?;
        require(
            activity_date(start_date)
                && activity_date(end_date)
                && start_date <= end_date
                && (1..=MAX_ACTIVITY_ROWS_PER_READ).contains(&maximum_rows),
        )?;
        let mut statement = self.connection.prepare("SELECT provider_date,tokens,binding_id,collection_run_id,observed_at_utc FROM current_activity_days WHERE profile_id=?1 AND source_scope=?2 AND provider_date BETWEEN ?3 AND ?4 ORDER BY provider_date LIMIT ?5").map_err(read_error)?;
        let mut rows = statement
            .query_map(
                params![
                    profile_id,
                    ACTIVITY_SOURCE_SCOPE,
                    start_date,
                    end_date,
                    (maximum_rows + 1) as i64
                ],
                |r| {
                    Ok(StoredActivityDay {
                        value: ActivityDayValue {
                            provider_date: r.get(0)?,
                            tokens: r.get(1)?,
                        },
                        binding_id: r.get(2)?,
                        collection_run_id: r.get(3)?,
                        observed_at_utc: r.get(4)?,
                    })
                },
            )
            .map_err(read_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(read_error)?;
        let truncated = rows.len() > maximum_rows;
        rows.truncate(maximum_rows);
        Ok(ActivityDayPage { rows, truncated })
    }
}

fn require_profile(connection: &Connection, id: &str, home: &str) -> Result<()> {
    let eligible: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM profiles WHERE id=?1 AND approved_home_reference=?2 AND archived=0 AND environment='native')",params![id,home],|r|r.get(0)).map_err(read_error)?;
    if eligible {
        Ok(())
    } else {
        Err(ActivityError::ProfileUnavailable)
    }
}
fn validate_reading(r: &ActivityReading) -> Result<()> {
    for id in [
        &r.run_id,
        &r.binding.profile_id,
        &r.binding.home_reference,
        &r.binding.binding_id,
    ] {
        require_uuid(id)?;
    }
    validate_evidence(&r.identity_evidence)?;
    require(safe_text(&r.provider_version, 64))?;
    require(
        activity_utc_timestamp(&r.started_at_utc)
            && activity_utc_timestamp(&r.observed_at_utc)
            && activity_utc_timestamp(&r.completed_at_utc)
            && r.started_at_utc <= r.observed_at_utc
            && r.observed_at_utc <= r.completed_at_utc,
    )?;
    let s = &r.summary;
    for count in [
        &s.lifetime_tokens,
        &s.peak_daily_tokens,
        &s.longest_running_turn_sec,
        &s.current_streak_days,
        &s.longest_streak_days,
    ]
    .into_iter()
    .flatten()
    {
        require(activity_decimal(count))?;
    }
    if let Some(rows) = &r.daily_usage {
        require(rows.len() <= MAX_ACTIVITY_ROWS_PER_READ)?;
        let mut dates = BTreeSet::new();
        for row in rows {
            require(
                activity_date(&row.provider_date)
                    && activity_decimal(&row.tokens)
                    && dates.insert(&row.provider_date),
            )?;
        }
    }
    Ok(())
}
fn validate_evidence(e: &ActivityIdentityEvidence) -> Result<()> {
    require(safe_text(&e.email, 254) && e.plan.as_ref().is_none_or(|v| safe_text(v, 64)))
}
fn safe_text(v: &str, max: usize) -> bool {
    !v.trim().is_empty() && v.len() <= max && !v.chars().any(char::is_control)
}
fn require_uuid(v: &str) -> Result<()> {
    require(crate::backup_validation::canonical_uuid(v))
}
fn require(v: bool) -> Result<()> {
    if v {
        Ok(())
    } else {
        Err(ActivityError::InvalidInput)
    }
}
fn read_error(_: rusqlite::Error) -> ActivityError {
    ActivityError::ReadFailed
}
fn write_error(_: rusqlite::Error) -> ActivityError {
    ActivityError::WriteFailed
}
fn new_uuid() -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| ActivityError::WriteFailed)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(uuid::Uuid::from_bytes(bytes).to_string())
}
pub(crate) fn activity_decimal(v: &str) -> bool {
    !v.is_empty()
        && v.bytes().all(|b| b.is_ascii_digit())
        && v.parse::<i64>().is_ok_and(|n| n >= 0 && n.to_string() == v)
}
pub(crate) fn activity_date(v: &str) -> bool {
    let b = v.as_bytes();
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || b.iter()
            .enumerate()
            .any(|(i, b)| i != 4 && i != 7 && !b.is_ascii_digit())
    {
        return false;
    }
    let year = v[..4].parse::<u32>().unwrap_or(0);
    let month = v[5..7].parse::<u8>().unwrap_or(0);
    let day = v[8..].parse::<u8>().unwrap_or(0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        _ => 0,
    };
    year > 0 && day > 0 && day <= days
}
pub(crate) fn activity_utc_timestamp(v: &str) -> bool {
    let b = v.as_bytes();
    b.len() == 24
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'.'
        && b[23] == b'Z'
        && activity_date(&v[..10])
        && [11..13, 14..16, 17..19, 20..23]
            .iter()
            .all(|range| b[range.clone()].iter().all(u8::is_ascii_digit))
        && v[11..13].parse::<u8>().is_ok_and(|n| n < 24)
        && v[14..16].parse::<u8>().is_ok_and(|n| n < 60)
        && v[17..19].parse::<u8>().is_ok_and(|n| n < 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    const KEY: [u8; 32] = [0x75; 32];
    const PROFILE: &str = "10000000-0000-4000-8000-000000000001";
    const OTHER: &str = "10000000-0000-4000-8000-000000000002";
    const HOME: &str = "20000000-0000-4000-8000-000000000001";
    const OTHER_HOME: &str = "20000000-0000-4000-8000-000000000002";
    const BINDING: &str = "30000000-0000-4000-8000-000000000001";
    const NEXT_BINDING: &str = "30000000-0000-4000-8000-000000000002";
    const TIME: &str = "2026-10-18T12:00:00.000Z";
    const LATER: &str = "2026-10-18T12:00:01.000Z";
    const LAST: &str = "2026-10-18T12:00:02.000Z";
    fn evidence() -> ActivityIdentityEvidence {
        ActivityIdentityEvidence {
            email: "synthetic-private@example.invalid".into(),
            plan: Some("synthetic-private-plan".into()),
        }
    }
    fn associate(
        storage: &mut Storage,
        profile: &str,
        home: &str,
        binding: &str,
        previous: Option<&str>,
    ) -> ActivityBinding {
        storage
            .associate_activity_binding(ActivityBindingInput {
                profile_id: profile.into(),
                expected_home_reference: home.into(),
                expected_previous_binding_id: previous.map(str::to_string),
                new_binding_id: binding.into(),
                evidence: evidence(),
                observed_at_utc: TIME.into(),
            })
            .unwrap()
    }
    fn setup() -> (tempfile::TempDir, Storage, ActivityBinding) {
        let directory = crate::tests::private_directory();
        let mut storage = Storage::create(&directory.path().join("activity.db"), &KEY).unwrap();
        storage
            .create_profile(PROFILE, "Synthetic activity", HOME)
            .unwrap();
        let binding = associate(&mut storage, PROFILE, HOME, BINDING, None);
        (directory, storage, binding)
    }
    fn reading(binding: &ActivityBinding, number: u32, time: &str) -> ActivityReading {
        ActivityReading {
            run_id: format!("40000000-0000-4000-8000-{number:012x}"),
            binding: binding.clone(),
            identity_evidence: evidence(),
            provider_version: "0.163.0".into(),
            source_kind: ActivitySourceKind::TestFixture,
            started_at_utc: TIME.into(),
            observed_at_utc: time.into(),
            completed_at_utc: time.into(),
            summary: ActivitySummaryValues {
                lifetime_tokens: Some("9007199254740993".into()),
                peak_daily_tokens: Some("9223372036854775807".into()),
                longest_running_turn_sec: None,
                current_streak_days: Some("0".into()),
                longest_streak_days: None,
            },
            daily_usage: Some(vec![
                ActivityDayValue {
                    provider_date: "2024-02-29".into(),
                    tokens: "9007199254740993".into(),
                },
                ActivityDayValue {
                    provider_date: "2024-03-01".into(),
                    tokens: "0".into(),
                },
            ]),
        }
    }
    fn counts(storage: &Storage) -> (i64, i64, i64, i64) {
        storage.connection.query_row("SELECT (SELECT count(*) FROM collection_runs),(SELECT count(*) FROM activity_summaries),(SELECT count(*) FROM current_activity_days),(SELECT count(*) FROM activity_day_history)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap()
    }

    #[test]
    fn activity_exact_replacements_keep_history_and_survive_encrypted_wal_reopen() {
        let (directory, mut storage, binding) = setup();
        storage
            .connection
            .pragma_update(None, "wal_autocheckpoint", 0)
            .unwrap();
        storage.accept_activity(reading(&binding, 1, TIME)).unwrap();
        let mut second = reading(&binding, 2, LATER);
        second.daily_usage.as_mut().unwrap()[0].tokens = "7".into();
        second.daily_usage.as_mut().unwrap().remove(1);
        storage.accept_activity(second).unwrap();
        assert_eq!(counts(&storage), (2, 2, 2, 3));
        let page = storage
            .activity_days(PROFILE, "2024-02-29", "2024-03-01", 1)
            .unwrap();
        assert!(page.truncated);
        assert_eq!(page.rows[0].value.tokens, "7");
        let prior:(String,String)=storage.connection.query_row("SELECT prior_tokens,accepted_tokens FROM activity_day_history WHERE collection_run_id='40000000-0000-4000-8000-000000000002'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!(prior, ("9007199254740993".into(), "7".into()));
        let current = storage.current_activity(PROFILE).unwrap().unwrap();
        assert_eq!(
            current.summary.peak_daily_tokens.as_deref(),
            Some("9223372036854775807")
        );
        assert_eq!(current.summary.current_streak_days.as_deref(), Some("0"));
        let path = directory.path().join("activity.db");
        assert!(
            std::fs::metadata(path.with_extension("db-wal"))
                .unwrap()
                .len()
                > 0
        );
        for path in [&path, &path.with_extension("db-wal")] {
            let bytes = std::fs::read(path).unwrap();
            assert!(
                !bytes
                    .windows(evidence().email.len())
                    .any(|w| w == evidence().email.as_bytes())
            );
            assert!(!bytes.windows(16).any(|w| w == b"9007199254740993"));
        }
        let unkeyed = Connection::open(&path).unwrap();
        assert!(
            unkeyed
                .query_row("SELECT count(*) FROM activity_summaries", [], |r| r
                    .get::<_, i64>(0))
                .is_err()
        );
        drop(unkeyed);
        drop(storage);
        let reopened = Storage::open(&path, &KEY).unwrap();
        assert_eq!(reopened.current_activity(PROFILE).unwrap(), Some(current));
        assert_eq!(counts(&reopened), (2, 2, 2, 3));
        assert_eq!(
            reopened
                .connection
                .pragma_query_value(None, "journal_mode", |r| r.get::<_, String>(0))
                .unwrap(),
            "wal"
        );
    }

    #[test]
    fn activity_null_empty_and_partial_days_never_erase_prior_current_dates() {
        let (_directory, mut storage, binding) = setup();
        storage.accept_activity(reading(&binding, 1, TIME)).unwrap();
        let mut absent = reading(&binding, 2, LATER);
        absent.summary = ActivitySummaryValues::default();
        absent.daily_usage = None;
        storage.accept_activity(absent).unwrap();
        let current = storage.current_activity(PROFILE).unwrap().unwrap();
        assert_eq!(current.summary, ActivitySummaryValues::default());
        assert_eq!(current.daily_state, ActivityDailyState::Unavailable);
        assert_eq!(counts(&storage), (2, 2, 2, 2));
        let mut empty = reading(&binding, 3, LAST);
        empty.daily_usage = Some(vec![]);
        storage.accept_activity(empty).unwrap();
        assert_eq!(
            storage
                .current_activity(PROFILE)
                .unwrap()
                .unwrap()
                .daily_state,
            ActivityDailyState::Empty
        );
        assert_eq!(counts(&storage), (3, 3, 2, 2));
        assert_eq!(
            storage
                .activity_days(PROFILE, "2024-02-29", "2024-03-01", 10)
                .unwrap()
                .rows
                .len(),
            2
        );
    }

    #[test]
    fn activity_binding_replay_clock_archive_and_transaction_faults_preserve_records() {
        let (_directory, mut storage, binding) = setup();
        let initial = reading(&binding, 1, LATER);
        storage.accept_activity(initial.clone()).unwrap();
        let saved = storage.current_activity(PROFILE).unwrap();
        let baseline = counts(&storage);
        assert_eq!(
            storage.accept_activity(initial),
            Err(ActivityError::AlreadyAccepted)
        );
        assert_eq!(
            storage.accept_activity(reading(&binding, 2, TIME)),
            Err(ActivityError::StaleObservation)
        );
        let mut changed = reading(&binding, 2, LAST);
        changed.identity_evidence.email = "other@example.invalid".into();
        assert_eq!(
            storage.accept_activity(changed),
            Err(ActivityError::IdentityChanged)
        );
        let mut wrong = reading(&binding, 2, LAST);
        wrong.binding.home_reference = OTHER_HOME.into();
        assert_eq!(
            storage.accept_activity(wrong),
            Err(ActivityError::ProfileUnavailable)
        );
        storage.set_profile_archived(PROFILE, true).unwrap();
        assert_eq!(
            storage.accept_activity(reading(&binding, 2, LAST)),
            Err(ActivityError::ProfileUnavailable)
        );
        storage.set_profile_archived(PROFILE, false).unwrap();
        storage.connection.execute_batch("CREATE TRIGGER fail_activity_commit BEFORE UPDATE ON current_activity BEGIN SELECT RAISE(ABORT,'synthetic failure after history'); END;").unwrap();
        assert_eq!(
            storage.accept_activity(reading(&binding, 2, LAST)),
            Err(ActivityError::WriteFailed)
        );
        storage
            .connection
            .execute_batch("DROP TRIGGER fail_activity_commit")
            .unwrap();
        assert_eq!(counts(&storage), baseline);
        assert_eq!(storage.current_activity(PROFILE).unwrap(), saved);
        let next = associate(&mut storage, PROFILE, HOME, NEXT_BINDING, Some(BINDING));
        assert!(storage.current_activity(PROFILE).unwrap().is_none());
        assert_eq!(counts(&storage), (1, 1, 0, 2));
        assert_eq!(
            storage.accept_activity(reading(&binding, 2, LAST)),
            Err(ActivityError::BindingChanged)
        );
        storage.accept_activity(reading(&next, 2, LAST)).unwrap();
        let old:(String,String)=storage.connection.query_row("SELECT r.binding_id,e.returned_email FROM collection_runs r JOIN profile_identity_events e ON e.id=r.binding_id WHERE r.id='40000000-0000-4000-8000-000000000001'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!(old, (BINDING.into(), evidence().email));
        assert_eq!(counts(&storage), (2, 2, 2, 4));
    }

    #[test]
    fn activity_other_profile_commit_does_not_invalidate_selected_profile_binding() {
        let (directory, mut storage, first) = setup();
        storage
            .create_profile(OTHER, "Other profile", OTHER_HOME)
            .unwrap();
        let second = associate(&mut storage, OTHER, OTHER_HOME, NEXT_BINDING, None);
        let before = storage.restore_state().unwrap().revision;
        let pending_first = reading(&first, 1, TIME);
        let mut external = Storage::open(&directory.path().join("activity.db"), &KEY).unwrap();
        external.accept_activity(reading(&second, 2, TIME)).unwrap();
        assert_ne!(storage.restore_state().unwrap().revision, before);
        storage.accept_activity(pending_first).unwrap();
        assert!(storage.current_activity(PROFILE).unwrap().is_some());
        assert!(storage.current_activity(OTHER).unwrap().is_some());
        let rejected = storage.associate_activity_binding(ActivityBindingInput {
            profile_id: PROFILE.into(),
            expected_home_reference: HOME.into(),
            expected_previous_binding_id: None,
            new_binding_id: "30000000-0000-4000-8000-000000000003".into(),
            evidence: evidence(),
            observed_at_utc: LAST.into(),
        });
        assert_eq!(rejected, Err(ActivityError::BindingChanged));
        assert_eq!(counts(&storage), (2, 2, 4, 4));
    }

    #[test]
    fn activity_invalid_dates_exact_counts_and_resource_bounds_are_rejected_without_writes() {
        let (_directory, mut storage, binding) = setup();
        for value in [
            "-1",
            "1.5",
            "9007199254740993.0",
            "9223372036854775808",
            "01",
            "",
        ] {
            let mut r = reading(&binding, 1, TIME);
            r.summary.lifetime_tokens = Some(value.into());
            assert_eq!(storage.accept_activity(r), Err(ActivityError::InvalidInput));
        }
        for value in ["2023-02-29", "2024-13-01", "0000-01-01", "2024-2-29"] {
            let mut r = reading(&binding, 1, TIME);
            r.daily_usage.as_mut().unwrap()[0].provider_date = value.into();
            assert_eq!(storage.accept_activity(r), Err(ActivityError::InvalidInput));
        }
        let mut duplicate = reading(&binding, 1, TIME);
        duplicate.daily_usage.as_mut().unwrap()[1].provider_date = "2024-02-29".into();
        assert_eq!(
            storage.accept_activity(duplicate),
            Err(ActivityError::InvalidInput)
        );
        let mut malformed = reading(&binding, 1, TIME);
        malformed.completed_at_utc = "2026-10-18T12:00:00Z".into();
        assert_eq!(
            storage.accept_activity(malformed),
            Err(ActivityError::InvalidInput)
        );
        assert_eq!(counts(&storage), (0, 0, 0, 0));
        let mut bounded = reading(&binding, 1, TIME);
        bounded.daily_usage = Some(
            (1..=4096)
                .map(|n| ActivityDayValue {
                    provider_date: format!("{:04}-01-01", n),
                    tokens: n.to_string(),
                })
                .collect(),
        );
        storage.accept_activity(bounded).unwrap();
        assert_eq!(counts(&storage), (1, 1, 4096, 4096));
        let mut oversized = reading(&binding, 2, LATER);
        oversized.daily_usage = Some(
            (1..=4097)
                .map(|n| ActivityDayValue {
                    provider_date: format!("{:04}-01-01", n),
                    tokens: n.to_string(),
                })
                .collect(),
        );
        assert_eq!(
            storage.accept_activity(oversized),
            Err(ActivityError::InvalidInput)
        );
        assert_eq!(counts(&storage), (1, 1, 4096, 4096));
        assert!(
            !format!("{:?}", storage.current_activity(PROFILE).unwrap())
                .contains("9007199254740993")
        );
        assert!(!format!("{:?}", evidence()).contains("synthetic-private"));
    }

    #[test]
    fn activity_v1_upgrade_and_failed_migration_preserve_encrypted_previous_state() {
        let directory = crate::tests::private_directory();
        let path = directory.path().join("legacy.db");
        let mut connection = Connection::open(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        connection
            .pragma_update(None, "key", format!("x'{}'", "75".repeat(32)))
            .unwrap();
        connection
            .execute_batch(include_str!("schema.sql"))
            .unwrap();
        connection.pragma_update(None, "user_version", 1).unwrap();
        connection
            .execute("UPDATE settings SET theme='dark'", [])
            .unwrap();
        let failing = [
            include_str!("schema.sql"),
            include_str!("schema_v2.sql"),
            "UPDATE settings SET theme='light'; SELECT missing_column FROM settings;",
        ];
        assert_eq!(
            crate::migrate(&mut connection, &failing),
            Err(crate::StorageError::MigrationFailed)
        );
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row("SELECT theme FROM settings", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "dark"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE name='activity_summaries'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        drop(connection);
        let mut upgraded = Storage::open(&path, &KEY).unwrap();
        assert_eq!(upgraded.metadata().unwrap().schema_version, 2);
        assert_eq!(upgraded.metadata().unwrap().theme, crate::Theme::Dark);
        upgraded.create_profile(PROFILE, "Upgraded", HOME).unwrap();
        let binding = associate(&mut upgraded, PROFILE, HOME, BINDING, None);
        upgraded
            .accept_activity(reading(&binding, 1, TIME))
            .unwrap();
        drop(upgraded);
        assert_eq!(
            Storage::open(&path, &KEY)
                .unwrap()
                .current_activity(PROFILE)
                .unwrap()
                .unwrap()
                .summary
                .lifetime_tokens
                .as_deref(),
            Some("9007199254740993")
        );
    }
}
