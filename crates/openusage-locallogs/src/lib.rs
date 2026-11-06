//! Local log totals from the pinned ccusage binary (SRS Section 5C).
//! Rust checks the binary, runs it with fixed arguments and parses its report strictly.

pub mod report;
pub mod runner;

pub use report::{ReportError, TOOLS, UsageRow, parse_daily_by_agent};
pub use runner::{
    CCUSAGE_VERSION, DEFAULT_MAX_OUTPUT, DEFAULT_TIMEOUT, ScanError, ScanRequest, pinned_sha256,
    scan, sha256_hex,
};
