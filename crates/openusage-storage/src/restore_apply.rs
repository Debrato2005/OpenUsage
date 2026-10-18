//! Replace only schema-one records. Native callers own profile directories and confirmation.
use crate::backup_validation::{canonical_uuid, validate_connection, with_validated_snapshot};
use crate::{
    BackupContentSummary, BackupValidationError, CURRENT_SCHEMA_VERSION, Storage, StorageRevision,
};
use rusqlite::{Connection, TransactionBehavior, params};
use std::collections::{BTreeMap, BTreeSet};

const RESTORE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoreHomeMapping {
    pub profile_id: String,
    pub managed_home_reference: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoreInputMetadata {
    pub summary: BackupContentSummary,
    pub profile_ids: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointRestoreMetadata {
    pub summary: BackupContentSummary,
    pub homes: Vec<RestoreHomeMapping>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestoreError {
    StalePreview,
    UnsupportedSchema,
    UnsupportedContent,
    InvalidBackup,
    InvalidHomeMapping,
    WriteFailed,
    Unavailable,
}
impl std::fmt::Display for RestoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::StalePreview => "The workspace changed. Prepare a new restore preview.",
            Self::UnsupportedSchema => "This database schema cannot be restored by this version.",
            Self::UnsupportedContent => "This backup contains unsupported records.",
            Self::InvalidBackup => "The backup database did not pass its content checks.",
            Self::InvalidHomeMapping => "The managed profile homes did not pass their checks.",
            Self::WriteFailed => "The restore transaction failed. Previous records were preserved.",
            Self::Unavailable => "The restore is unavailable.",
        })
    }
}
impl std::error::Error for RestoreError {}
impl From<BackupValidationError> for RestoreError {
    fn from(error: BackupValidationError) -> Self {
        match error {
            BackupValidationError::UnsupportedSchema => Self::UnsupportedSchema,
            BackupValidationError::UnsupportedContent => Self::UnsupportedContent,
            BackupValidationError::InvalidDatabase => Self::InvalidBackup,
            BackupValidationError::Unavailable => Self::Unavailable,
        }
    }
}
type Result<T> = std::result::Result<T, RestoreError>;

