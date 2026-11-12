//! Complete reports for the additional tools. A folder group is one accounting scope (SC-022).
use crate::runner::{ONE_RUN, run_roots};
use crate::tools::{PeriodPolicy, SessionShape, tool};
use crate::{
    AgentPeriod, AgentSession, ScanError, ScanRequest, parse_agent_periods, parse_agent_sessions,
};
use std::path::Path;
use std::time::{Duration, Instant};

pub struct GroupedScanRequest<'a> {
    pub binary: &'a Path,
    pub expected_sha256: &'a str,
    pub tool: &'a str,
    /// Resolved, consented roots. The runner removes repeats and uses path order.
    pub folders: &'a [&'a Path],
    pub empty_home: &'a Path,
    pub timezone: &'a str,
    pub timeout: Duration,
    /// The combined output bound for all required reports.
    pub max_output: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupedScanResult {
    pub periods: Vec<AgentPeriod>,
    pub sessions: Vec<AgentSession>,
}

/// A full scan only. Disabled tools never run. Qualification is a separate release gate.
pub fn scan_grouped(request: &GroupedScanRequest<'_>) -> Result<GroupedScanResult, ScanError> {
    scan_grouped_with_check(request, || true)
}

/// Recheck consent before each process and before returning a complete result.
pub fn scan_grouped_with_check(
    request: &GroupedScanRequest<'_>,
    check: impl FnMut() -> bool,
) -> Result<GroupedScanResult, ScanError> {
    tool(request.tool)
        .filter(|tool| tool.enabled)
        .ok_or(ScanError::InvalidRequest)?;
    scan_grouped_checked(request, check)
}

/// Exercise the grouped path for the disabled Pi candidate in the qualification harness.
#[cfg(feature = "pi-qualification")]
pub fn qualify_pi_candidate(
    request: &GroupedScanRequest<'_>,
    check: impl FnMut() -> bool,
) -> Result<GroupedScanResult, ScanError> {
    if request.tool != "pi" {
        return Err(ScanError::InvalidRequest);
    }
    scan_grouped_checked(request, check)
}

#[cfg(all(feature = "codex-qualification", target_os = "linux"))]
/// Exercise one Codex snapshot through the isolated Linux qualification path.
pub fn qualify_codex_candidate(
    request: &GroupedScanRequest<'_>,
    mut check: impl FnMut() -> bool,
) -> Result<GroupedScanResult, ScanError> {
    if request.tool != "codex" {
        return Err(ScanError::InvalidRequest);
    }
    let folder = request.folders.first().ok_or(ScanError::InvalidRequest)?;
    let _one = ONE_RUN.try_lock().map_err(|_| ScanError::Busy)?;
    let started = Instant::now();
    if !check() {
        return Err(ScanError::InvalidRequest);
    }
    let output = run_roots(
        &ScanRequest {
            binary: request.binary,
            expected_sha256: request.expected_sha256,
            tool: request.tool,
            folder,
            empty_home: request.empty_home,
            timezone: request.timezone,
            since: None,
            timeout: request
                .timeout
                .checked_sub(started.elapsed())
                .ok_or(ScanError::Timeout)?,
            max_output: request.max_output,
        },
        &["codex", "snapshot", "--json", "--offline", "--no-color"],
        request.folders,
    )?;
    let result = crate::parse_codex_snapshot(&output).map_err(ScanError::Report)?;
    if !check()
        || request
            .folders
            .iter()
            .any(|folder| !crate::runner::valid_resolved_folder(folder))
    {
        return Err(ScanError::InvalidRequest);
    }
    Ok(result)
}

fn scan_grouped_checked(
    request: &GroupedScanRequest<'_>,
    mut check: impl FnMut() -> bool,
) -> Result<GroupedScanResult, ScanError> {
    let definition = tool(request.tool)
        .filter(|tool| tool.session_shape == Some(SessionShape::Agent))
        .ok_or(ScanError::InvalidRequest)?;
    let policy = definition
        .agent_report
        .as_ref()
        .ok_or(ScanError::InvalidRequest)?;
    let folder = request.folders.first().ok_or(ScanError::InvalidRequest)?;
    let _one = ONE_RUN.try_lock().map_err(|_| ScanError::Busy)?;
    let started = Instant::now();
    let mut remaining_output = request.max_output;
    let mut run = |fixed: &[&str]| {
        if !check() {
            return Err(ScanError::InvalidRequest);
        }
        let timeout = request
            .timeout
            .checked_sub(started.elapsed())
            .ok_or(ScanError::Timeout)?;
        let output = run_roots(
            &ScanRequest {
                binary: request.binary,
                expected_sha256: request.expected_sha256,
                tool: request.tool,
                folder,
                empty_home: request.empty_home,
                timezone: request.timezone,
                since: None,
                timeout,
                max_output: remaining_output,
            },
            fixed,
            request.folders,
        )?;
        remaining_output -= output.len();
        Ok::<_, ScanError>(output)
    };
    if request.tool == "pi" {
        let output = run(&["pi", "snapshot", "--json", "--offline", "--no-color"])?;
        let result =
            crate::agent_snapshot::parse_pi_snapshot(&output).map_err(ScanError::Report)?;
        if !check()
            || request
                .folders
                .iter()
                .any(|folder| !crate::runner::valid_resolved_folder(folder))
        {
            return Err(ScanError::InvalidRequest);
        }
        return Ok(result);
    }
    let dated = if policy.period == PeriodPolicy::Day {
        let output = run(&["daily", "--json", "--offline", "--by-agent", "--no-color"])?;
        Some(parse_agent_periods(request.tool, &output).map_err(ScanError::Report)?)
    } else {
        None
    };
    let output = run(&[
        definition.id.as_str(),
        "session",
        "--json",
        "--offline",
        "--no-color",
    ])?;
    let periods = match dated {
        Some(report) => report,
        None => parse_agent_periods(request.tool, &output).map_err(ScanError::Report)?,
    }
    .map(|report| report.periods)
    .unwrap_or_default();
    let sessions = parse_agent_sessions(request.tool, &output)
        .map_err(ScanError::Report)?
        .map(|report| report.sessions)
        .unwrap_or_default();
    if !check()
        || request
            .folders
            .iter()
            .any(|folder| !crate::runner::valid_resolved_folder(folder))
    {
        return Err(ScanError::InvalidRequest);
    }
    Ok(GroupedScanResult { periods, sessions })
}

#[cfg(all(test, unix))]
mod tests;
