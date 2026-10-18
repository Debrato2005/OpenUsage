//! Read current counts, logical size, and an opaque change token from one database snapshot.

use crate::{BackupContentSummary, CURRENT_SCHEMA_VERSION, Storage, StorageError};
use std::{fmt, sync::Arc};

/// Compare tokens only in Rust. Reopening storage always changes the token's scope.
/// This detects application writes and commits from other SQLite connections, not hostile file edits.
#[derive(Clone)]
pub struct StorageRevision {
    scope: Arc<()>,
    data_version: u32,
    total_changes: u64,
    schema_cookie: u32,
    schema_version: u32,
}

impl PartialEq for StorageRevision {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.scope, &other.scope)
            && self.data_version == other.data_version
            && self.total_changes == other.total_changes
            && self.schema_cookie == other.schema_cookie
            && self.schema_version == other.schema_version
    }
}

impl Eq for StorageRevision {}

impl StorageRevision {
    pub(crate) fn matches(
        &self,
        scope: &Arc<()>,
        connection: &rusqlite::Connection,
    ) -> rusqlite::Result<bool> {
        Ok(Arc::ptr_eq(&self.scope, scope)
            && self.data_version
                == connection
                    .pragma_query_value(Some("main"), "data_version", |r| r.get::<_, u32>(0))?
            && self.total_changes == connection.total_changes()
            && self.schema_cookie
                == connection
                    .pragma_query_value(Some("main"), "schema_version", |r| r.get::<_, u32>(0))?
            && self.schema_version
                == connection
                    .pragma_query_value(Some("main"), "user_version", |r| r.get::<_, u32>(0))?)
    }
}

impl fmt::Debug for StorageRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("StorageRevision { .. }")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoreState {
    pub revision: StorageRevision,
    pub summary: BackupContentSummary,
    /// Main database page count times page size, including pages visible through WAL.
    /// This logical estimate excludes sidecars and is not a checkpoint space guarantee.
    pub logical_bytes: u64,
}

impl Storage {
    /// Read one consistent snapshot without changing application records.
    /// Compare revisions before and after checkpoint creation; retain only an unchanged preview.
    /// A future apply must compare again inside its write transaction before replacing records.
    pub fn restore_state(&mut self) -> Result<RestoreState, StorageError> {
        let transaction = self
            .connection
            .transaction()
            .map_err(|_| StorageError::ReadFailed)?;
        // Reading the header opens the main read snapshot before counts or page size are read.
        let data_version = transaction
            .pragma_query_value(Some("main"), "data_version", |row| row.get(0))
            .map_err(|_| StorageError::ReadFailed)?;
        let schema_version: u32 = transaction
            .pragma_query_value(Some("main"), "user_version", |row| row.get(0))
            .map_err(|_| StorageError::ReadFailed)?;
        if schema_version != CURRENT_SCHEMA_VERSION {
            return Err(StorageError::UnsupportedSchema);
        }
        let schema_cookie = transaction
            .pragma_query_value(Some("main"), "schema_version", |row| row.get(0))
            .map_err(|_| StorageError::ReadFailed)?;
        let summary = transaction
            .query_row(
                "SELECT (SELECT count(*) FROM profiles WHERE archived=0), \
                 (SELECT count(*) FROM profiles WHERE archived=1), \
                 (SELECT count(*) FROM accounts)",
                [],
                |row| {
                    Ok(BackupContentSummary {
                        active_profiles: row.get(0)?,
                        archived_profiles: unsigned_value(row, 1)?,
                        accounts: unsigned_value(row, 2)?,
                    })
                },
            )
            .map_err(|_| StorageError::ReadFailed)?;
        if summary.active_profiles > 10 {
            return Err(StorageError::ReadFailed);
        }
        let pages: u64 = transaction
            .pragma_query_value(Some("main"), "page_count", |row| unsigned_value(row, 0))
            .map_err(|_| StorageError::ReadFailed)?;
        // SQLCipher returns its page-size pragma as decimal text, unlike SQLite's integer pragma.
        let page_size: String = transaction
            .pragma_query_value(Some("main"), "cipher_page_size", |row| row.get(0))
            .map_err(|_| StorageError::ReadFailed)?;
        let page_size: u64 = page_size.parse().map_err(|_| StorageError::ReadFailed)?;
        if !(512..=65536).contains(&page_size) || !page_size.is_power_of_two() {
            return Err(StorageError::ReadFailed);
        }
        let logical_bytes = pages
            .checked_mul(page_size)
            .filter(|bytes| *bytes > 0)
            .ok_or(StorageError::ReadFailed)?;
        let total_changes = transaction.total_changes();
        transaction.commit().map_err(|_| StorageError::ReadFailed)?;
        Ok(RestoreState {
            revision: StorageRevision {
                scope: Arc::clone(&self.revision_scope),
                data_version,
                total_changes,
                schema_cookie,
                schema_version,
            },
            summary,
            logical_bytes,
        })
    }
}

