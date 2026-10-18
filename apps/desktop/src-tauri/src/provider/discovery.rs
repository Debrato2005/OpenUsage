//! Advisory executable locations only. No process launch, directory listing, or credential reads.
//! Pinned layouts: https://github.com/openai/codex/blob/rust-v0.163.0/codex-cli/bin/codex.js
//! Installer boundary: https://github.com/openai/codex/blob/rust-v0.163.0/scripts/install/install.sh

use super::*;
use std::{
    collections::{HashSet, VecDeque},
    ffi::{OsStr, OsString},
    path::Component,
};

const MAX_PATH_ENTRIES: usize = 64;
const MAX_CANDIDATES: usize = 16;
const MAX_SYMLINKS: usize = 32;
const MAX_COMPONENTS: usize = 256;

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiscoveryLocation {
    Path,
    Standard,
    NpmPackage,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub(crate) struct DiscoveredExecutable {
    path: String,
    location: DiscoveryLocation,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub(crate) struct ExecutableDiscovery {
    candidates: Vec<DiscoveredExecutable>,
    truncated: bool,
}

struct Scope {
    excluded: Vec<PathBuf>,
}

impl Scope {
    fn excludes(&self, path: &Path) -> bool {
        path.components()
            .any(|component| component.as_os_str() == ".codex")
            || self.excluded.iter().any(|root| path.starts_with(root))
    }

    fn resolve(&self, path: &Path) -> Option<PathBuf> {
        self.resolve_with(path, &mut |path: &Path| fs::symlink_metadata(path))
    }

    fn resolve_with(
        &self,
        path: &Path,
        metadata: &mut impl FnMut(&Path) -> std::io::Result<fs::Metadata>,
    ) -> Option<PathBuf> {
        if !clean_absolute(path) {
            return None;
        }
        let mut pending = steps(path)?;
        let mut resolved = PathBuf::from("/");
        if self.excludes(&resolved) || !traversable(&metadata(&resolved).ok()?) {
            return None;
        }
        let mut symlinks = 0;
        let mut visits = 0;
        while let Some(step) = pending.pop_front() {
            visits += 1;
            if visits > MAX_COMPONENTS {
                return None;
            }
            if step == "/" {
                resolved = PathBuf::from("/");
                continue;
            }
            if step == ".." {
                resolved.pop();
                continue;
            }
            if step == "." {
                continue;
            }
            resolved.push(step);
            // Check before lstat or read_link, including each symlinked PATH-directory hop.
            if self.excludes(&resolved) || !displayable(&resolved) {
                return None;
            }
            let entry = metadata(&resolved).ok()?;
            // Refuse writable or foreign-owned ancestors before descending. The final source
            // still passes the shared core policy, and approval checks it again before execution.
            if !traversable(&entry) {
                return None;
            }
            if entry.file_type().is_symlink() {
                symlinks += 1;
                if symlinks > MAX_SYMLINKS {
                    return None;
                }
                let target = fs::read_link(&resolved).ok()?;
                if !displayable(&target) {
                    return None;
                }
                resolved.pop();
                let mut replacement = steps(&target)?;
                replacement.append(&mut pending);
                if replacement.len() > MAX_COMPONENTS {
                    return None;
                }
                pending = replacement;
            } else if !pending.is_empty() && !entry.is_dir() {
                return None;
            }
        }
        (!self.excludes(&resolved)).then_some(resolved)
    }
}

fn traversable(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        (metadata.uid() == 0 || metadata.uid() == unsafe { libc::geteuid() })
            && (!metadata.is_dir() || metadata.mode() & 0o022 == 0 || metadata.mode() & 0o1000 != 0)
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        false
    }
}

fn steps(path: &Path) -> Option<VecDeque<OsString>> {
    path.components()
        .map(|component| match component {
            Component::RootDir => Some(OsString::from("/")),
            Component::Normal(value) => Some(value.to_owned()),
            Component::ParentDir => Some(OsString::from("..")),
            Component::CurDir => Some(OsString::from(".")),
            Component::Prefix(_) => None,
        })
        .collect()
}

fn displayable(path: &Path) -> bool {
    path.to_str().is_some_and(|value| {
        !value.is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control)
    })
}
fn clean_absolute(path: &Path) -> bool {
    path.is_absolute()
        && displayable(path)
        && !path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
}

