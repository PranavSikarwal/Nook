use crate::config::{RootAccess, RootConfig};
use std::path::{Component, Path, PathBuf};
use tracing::warn;

pub const OUTSIDE_ROOTS: &str = "Path is outside the allowed folders";
pub const NOT_ABSOLUTE: &str = "Path must be absolute";
pub const HAS_PARENT: &str = "Path must not contain ..";
pub const BAD_PATH: &str = "Path is not valid";

#[derive(Debug, Clone)]
pub struct RootEntry {
    pub configured: PathBuf,
    pub canonical: PathBuf,
    pub access: RootAccess,
}

#[derive(Debug, Clone)]
pub struct ResolvedPath {
    pub index: usize,
    pub relative: PathBuf,
    pub canonical_path: PathBuf,
}

/// The folders the user allowed, in the order of the config.
#[derive(Debug, Clone, Default)]
pub struct RootSet {
    entries: Vec<RootEntry>,
}

impl RootSet {
    /// Keeps the usable roots and logs the others.
    pub fn from_config(roots: &[RootConfig]) -> Self {
        let mut entries = Vec::new();
        for entry in roots {
            let configured = PathBuf::from(&entry.path);
            if !configured.is_absolute() {
                warn!("Ignored filesystem root {}: it is not absolute", entry.path);
                continue;
            }
            match configured.canonicalize() {
                Ok(canonical) if canonical.is_dir() => entries.push(RootEntry {
                    configured,
                    canonical,
                    access: entry.access,
                }),
                Ok(_) => warn!("Ignored filesystem root {}: it is not a folder", entry.path),
                Err(error) => warn!("Ignored filesystem root {}: {error}", entry.path),
            }
        }
        Self { entries }
    }

    pub fn from_entries(entries: Vec<RootEntry>) -> Self {
        Self { entries }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[RootEntry] {
        &self.entries
    }

    /// One sentence for a tool description, so the model knows which paths to use.
    pub fn describe(&self) -> String {
        let folders: Vec<String> = self
            .entries
            .iter()
            .map(|entry| {
                let access = match entry.access {
                    RootAccess::Read => "read only",
                    RootAccess::ReadWrite => "read and write",
                };
                format!("{} ({access})", entry.configured.display())
            })
            .collect();
        format!("Allowed folders: {}.", folders.join("; "))
    }

    pub fn has_write_access(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.access == RootAccess::ReadWrite)
    }

    /// The root a command runs in when the model gives no folder.
    pub fn default_index(&self) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| entry.access == RootAccess::ReadWrite)
            .or(if self.entries.is_empty() {
                None
            } else {
                Some(0)
            })
    }

    pub fn resolve(&self, raw: &str) -> Result<ResolvedPath, &'static str> {
        if raw.contains('\0') {
            return Err(BAD_PATH);
        }
        let path = Path::new(raw);
        if !path.is_absolute() {
            return Err(NOT_ABSOLUTE);
        }
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::ParentDir => return Err(HAS_PARENT),
                Component::CurDir => {}
                other => normalized.push(other.as_os_str()),
            }
        }
        let mut best: Option<(usize, &Path)> = None;
        for (index, root) in self.entries.iter().enumerate() {
            for prefix in [&root.canonical, &root.configured] {
                if normalized.starts_with(prefix)
                    && best
                        .is_none_or(|(_, known)| prefix.as_os_str().len() > known.as_os_str().len())
                {
                    best = Some((index, prefix.as_path()));
                }
            }
        }
        let (index, prefix) = best.ok_or(OUTSIDE_ROOTS)?;
        let relative = normalized
            .strip_prefix(prefix)
            .map_err(|_| OUTSIDE_ROOTS)?
            .to_path_buf();
        let canonical_path = self.entries[index].canonical.join(&relative);
        Ok(ResolvedPath {
            index,
            relative,
            canonical_path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(paths: &[(&Path, RootAccess)]) -> RootSet {
        RootSet::from_config(
            &paths
                .iter()
                .map(|(path, access)| RootConfig {
                    path: path.display().to_string(),
                    access: *access,
                })
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn nested_roots_use_the_longest_match() {
        let dir = tempfile::tempdir().unwrap();
        let outer = dir.path().canonicalize().unwrap();
        let inner = outer.join("inner");
        std::fs::create_dir(&inner).unwrap();
        let roots = set(&[(&outer, RootAccess::ReadWrite), (&inner, RootAccess::Read)]);

        let in_outer = roots
            .resolve(&outer.join("a.txt").display().to_string())
            .unwrap();
        let in_inner = roots
            .resolve(&inner.join("b.txt").display().to_string())
            .unwrap();

        assert_eq!(in_outer.index, 0);
        assert_eq!(in_inner.index, 1);
        assert_eq!(in_inner.relative, PathBuf::from("b.txt"));
    }

    #[test]
    fn the_root_itself_resolves_to_an_empty_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let roots = set(&[(&root, RootAccess::Read)]);

        let resolved = roots.resolve(&root.display().to_string()).unwrap();

        assert!(resolved.relative.as_os_str().is_empty());
        assert_eq!(resolved.canonical_path, root);
    }

    #[test]
    fn dot_components_are_dropped_and_parent_components_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let roots = set(&[(&root, RootAccess::Read)]);

        let dotted = format!("{}/./a/./b", root.display());
        let parent = format!("{}/a/../b", root.display());

        assert_eq!(
            roots.resolve(&dotted).unwrap().relative,
            PathBuf::from("a/b")
        );
        assert_eq!(roots.resolve(&parent).unwrap_err(), HAS_PARENT);
    }

    #[test]
    fn the_default_root_prefers_a_writable_one() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let (read, write) = (base.join("r"), base.join("w"));
        std::fs::create_dir(&read).unwrap();
        std::fs::create_dir(&write).unwrap();

        let roots = set(&[(&read, RootAccess::Read), (&write, RootAccess::ReadWrite)]);

        assert_eq!(roots.default_index(), Some(1));
        assert_eq!(RootSet::default().default_index(), None);
    }

    #[test]
    fn unusable_roots_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "x").unwrap();

        let roots = RootSet::from_config(&[
            RootConfig {
                path: "relative".to_string(),
                access: RootAccess::Read,
            },
            RootConfig {
                path: file.display().to_string(),
                access: RootAccess::Read,
            },
            RootConfig {
                path: "/definitely/missing".to_string(),
                access: RootAccess::Read,
            },
        ]);

        assert!(roots.is_empty());
    }
}
