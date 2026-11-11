//! The explicit Pi descriptor for a native qualification build.
use super::PinnedScanner;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pin {
    bounded_release_scanner_version: String,
    bounded_release_scanner_sha256: String,
}

pub(super) fn scanner() -> Option<PinnedScanner> {
    let binary = PathBuf::from(std::env::var_os("OPENUSAGE_NATIVE_PI_SCANNER")?);
    if !binary.is_absolute() {
        return None;
    }
    let pin: Pin = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/tools/fixtures/pi-producer/pin.json"
    )))
    .ok()?;
    Some(PinnedScanner {
        binary,
        digest: pin.bounded_release_scanner_sha256,
        version: pin.bounded_release_scanner_version,
    })
}
