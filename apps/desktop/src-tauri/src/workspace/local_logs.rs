//! Local log sources and scans in the encrypted store (SRS Section 5C).
use super::*;
use crate::local_logs::ScanGroup;
use openusage_locallogs::{self as locallogs, CCUSAGE_VERSION, HourRow, SessionRow, UsageRow};
use openusage_storage::{
    LocalHourScan, LocalSessionScan, LocalUsageDay, LocalUsageDifference, LocalUsageHour,
    LocalUsageRow, LocalUsageScan, LocalUsageSession, LogConsent, LogSource, SavedLocalHour,
    SavedLocalSession,
};
use std::collections::BTreeMap;
use std::path::Path;

pub(crate) struct LocalUsageRange {
    pub days: Vec<LocalUsageDay>,
    pub differences: Vec<LocalUsageDifference>,
    pub sources: Vec<LogSource>,
    /// Sessions that ended in the range, newest first (SC-021).
    pub sessions: Vec<SavedLocalSession>,
    /// Hours that start in the range, in time order (SC-021).
    pub hours: Vec<SavedLocalHour>,
    /// The UTC instants of the range start and of the day after it ends.
    pub window: Option<(String, String)>,
}

impl Workspace {
    fn local_logs_storage(&mut self) -> Result<&mut Storage, CommandError> {
        self.storage.as_mut().ok_or(CommandError::WorkspaceLocked)
    }

    pub(crate) fn log_sources(&mut self) -> Result<Vec<LogSource>, CommandError> {
        self.local_logs_storage()?
            .log_sources()
            .map_err(|_| CommandError::StorageUnavailable)
    }

    /// Grants the detected folders of the chosen tools and withdraws every other source (OU-LOG-002).
    pub(crate) fn set_local_log_consent(
        &mut self,
        detected: &[(&str, &str)],
        tools: &[String],
        now_utc: &str,
    ) -> Result<(), CommandError> {
        let chosen = |tool: &str| tools.iter().any(|t| t == tool);
        let storage = self.local_logs_storage()?;
        let saved = storage
            .log_sources()
            .map_err(|_| CommandError::StorageUnavailable)?;
        let unchosen = saved
            .iter()
            .filter(|s| s.consent == LogConsent::Granted && !chosen(&s.tool))
            .map(|s| (s.tool.as_str(), s.folder.as_str()));
        for (tool, folder) in detected.iter().copied().chain(unchosen) {
            let consent = if chosen(tool) {
                LogConsent::Granted
            } else {
                LogConsent::Withdrawn
            };
            storage
                .set_log_source(tool, folder, consent, now_utc)
                .map_err(|_| CommandError::StorageUnavailable)?;
        }
        Ok(())
    }

    /// Additional tools use one scope. Legacy consumers keep their source zones until L-1f10.
    pub(crate) fn local_log_scan_plan(&mut self) -> Result<Vec<ScanGroup>, CommandError> {
        let storage = self.local_logs_storage()?;
        let zone = storage
            .time_zone()
            .map_err(|_| CommandError::StorageUnavailable)?;
        let sources = storage
            .log_sources()
            .map_err(|_| CommandError::StorageUnavailable)?;
        scan_groups(
            storage,
            sources
                .into_iter()
                .filter(|s| locallogs::tool(&s.tool).is_some_and(|t| t.enabled))
                .collect(),
            &zone,
        )
    }

    pub(crate) fn save_grouped_log_reports(
        &mut self,
        group: &ScanGroup,
        scanned_at: &str,
        report: &locallogs::GroupedScanResult,
    ) -> Result<(), CommandError> {
        let storage = self.local_logs_storage()?;
        validate_sources(storage, &group.sources, true)?;
        let ids: Vec<_> = group.sources.iter().map(|s| s.id.clone()).collect();
        storage
            .save_grouped_local_scan(&openusage_storage::GroupedLocalScan {
                tool: &group.sources[0].tool,
                source_ids: &ids,
                time_zone: group.zone.as_deref(),
                scanner_version: CCUSAGE_VERSION,
                observed_at_utc: scanned_at,
                periods: &report.periods,
                sessions: Some(&report.sessions),
            })
            .map(|_| ())
            .map_err(|_| CommandError::StorageUnavailable)
    }

