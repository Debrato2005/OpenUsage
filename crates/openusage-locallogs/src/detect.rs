//! Detection checks only whether known folders exist. It opens no log (OU-LOG-001).

use std::collections::BTreeSet;
use std::path::Path;

/// Known folders under a home, with the log folder whose existence detects each one.
pub const KNOWN_FOLDERS: &[(&str, &str, &str)] = &[
    ("claude", ".claude", "projects"),
    ("claude", ".config/claude", "projects"),
    ("codex", ".codex", "sessions"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    pub tool: &'static str,
    /// The resolved folder. Links to one folder give one entry (OU-DUP-008).
    pub folder: String,
}

pub fn detect(home: &Path) -> Vec<Detected> {
    let mut seen = BTreeSet::new();
    KNOWN_FOLDERS
        .iter()
        .filter_map(|(tool, folder, logs)| {
            let folder = home.join(folder);
            if !folder.join(logs).is_dir() {
                return None;
            }
            let resolved = std::fs::canonicalize(&folder).ok()?.to_str()?.to_string();
            seen.insert(resolved.clone()).then_some(Detected {
                tool,
                folder: resolved,
            })
        })
        .collect()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

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
