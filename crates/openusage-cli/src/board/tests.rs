//! Command-entry acceptance with private encrypted workspaces and synthetic data.
use super::*;
use openusage_locallogs::{AgentPeriod, UsagePeriod};
use openusage_locallogs::{ModelDetail, TokenDetails};
use openusage_storage::Storage;
use openusage_storage::{GroupedLocalScan, LocalUsageRow, LocalUsageScan, LogConsent};
use std::path::{Path, PathBuf};

const DEVICE: &str = "f9168c5e-ceb2-4faa-b6bf-329bf39fa1e4";
const KEY: [u8; 32] = [143; 32];
const NOW: &str = "2026-11-08T10:00:00.000Z";
const MAX_COUNT: u64 = 1_000_000_000_000;

fn create_workspace() -> (tempfile::TempDir, PathBuf, Storage) {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let workspace = directory.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).unwrap();
    let storage = Storage::create(&workspace.join("openusage.db"), &KEY).unwrap();
    (directory, workspace, storage)
}
fn source(store: &mut Storage, tool: &str, suffix: &str) -> String {
    store
        .set_log_source(
            tool,
            &format!("/srv/synthetic/private-project/{suffix}"),
            LogConsent::Granted,
            NOW,
        )
        .unwrap()
        .id
}
fn save(store: &mut Storage, ids: &[String], zone: &str, rows: &[LocalUsageRow]) {
    store
        .save_local_usage_scan(&LocalUsageScan {
            log_source_ids: ids,
            scanner_version: "20.0.48",
            time_zone: zone,
            since: None,
            scanned_at_utc: NOW,
            rows,
        })
        .unwrap();
}
fn grouped(store: &mut Storage, tool: &str, id: &str, zone: Option<&str>, periods: &[AgentPeriod]) {
    store
        .save_grouped_local_scan(&GroupedLocalScan {
            tool,
            source_ids: &[id.into()],
            time_zone: zone,
            scanner_version: "20.0.48",
            observed_at_utc: NOW,
            periods,
            sessions: Some(&[]),
        })
        .unwrap();
}
fn row(model: &str, input: u64) -> LocalUsageRow {
    LocalUsageRow {
        local_date: "2026-11-08".into(),
        tool: "claude".into(),
        model: model.into(),
        input_tokens: input,
        output_tokens: 0,
        cache_write_tokens: 0,
        cache_read_tokens: 0,
        total_tokens: input,
        estimated_value_usd_micros: None,
    }
}
fn preview(workspace: &Path, now: &str, extra: &[&str]) -> Result<String, CliError> {
    let env = Env {
        workspace: Some(workspace.into()),
        key: Box::new(|| Ok(Some(zeroize::Zeroizing::new(KEY)))),
        now: Some(now.parse().unwrap()),
        ..Env::for_tests()
    };
    let mut args = vec!["board", "sync", "--dry-run", "--device-id", DEVICE];
    args.extend_from_slice(extra);
    let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
    let mut prompt = Vec::new();
    let result = crate::run(&args, &env, &mut std::io::empty(), &mut prompt).0;
    assert!(prompt.is_empty());
    result
}
fn directory_bytes(directory: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().into_string().unwrap(),
                std::fs::read(entry.path()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

#[test]
fn saved_scanner_report_becomes_exact_private_repeatable_prospective_body() {
    let (_directory, workspace, mut store) = create_workspace();
    let rows = openusage_locallogs::parse_daily_by_agent(include_bytes!(
        "../../../openusage-locallogs/tests/fixtures/ccusage-20.0.48-daily-by-agent.json"
    ))
    .unwrap();
    let sources = [
        source(&mut store, "claude", "claude"),
        source(&mut store, "codex", "codex"),
    ];
    let rows = rows
        .into_iter()
        .map(|r| LocalUsageRow {
            local_date: r.date,
            tool: r.tool.into(),
            model: r.model,
            input_tokens: r.input_tokens,
            output_tokens: r.output_tokens,
            cache_write_tokens: r.cache_write_tokens,
            cache_read_tokens: r.cache_read_tokens,
            total_tokens: r.total_tokens,
            estimated_value_usd_micros: r.estimated_value_micros,
        })
        .collect::<Vec<_>>();
    save(&mut store, &sources, "Asia/Kolkata", &rows);
    let profile = "00000001-0000-4000-8000-000000000001";
    store
        .create_profile(
            profile,
            "private-account-label",
            "00000001-0000-4000-8000-000000000002",
        )
        .unwrap();
    drop(store);
    let before = directory_bytes(&workspace);
    assert!(
        !std::fs::read(workspace.join("openusage.db"))
            .unwrap()
            .starts_with(b"SQLite format 3")
    );
    let output = preview(&workspace, NOW, &[]).unwrap();
    assert_eq!(preview(&workspace, NOW, &["--json"]).unwrap(), output);
    assert_eq!(directory_bytes(&workspace), before);
    let body: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        body.as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["deviceId", "rows"]
    );
    assert_eq!(body["deviceId"], DEVICE);
    let rows = body["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 4);
    let sonnet = rows
        .iter()
        .find(|r| r["model"] == "claude-sonnet-4-5-20250929")
        .unwrap();
    assert_eq!(
        sonnet,
        &serde_json::json!({"date":"2026-11-04","tool":"claude","model":"claude-sonnet-4-5-20250929","inputTokens":3000,"outputTokens":1300,"cacheReadTokens":8000,"cacheWriteTokens":200,"totalTokens":12500,"estimatedValueMicros":31650})
    );
    let unknown = rows
        .iter()
        .find(|r| r["model"] == "totally-unknown-model-x")
        .unwrap();
    assert!(unknown["estimatedValueMicros"].is_null());
    assert_eq!(rows.iter().filter(|r| r["tool"] == "codex").count(), 1);
    for row in rows {
        assert_eq!(row.as_object().unwrap().len(), 9);
    }
    for private in sources.iter().map(String::as_str).chain([
        profile,
        "private-account-label",
        "private-project",
        "/srv",
        "email",
        "project",
        "session",
        "reasoning",
    ]) {
        assert!(!output.contains(private), "private field: {private}");
    }
}

#[test]
fn empty_workspace_and_complete_grouped_zero_preserve_unknown_estimates() {
    let (_directory, workspace, mut store) = create_workspace();
    drop(store);
    let empty = preview(&workspace, NOW, &[]).unwrap();
    assert_eq!(empty, format!("{{\"deviceId\":\"{DEVICE}\",\"rows\":[]}}"));
    store = Storage::open(&workspace.join("openusage.db"), &KEY).unwrap();
    let id = source(&mut store, "codex", "codex");
    let details = TokenDetails {
        input_tokens: Some(0),
        output_tokens: Some(0),
        cache_read_tokens: Some(0),
        cache_write_tokens: Some(0),
        reasoning_tokens: Some(0),
    };
    grouped(
        &mut store,
        "codex",
        &id,
        Some("UTC"),
        &[AgentPeriod {
            period: UsagePeriod::Day("2026-11-08".into()),
            total_tokens: 0,
            details: details.clone(),
            estimated_value_micros: None,
            models: vec![ModelDetail {
                model: "gpt-5".into(),
                total_tokens: Some(0),
                details,
                estimated_value_micros: None,
            }],
        }],
    );
    drop(store);
    let output = preview(&workspace, NOW, &[]).unwrap();
    let body: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(body["rows"][0]["totalTokens"], 0);
    assert!(body["rows"][0]["estimatedValueMicros"].is_null());
    assert!(body["rows"][0].get("reasoningTokens").is_none());
}

#[test]
fn partial_grouped_categories_and_undated_periods_refuse_without_fabrication() {
    for (tool, zone, period) in [
        (
            "codex",
            Some("UTC"),
            AgentPeriod {
                period: UsagePeriod::Day("2026-11-08".into()),
                total_tokens: 25,
                details: TokenDetails::default(),
                estimated_value_micros: None,
                models: vec![ModelDetail {
                    model: "gpt-5".into(),
                    total_tokens: Some(25),
                    details: TokenDetails {
                        input_tokens: Some(25),
                        ..TokenDetails::default()
                    },
                    estimated_value_micros: None,
                }],
            },
        ),
        (
            "codex",
            Some("UTC"),
            AgentPeriod {
                period: UsagePeriod::Day("2026-11-08".into()),
                total_tokens: 25,
                details: TokenDetails::default(),
                estimated_value_micros: None,
                models: vec![],
            },
        ),
        (
            "opencode",
            None,
            AgentPeriod {
                period: UsagePeriod::Unplaced,
                total_tokens: 25,
                details: TokenDetails::default(),
                estimated_value_micros: None,
                models: vec![],
            },
        ),
    ] {
        let (_directory, workspace, mut store) = create_workspace();
        let id = source(&mut store, tool, "missing");
        grouped(&mut store, tool, &id, zone, &[period]);
        drop(store);
        let before = directory_bytes(&workspace);
        let error = preview(&workspace, NOW, &[]).unwrap_err();
        assert_eq!((error.code, error.exit), ("board_incomplete_data", 11));
        assert!(!error.message.contains(&id));
        assert!(!error.message.contains("/srv"));
        assert_eq!(directory_bytes(&workspace), before);
    }
}

#[test]
fn retained_multiple_folder_and_mixed_generation_history_refuse_atomically() {
    for case in ["retained", "multiple", "mixed"] {
        let (_directory, workspace, mut store) = create_workspace();
        let id = source(&mut store, "claude", "one");
        save(
            &mut store,
            std::slice::from_ref(&id),
            "UTC",
            &[row("claude-sonnet", 100)],
        );
        match case {
            "retained" => save(
                &mut store,
                std::slice::from_ref(&id),
                "UTC",
                &[row("claude-sonnet", 80)],
            ),
            "multiple" => {
                let other = source(&mut store, "claude", "copy");
                save(&mut store, &[other], "UTC", &[row("claude-sonnet", 100)]);
            }
            "mixed" => grouped(
                &mut store,
                "claude",
                &id,
                Some("UTC"),
                &[AgentPeriod {
                    period: UsagePeriod::Day("2026-11-08".into()),
                    total_tokens: 100,
                    details: TokenDetails::default(),
                    estimated_value_micros: None,
                    models: vec![ModelDetail {
                        model: "claude-sonnet".into(),
                        total_tokens: Some(100),
                        details: TokenDetails {
                            input_tokens: Some(100),
                            output_tokens: Some(0),
                            cache_read_tokens: Some(0),
                            cache_write_tokens: Some(0),
                            reasoning_tokens: None,
                        },
                        estimated_value_micros: None,
                    }],
                }],
            ),
            _ => unreachable!(),
        }
        drop(store);
        let error = preview(&workspace, NOW, &[]).unwrap_err();
        assert_eq!(
            (error.code, error.exit),
            ("board_incomplete_data", 11),
            "{case}"
        );
        assert!(!error.message.contains(&id));
    }
}

#[test]
fn service_count_identifier_row_and_body_limits_refuse_the_entire_preview() {
    for case in ["count", "estimate", "model", "rows", "body"] {
        let (_directory, workspace, mut store) = create_workspace();
        let id = source(&mut store, "claude", "limits");
        let rows = match case {
            "count" => vec![row("claude-sonnet", MAX_COUNT + 1)],
            "estimate" => {
                let mut value = row("claude-sonnet", 1);
                value.estimated_value_usd_micros = Some(MAX_COUNT + 1);
                vec![value]
            }
            "model" => vec![row("private@example.invalid", 1)],
            "rows" => (0..401).map(|n| row(&format!("model-{n}"), 1)).collect(),
            "body" => (0..400)
                .map(|n| row(&format!("model-{n:03}-{}", "a".repeat(54)), MAX_COUNT))
                .collect(),
            _ => unreachable!(),
        };
        save(&mut store, &[id], "UTC", &rows);
        drop(store);
        let error = preview(&workspace, NOW, &[]).unwrap_err();
        let expected = if matches!(case, "rows" | "body") {
            "board_too_large"
        } else {
            "board_invalid_data"
        };
        assert_eq!(error.code, expected, "{case}");
        assert!(!error.message.contains("private@example.invalid"));
    }
}

#[test]
fn pinned_day_zone_controls_selection_and_service_future_dates_refuse() {
    let (_directory, workspace, mut store) = create_workspace();
    let id = source(&mut store, "claude", "zone");
    save(
        &mut store,
        &[id],
        "America/Los_Angeles",
        &[row("claude-sonnet", MAX_COUNT)],
    );
    drop(store);
    let output = preview(&workspace, "2026-11-09T01:00:00Z", &["--days", "1"]).unwrap();
    let body: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(body["rows"][0]["date"], "2026-11-08");
    assert_eq!(body["rows"][0]["inputTokens"], MAX_COUNT);

    let (_directory, workspace, mut store) = create_workspace();
    let id = source(&mut store, "claude", "ahead");
    let mut value = row("claude-sonnet", 1);
    value.local_date = "2026-11-09".into();
    save(&mut store, &[id], "Asia/Kolkata", &[value]);
    drop(store);
    let error = preview(&workspace, "2026-11-08T19:00:00Z", &["--days", "1"]).unwrap_err();
    assert_eq!(error.code, "board_invalid_data");
}

#[test]
fn preview_options_require_an_explicit_v4_identity_before_any_workspace_access() {
    let env = Env {
        key: Box::new(|| panic!("invalid options must not read a credential store")),
        ..Env::for_tests()
    };
    for args in [
        vec!["board", "sync", "--dry-run"],
        vec![
            "board",
            "sync",
            "--dry-run",
            "--device-id",
            "00000000-0000-0000-0000-000000000000",
        ],
        vec![
            "board",
            "sync",
            "--dry-run",
            "--device-id",
            "F9168C5E-CEB2-4FAA-B6BF-329BF39FA1E4",
        ],
        vec![
            "board",
            "sync",
            "--dry-run",
            "--device-id",
            DEVICE,
            "--days",
            "0",
        ],
        vec![
            "board",
            "sync",
            "--dry-run",
            "--device-id",
            DEVICE,
            "--days",
            "367",
        ],
        vec![
            "board",
            "sync",
            "--dry-run",
            "--device-id",
            DEVICE,
            "--dry-run",
        ],
    ] {
        let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let result = crate::run(&args, &env, &mut std::io::empty(), &mut std::io::sink()).0;
        assert_eq!(result.unwrap_err().code, "usage");
    }
}

#[test]
fn unconfigured_online_commands_do_not_read_credentials_or_prompt() {
    let env = Env {
        key: Box::new(|| panic!("unconfigured board must stay inert")),
        ..Env::for_tests()
    };
    for action in ["join", "sync", "leave"] {
        let mut prompt = Vec::new();
        let args = [
            "board".into(),
            action.into(),
            "--yes".into(),
            "--json".into(),
        ];
        let error = crate::run(&args, &env, &mut std::io::empty(), &mut prompt)
            .0
            .unwrap_err();
        assert_eq!((error.code, error.exit), ("board_unconfigured", 14));
        assert!(prompt.is_empty());
    }
}

#[test]
fn recovery_flags_are_explicit_and_cannot_apply_to_other_commands() {
    let env = Env::for_tests();
    for args in [
        vec!["join", "--reauthorize"],
        vec!["forget", "--reauthorize"],
        vec!["leave", "--reauthorize", "--reauthorize"],
        vec!["forget", "--days", "1"],
    ] {
        let mut all = vec!["board".into()];
        all.extend(args.into_iter().map(str::to_owned));
        assert_eq!(
            crate::run(&all, &env, &mut std::io::empty(), &mut std::io::sink())
                .0
                .unwrap_err()
                .exit,
            2
        );
    }
    let args = ["board", "leave", "--reauthorize", "--yes", "--json"].map(str::to_owned);
    assert_eq!(
        crate::run(&args, &env, &mut std::io::empty(), &mut std::io::sink())
            .0
            .unwrap_err()
            .code,
        "board_unconfigured"
    );
}