fn unsigned_value(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    value
        .try_into()
        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Theme;

    const KEY: [u8; 32] = [0x67; 32];

    #[test]
    fn revisions_detect_local_external_and_reopened_changes() {
        let temporary = crate::tests::private_directory();
        let path = temporary.path().join("live.db");
        let mut storage = Storage::create(&path, &KEY).unwrap();
        let initial = storage.restore_state().unwrap();
        assert_eq!(storage.restore_state().unwrap(), initial);
        storage.set_theme(Theme::Dark).unwrap();
        let local = storage.restore_state().unwrap();
        assert_ne!(local.revision, initial.revision);
        assert_eq!(local.summary, initial.summary);

        let mut second = Storage::open(&path, &KEY).unwrap();
        let before_external = storage.restore_state().unwrap();
        second.set_theme(Theme::Light).unwrap();
        let external = storage.restore_state().unwrap();
        assert_ne!(external.revision, before_external.revision);
        assert_eq!(storage.metadata().unwrap().theme, Theme::Light);
        assert_eq!(external.summary, before_external.summary);
        drop(second);
        drop(storage);

        let mut reopened = Storage::open(&path, &KEY).unwrap();
        let after_reopen = reopened.restore_state().unwrap();
        assert_ne!(after_reopen.revision, external.revision);
        assert_eq!(after_reopen.summary, external.summary);
        assert_eq!(after_reopen.logical_bytes, external.logical_bytes);
    }

    #[cfg(unix)]
    #[test]
    fn active_wal_counts_and_size_survive_checkpoint_without_revision_changes() {
        let temporary = crate::tests::private_directory();
        let path = temporary.path().join("live.db");
        let mut storage = Storage::create(&path, &KEY).unwrap();
        storage
            .connection
            .pragma_update(None, "wal_autocheckpoint", 0)
            .unwrap();
        storage.connection.execute_batch(
            "INSERT INTO accounts VALUES ('00000099-0000-4000-8000-000000000000',NULL,'unknown',NULL,NULL,'2024-01-01T00:00:00Z','2024-01-01T00:00:00Z')",
        ).unwrap();
        for number in 1..=20 {
            let id = format!("{number:08x}-0000-4000-8000-000000000000");
            storage.create_profile(&id, &"😀".repeat(100), &id).unwrap();
            if number <= 18 {
                storage.set_profile_archived(&id, true).unwrap();
            }
        }
        let before = storage.restore_state().unwrap();
        assert_eq!(
            before.summary,
            BackupContentSummary {
                active_profiles: 2,
                archived_profiles: 18,
                accounts: 1,
            }
        );
        assert!(path.with_extension("db-wal").metadata().unwrap().len() > 0);
        assert!(before.logical_bytes > path.metadata().unwrap().len());
        let checkpoint = temporary.path().join("checkpoint.pending");
        storage.create_encrypted_checkpoint(&checkpoint).unwrap();
        assert_eq!(storage.restore_state().unwrap(), before);
        let mut saved = Storage::open(&checkpoint, &KEY).unwrap();
        assert_eq!(saved.restore_state().unwrap().summary, before.summary);
        storage.backup_snapshot().unwrap();
        assert_eq!(storage.restore_state().unwrap(), before);
    }
}
