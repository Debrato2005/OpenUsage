//! Detection checks only whether known folders exist. It opens no log (OU-LOG-001).

use crate::tools::{MarkerKind, Tool, enabled_tools};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    pub tool: &'static str,
    /// The resolved folder. Links to one folder give one entry (OU-DUP-008).
    pub folder: String,
}

pub fn detect(home: &Path) -> Vec<Detected> {
    detect_tools(home, enabled_tools())
}

/// Detect the disabled Pi candidate only for qualification.
#[cfg(feature = "pi-qualification")]
pub fn detect_pi_candidate(home: &Path) -> Vec<Detected> {
    detect_tools(home, crate::tools::tool("pi").into_iter())
}

fn detect_tools(home: &Path, tools: impl Iterator<Item = &'static Tool>) -> Vec<Detected> {
    let mut seen = BTreeSet::new();
    let mut detected = Vec::new();
    for tool in tools {
        for root in &tool.roots {
            let folder = home.join(&root.path);
            let marker = folder.join(&root.marker);
            let exists = match root.kind {
                MarkerKind::Directory => marker.is_dir(),
                MarkerKind::File => marker.is_file(),
            };
            if !exists {
                continue;
            }
            let Some(resolved) = std::fs::canonicalize(&folder)
                .ok()
                .and_then(|path| path.to_str().map(str::to_owned))
            else {
                continue;
            };
            if seen.insert(resolved.clone()) {
                detected.push(Detected {
                    tool: tool.id.as_str(),
                    folder: resolved,
                });
            }
        }
    }
    detected
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[cfg(feature = "pi-qualification")]
    #[test]
    fn candidate_detection_does_not_enable_production_or_parse_records() {
        let home = tempfile::tempdir().unwrap();
        let sessions = home.path().join(".pi/agent/sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        std::fs::write(sessions.join("broken.jsonl"), b"\xff\xfe").unwrap();
        assert!(detect(home.path()).is_empty());
        assert_eq!(
            detect_pi_candidate(home.path()),
            [Detected {
                tool: "pi",
                folder: std::fs::canonicalize(&sessions)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .into(),
            }]
        );
        assert!(!crate::tools::tool("pi").unwrap().enabled);
    }

    #[test]
    fn detection_checks_known_folders_and_resolves_links() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path();
        assert!(detect(path).is_empty());
        std::fs::create_dir_all(path.join(".claude/projects")).unwrap();
        std::fs::create_dir_all(path.join(".config")).unwrap();
        // A link to the same folder is one folder (OU-DUP-008).
        symlink(path.join(".claude"), path.join(".config/claude")).unwrap();
        // A Codex home without sessions has no logs to offer.
        std::fs::create_dir(path.join(".codex")).unwrap();
        let claude = std::fs::canonicalize(path.join(".claude")).unwrap();
        assert_eq!(
            detect(path),
            [Detected {
                tool: "claude",
                folder: claude.to_str().unwrap().into()
            }]
        );
        std::fs::write(path.join(".codex/sessions"), "not a folder").unwrap();
        assert_eq!(detect(path).len(), 1);
        std::fs::remove_file(path.join(".codex/sessions")).unwrap();
        std::fs::create_dir(path.join(".codex/sessions")).unwrap();
        assert_eq!(
            detect(path).iter().map(|d| d.tool).collect::<Vec<_>>(),
            ["claude", "codex"]
        );
    }
}
