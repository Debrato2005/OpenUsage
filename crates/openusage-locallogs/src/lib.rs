//! Local log totals from the pinned ccusage binary (SRS Section 5C).
//! Rust checks the binary, runs it with fixed arguments and parses its report strictly.

pub mod agent_period;
pub mod agent_report;
pub mod detect;
pub mod hours;
pub mod report;
pub mod runner;
pub mod session;
pub mod tools;

pub use agent_period::{AgentPeriod, AgentPeriodReport, UsagePeriod, parse_agent_periods};
pub use agent_report::{
    AgentSession, AgentSessionReport, ModelDetail, TokenDetails, parse_agent_sessions,
};
pub use detect::{Detected, detect};
pub use hours::{HourRow, parse_hours};
pub use report::{ReportError, UsageRow, parse_daily_by_agent};
pub use runner::{
    BINARY_NAME, CCUSAGE_VERSION, DEFAULT_MAX_OUTPUT, DEFAULT_TIMEOUT, ScanError, ScanRequest,
    pinned_sha256, pinned_sha256_here, scan, scan_hours, scan_sessions, sha256_hex,
};
pub use session::{SessionRow, parse_sessions};
pub use tools::{MarkerKind, SessionShape, Tool, enabled_tools, hour_tools, tool, tools};