pub fn validate_restore_input(bytes: &[u8], version: u32) -> Result<RestoreInputMetadata> {
    require_version(version)?;
    with_validated_snapshot(bytes, version, true, |source, summary| {
        Ok(RestoreInputMetadata {
            summary,
            profile_ids: read_homes(source)?
                .into_iter()
                .map(|home| home.profile_id)
                .collect(),
        })
    })
}
pub fn validate_checkpoint_restore_input(
    bytes: &[u8],
    version: u32,
) -> Result<CheckpointRestoreMetadata> {
    require_version(version)?;
    with_validated_snapshot(bytes, version, true, |source, summary| {
        Ok(CheckpointRestoreMetadata {
            summary,
            homes: read_homes(source)?,
        })
    })
}
fn require_version(version: u32) -> Result<()> {
    if version != RESTORE_SCHEMA_VERSION || CURRENT_SCHEMA_VERSION != RESTORE_SCHEMA_VERSION {
        return Err(RestoreError::UnsupportedSchema);
    }
    Ok(())
}
fn read_homes(connection: &Connection) -> Result<Vec<RestoreHomeMapping>> {
    let mut statement = connection
        .prepare("SELECT id,approved_home_reference FROM profiles ORDER BY id")
        .map_err(|_| RestoreError::InvalidBackup)?;
    statement
        .query_map([], |row| {
            Ok(RestoreHomeMapping {
                profile_id: row.get(0)?,
                managed_home_reference: row.get(1)?,
            })
        })
        .map_err(|_| RestoreError::InvalidBackup)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| RestoreError::InvalidBackup)
}
impl Storage {
    /// Rust-only references. No credential files or paths enter the renderer.
    pub fn restore_home_mappings(
        &mut self,
        expected: &StorageRevision,
    ) -> Result<Vec<RestoreHomeMapping>> {
        let transaction = self
            .connection
            .transaction()
            .map_err(|_| RestoreError::Unavailable)?;
        if !expected
            .matches(&self.revision_scope, &transaction)
            .map_err(|_| RestoreError::Unavailable)?
        {
            return Err(RestoreError::StalePreview);
        }
        let homes = read_homes(&transaction)?;
        transaction
            .commit()
            .map_err(|_| RestoreError::Unavailable)?;
        Ok(homes)
    }
    /// Archive imports always need fresh homes, including archived profiles.
    pub fn replace_records_from_backup(
        &mut self,
        bytes: &[u8],
        version: u32,
        expected: &StorageRevision,
        homes: &[RestoreHomeMapping],
    ) -> Result<BackupContentSummary> {
        self.replace_records(bytes, version, expected, homes, false)
    }
    /// A checkpoint must use its exact original mappings. A live reference can belong only to the same profile.
    pub fn replace_records_from_checkpoint(
        &mut self,
        bytes: &[u8],
        version: u32,
        expected: &StorageRevision,
        homes: &[RestoreHomeMapping],
    ) -> Result<BackupContentSummary> {
        self.replace_records(bytes, version, expected, homes, true)
    }
    fn replace_records(
        &mut self,
        bytes: &[u8],
        version: u32,
        expected: &StorageRevision,
        homes: &[RestoreHomeMapping],
        checkpoint: bool,
    ) -> Result<BackupContentSummary> {
        require_version(version)?;
        let scope = &self.revision_scope;
        let target = &mut self.connection;
        let (transaction, summary) = with_validated_snapshot::<_, RestoreError>(
            bytes,
            version,
            true,
            |source, summary| {
                let transaction = target
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|_| RestoreError::WriteFailed)?;
                if !expected
                    .matches(scope, &transaction)
                    .map_err(|_| RestoreError::Unavailable)?
                {
                    return Err(RestoreError::StalePreview);
                }
                validate_connection(&transaction, version)
                    .map_err(|_| RestoreError::Unavailable)?;
                let source_homes = read_homes(source)?;
                let current_homes = read_homes(&transaction)?;
                let mappings = check_mappings(&source_homes, &current_homes, homes, checkpoint)?;
                let when: String = transaction
                    .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |r| {
                        r.get(0)
                    })
                    .map_err(|_| RestoreError::WriteFailed)?;
                transaction
                    .execute_batch(
                        "DELETE FROM profiles; DELETE FROM accounts; DELETE FROM settings;",
                    )
                    .map_err(|_| RestoreError::WriteFailed)?;
                // Table and column names are trusted application SQL. Archive SQL is never executed.
                copy_rows(
                    source,
                    &transaction,
                    "SELECT id,verified_identity_key,identity_confidence,returned_email,returned_plan,first_observed_at,last_observed_at FROM accounts",
                    "INSERT INTO accounts VALUES (?1,?2,?3,?4,?5,?6,?7)",
                )?;
                let mut statement=source.prepare("SELECT id,account_id,name,environment,archived,created_at FROM profiles ORDER BY id").map_err(|_|RestoreError::InvalidBackup)?;
                let mut rows = statement
                    .query([])
                    .map_err(|_| RestoreError::InvalidBackup)?;
                while let Some(row) = rows.next().map_err(|_| RestoreError::InvalidBackup)? {
                    let id: String = row.get(0).map_err(|_| RestoreError::InvalidBackup)?;
                    transaction.execute("INSERT INTO profiles(id,account_id,name,environment,approved_home_reference,connection_state,archived,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,'authentication_required',?6,?7,?8)",params![id,row.get::<_,Option<String>>(1).map_err(|_|RestoreError::InvalidBackup)?,row.get::<_,String>(2).map_err(|_|RestoreError::InvalidBackup)?,row.get::<_,String>(3).map_err(|_|RestoreError::InvalidBackup)?,mappings.get(&id).ok_or(RestoreError::InvalidHomeMapping)?,row.get::<_,i64>(4).map_err(|_|RestoreError::InvalidBackup)?,row.get::<_,String>(5).map_err(|_|RestoreError::InvalidBackup)?,when]).map_err(|_|RestoreError::WriteFailed)?;
                }
                let (timezone,locale,theme,retention,created):(String,String,String,String,String)=source.query_row("SELECT timezone,locale,theme,retention_policy,created_at FROM settings WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(|_|RestoreError::InvalidBackup)?;
                transaction
                    .execute(
                        "INSERT INTO settings VALUES (1,?1,?2,?3,'stopped',?4,?5,?6)",
                        params![timezone, locale, theme, retention, created, when],
                    )
                    .map_err(|_| RestoreError::WriteFailed)?;
                if validate_connection(&transaction, version)
                    .map_err(|_| RestoreError::WriteFailed)?
                    != summary
                {
                    return Err(RestoreError::WriteFailed);
                }
                Ok((transaction, summary))
            },
        )?;
        // The source connection closed and its borrowed image was wiped before committing live data.
        transaction
            .commit()
            .map_err(|_| RestoreError::WriteFailed)?;
        Ok(summary)
    }
}
fn copy_rows(source: &Connection, target: &Connection, select: &str, insert: &str) -> Result<()> {
    let mut statement = source
        .prepare(select)
        .map_err(|_| RestoreError::InvalidBackup)?;
    let columns = statement.column_count();
    let mut rows = statement
        .query([])
        .map_err(|_| RestoreError::InvalidBackup)?;
    let mut write = target
        .prepare(insert)
        .map_err(|_| RestoreError::WriteFailed)?;
    while let Some(row) = rows.next().map_err(|_| RestoreError::InvalidBackup)? {
        let values = (0..columns)
            .map(|index| row.get_ref(index))
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|_| RestoreError::InvalidBackup)?;
        write
            .execute(rusqlite::params_from_iter(values))
            .map_err(|_| RestoreError::WriteFailed)?;
    }
    Ok(())
}
fn check_mappings<'a>(
    source: &[RestoreHomeMapping],
    current: &[RestoreHomeMapping],
    supplied: &'a [RestoreHomeMapping],
    checkpoint: bool,
) -> Result<BTreeMap<String, &'a str>> {
    if source.len() != supplied.len() {
        return Err(RestoreError::InvalidHomeMapping);
    }
    let originals: BTreeMap<_, _> = source
        .iter()
        .map(|h| (h.profile_id.as_str(), h.managed_home_reference.as_str()))
        .collect();
    let occupied: BTreeMap<_, _> = current
        .iter()
        .map(|h| (h.managed_home_reference.as_str(), h.profile_id.as_str()))
        .collect();
    let mut result = BTreeMap::new();
    let mut unique = BTreeSet::new();
    for home in supplied {
        if !canonical_uuid(&home.profile_id)
            || !canonical_uuid(&home.managed_home_reference)
            || !originals.contains_key(home.profile_id.as_str())
            || (checkpoint
                && originals.get(home.profile_id.as_str()).copied()
                    != Some(home.managed_home_reference.as_str()))
            || occupied
                .get(home.managed_home_reference.as_str())
                .is_some_and(|owner| !checkpoint || *owner != home.profile_id)
            || !unique.insert(home.managed_home_reference.as_str())
            || result
                .insert(
                    home.profile_id.clone(),
                    home.managed_home_reference.as_str(),
                )
                .is_some()
        {
            return Err(RestoreError::InvalidHomeMapping);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProfileConnectionState, Theme};
    const KEY: [u8; 32] = [0x6a; 32];
    fn id(n: u32) -> String {
        format!("{n:08x}-0000-4000-8000-000000000000")
    }
    fn mappings(profile: u32, home: u32) -> Vec<RestoreHomeMapping> {
        vec![RestoreHomeMapping {
            profile_id: id(profile),
            managed_home_reference: id(home),
        }]
    }
    fn source() -> (tempfile::TempDir, Storage) {
        let directory = crate::tests::private_directory();
        let mut source = Storage::create(&directory.path().join("source"), &KEY).unwrap();
        source
            .create_profile(&id(1), "Synthetic source", &id(1))
            .unwrap();
        source.connection.execute_batch("INSERT INTO accounts VALUES ('00000099-0000-4000-8000-000000000000',NULL,'unknown','synthetic@example.invalid',NULL,'2024-02-29T00:00:00Z','2024-02-29T01:00:00Z'); UPDATE profiles SET account_id='00000099-0000-4000-8000-000000000000',connection_state='connected'; UPDATE settings SET monitoring='active';").unwrap();
        (directory, source)
    }
    #[test]
    fn replacement_preserves_history_stops_monitoring_and_uses_fresh_homes() {
        let (_directory, mut source) = source();
        let snapshot = source.backup_snapshot().unwrap();
        assert_eq!(
            validate_restore_input(&snapshot.bytes, 1)
                .unwrap()
                .profile_ids,
            vec![id(1)]
        );
        let directory = crate::tests::private_directory();
        let path = directory.path().join("live");
        let mut live = Storage::create(&path, &KEY).unwrap();
        live.create_profile(&id(2), "Synthetic target", &id(2))
            .unwrap();
        live.set_theme(Theme::Dark).unwrap();
        let expected = live.restore_state().unwrap().revision;
        let summary = live
            .replace_records_from_backup(&snapshot.bytes, 1, &expected, &mappings(1, 3))
            .unwrap();
        assert_eq!(summary.accounts, 1);
        assert_eq!(live.metadata().unwrap().theme, Theme::System);
        assert_eq!(
            live.list_profiles().unwrap()[0].connection_state,
            ProfileConnectionState::AuthenticationRequired
        );
        assert_eq!(read_homes(&live.connection).unwrap(), mappings(1, 3));
        assert_eq!(
            live.connection
                .query_row("SELECT monitoring FROM settings", [], |r| r
                    .get::<_, String>(0))
                .unwrap(),
            "stopped"
        );
        assert_eq!(
            live.connection
                .query_row("SELECT last_observed_at FROM accounts", [], |r| r
                    .get::<_, String>(0))
                .unwrap(),
            "2024-02-29T01:00:00Z"
        );
        assert_eq!(
            live.connection
                .pragma_query_value(None, "journal_mode", |r| r.get::<_, String>(0))
                .unwrap(),
            "wal"
        );
        drop(live);
        assert_eq!(
            Storage::open(&path, &KEY)
                .unwrap()
                .metadata()
                .unwrap()
                .profile_count,
            1
        );
        assert!(
            Connection::open(&path)
                .unwrap()
                .query_row("SELECT count(*) FROM profiles", [], |r| r.get::<_, u32>(0))
                .is_err()
        );
    }
    #[test]
    fn stale_tokens_bad_mappings_and_invalid_images_preserve_live_records() {
        let (_directory, mut source) = source();
        let snapshot = source.backup_snapshot().unwrap();
        let directory = crate::tests::private_directory();
        let mut live = Storage::create(&directory.path().join("live"), &KEY).unwrap();
        live.create_profile(&id(2), "Preserved target", &id(2))
            .unwrap();
        let old = live.restore_state().unwrap().revision;
        live.set_theme(Theme::Dark).unwrap();
        assert_eq!(
            live.replace_records_from_backup(&snapshot.bytes, 1, &old, &mappings(1, 3)),
            Err(RestoreError::StalePreview)
        );
        let expected = live.restore_state().unwrap().revision;
        for homes in [
            vec![],
            mappings(1, 2),
            mappings(9, 3),
            mappings(1, 0)
                .into_iter()
                .map(|mut h| {
                    h.managed_home_reference = "../unsafe".into();
                    h
                })
                .collect(),
        ] {
            assert_eq!(
                live.replace_records_from_backup(&snapshot.bytes, 1, &expected, &homes),
                Err(RestoreError::InvalidHomeMapping)
            );
        }
        assert_eq!(
            live.replace_records_from_backup(b"invalid", 1, &expected, &mappings(1, 3)),
            Err(RestoreError::InvalidBackup)
        );
        assert_eq!(live.restore_state().unwrap().revision, expected);
        assert_eq!(live.list_profiles().unwrap()[0].label, "Preserved target");
    }
    #[test]
    fn a_real_full_database_failure_rolls_back_partial_writes_and_allows_retry() {
        let (_directory, mut source) = source();
        source.set_profile_archived(&id(1), true).unwrap();
        for n in 10..510 {
            source
                .create_profile(&id(n), &"😀".repeat(100), &id(n))
                .unwrap();
            source.set_profile_archived(&id(n), true).unwrap();
        }
        let snapshot = source.backup_snapshot().unwrap();
        let homes: Vec<_> = validate_restore_input(&snapshot.bytes, 1)
            .unwrap()
            .profile_ids
            .into_iter()
            .enumerate()
            .map(|(n, profile_id)| RestoreHomeMapping {
                profile_id,
                managed_home_reference: id(n as u32 + 1000),
            })
            .collect();
        let directory = crate::tests::private_directory();
        let mut live = Storage::create(&directory.path().join("live"), &KEY).unwrap();
        live.create_profile(&id(2), "Preserved target", &id(2))
            .unwrap();
        live.set_theme(Theme::Dark).unwrap();
        let expected = live.restore_state().unwrap().revision;
        let pages: u64 = live
            .connection
            .pragma_query_value(None, "page_count", |r| r.get(0))
            .unwrap();
        live.connection
            .pragma_update(None, "max_page_count", pages)
            .unwrap();
        let before = live.connection.total_changes();
        assert_eq!(
            live.replace_records_from_backup(&snapshot.bytes, 1, &expected, &homes),
            Err(RestoreError::WriteFailed)
        );
        assert!(
            live.connection.total_changes() > before,
            "failure must occur after real writes"
        );
        assert_eq!(live.list_profiles().unwrap()[0].label, "Preserved target");
        assert_eq!(live.metadata().unwrap().theme, Theme::Dark);
        live.connection
            .pragma_update(None, "max_page_count", 1_000_000)
            .unwrap();
        let expected = live.restore_state().unwrap().revision;
        assert_eq!(
            live.replace_records_from_backup(&snapshot.bytes, 1, &expected, &homes)
                .unwrap()
                .archived_profiles,
            501
        );
    }
    #[test]
    fn checkpoint_recovery_requires_exact_original_homes_and_rejects_cross_profile_reuse() {
        let (_directory, mut source) = source();
        let checkpoint = source.backup_snapshot().unwrap();
        let incoming = validate_checkpoint_restore_input(&checkpoint.bytes, 1).unwrap();
        let expected = source.restore_state().unwrap().revision;
        assert_eq!(
            source.replace_records_from_checkpoint(
                &checkpoint.bytes,
                1,
                &expected,
                &mappings(1, 3)
            ),
            Err(RestoreError::InvalidHomeMapping)
        );
        source
            .replace_records_from_checkpoint(&checkpoint.bytes, 1, &expected, &incoming.homes)
            .unwrap();
        source
            .connection
            .execute("UPDATE profiles SET id=?1", [id(2)])
            .unwrap();
        let expected = source.restore_state().unwrap().revision;
        assert_eq!(
            source.replace_records_from_checkpoint(
                &checkpoint.bytes,
                1,
                &expected,
                &incoming.homes
            ),
            Err(RestoreError::InvalidHomeMapping)
        );
    }
}