    pub(crate) fn check_local_scan_sources(
        &mut self,
        sources: &[LogSource],
        grouped: bool,
    ) -> Result<(), CommandError> {
        validate_sources(self.local_logs_storage()?, sources, grouped)
    }

    /// Save days, sessions and required hours together after the final consent check.
    pub(crate) fn save_complete_log_reports(
        &mut self,
        source: &LogSource,
        zone: &str,
        scanned_at_utc: &str,
        rows: &[UsageRow],
        session_rows: &[SessionRow],
        hour_rows: Option<&[HourRow]>,
    ) -> Result<(), CommandError> {
        let rows: Vec<LocalUsageRow> = rows
            .iter()
            .map(|row| LocalUsageRow {
                local_date: row.date.clone(),
                tool: row.tool.into(),
                model: row.model.clone(),
                input_tokens: row.input_tokens,
                output_tokens: row.output_tokens,
                cache_write_tokens: row.cache_write_tokens,
                cache_read_tokens: row.cache_read_tokens,
                total_tokens: row.total_tokens,
                estimated_value_usd_micros: row.estimated_value_micros,
            })
            .collect();
        let sessions: Vec<LocalUsageSession> = session_rows
            .iter()
            .map(|row| LocalUsageSession {
                tool: row.tool.into(),
                session_key: row.session_key.clone(),
                project_name: row.project.clone(),
                start_utc: row.start_utc.clone(),
                end_utc: row.end_utc.clone(),
                input_tokens: row.input_tokens,
                output_tokens: row.output_tokens,
                cache_write_tokens: row.cache_write_tokens,
                cache_read_tokens: row.cache_read_tokens,
                reasoning_tokens: row.reasoning_tokens,
                total_tokens: row.total_tokens,
                estimated_value_usd_micros: row.estimated_value_micros,
            })
            .collect();
        let hours: Vec<LocalUsageHour> = hour_rows
            .unwrap_or_default()
            .iter()
            .map(|row| LocalUsageHour {
                tool: row.tool.into(),
                hour_start_utc: row.start_utc.clone(),
                input_tokens: row.input_tokens,
                output_tokens: row.output_tokens,
                cache_write_tokens: row.cache_write_tokens,
                cache_read_tokens: row.cache_read_tokens,
                total_tokens: row.total_tokens,
            })
            .collect();
        let storage = self.local_logs_storage()?;
        validate_sources(storage, std::slice::from_ref(source), false)?;
        storage
            .save_complete_local_scan(
                &LocalUsageScan {
                    log_source_ids: std::slice::from_ref(&source.id),
                    scanner_version: CCUSAGE_VERSION,
                    time_zone: zone,
                    since: None,
                    scanned_at_utc,
                    rows: &rows,
                },
                &LocalSessionScan {
                    log_source_id: &source.id,
                    scanner_version: CCUSAGE_VERSION,
                    scanned_at_utc,
                    sessions: &sessions,
                },
                hour_rows
                    .map(|_| LocalHourScan {
                        log_source_id: &source.id,
                        scanner_version: CCUSAGE_VERSION,
                        scanned_at_utc,
                        hours: &hours,
                    })
                    .as_ref(),
            )
            .map_err(|_| CommandError::StorageUnavailable)
    }

    /// The zone of the saved days: the first pinned source zone, else the display zone (OU-LOG-010).
    pub(crate) fn local_usage_zone(&mut self) -> Result<String, CommandError> {
        self.local_logs_storage()?
            .local_usage_zone()
            .map_err(|_| CommandError::StorageUnavailable)
    }

    pub(crate) fn local_usage_read(
        &mut self,
        since: &str,
        until: &str,
    ) -> Result<openusage_storage::LocalUsageRead, CommandError> {
        self.local_logs_storage()?
            .local_usage_read(since, until)
            .map_err(|_| CommandError::StorageUnavailable)
    }

    pub(crate) fn local_usage_scopes(
        &mut self,
    ) -> Result<Vec<openusage_storage::LocalToolScope>, CommandError> {
        let storage = self.local_logs_storage()?;
        locallogs::tools()
            .iter()
            .filter_map(|tool| match storage.local_tool_scope(&tool.id) {
                Ok(Some(scope)) => Some(Ok(scope)),
                Ok(None) => None,
                Err(_) => Some(Err(CommandError::StorageUnavailable)),
            })
            .collect()
    }

