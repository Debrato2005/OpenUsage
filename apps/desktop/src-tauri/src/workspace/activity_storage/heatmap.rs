//! Read-only calendar projections of saved provider dates. Missing dates remain unknown.

use super::*;
use std::collections::BTreeMap;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CombinedHeatmapEligibility {
    status: &'static str,
    included_profile_ids: Vec<String>,
    exclusions: Vec<HeatmapExclusion>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HeatmapExclusion {
    profile_id: String,
    reason: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProfileHeatmap {
    profile_id: String,
    binding_id: String,
    collection_run_id: String,
    source_kind: &'static str,
    scope: &'static str,
    coverage: &'static str,
    observed_at_utc: String,
    mode: String,
    period_start: String,
    period_end: String,
    scale_maximum: Option<String>,
    cells: Vec<HeatmapCell>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HeatmapCell {
    start_date: String,
    end_date: String,
    tokens: Option<String>,
    level: u8,
    known_days: u16,
    expected_days: u16,
    observed_at_utc: Option<String>,
    freshness: ActivityFreshness,
}

impl Workspace {
    /// The current official collector supplies provisional identity and unknown scope only.
    /// Expose exclusions instead of treating descriptive emails as distinct verified identities.
    pub fn load_combined_heatmap(
        &self,
        ids: &[String],
    ) -> Result<CombinedHeatmapEligibility, CommandError> {
        if ids.is_empty() || ids.len() > 10 {
            return Err(CommandError::InvalidArguments);
        }
        let mut selected = std::collections::BTreeSet::new();
        for id in ids {
            validate_profile_id(id)?;
            if !selected.insert(id) {
                return Err(CommandError::InvalidArguments);
            }
        }
        let storage = self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        let profiles = storage.list_profiles().map_err(profile_error)?;
        let exclusions = ids
            .iter()
            .map(|id| {
                let profile = profiles
                    .iter()
                    .find(|profile| &profile.id == id)
                    .ok_or(CommandError::ProfileNotFound)?;
                if profile.archived {
                    return Err(CommandError::ProfileUnavailable);
                }
                let reason = if profile.connection_state != ProfileConnectionState::Connected {
                    "not_connected"
                } else if storage.current_activity(id).map_err(load_error)?.is_none() {
                    "activity_unavailable"
                } else {
                    "identity_unverified"
                };
                Ok(HeatmapExclusion {
                    profile_id: id.clone(),
                    reason,
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        Ok(CombinedHeatmapEligibility {
            status: "unavailable",
            included_profile_ids: Vec::new(),
            exclusions,
        })
    }

    /// Calendar rendering never starts a provider or changes saved readings.
    pub fn load_profile_heatmap(
        &self,
        id: &str,
        mode: &str,
        period_days: u16,
    ) -> Result<Option<ProfileHeatmap>, CommandError> {
        validate_selection(mode, period_days)?;
        let Some(saved) = self.load_profile_activity(id)? else {
            return Ok(None);
        };
        let today = restore::timestamp(SystemTime::now())
            .ok_or(CommandError::ProviderActivityLoadFailed)?;
        build(&saved, mode, period_days, &today[..10]).map(Some)
    }
}

fn validate_selection(mode: &str, period_days: u16) -> Result<(), CommandError> {
    if !matches!(mode, "daily" | "weekly" | "cumulative")
        || !matches!(period_days, 30 | 90 | 180 | 365)
    {
        return Err(CommandError::InvalidArguments);
    }
    Ok(())
}

fn build(
    saved: &SavedProfileActivity,
    mode: &str,
    period_days: u16,
    today: &str,
) -> Result<ProfileHeatmap, CommandError> {
    validate_selection(mode, period_days)?;
    let end = ordinal(today).ok_or(CommandError::ProviderActivityLoadFailed)?;
    let start = end
        .checked_sub(u32::from(period_days) - 1)
        .filter(|start| *start > 0)
        .ok_or(CommandError::ProviderActivityLoadFailed)?;
    let mut days = BTreeMap::new();
    for day in &saved.daily_usage_buckets {
        let date = ordinal(&day.date).ok_or(CommandError::ProviderActivityLoadFailed)?;
        let tokens = day
            .tokens
            .parse::<u128>()
            .ok()
            .filter(|tokens| tokens.to_string() == day.tokens)
            .ok_or(CommandError::ProviderActivityLoadFailed)?;
        if days.insert(date, (tokens, day)).is_some() {
            return Err(CommandError::ProviderActivityLoadFailed);
        }
    }
    let mut cells = Vec::with_capacity(usize::from(period_days));
    if mode == "weekly" {
        // 0001-01-01 was Monday. Keep full ISO week bounds even at period edges.
        let mut monday = start - (start - 1) % 7;
        while monday <= end {
            let sunday = monday + 6;
            cells.push(cell(
                &days,
                monday.max(start),
                sunday.min(end),
                monday,
                sunday,
                7,
            )?);
            monday += 7;
        }
    } else {
        for date in start..=end {
            let first = if mode == "cumulative" { start } else { date };
            cells.push(cell(
                &days,
                first,
                date,
                date,
                date,
                (date - first + 1) as u16,
            )?);
        }
    }
    let maximum = cells
        .iter()
        .filter_map(|cell| {
            cell.tokens
                .as_deref()
                .and_then(|value| value.parse::<u128>().ok())
        })
        .max();
    for cell in &mut cells {
        if let (Some(tokens), Some(maximum)) = (&cell.tokens, maximum) {
            let value = tokens
                .parse::<u128>()
                .map_err(|_| CommandError::ProviderActivityLoadFailed)?;
            cell.level = level(value, maximum);
        }
    }
    Ok(ProfileHeatmap {
        profile_id: saved.profile_id.clone(),
        binding_id: saved.binding_id.clone(),
        collection_run_id: saved.collection_run_id.clone(),
        source_kind: saved.source_kind,
        scope: saved.scope,
        coverage: "unknown",
        observed_at_utc: saved.observed_at_utc.clone(),
        mode: mode.into(),
        period_start: date_string(start),
        period_end: date_string(end),
        scale_maximum: maximum.map(|value| value.to_string()),
        cells,
    })
}

fn cell(
    days: &BTreeMap<u32, (u128, &SavedActivityDay)>,
    first: u32,
    last: u32,
    label_start: u32,
    label_end: u32,
    expected_days: u16,
) -> Result<HeatmapCell, CommandError> {
    let mut total = 0_u128;
    let mut known_days = 0;
    let mut observed: Option<&str> = None;
    let mut freshness = ActivityFreshness::Fresh;
    for (tokens, day) in days.range(first..=last).map(|(_, day)| day) {
        total = total
            .checked_add(*tokens)
            .ok_or(CommandError::ProviderActivityLoadFailed)?;
        known_days += 1;
        observed = Some(observed.map_or(day.observed_at_utc.as_str(), |previous| {
            previous.min(day.observed_at_utc.as_str())
        }));
        freshness = match (freshness, day.freshness) {
            (ActivityFreshness::Stale, _) | (_, ActivityFreshness::Stale) => {
                ActivityFreshness::Stale
            }
            (ActivityFreshness::Unknown, _) | (_, ActivityFreshness::Unknown) => {
                ActivityFreshness::Unknown
            }
            _ => ActivityFreshness::Fresh,
        };
    }
    Ok(HeatmapCell {
        start_date: date_string(label_start),
        end_date: date_string(label_end),
        tokens: (known_days > 0).then(|| total.to_string()),
        level: 0,
        known_days,
        expected_days,
        observed_at_utc: observed.map(String::from),
        freshness: if known_days == 0 {
            ActivityFreshness::Unknown
        } else {
            freshness
        },
    })
}

fn level(value: u128, maximum: u128) -> u8 {
    if value == 0 || maximum == 0 {
        return 0;
    }
    // ceil(value * 4 / maximum), without overflowing at u128::MAX.
    (1..=4)
        .find(|step| value <= maximum / 4 * step + maximum % 4 * step / 4)
        .unwrap_or(4) as u8
}

fn leap(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

fn month_days(year: u32, month: u32) -> u32 {
    match month {
        2 => {
            if leap(year) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn ordinal(date: &str) -> Option<u32> {
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
    {
        return None;
    }
    let year: u32 = date[..4].parse().ok()?;
    let month: u32 = date[5..7].parse().ok()?;
    let day: u32 = date[8..].parse().ok()?;
    if !(1..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || !(1..=month_days(year, month)).contains(&day)
    {
        return None;
    }
    let previous = year - 1;
    Some(
        previous * 365 + previous / 4 - previous / 100
            + previous / 400
            + (1..month).map(|month| month_days(year, month)).sum::<u32>()
            + day,
    )
}

fn date_string(ordinal: u32) -> String {
    let mut remaining = ordinal - 1;
    let mut year = remaining / 146_097 * 400 + 1;
    remaining %= 146_097;
    while remaining >= if leap(year) { 366 } else { 365 } {
        remaining -= if leap(year) { 366 } else { 365 };
        year += 1;
    }
    let mut month = 1;
    while remaining >= month_days(year, month) {
        remaining -= month_days(year, month);
        month += 1;
    }
    format!("{year:04}-{month:02}-{:02}", remaining + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(rows: &[(&str, &str)]) -> SavedProfileActivity {
        let daily_usage_buckets = rows
            .iter()
            .map(|(date, tokens)| SavedActivityDay {
                date: (*date).into(),
                tokens: (*tokens).into(),
                observed_at_utc: "2026-10-21T00:00:00.000Z".into(),
                freshness: ActivityFreshness::Unknown,
            })
            .collect();
        SavedProfileActivity {
            profile_id: "profile".into(),
            binding_id: "binding".into(),
            collection_run_id: "run".into(),
            provider_version: "test".into(),
            source_kind: "test_fixture",
            observed_at_utc: "2026-10-21T00:00:00.000Z".into(),
            identity_confidence: "provisional",
            verified_identity_key: None,
            monitoring: "stopped",
            accepted: true,
            coverage: "unknown",
            scope: "test-scope",
            freshness: ActivityFreshness::Unknown,
            summary: SavedActivitySummary {
                lifetime_tokens: Some("99999999".into()),
                peak_daily_tokens: None,
                longest_running_turn_sec: None,
                current_streak_days: None,
                longest_streak_days: None,
            },
            daily_state: "returned",
            daily_usage_buckets,
            daily_chart: SavedActivityChart {
                max_tokens: None,
                bars: vec![],
            },
            daily_truncated: false,
        }
    }

    #[test]
    fn daily_calendar_preserves_leap_dates_exact_counts_gaps_zero_and_input() {
        let input = saved(&[
            ("2024-02-28", "0"),
            ("2024-02-29", "9223372036854775807"),
            ("2024-03-02", "1"),
        ]);
        let before = serde_json::to_value(&input).unwrap();
        let result = build(&input, "daily", 90, "2024-03-02").unwrap();
        assert_eq!(result.cells.len(), 90);
        assert_eq!(result.scale_maximum.as_deref(), Some("9223372036854775807"));
        let tail = &result.cells[86..];
        assert_eq!(
            tail.iter()
                .map(|cell| cell.tokens.as_deref())
                .collect::<Vec<_>>(),
            [Some("0"), Some("9223372036854775807"), None, Some("1")]
        );
        assert_eq!(
            tail.iter().map(|cell| cell.level).collect::<Vec<_>>(),
            [0, 4, 0, 1]
        );
        assert_eq!(tail[1].start_date, "2024-02-29");
        assert_eq!(tail[2].known_days, 0);
        assert!(tail[2].observed_at_utc.is_none());
        assert_eq!(result.coverage, "unknown");
        assert_eq!(serde_json::to_value(&input).unwrap(), before);
    }

    #[test]
    fn weekly_uses_monday_sunday_across_year_and_marks_partial_weeks() {
        let rows = saved(&[
            ("2020-12-28", "9223372036854775807"),
            ("2021-01-01", "9223372036854775807"),
            ("2021-01-03", "0"),
            ("2021-01-04", "1"),
        ]);
        let result = build(&rows, "weekly", 90, "2021-01-04").unwrap();
        let last = &result.cells[result.cells.len() - 2..];
        assert_eq!(
            (&last[0].start_date[..], &last[0].end_date[..]),
            ("2020-12-28", "2021-01-03")
        );
        assert_eq!(last[0].tokens.as_deref(), Some("18446744073709551614"));
        assert_eq!((last[0].known_days, last[0].expected_days), (3, 7));
        assert_eq!(
            (&last[1].start_date[..], &last[1].end_date[..]),
            ("2021-01-04", "2021-01-10")
        );
        assert_eq!((last[1].known_days, last[1].expected_days), (1, 7));
    }

    #[test]
    fn cumulative_sums_only_selected_available_days_without_claiming_lifetime_coverage() {
        let rows = saved(&[
            ("2025-01-01", "999"),
            ("2026-10-18", "0"),
            ("2026-10-19", "9007199254740993"),
            ("2026-10-21", "2"),
            ("2026-10-22", "999"),
        ]);
        let result = build(&rows, "cumulative", 90, "2026-10-21").unwrap();
        let tail = &result.cells[86..];
        assert_eq!(
            tail.iter()
                .map(|cell| cell.tokens.as_deref())
                .collect::<Vec<_>>(),
            [
                Some("0"),
                Some("9007199254740993"),
                Some("9007199254740993"),
                Some("9007199254740995")
            ]
        );
        assert_eq!((tail[2].known_days, tail[2].expected_days), (2, 89));
        assert_eq!((tail[3].known_days, tail[3].expected_days), (3, 90));
        assert_eq!(result.scale_maximum.as_deref(), Some("9007199254740995"));
        assert_eq!(result.coverage, "unknown");
        assert!(result.cells[0].tokens.is_none());
    }

    #[test]
    fn strict_bounds_calendar_and_checked_arithmetic_refuse_invalid_inputs() {
        for (mode, days) in [("monthly", 90), ("Daily", 90), ("daily", 0), ("daily", 366)] {
            assert!(matches!(
                build(&saved(&[]), mode, days, "2026-10-21"),
                Err(CommandError::InvalidArguments)
            ));
        }
        for date in [
            "2023-02-29",
            "2024-02-30",
            "0000-01-01",
            "2024-1-01",
            "２０２４-01-01",
        ] {
            assert!(build(&saved(&[]), "daily", 90, date).is_err());
        }
        for token in [
            "-1",
            "01",
            "+1",
            "1.0",
            "340282366920938463463374607431768211456",
        ] {
            assert!(build(&saved(&[("2026-10-21", token)]), "daily", 90, "2026-10-21").is_err());
        }
        let max = u128::MAX.to_string();
        assert_eq!(level(u128::MAX, u128::MAX), 4);
        assert_eq!(level(1, u128::MAX), 1);
        assert!(
            build(
                &saved(&[("2026-10-20", &max), ("2026-10-21", "1")]),
                "cumulative",
                90,
                "2026-10-21"
            )
            .is_err()
        );
        for days in [30, 90, 180, 365] {
            assert_eq!(
                build(&saved(&[]), "daily", days, "2026-10-21")
                    .unwrap()
                    .cells
                    .len(),
                usize::from(days)
            );
        }
    }

    #[test]
    fn saved_heatmap_requires_unlock_and_valid_profile_without_provider_setup() {
        let temporary = tempfile::tempdir().unwrap();
        let workspace = Workspace::new(temporary.path().join("workspace")).unwrap();
        let id = "00000000-0000-4000-8000-000000000001";
        assert!(matches!(
            workspace.load_profile_heatmap(id, "daily", 90),
            Err(CommandError::WorkspaceLocked)
        ));
        assert!(matches!(
            workspace.load_profile_heatmap("invalid", "daily", 90),
            Err(CommandError::InvalidProfileId)
        ));
        assert!(matches!(
            workspace.load_profile_heatmap(id, "other", 90),
            Err(CommandError::InvalidArguments)
        ));
        assert!(
            !temporary
                .path()
                .join("workspace")
                .join(DATABASE_FILE)
                .exists()
        );
    }

    #[test]
    fn combined_selection_validates_profiles_and_reports_exclusions_without_writes() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("workspace");
        let mut workspace = Workspace::new(directory.clone()).unwrap();
        let ids: Vec<String> = (1..=3)
            .map(|index| format!("00000000-0000-4000-8000-{index:012}"))
            .collect();
        assert!(matches!(
            workspace.load_combined_heatmap(&ids),
            Err(CommandError::WorkspaceLocked)
        ));
        assert!(matches!(
            workspace.load_combined_heatmap(&[]),
            Err(CommandError::InvalidArguments)
        ));
        assert!(matches!(
            workspace.load_combined_heatmap(&vec![ids[0].clone(); 11]),
            Err(CommandError::InvalidArguments)
        ));
        assert!(matches!(
            workspace.load_combined_heatmap(&[ids[0].clone(), ids[0].clone()]),
            Err(CommandError::InvalidArguments)
        ));
        assert!(matches!(
            workspace.load_combined_heatmap(&["invalid".into()]),
            Err(CommandError::InvalidProfileId)
        ));
        let mut storage = Storage::create(&directory.join(DATABASE_FILE), &[17; 32]).unwrap();
        for id in &ids {
            storage.create_profile(id, "Synthetic account", id).unwrap();
        }
        for id in &ids[1..] {
            storage
                .record_profile_connection(id, id, ProfileConnectionState::Connected)
                .unwrap();
        }
        let observed = "2026-10-21T00:00:00.000Z".to_string();
        let evidence = ProfileIdentityEvidence {
            email: "synthetic@example.invalid".into(),
            plan: Some("plus".into()),
        };
        let binding_id = "10000000-0000-4000-8000-000000000001".to_string();
        storage
            .accept_activity_with_binding(
                ActivityReading {
                    run_id: "20000000-0000-4000-8000-000000000001".into(),
                    binding: openusage_storage::ProfileBinding {
                        profile_id: ids[2].clone(),
                        home_reference: ids[2].clone(),
                        binding_id: binding_id.clone(),
                    },
                    identity_evidence: evidence.clone(),
                    provider_version: "0.163.0".into(),
                    source_kind: ActivitySourceKind::TestFixture,
                    started_at_utc: observed.clone(),
                    observed_at_utc: observed.clone(),
                    completed_at_utc: observed.clone(),
                    summary: ActivitySummaryValues::default(),
                    daily_usage: Some(vec![ActivityDayValue {
                        provider_date: "2026-10-21".into(),
                        tokens: "7".into(),
                    }]),
                },
                Some(ProfileBindingInput {
                    profile_id: ids[2].clone(),
                    expected_home_reference: ids[2].clone(),
                    expected_previous_binding_id: None,
                    new_binding_id: binding_id,
                    evidence,
                    observed_at_utc: observed,
                }),
            )
            .unwrap();
        workspace.storage = Some(storage);
        let bytes = || {
            [DATABASE_FILE, "openusage.db-wal"].map(|name| fs::read(directory.join(name)).unwrap())
        };
        let before = bytes();
        let result = workspace.load_combined_heatmap(&ids).unwrap();
        assert_eq!(result.status, "unavailable");
        assert!(result.included_profile_ids.is_empty());
        assert_eq!(
            result
                .exclusions
                .iter()
                .map(|item| item.reason)
                .collect::<Vec<_>>(),
            [
                "not_connected",
                "activity_unavailable",
                "identity_unverified"
            ]
        );
        assert_eq!(bytes(), before);
        assert!(!directory.join("profiles").exists());
        assert!(matches!(
            workspace.load_combined_heatmap(&["00000000-0000-4000-8000-000000000099".into()]),
            Err(CommandError::ProfileNotFound)
        ));
        workspace
            .storage
            .as_mut()
            .unwrap()
            .set_profile_archived(&ids[0], true)
            .unwrap();
        assert!(matches!(
            workspace.load_combined_heatmap(&ids),
            Err(CommandError::ProfileUnavailable)
        ));
    }
}
