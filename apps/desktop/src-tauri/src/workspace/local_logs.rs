//! Local log sources and scans in the encrypted store (SRS Section 5C).
use super::*;
use openusage_locallogs::{CCUSAGE_VERSION, UsageRow};
use openusage_storage::{
    LocalUsageDay, LocalUsageDifference, LocalUsageRow, LocalUsageScan, LogConsent, LogSource,
};

pub(crate) struct LocalUsageRange {
    pub days: Vec<LocalUsageDay>,
    pub differences: Vec<LocalUsageDifference>,
    pub sources: Vec<LogSource>,
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

    /// The granted sources and the display zone, which a source without a pinned zone uses.
    pub(crate) fn local_log_scan_plan(&mut self) -> Result<(Vec<LogSource>, String), CommandError> {
        let storage = self.local_logs_storage()?;
        let zone = storage
            .time_zone()
            .map_err(|_| CommandError::StorageUnavailable)?;
        let sources = storage
            .log_sources()
            .map_err(|_| CommandError::StorageUnavailable)?
            .into_iter()
            .filter(|source| source.consent == LogConsent::Granted)
            .collect();
        Ok((sources, zone))
    }

    /// Saves one run of one source in one transaction (SRS Section 18.3, OU-LOG-011).
    pub(crate) fn save_local_log_scan(
        &mut self,
        source_id: &str,
        zone: &str,
        scanned_at_utc: &str,
        rows: &[UsageRow],
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
        self.local_logs_storage()?
            .save_local_usage_scan(&LocalUsageScan {
                log_source_ids: &[source_id.to_string()],
                scanner_version: CCUSAGE_VERSION,
                time_zone: zone,
                since: None,
                scanned_at_utc,
                rows: &rows,
            })
            .map(|_| ())
            .map_err(|_| CommandError::StorageUnavailable)
    }

    /// The zone of the saved days: the first pinned source zone, else the display zone (OU-LOG-010).
    pub(crate) fn local_usage_zone(&mut self) -> Result<String, CommandError> {
        let storage = self.local_logs_storage()?;
        let pinned = storage
            .log_sources()
            .map_err(|_| CommandError::StorageUnavailable)?
            .into_iter()
            .find_map(|source| source.time_zone);
        match pinned {
            Some(zone) => Ok(zone),
            None => storage
                .time_zone()
                .map_err(|_| CommandError::StorageUnavailable),
        }
    }

    /// Saved days and differences from `since` to `until`, with the sources that own them.
    pub(crate) fn local_usage_range(
        &mut self,
        since: &str,
        until: &str,
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