    /// Saved days and differences from `since` to `until`, and the sources that own them. With a
    /// UTC window, also the sessions that ended in it and the hours that start in it. Share images
    /// ask for neither (OU-LOG-004).
    pub(crate) fn local_usage_range(
        &mut self,
        (since, until): (&str, &str),
        sessions: Option<(&str, &str)>,
    ) -> Result<LocalUsageRange, CommandError> {
        let storage = self.local_logs_storage()?;
        let unavailable = |_| CommandError::StorageUnavailable;
        Ok(LocalUsageRange {
            days: storage
                .local_usage_days(since, until)
                .map_err(unavailable)?,
            differences: storage
                .local_usage_differences(since, until)
                .map_err(unavailable)?,
            sources: storage.log_sources().map_err(unavailable)?,
            sessions: match sessions {
                Some((since_utc, until_utc)) => storage
                    .local_usage_sessions(since_utc, until_utc)
                    .map_err(unavailable)?,
                None => Vec::new(),
            },
            hours: match sessions {
                Some((since_utc, until_utc)) => storage
                    .local_usage_hours(since_utc, until_utc)
                    .map_err(unavailable)?,
                None => Vec::new(),
            },
            window: sessions.map(|(since_utc, until_utc)| (since_utc.into(), until_utc.into())),
        })
    }

    #[cfg(test)]
    pub(crate) fn local_logs_days_for_test(&mut self) -> Vec<openusage_storage::LocalUsageDay> {
        self.local_logs_storage()
            .unwrap()
            .local_usage_days("2000-01-01", "2099-12-31")
            .unwrap()
    }
}

// Run under the workspace lock immediately before the storage transaction.
fn validate_sources(
    storage: &Storage,
    selected: &[LogSource],
    grouped: bool,
) -> Result<(), CommandError> {
    let first = selected.first().ok_or(CommandError::InvalidArguments)?;
    let current = storage
        .log_sources()
        .map_err(|_| CommandError::StorageUnavailable)?;
    if selected.iter().any(|source| {
        source.tool != first.tool
            || !locallogs::valid_resolved_folder(Path::new(&source.folder))
            || !current.iter().any(|s| {
                s.id == source.id
                    && s.tool == source.tool
                    && s.folder == source.folder
                    && s.consent == LogConsent::Granted
            })
    }) || (grouped
        && current.iter().any(|s| {
            s.tool == first.tool
                && s.consent == LogConsent::Granted
                && Path::new(&s.folder).exists()
                && !selected.iter().any(|x| x.id == s.id)
        }))
    {
        return Err(CommandError::InvalidArguments);
    }
    Ok(())
}

