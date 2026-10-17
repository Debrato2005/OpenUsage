use crate::Storage;
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileEnvironment {
    Native,
    Wsl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileConnectionState {
    Connected,
    AuthenticationRequired,
    Unauthorized,
    Offline,
    Unsupported,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileSummary {
    pub id: String,
    pub label: String,
    pub environment: ProfileEnvironment,
    pub connection_state: ProfileConnectionState,
    pub archived: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileError {
    InvalidId,
    InvalidLabel,
    InvalidHomeReference,
    DuplicateId,
    HomeInUse,
    CapacityReached,
    NotFound,
    ReadFailed,
    WriteFailed,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidId => "Use a canonical lowercase profile UUID.",
            Self::InvalidLabel => {
                "Use 1 to 100 characters without control characters for the profile label."
            }
            Self::InvalidHomeReference => {
                "Use a canonical lowercase UUID for the managed home reference."
            }
            Self::DuplicateId => "This profile UUID already exists.",
            Self::HomeInUse => "Another profile already uses this managed home reference.",
            Self::CapacityReached => {
                "Archive an active profile before adding another. The active limit is 10."
            }
            Self::NotFound => "The profile does not exist.",
            Self::ReadFailed => "The profile records could not be read.",
            Self::WriteFailed => "The profile change could not be saved.",
        })
    }
}

impl std::error::Error for ProfileError {}

impl Storage {
    /// Return active and archived profiles without managed home references or credentials.
    pub fn list_profiles(&self) -> Result<Vec<ProfileSummary>, ProfileError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, name, environment, connection_state, archived FROM profiles \
             ORDER BY archived, created_at, id",
            )
            .map_err(|_| ProfileError::ReadFailed)?;
        statement
            .query_map([], read_profile)
            .map_err(|_| ProfileError::ReadFailed)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ProfileError::ReadFailed)
    }

    /// The native caller supplies UUIDs and owns all managed directory creation and approval.
    /// This operation creates only a native profile that requires authentication.
    pub fn create_profile(
        &mut self,
        id: &str,
        label: &str,
        managed_home_reference: &str,
    ) -> Result<ProfileSummary, ProfileError> {
        validate_uuid(id).map_err(|()| ProfileError::InvalidId)?;
        validate_uuid(managed_home_reference).map_err(|()| ProfileError::InvalidHomeReference)?;
        let label = validate_label(label)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ProfileError::WriteFailed)?;
        let duplicate: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE id = ?1)",
                [id],
                |row| row.get(0),
            )
            .map_err(|_| ProfileError::ReadFailed)?;
        if duplicate {
            return Err(ProfileError::DuplicateId);
        }
        let home_in_use: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE approved_home_reference = ?1)",
                [managed_home_reference],
                |row| row.get(0),
            )
            .map_err(|_| ProfileError::ReadFailed)?;
        if home_in_use {
            return Err(ProfileError::HomeInUse);
        }
        require_capacity(&transaction)?;
        transaction.execute(
            "INSERT INTO profiles \
             (id, name, environment, approved_home_reference, connection_state, archived, created_at, updated_at) \
             VALUES (?1, ?2, 'native', ?3, 'authentication_required', 0, \
                     strftime('%Y-%m-%dT%H:%M:%fZ','now'), strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![id, label, managed_home_reference],
        ).map_err(|_| ProfileError::WriteFailed)?;
        let profile = profile_by_id(&transaction, id)?;
        transaction
            .commit()
            .map_err(|_| ProfileError::WriteFailed)?;
        Ok(profile)
    }

    pub fn rename_profile(
        &mut self,
        id: &str,
        label: &str,
    ) -> Result<ProfileSummary, ProfileError> {
        validate_uuid(id).map_err(|()| ProfileError::InvalidId)?;
        let label = validate_label(label)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ProfileError::WriteFailed)?;
        let changed = transaction.execute(
            "UPDATE profiles SET name = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?2",
            params![label, id],
        ).map_err(|_| ProfileError::WriteFailed)?;
        if changed == 0 {
            return Err(ProfileError::NotFound);
        }
        let profile = profile_by_id(&transaction, id)?;
        transaction
            .commit()
            .map_err(|_| ProfileError::WriteFailed)?;
        Ok(profile)
    }

    /// Archiving preserves the UUID, account association, and stored records.
    pub fn set_profile_archived(
        &mut self,
        id: &str,
        archived: bool,
    ) -> Result<ProfileSummary, ProfileError> {
        validate_uuid(id).map_err(|()| ProfileError::InvalidId)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ProfileError::WriteFailed)?;
        let mut profile = profile_by_id(&transaction, id)?;
        if profile.archived != archived {
            if !archived {
                require_capacity(&transaction)?;
            }
            transaction.execute(
                "UPDATE profiles SET archived = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?2",
                params![archived, id],
            ).map_err(|_| ProfileError::WriteFailed)?;
            profile.archived = archived;
        }
        transaction
            .commit()
            .map_err(|_| ProfileError::WriteFailed)?;
        Ok(profile)
    }
}

