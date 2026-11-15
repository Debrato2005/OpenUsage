use super::*;
use crate::{LocalUsageRow, LocalUsageScan, LogConsent};
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use std::time::Duration;

const KEY: [u8; 32] = [157; 32];
const DEVICE: &str = "f9168c5e-ceb2-4faa-b6bf-329bf39fa1e4";
const NOW: &str = "2026-11-15T10:00:00.000Z";

fn save(storage: &mut Storage, source: &str, count: u64) {
    storage
        .save_local_usage_scan(&LocalUsageScan {
            log_source_ids: &[source.into()],
            scanner_version: "20.0.48",
            time_zone: "UTC",
            since: None,
            scanned_at_utc: NOW,
            rows: &[LocalUsageRow {
                local_date: "2026-11-15".into(),
                tool: "claude".into(),
                model: "claude-sonnet".into(),
                input_tokens: count,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                total_tokens: count,
                estimated_value_usd_micros: None,
            }],
        })
        .unwrap();
}
fn setup() -> (tempfile::TempDir, Storage, String) {
    let directory = crate::tests::private_directory();
    let mut storage = Storage::create(&directory.path().join("upload.db"), &KEY).unwrap();
    let source = storage
        .set_log_source(
            "claude",
            "/synthetic/private-upload-project",
            LogConsent::Granted,
            NOW,
        )
        .unwrap()
        .id;
    save(&mut storage, &source, 1);
    (directory, storage, source)
}
fn expected(count: u64) -> String {
    format!(
        "{{\"deviceId\":\"{DEVICE}\",\"rows\":[{{\"date\":\"2026-11-15\",\"tool\":\"claude\",\"model\":\"claude-sonnet\",\"inputTokens\":{count},\"outputTokens\":0,\"cacheReadTokens\":0,\"cacheWriteTokens\":0,\"totalTokens\":{count},\"estimatedValueMicros\":null}}]}}"
    )
}

#[test]
fn desktop_and_query_only_connections_return_identical_bytes_without_writes() {
    let (directory, storage, _) = setup();
    storage
        .connection
        .authorizer(Some(|context: AuthContext<'_>| match context.action {
            AuthAction::Insert { .. } | AuthAction::Update { .. } | AuthAction::Delete { .. } => {
                Authorization::Deny
            }
            _ => Authorization::Allow,
        }))
        .unwrap();
    let changes = storage.connection.total_changes();
    assert_eq!(
        storage.leaderboard_upload(DEVICE, 30, NOW.parse().unwrap()),
        Ok(expected(1))
    );
    assert_eq!(storage.connection.total_changes(), changes);
    let reader = Storage::open_read_only(&directory.path().join("upload.db"), &KEY).unwrap();
    assert_eq!(
        reader.leaderboard_upload(DEVICE, 30, NOW.parse().unwrap()),
        Ok(expected(1))
    );
}

#[test]
fn both_connection_modes_keep_one_snapshot_during_a_concurrent_commit() {
    for read_only in [false, true] {
        let (directory, initial, source) = setup();
        let mut writer = Storage::open(&directory.path().join("upload.db"), &KEY).unwrap();
        let storage = if read_only {
            drop(initial);
            Storage::open_read_only(&directory.path().join("upload.db"), &KEY).unwrap()
        } else {
            initial
        };
        let (begin_tx, begin_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let writer = std::thread::spawn(move || {
            begin_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            save(&mut writer, &source, 2);
            done_tx.send(()).unwrap();
        });
        let mut triggered = false;
        storage
            .connection
            .authorizer(Some(move |context: AuthContext<'_>| {
                if !triggered
                    && matches!(
                        context.action,
                        AuthAction::Read {
                            table_name: "log_sources",
                            ..
                        }
                    )
                {
                    triggered = true;
                    begin_tx.send(()).unwrap();
                    done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                }
                Authorization::Allow
            }))
            .unwrap();
        let body = storage.leaderboard_upload(DEVICE, 30, NOW.parse().unwrap());
        storage
            .connection
            .authorizer(None::<fn(AuthContext<'_>) -> Authorization>)
            .unwrap();
        writer.join().unwrap();
        assert_eq!(body, Ok(expected(1)), "read_only={read_only}");
        assert_eq!(
            storage.leaderboard_upload(DEVICE, 30, NOW.parse().unwrap()),
            Ok(expected(2))
        );
    }
}

#[test]
fn the_shared_boundary_refuses_invalid_selection_and_releases_failed_reads() {
    let (_directory, mut storage, source) = setup();
    for (device, days) in [("invalid", 30), (DEVICE, 0), (DEVICE, 367)] {
        assert_eq!(
            storage.leaderboard_upload(device, days, NOW.parse().unwrap()),
            Err(BoardUploadError::InvalidSelection)
        );
    }
    storage
        .connection
        .authorizer(Some(|context: AuthContext<'_>| match context.action {
            AuthAction::Read {
                table_name: "local_usage_days",
                ..
            } => Authorization::Deny,
            _ => Authorization::Allow,
        }))
        .unwrap();
    assert_eq!(
        storage.leaderboard_upload(DEVICE, 30, NOW.parse().unwrap()),
        Err(BoardUploadError::ReadFailed)
    );
    storage
        .connection
        .authorizer(None::<fn(AuthContext<'_>) -> Authorization>)
        .unwrap();
    save(&mut storage, &source, 3);
    assert_eq!(
        storage.leaderboard_upload(DEVICE, 30, NOW.parse().unwrap()),
        Ok(expected(3))
    );
}