pub(crate) fn discover(managed_profiles: &Path) -> Result<ExecutableDiscovery, CommandError> {
    if !cfg!(unix) {
        return Err(CommandError::ProviderUnsupported);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| clean_absolute(path));
    let mut excluded = vec![managed_profiles.to_owned()];
    if let Some(home) = &home {
        excluded.push(home.join(".codex"));
    }
    if let Some(path) = std::env::var_os("CODEX_HOME") {
        let path = PathBuf::from(path);
        if !clean_absolute(&path) {
            // Do not guess a credential root when inherited configuration is relative or malformed.
            return Err(CommandError::ProviderDiscoveryUnavailable);
        }
        excluded.push(path);
    }
    let (mut locations, truncated) = std::env::var_os("PATH")
        .map(|path| path_locations(&path))
        .unwrap_or_default();
    for directory in [
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
        "/opt/homebrew/bin",
        "/home/linuxbrew/.linuxbrew/bin",
    ] {
        locations.push((
            Path::new(directory).join("codex"),
            DiscoveryLocation::Standard,
        ));
    }
    if let Some(home) = home {
        locations.push((home.join(".local/bin/codex"), DiscoveryLocation::Standard));
    }
    Ok(find(&Scope { excluded }, locations, truncated))
}

fn path_locations(path: &OsStr) -> (Vec<(PathBuf, DiscoveryLocation)>, bool) {
    let mut locations = vec![];
    for (index, directory) in std::env::split_paths(path).enumerate() {
        if index >= MAX_PATH_ENTRIES {
            return (locations, true);
        }
        if clean_absolute(&directory) {
            locations.push((directory.join("codex"), DiscoveryLocation::Path));
        }
    }
    (locations, false)
}

fn find(
    scope: &Scope,
    locations: Vec<(PathBuf, DiscoveryLocation)>,
    mut truncated: bool,
) -> ExecutableDiscovery {
    let mut candidates = vec![];
    let mut seen = HashSet::new();
    for (path, location) in locations {
        let Some(path) = scope.resolve(&path) else {
            continue;
        };
        let probes = npm_binaries(&path);
        let entries = if probes.is_empty() {
            vec![(path, location)]
        } else {
            probes
                .into_iter()
                .map(|path| (path, DiscoveryLocation::NpmPackage))
                .collect()
        };
        for (path, location) in entries {
            let Some(path) = scope.resolve(&path) else {
                continue;
            };
            if !safe_executable(&path) || !seen.insert(path.clone()) {
                continue;
            }
            if candidates.len() == MAX_CANDIDATES {
                truncated = true;
                break;
            }
            candidates.push(DiscoveredExecutable {
                path: path.to_string_lossy().into_owned(),
                location,
            });
        }
        if truncated && candidates.len() == MAX_CANDIDATES {
            break;
        }
    }
    ExecutableDiscovery {
        candidates,
        truncated,
    }
}

fn safe_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let Ok(metadata) = fs::symlink_metadata(path) else {
            return false;
        };
        metadata.is_file()
            && !metadata.file_type().is_symlink()
            && metadata.mode() & 0o111 != 0
            && metadata.mode() & 0o6022 == 0
            && (metadata.uid() == 0 || metadata.uid() == unsafe { libc::geteuid() })
            && (1..=MAX_EXECUTABLE_BYTES).contains(&metadata.len())
            && require_local_filesystem(path).is_ok()
            && openusage_codex::supervisor::validate_executable_source_path(path).is_ok()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