fn validate_uuid(value: &str) -> Result<(), ()> {
    let id = Uuid::parse_str(value).map_err(|_| ())?;
    if id.hyphenated().to_string() != value {
        return Err(());
    }
    Ok(())
}

fn validate_label(value: &str) -> Result<&str, ProfileError> {
    if value.len() > 1024 || value.chars().any(char::is_control) {
        return Err(ProfileError::InvalidLabel);
    }
    let label = value.trim();
    if !(1..=100).contains(&label.chars().count()) {
        return Err(ProfileError::InvalidLabel);
    }
    Ok(label)
}

fn require_capacity(connection: &Connection) -> Result<(), ProfileError> {
    let active: u32 = connection
        .query_row(
            "SELECT count(*) FROM profiles WHERE archived = 0",
            [],
            |row| row.get(0),
        )
        .map_err(|_| ProfileError::ReadFailed)?;
    if active >= 10 {
        return Err(ProfileError::CapacityReached);
    }
    Ok(())
}

fn profile_by_id(connection: &Connection, id: &str) -> Result<ProfileSummary, ProfileError> {
    connection
        .query_row(
            "SELECT id, name, environment, connection_state, archived FROM profiles WHERE id = ?1",
            [id],
            read_profile,
        )
        .optional()
        .map_err(|_| ProfileError::ReadFailed)?
        .ok_or(ProfileError::NotFound)
}