fn scan_groups(
    storage: &Storage,
    sources: Vec<LogSource>,
    zone: &str,
) -> Result<Vec<ScanGroup>, CommandError> {
    let mut grouped: BTreeMap<String, Vec<LogSource>> = BTreeMap::new();
    let mut plan = Vec::new();
    for source in sources
        .into_iter()
        .filter(|s| s.consent == LogConsent::Granted)
    {
        let Some(tool) = locallogs::tool(&source.tool) else {
            continue;
        };
        // Missing roots do not erase accepted history. Invalid extant roots fail validation.
        if !Path::new(&source.folder).exists() {
            continue;
        }
        if tool.agent_report.is_some() {
            grouped.entry(source.tool.clone()).or_default().push(source);
        } else {
            plan.push(ScanGroup {
                zone: Some(source.time_zone.clone().unwrap_or_else(|| zone.to_string())),
                sources: vec![source],
            });
        }
    }
    for (tool_id, mut sources) in grouped {
        sources.sort_by(|a, b| a.folder.cmp(&b.folder));
        let dated = locallogs::tool(&tool_id)
            .and_then(|t| t.agent_report.as_ref())
            .is_some_and(|p| p.period == locallogs::tools::PeriodPolicy::Day);
        let pinned = storage
            .local_tool_scope(&tool_id)
            .map_err(|_| CommandError::StorageUnavailable)?;
        plan.push(ScanGroup {
            sources,
            zone: if dated {
                Some(
                    pinned
                        .and_then(|s| s.time_zone)
                        .unwrap_or_else(|| zone.to_string()),
                )
            } else {
                None
            },
        });
    }
    Ok(plan)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    const NOW: &str = "2026-11-09T10:00:00.000Z";

    #[test]
    fn grouped_plans_pin_zones_and_recheck_membership_consent_and_resolved_paths() {
        let dir = tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let mut workspace = Workspace::new(dir.path().join("workspace")).unwrap();
        workspace.storage = Some(Storage::create(&dir.path().join("test.db"), &[43; 32]).unwrap());
        let mut sources = Vec::new();
        for name in ["b", "a"] {
            let folder = dir.path().join(name);
            std::fs::create_dir(&folder).unwrap();
            sources.push(
                workspace
                    .storage
                    .as_mut()
                    .unwrap()
                    .set_log_source("amp", folder.to_str().unwrap(), LogConsent::Granted, NOW)
                    .unwrap(),
            );
        }
        let mut plan = scan_groups(
            workspace.storage.as_ref().unwrap(),
            sources.clone(),
            "Asia/Kolkata",
        )
        .unwrap();
        assert_eq!(plan.len(), 1);
        let group = plan.pop().unwrap();
        assert!(group.sources[0].folder < group.sources[1].folder);
        assert_eq!(group.zone.as_deref(), Some("Asia/Kolkata"));
        // The actual worker still refuses every disabled tool.
        assert!(workspace.local_log_scan_plan().unwrap().is_empty());
        let report = locallogs::GroupedScanResult {
            periods: vec![],
            sessions: vec![],
        };
        workspace
            .save_grouped_log_reports(&group, NOW, &report)
            .unwrap();
        let saved = workspace
            .storage
            .as_ref()
            .unwrap()
            .local_tool_scope("amp")
            .unwrap()
            .unwrap();
        assert_eq!(saved.latest_scan.source_ids.len(), 2);
        let plan = scan_groups(
            workspace.storage.as_ref().unwrap(),
            sources.clone(),
            "America/New_York",
        )
        .unwrap();
        assert_eq!(plan[0].zone, group.zone);
        let incomplete = ScanGroup {
            sources: vec![sources[0].clone()],
            zone: group.zone.clone(),
        };
        assert!(
            workspace
                .save_grouped_log_reports(&incomplete, NOW, &report)
                .is_err()
        );
        workspace
            .storage
            .as_mut()
            .unwrap()
            .set_log_source("amp", &sources[0].folder, LogConsent::Withdrawn, NOW)
            .unwrap();
        assert!(
            workspace
                .save_grouped_log_reports(&group, NOW, &report)
                .is_err()
        );
        workspace
            .storage
            .as_mut()
            .unwrap()
            .set_log_source("amp", &sources[0].folder, LogConsent::Granted, NOW)
            .unwrap();
        std::fs::remove_dir(&sources[0].folder).unwrap();
        symlink(&sources[1].folder, &sources[0].folder).unwrap();
        assert!(
            workspace
                .save_grouped_log_reports(&group, NOW, &report)
                .is_err()
        );
        assert_eq!(
            workspace
                .storage
                .as_ref()
                .unwrap()
                .local_tool_scope("amp")
                .unwrap(),
            Some(saved)
        );
        std::fs::remove_file(&sources[0].folder).unwrap();
        // A removed root reduces membership, without adding any saved root totals.
        let remaining = scan_groups(workspace.storage.as_ref().unwrap(), sources, "UTC").unwrap();
        assert_eq!(remaining[0].sources.len(), 1);
        workspace
            .save_grouped_log_reports(&remaining[0], NOW, &report)
            .unwrap();
        let folder = dir.path().join("unplaced");
        std::fs::create_dir(&folder).unwrap();
        let source = workspace
            .storage
            .as_mut()
            .unwrap()
            .set_log_source(
                "opencode",
                folder.to_str().unwrap(),
                LogConsent::Granted,
                NOW,
            )
            .unwrap();
        let unplaced = scan_groups(
            workspace.storage.as_ref().unwrap(),
            vec![source],
            "Asia/Kolkata",
        )
        .unwrap();
        assert_eq!(unplaced[0].zone, None);
        workspace
            .save_grouped_log_reports(&unplaced[0], NOW, &report)
            .unwrap();
    }
}
