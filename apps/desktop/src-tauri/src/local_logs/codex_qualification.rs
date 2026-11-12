//! The explicit Codex descriptor for a native qualification build.
use super::PinnedScanner;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pin {
    version: String,
    executable_sha256: String,
}

pub(super) fn scanner() -> Option<PinnedScanner> {
    scanner_at(std::env::var_os("OPENUSAGE_NATIVE_CODEX_SCANNER").map(PathBuf::from))
}

fn scanner_at(binary: Option<PathBuf>) -> Option<PinnedScanner> {
    let binary = binary.filter(|path| path.is_absolute())?;
    let pin: Pin = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/tools/fixtures/codex-snapshot-candidate/pin.json"
    )))
    .ok()?;
    Some(PinnedScanner {
        binary,
        digest: pin.executable_sha256,
        version: pin.version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_candidate_keeps_the_pinned_identity_and_requires_an_absolute_path() {
        assert!(scanner_at(None).is_none());
        assert!(scanner_at(Some(PathBuf::from("relative-scanner"))).is_none());
        let descriptor = scanner_at(Some(PathBuf::from("/synthetic/codex-candidate"))).unwrap();
        assert_eq!(
            descriptor.binary,
            PathBuf::from("/synthetic/codex-candidate")
        );
        assert_eq!(descriptor.version, "20.0.48-openusage.2");
        assert_eq!(
            descriptor.digest,
            "88c3b58173e6ad94e02b8b6e23a802f4aa18f9b5e6713832f5f1522859e9cb2b"
        );
    }
}