fn read_profile(row: &Row<'_>) -> rusqlite::Result<ProfileSummary> {
    let environment = match row.get::<_, String>(2)?.as_str() {
        "native" => ProfileEnvironment::Native,
        "wsl" => ProfileEnvironment::Wsl,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let connection_state = match row.get::<_, String>(3)?.as_str() {
        "connected" => ProfileConnectionState::Connected,
        "authentication_required" => ProfileConnectionState::AuthenticationRequired,
        "unauthorized" => ProfileConnectionState::Unauthorized,
        "offline" => ProfileConnectionState::Offline,
        "unsupported" => ProfileConnectionState::Unsupported,
        "error" => ProfileConnectionState::Error,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(ProfileSummary {
        id: row.get(0)?,
        label: row.get(1)?,
        environment,
        connection_state,
        archived: row.get(4)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    const KEY: [u8; 32] = [0x71; 32];

    fn id(number: u32) -> String {
        format!("{number:08x}-0000-4000-8000-000000000000")
    }

    #[test]
    fn concurrent_capacity_and_archive_preserve_profiles_across_reopen() {
        let directory = crate::tests::private_directory();
        let path = directory.path().join("profiles.db");
        let mut storage = Storage::create(&path, &KEY).unwrap();
        for number in 1..=9 {
            storage
                .create_profile(&id(number), "Synthetic profile", &id(number))
                .unwrap();
        }
        storage.connection.execute(
            "INSERT INTO accounts (id, identity_confidence, first_observed_at, last_observed_at) \
             VALUES (?1, 'unknown', '2026-10-18T00:00:00Z', '2026-10-18T00:00:00Z')", [&id(1)],
        ).unwrap();
        storage
            .connection
            .execute(
                "UPDATE profiles SET account_id = ?1, connection_state = 'offline' WHERE id = ?1",
                [&id(1)],
            )
            .unwrap();
        let original_created: String = storage
            .connection
            .query_row(
                "SELECT created_at FROM profiles WHERE id = ?1",
                [&id(1)],
                |row| row.get(0),
            )
            .unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = (10..=11)
            .map(|number| {
                let mut connection = Storage::open(&path, &KEY).unwrap();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    connection.create_profile(&id(number), "Concurrent profile", &id(number))
                })
            })
            .collect();
        let results: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| **result == Err(ProfileError::CapacityReached))
                .count(),
            1
        );
        assert_eq!(storage.metadata().unwrap().profile_count, 10);
        assert_eq!(
            storage.create_profile(&id(12), "Eleventh profile", &id(12)),
            Err(ProfileError::CapacityReached)
        );
        assert!(storage.set_profile_archived(&id(1), true).unwrap().archived);
        storage
            .create_profile(&id(12), "New active profile", &id(12))
            .unwrap();
        assert_eq!(
            storage.set_profile_archived(&id(1), false),
            Err(ProfileError::CapacityReached)
        );
        assert!(
            !storage
                .set_profile_archived(&id(2), false)
                .unwrap()
                .archived
        );
        storage.set_profile_archived(&id(2), true).unwrap();
        assert!(
            !storage
                .set_profile_archived(&id(1), false)
                .unwrap()
                .archived
        );
        drop(storage);

        let reopened = Storage::open(&path, &KEY).unwrap();
        assert_eq!(reopened.metadata().unwrap().profile_count, 10);
        let profiles = reopened.list_profiles().unwrap();
        assert_eq!(profiles.len(), 11);
        let first = profiles.iter().find(|profile| profile.id == id(1)).unwrap();
        assert_eq!(first.connection_state, ProfileConnectionState::Offline);
        assert!(!first.archived);
        assert!(
            profiles
                .iter()
                .find(|profile| profile.id == id(2))
                .unwrap()
                .archived
        );
        let (account, home, created): (String, String, String) = reopened.connection.query_row(
            "SELECT account_id, approved_home_reference, created_at FROM profiles WHERE id = ?1", [&id(1)],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap();
        assert_eq!(account, id(1));
        assert_eq!(home, id(1));
        assert_eq!(created, original_created);
    }

    #[test]
    fn labels_ids_and_duplicate_homes_are_checked_without_losing_records() {
        let directory = crate::tests::private_directory();
        let path = directory.path().join("profiles.db");
        let mut storage = Storage::create(&path, &KEY).unwrap();
        let too_long = "é".repeat(101);
        for label in [
            "",
            " \u{2003} ",
            "line\nbreak",
            "tab\tlabel",
            "nul\0label",
            "\u{7f}",
            &too_long,
        ] {
            assert_eq!(
                storage.create_profile(&id(1), label, &id(1)),
                Err(ProfileError::InvalidLabel)
            );
        }
        for invalid in [
            "../external",
            "ABCDEF01-0000-4000-8000-000000000000",
            "00000001000040008000000000000000",
        ] {
            assert_eq!(
                storage.create_profile(invalid, "Label", &id(1)),
                Err(ProfileError::InvalidId)
            );
            assert_eq!(
                storage.create_profile(&id(1), "Label", invalid),
                Err(ProfileError::InvalidHomeReference)
            );
        }
        let created = storage
            .create_profile(&id(1), " \u{2003} Trimmed label\u{a0} ", &id(1))
            .unwrap();
        assert_eq!(created.label, "Trimmed label");
        assert_eq!(created.environment, ProfileEnvironment::Native);
        assert_eq!(
            created.connection_state,
            ProfileConnectionState::AuthenticationRequired
        );
        let padded_boundary = format!("{}A", " ".repeat(1023));
        assert_eq!(
            storage
                .rename_profile(&id(1), &padded_boundary)
                .unwrap()
                .label,
            "A"
        );
        let oversized = format!("{}A", " ".repeat(1024));
        assert_eq!(
            storage.create_profile(&id(2), &oversized, &id(2)),
            Err(ProfileError::InvalidLabel)
        );
        assert_eq!(
            storage.rename_profile(&id(1), &oversized),
            Err(ProfileError::InvalidLabel)
        );
        let boundary = "é".repeat(100);
        assert_eq!(
            storage.rename_profile(&id(1), &boundary).unwrap().label,
            boundary
        );
        assert_eq!(
            storage.rename_profile(&id(1), &too_long),
            Err(ProfileError::InvalidLabel)
        );
        assert_eq!(
            storage.rename_profile(&id(1), "bad\nlabel"),
            Err(ProfileError::InvalidLabel)
        );
        let label = "Profile '; DELETE FROM profiles; --";
        let renamed = storage.rename_profile(&id(1), label).unwrap();
        assert_eq!(renamed.id, id(1));
        assert_eq!(renamed.label, label);
        storage.create_profile(&id(2), label, &id(2)).unwrap();
        assert_eq!(
            storage.create_profile(&id(1), "Duplicate ID", &id(3)),
            Err(ProfileError::DuplicateId)
        );
        assert_eq!(
            storage.create_profile(&id(3), "Duplicate home", &id(1)),
            Err(ProfileError::HomeInUse)
        );
        assert_eq!(
            storage.rename_profile(&id(99), "Missing"),
            Err(ProfileError::NotFound)
        );
        assert_eq!(
            storage.set_profile_archived(&id(99), true),
            Err(ProfileError::NotFound)
        );
        drop(storage);

        let reopened = Storage::open(&path, &KEY).unwrap();
        let profiles = reopened.list_profiles().unwrap();
        assert_eq!(profiles.len(), 2);
        assert!(profiles.iter().all(|profile| profile.label == label));
        assert!(profiles.iter().any(|profile| profile.id == renamed.id));
    }
}