fn npm_binaries(wrapper: &Path) -> Vec<PathBuf> {
    let Some(bin) = wrapper.parent() else {
        return vec![];
    };
    let Some(package) = bin.parent() else {
        return vec![];
    };
    if wrapper.file_name().is_none_or(|name| name != "codex.js")
        || bin.file_name().is_none_or(|name| name != "bin")
        || package.file_name().is_none_or(|name| name != "codex")
        || package
            .parent()
            .and_then(Path::file_name)
            .is_none_or(|name| name != "@openai")
    {
        return vec![];
    }
    let (platform, target) = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        ("codex-linux-x64", "x86_64-unknown-linux-musl")
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        ("codex-linux-arm64", "aarch64-unknown-linux-musl")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        ("codex-darwin-x64", "x86_64-apple-darwin")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        ("codex-darwin-arm64", "aarch64-apple-darwin")
    } else {
        return vec![];
    };
    let leaf = Path::new("vendor").join(target).join("bin/codex");
    vec![
        package.parent().unwrap().join(platform).join(&leaf),
        package
            .join("node_modules/@openai")
            .join(platform)
            .join(&leaf),
        package.join(leaf),
    ]
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn executable(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"#!/usr/bin/python3\nfrom pathlib import Path\nPath(__file__).parent.joinpath('unexpected-execution').write_text('executed')\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[test]
    fn discovery_resolves_visible_aliases_and_fixed_npm_binary_without_executing_or_approving() {
        let temporary = tempfile::tempdir().unwrap();
        let native = temporary.path().join("native/codex");
        executable(&native);
        let alias = temporary.path().join("alias");
        symlink("native", &alias).unwrap();
        let wrapper = temporary
            .path()
            .join("lib/node_modules/@openai/codex/bin/codex.js");
        executable(&wrapper);
        let vendor = npm_binaries(&wrapper).into_iter().next().unwrap();
        executable(&vendor);
        let npm_link = temporary.path().join("codex");
        symlink(&wrapper, &npm_link).unwrap();
        let scope = Scope { excluded: vec![] };
        let result = find(
            &scope,
            vec![
                (alias.join("codex"), DiscoveryLocation::Path),
                (native.clone(), DiscoveryLocation::Standard),
                (npm_link, DiscoveryLocation::Path),
            ],
            false,
        );
        assert_eq!(
            result.candidates,
            vec![
                DiscoveredExecutable {
                    path: native.to_str().unwrap().into(),
                    location: DiscoveryLocation::Path
                },
                DiscoveredExecutable {
                    path: vendor.to_str().unwrap().into(),
                    location: DiscoveryLocation::NpmPackage
                }
            ]
        );
        assert!(!result.truncated);
        let provider = ProviderCoordinator::default();
        let candidate = provider.inspect(&result.candidates[0].path).unwrap();
        assert!(!candidate.source_verified);
        assert_eq!(
            provider.approve(&candidate.candidate_id, true).err(),
            Some(CommandError::ProviderSourceUnverified)
        );
        for path in [&native, &wrapper, &vendor] {
            assert!(!path.parent().unwrap().join("unexpected-execution").exists());
        }
    }

    #[test]
    fn discovery_excludes_credential_and_managed_roots_before_each_filesystem_lookup() {
        let temporary = tempfile::tempdir().unwrap();
        let defaults = temporary.path().join("synthetic-home/.codex");
        let configured = temporary.path().join("configured-provider-home");
        let managed = temporary.path().join("workspace/profiles");
        for root in [&defaults, &configured, &managed] {
            executable(&root.join("codex"));
        }
        let scope = Scope {
            excluded: vec![configured.clone(), managed.clone()],
        };
        for (number, root) in [&defaults, &configured, &managed].into_iter().enumerate() {
            let alias = temporary.path().join(format!("alias-{number}"));
            symlink(root, &alias).unwrap();
            let executable_link = temporary.path().join(format!("binary-{number}"));
            symlink(root.join("codex"), &executable_link).unwrap();
            for path in [root.join("codex"), alias.join("codex"), executable_link] {
                let resolved = scope.resolve_with(&path, &mut |lookup| {
                    assert!(
                        ![&defaults, &configured, &managed]
                            .into_iter()
                            .any(|root| lookup.starts_with(root)),
                        "Credential-root metadata must not be read"
                    );
                    fs::symlink_metadata(lookup)
                });
                assert_eq!(resolved, None);
            }
        }
    }

    #[test]
    fn discovery_bounds_paths_results_and_symlinks_and_refuses_unsafe_sources() {
        let temporary = tempfile::tempdir().unwrap();
        let directories: Vec<_> = (0..70)
            .map(|index| temporary.path().join(format!("bin-{index}")))
            .collect();
        let (locations, truncated) = path_locations(&std::env::join_paths(&directories).unwrap());
        assert_eq!(locations.len(), 64);
        assert!(truncated);
        let (relative, _) = path_locations(OsStr::new(":relative:.:../bin"));
        assert!(relative.is_empty());
        for directory in directories.iter().take(18) {
            executable(&directory.join("codex"));
        }
        let scope = Scope { excluded: vec![] };
        let result = find(&scope, locations, truncated);
        assert_eq!(result.candidates.len(), 16);
        assert!(result.truncated);
        let unsafe_source = directories[0].join("codex");
        fs::set_permissions(&unsafe_source, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(
            find(
                &scope,
                vec![(unsafe_source, DiscoveryLocation::Path)],
                false
            )
            .candidates
            .is_empty()
        );
        fs::set_permissions(&directories[1], fs::Permissions::from_mode(0o777)).unwrap();
        assert!(
            find(
                &scope,
                vec![(directories[1].join("codex"), DiscoveryLocation::Path)],
                false
            )
            .candidates
            .is_empty()
        );
        for index in 0..33 {
            symlink(
                format!("link-{}", index + 1),
                temporary.path().join(format!("link-{index}")),
            )
            .unwrap();
        }
        executable(&temporary.path().join("link-33"));
        assert_eq!(scope.resolve(&temporary.path().join("link-0")), None);
    }
}
