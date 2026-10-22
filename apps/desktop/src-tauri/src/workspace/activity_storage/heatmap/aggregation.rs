//! One read-only projection for activity cells, observed subtotals and contribution shares.
use super::*;
use std::collections::BTreeSet;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActivityDashboard {
    view: String,
    status: &'static str,
    mode: String,
    period_start: String,
    period_end: String,
    source_kind: &'static str,
    observed_at_utc: Option<String>,
    scope_verified: bool,
    total_tokens: Option<String>,
    known_contributions: u16,
    expected_contributions: u16,
    scale_maximum: Option<String>,
    members: Vec<Member>,
    exclusions: Vec<Exclusion>,
    cells: Vec<DashboardCell>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Member {
    profile_id: String,
    label: String,
    tokens: Option<String>,
    share_basis_points: Option<u16>,
    known_days: u16,
    source_kind: &'static str,
    scope: &'static str,
    observed_at_utc: String,
    binding_id: String,
    collection_run_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Exclusion {
    profile_id: String,
    reason: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardCell {
    #[serde(flatten)]
    cell: HeatmapCell,
    known_contributions: u16,
    expected_contributions: u16,
}

struct Input {
    label: String,
    saved: SavedProfileActivity,
    // Only an approved provider adapter can establish these semantics. The current
    // managed adapter supplies None; an email or a matching wire shape is not proof.
    verified_semantics: Option<(String, String)>,
}

impl Workspace {
    pub fn load_activity_dashboard(
        &self,
        ids: &[String],
        view: &str,
        mode: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<ActivityDashboard, CommandError> {
        let (start, end) = selection(ids, view, mode, start_date, end_date)?;
        let storage = self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        let profiles = storage.list_profiles().map_err(profile_error)?;
        let mut inputs = Vec::new();
        let mut exclusions = Vec::new();
        for id in ids {
            let profile = profiles
                .iter()
                .find(|p| &p.id == id)
                .ok_or(CommandError::ProfileNotFound)?;
            let reason = if profile.archived {
                Some("archived")
            } else if view != "individual"
                && profile.connection_state != ProfileConnectionState::Connected
            {
                Some("not_connected")
            } else {
                None
            };
            if let Some(reason) = reason {
                exclusions.push(Exclusion {
                    profile_id: id.clone(),
                    reason,
                });
                continue;
            }
            match self.load_profile_activity(id)? {
                Some(saved) if !saved.daily_truncated => inputs.push(Input {
                    label: profile.label.clone(),
                    saved,
                    verified_semantics: None,
                }),
                saved => exclusions.push(Exclusion {
                    profile_id: id.clone(),
                    reason: if saved.is_some() {
                        "truncated_history"
                    } else {
                        "activity_unavailable"
                    },
                }),
            }
        }
        aggregate(inputs, exclusions, view, mode, start, end)
    }
}

fn selection(
    ids: &[String],
    view: &str,
    mode: &str,
    start: &str,
    end: &str,
) -> Result<(u32, u32), CommandError> {
    if ids.is_empty()
        || ids.len() > 10
        || !matches!(view, "individual" | "verified" | "profile_sum")
        || (view == "individual" && ids.len() != 1)
        || !matches!(mode, "daily" | "weekly" | "cumulative")
    {
        return Err(CommandError::InvalidArguments);
    }
    let mut seen = BTreeSet::new();
    for id in ids {
        validate_profile_id(id)?;
        if !seen.insert(id) {
            return Err(CommandError::InvalidArguments);
        }
    }
    let (Some(start), Some(end)) = (ordinal(start), ordinal(end)) else {
        return Err(CommandError::InvalidArguments);
    };
    if start > end || end - start >= 365 {
        return Err(CommandError::InvalidArguments);
    }
    Ok((start, end))
}

fn add(a: u128, b: u128) -> Result<u128, CommandError> {
    a.checked_add(b)
        .ok_or(CommandError::ProviderActivityLoadFailed)
}

fn aggregate(
    mut inputs: Vec<Input>,
    mut exclusions: Vec<Exclusion>,
    view: &str,
    mode: &str,
    start: u32,
    end: u32,
) -> Result<ActivityDashboard, CommandError> {
    // Newest accepted source wins for a known duplicate. UUID resolves equal timestamps.
    inputs.sort_by(|a, b| {
        b.saved
            .observed_at_utc
            .cmp(&a.saved.observed_at_utc)
            .then(a.saved.profile_id.cmp(&b.saved.profile_id))
    });
    let mut identities = BTreeSet::new();
    let mut semantics = None;
    let mut wire_contract = None;
    let mut included = Vec::new();
    for input in inputs {
        let saved = &input.saved;
        let reason = if view == "verified"
            && (saved.identity_confidence != "verified" || saved.verified_identity_key.is_none())
        {
            Some("identity_unverified")
        } else if view == "verified" && input.verified_semantics.is_none() {
            Some("scope_unverified")
        } else if wire_contract.as_ref().is_some_and(|v| {
            v != &(
                saved.provider_version.clone(),
                saved.source_kind,
                saved.scope,
            )
        }) || semantics
            .as_ref()
            .is_some_and(|v| v != &input.verified_semantics)
        {
            Some("incompatible_scope")
        } else if saved
            .verified_identity_key
            .as_ref()
            .is_some_and(|key| identities.contains(key))
        {
            Some("duplicate_identity")
        } else {
            None
        };
        if let Some(reason) = reason {
            exclusions.push(Exclusion {
                profile_id: saved.profile_id.clone(),
                reason,
            });
            continue;
        }
        if let Some(key) = &saved.verified_identity_key {
            identities.insert(key.clone());
        }
        wire_contract = Some((
            saved.provider_version.clone(),
            saved.source_kind,
            saved.scope,
        ));
        semantics = Some(input.verified_semantics.clone());
        included.push(input);
    }
    included.sort_by(|a, b| a.saved.profile_id.cmp(&b.saved.profile_id));
    let scope_verified =
        !included.is_empty() && included.iter().all(|i| i.verified_semantics.is_some());
    let count = included.len() as u16;
    let expected_contributions = (end - start + 1) as u16 * count;
    let observed_at_utc = included
        .iter()
        .map(|i| i.saved.observed_at_utc.clone())
        .min();
    let source_kind = included
        .first()
        .map_or("unverified", |i| i.saved.source_kind);
    let mut days: BTreeMap<u32, (u128, u16, String, ActivityFreshness)> = BTreeMap::new();
    let mut total = None;
    let mut members = Vec::new();
    for input in included {
        let saved = input.saved;
        let mut tokens = None;
        let mut dates = BTreeSet::new();
        for day in saved.daily_usage_buckets {
            let date = ordinal(&day.date).ok_or(CommandError::ProviderActivityLoadFailed)?;
            if !dates.insert(date) {
                return Err(CommandError::ProviderActivityLoadFailed);
            }
            if !(start..=end).contains(&date) {
                continue;
            }
            let value = day
                .tokens
                .parse::<u128>()
                .ok()
                .filter(|v| v.to_string() == day.tokens)
                .ok_or(CommandError::ProviderActivityLoadFailed)?;
            tokens = Some(add(tokens.unwrap_or(0), value)?);
            let entry = days.entry(date).or_insert((
                0,
                0,
                day.observed_at_utc.clone(),
                ActivityFreshness::Fresh,
            ));
            entry.0 = add(entry.0, value)?;
            entry.1 += 1;
            entry.2 = entry.2.clone().min(day.observed_at_utc);
            entry.3 = match (entry.3, day.freshness) {
                (ActivityFreshness::Stale, _) | (_, ActivityFreshness::Stale) => {
                    ActivityFreshness::Stale
                }
                (ActivityFreshness::Unknown, _) | (_, ActivityFreshness::Unknown) => {
                    ActivityFreshness::Unknown
                }
                _ => ActivityFreshness::Fresh,
            };
        }
        if let Some(value) = tokens {
            total = Some(add(total.unwrap_or(0), value)?);
        }
        members.push(Member {
            profile_id: saved.profile_id,
            label: input.label,
            tokens: tokens.map(|v| v.to_string()),
            share_basis_points: None,
            known_days: days_for_member(&dates, start, end),
            source_kind: saved.source_kind,
            scope: saved.scope,
            observed_at_utc: saved.observed_at_utc,
            binding_id: saved.binding_id,
            collection_run_id: saved.collection_run_id,
        });
    }
    shares(&mut members, total)?;
    let rows: Vec<_> = days
        .iter()
        .map(
            |(date, (tokens, _, observed, freshness))| SavedActivityDay {
                date: date_string(*date),
                tokens: tokens.to_string(),
                observed_at_utc: observed.clone(),
                freshness: *freshness,
            },
        )
        .collect();
    let known_contributions = days.values().map(|d| d.1).sum();
    let (cells, maximum) = if count == 0 {
        (Vec::new(), None)
    } else {
        project(&rows, mode, start, end)?
    };
    let cells = cells
        .into_iter()
        .map(|cell| {
            let first = if mode == "cumulative" {
                start
            } else {
                ordinal(&cell.start_date).expect("Projected date")
            };
            let last = ordinal(&cell.end_date).expect("Projected date");
            let known = days
                .range(first.max(start)..=last.min(end))
                .map(|(_, day)| day.1)
                .sum();
            DashboardCell {
                expected_contributions: cell.expected_days * count,
                known_contributions: known,
                cell,
            }
        })
        .collect();
    Ok(ActivityDashboard {
        view: view.into(),
        status: if count > 0 {
            "available"
        } else {
            "unavailable"
        },
        mode: mode.into(),
        period_start: date_string(start),
        period_end: date_string(end),
        source_kind,
        observed_at_utc,
        scope_verified,
        total_tokens: total.map(|v| v.to_string()),
        known_contributions,
        expected_contributions,
        scale_maximum: maximum.map(|v| v.to_string()),
        members,
        exclusions,
        cells,
    })
}

fn days_for_member(dates: &BTreeSet<u32>, start: u32, end: u32) -> u16 {
    dates.range(start..=end).count() as u16
}

fn shares(members: &mut [Member], total: Option<u128>) -> Result<(), CommandError> {
    let Some(total) = total.filter(|v| *v > 0) else {
        return Ok(());
    };
    let mut remainders = Vec::new();
    let mut assigned = 0_u16;
    for (index, member) in members.iter_mut().enumerate() {
        if let Some(tokens) = &member.tokens {
            let numerator = tokens
                .parse::<u128>()
                .map_err(|_| CommandError::ProviderActivityLoadFailed)?
                .checked_mul(10000)
                .ok_or(CommandError::ProviderActivityLoadFailed)?;
            let share = (numerator / total) as u16;
            member.share_basis_points = Some(share);
            assigned += share;
            remainders.push((index, numerator % total));
        }
    }
    remainders.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (index, _) in remainders.into_iter().take(usize::from(10000 - assigned)) {
        *members[index]
            .share_basis_points
            .as_mut()
            .expect("Known subtotal") += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(id: u8, rows: &[(&str, &str)], identity: Option<&str>, proof: bool) -> Input {
        let mut saved = super::super::tests::saved(rows);
        saved.profile_id = format!("00000000-0000-4000-8000-{id:012}");
        saved.verified_identity_key = identity.map(String::from);
        if identity.is_some() {
            saved.identity_confidence = "verified";
        }
        Input {
            label: format!("Fixture {id}"),
            saved,
            verified_semantics: proof
                .then(|| ("synthetic tokens".into(), "synthetic UTC dates".into())),
        }
    }
    fn run(inputs: Vec<Input>, view: &str, mode: &str) -> ActivityDashboard {
        aggregate(
            inputs,
            Vec::new(),
            view,
            mode,
            ordinal("2026-10-19").unwrap(),
            ordinal("2026-10-21").unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn exact_profile_sums_keep_missing_zero_and_one_denominator_for_shares() {
        for mode in ["daily", "weekly", "cumulative"] {
            let result = run(
                vec![
                    input(
                        1,
                        &[("2026-10-19", "9007199254740993"), ("2026-10-20", "0")],
                        None,
                        false,
                    ),
                    input(2, &[("2026-10-19", "7")], None, false),
                ],
                "profile_sum",
                mode,
            );
            assert_eq!(result.total_tokens.as_deref(), Some("9007199254741000"));
            assert_eq!(result.known_contributions, 3);
            assert_eq!(result.expected_contributions, 6);
            assert!(!result.scope_verified);
            assert_eq!(
                result
                    .members
                    .iter()
                    .map(|m| m.share_basis_points.unwrap())
                    .sum::<u16>(),
                10000
            );
            if mode == "daily" {
                assert_eq!(
                    result.cells[0].cell.tokens.as_deref(),
                    Some("9007199254741000")
                );
                assert_eq!(result.cells[1].cell.tokens.as_deref(), Some("0"));
                assert_eq!(result.cells[1].known_contributions, 1);
                assert_eq!(result.cells[1].expected_contributions, 2);
                assert_eq!(result.cells[2].cell.tokens, None);
            } else if mode == "cumulative" {
                assert_eq!(
                    result.cells[2].cell.tokens.as_deref(),
                    Some("9007199254741000")
                );
                assert_eq!(result.cells[2].known_contributions, 3);
                assert_eq!(result.cells[2].expected_contributions, 6);
            } else {
                assert_eq!(result.cells[0].known_contributions, 3);
                assert_eq!(result.cells[0].expected_contributions, 14);
            }
        }
    }
    #[test]
    fn known_duplicates_use_newest_compatible_source_and_verified_requires_both_proofs() {
        let mut newest = input(2, &[("2026-10-19", "7")], Some("synthetic-stable-id"), true);
        newest.saved.observed_at_utc = "2026-10-22T00:00:00.000Z".into();
        let result = run(
            vec![
                input(
                    1,
                    &[("2026-10-19", "999")],
                    Some("synthetic-stable-id"),
                    true,
                ),
                newest,
                input(3, &[("2026-10-19", "3")], Some("synthetic-other-id"), true),
            ],
            "verified",
            "daily",
        );
        assert_eq!(result.total_tokens.as_deref(), Some("10"));
        assert!(result.scope_verified);
        assert_eq!(result.exclusions[0].reason, "duplicate_identity");
        assert_eq!(result.members[0].share_basis_points, Some(7000));
        let blocked = run(
            vec![
                input(1, &[("2026-10-19", "7")], None, false),
                input(2, &[("2026-10-19", "7")], Some("id"), false),
            ],
            "verified",
            "daily",
        );
        assert_eq!(blocked.status, "unavailable");
        assert_eq!(blocked.exclusions[0].reason, "identity_unverified");
        assert_eq!(blocked.exclusions[1].reason, "scope_unverified");
    }
    #[test]
    fn incompatible_contracts_zero_denominators_and_invalid_selections_fail_safely() {
        let mut other = input(2, &[("2026-10-19", "100")], None, false);
        other.saved.provider_version = "incompatible-version".into();
        let result = run(
            vec![input(1, &[("2026-10-19", "0")], None, false), other],
            "profile_sum",
            "daily",
        );
        assert_eq!(result.total_tokens.as_deref(), Some("0"));
        assert_eq!(result.members[0].share_basis_points, None);
        assert_eq!(result.exclusions[0].reason, "incompatible_scope");
        let id = "00000000-0000-4000-8000-000000000001".to_string();
        for ids in [vec![], vec![id.clone(), id.clone()], vec![id.clone(); 11]] {
            assert!(selection(&ids, "profile_sum", "daily", "2026-10-19", "2026-10-21").is_err());
        }
        for (start, end) in [
            ("2026-02-30", "2026-03-01"),
            ("2026-10-21", "2026-10-19"),
            ("2025-01-01", "2026-01-01"),
        ] {
            assert!(
                selection(
                    std::slice::from_ref(&id),
                    "profile_sum",
                    "daily",
                    start,
                    end
                )
                .is_err()
            );
        }
    }
}
