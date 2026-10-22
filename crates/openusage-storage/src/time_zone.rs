use crate::{Storage, StorageError};

pub fn valid_time_zone(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_+-".contains(&b))
        && jiff::tz::TimeZone::get(value)
            .is_ok_and(|zone| zone.iana_name() == Some(value) || matches!(value, "UTC" | "Etc/UTC"))
}

impl Storage {
    pub fn time_zone(&self) -> Result<String, StorageError> {
        let value: String = self
            .connection
            .query_row("SELECT timezone FROM settings WHERE id=1", [], |row| {
                row.get(0)
            })
            .map_err(|_| StorageError::ReadFailed)?;
        if valid_time_zone(&value) {
            Ok(value)
        } else {
            Err(StorageError::ReadFailed)
        }
    }

    pub fn set_time_zone(&mut self, value: &str) -> Result<(), StorageError> {
        if !valid_time_zone(value) {
            return Err(StorageError::WriteFailed);
        }
        let changed = self.connection.execute(
            "UPDATE settings SET timezone=?1,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=1", [value],
        ).map_err(|_| StorageError::WriteFailed)?;
        if changed == 1 {
            Ok(())
        } else {
            Err(StorageError::WriteFailed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zone_survives_restart_and_validates_backup_without_changing_other_settings() {
        let dir = crate::tests::private_directory();
        let path = dir.path().join("zones.db");
        let key = [47; 32];
        let mut storage = Storage::create(&path, &key).unwrap();
        assert_eq!(storage.time_zone().unwrap(), "UTC");
        storage.set_time_zone("Asia/Kolkata").unwrap();
        for invalid in [
            "system",
            "Invalid/Zone",
            "../UTC",
            "",
            "+05:30",
            "Asia/Kolkata\n",
        ] {
            assert!(storage.set_time_zone(invalid).is_err());
            assert_eq!(storage.time_zone().unwrap(), "Asia/Kolkata");
        }
        let snapshot = storage.backup_snapshot().unwrap();
        crate::validate_backup_snapshot(&snapshot.bytes, crate::CURRENT_SCHEMA_VERSION).unwrap();
        drop(storage);
        let mut storage = Storage::open(&path, &key).unwrap();
        assert_eq!(storage.time_zone().unwrap(), "Asia/Kolkata");
        storage.set_time_zone("America/New_York").unwrap();
        assert_eq!(storage.time_zone().unwrap(), "America/New_York");
        assert_eq!(storage.metadata().unwrap().theme, crate::Theme::System);
    }
}
