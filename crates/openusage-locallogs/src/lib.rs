//! Local log totals from the pinned ccusage binary (SRS Section 5C).
//! Rust checks the binary, runs it with fixed arguments and parses its report strictly.

pub mod detect;
pub mod hours;
pub mod report;
pub mod runner;
pub mod session;

pub use detect::{Detected, KNOWN_FOLDERS, detect};
pub use hours::{HOUR_TOOLS, HourRow, parse_hours};
pub use report::{ReportError, TOOLS, UsageRow, parse_daily_by_agent};
pub use runner::{
    BINARY_NAME, CCUSAGE_VERSION, DEFAULT_MAX_OUTPUT, DEFAULT_TIMEOUT, FOLDER_VARIABLES, ScanError,
    ScanRequest, pinned_sha256, pinned_sha256_here, scan, scan_hours, scan_sessions, sha256_hex,
};
pub use session::{SessionRow, parse_sessions};
