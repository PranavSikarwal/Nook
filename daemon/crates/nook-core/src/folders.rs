use crate::config::{Config, RootAccess, RootConfig};
use crate::protocol::RootInfo;
use crate::providers::roots::{BAD_PATH, HAS_PARENT, NOT_ABSOLUTE};
use crate::server::AppState;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

pub const MAX_ROOTS: usize = 16;

const NOT_A_FOLDER: &str = "That path is not a folder";
const HOME_OR_ABOVE: &str =
    "Choose a folder inside your home folder. Your home folder and the folders above it give too much access";
const APP_FOLDER: &str = "That folder holds the Nook settings and cannot be shared";
const ALREADY_LISTED: &str = "That folder overlaps a folder that is already in the list";
const TOO_MANY: &str = "The list is full";
const UNKNOWN_FOLDER: &str = "That folder is not in the list";
const NOT_SAVED: &str = "The list could not be saved to config.toml";

/// The places a new folder must stay away from.
pub struct Keepout {
    pub home: Option<PathBuf>,
    pub app_dir: PathBuf,
}

impl Keepout {
    pub fn current() -> Self {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .and_then(|home| home.canonicalize().ok());
        let app_dir = Config::app_dir();
        Self {
            home,
            app_dir: app_dir.canonicalize().unwrap_or(app_dir),
        }
    }
}

/// Lists every configured folder, also the ones that cannot be used now.
pub fn describe(configured: &[RootConfig]) -> Vec<RootInfo> {
    configured
        .iter()
        .map(|root| {
            let reason = unusable_reason(&root.path);
            RootInfo {
                path: root.path.clone(),
                access: root.access,
                usable: reason.is_none(),
                reason: reason.map(str::to_string),
            }
        })
        .collect()
}

fn unusable_reason(path: &str) -> Option<&'static str> {
    let path = Path::new(path);
    if !path.is_absolute() {
        return Some("The path is not absolute");
    }
    match path.canonicalize() {
        Ok(canonical) if canonical.is_dir() => None,
        Ok(_) => Some("The path is not a folder"),
        Err(_) => Some("The folder does not exist"),
    }
}

/// Checks a folder that the user wants to add. Returns the path to store, with
/// links resolved.
pub fn check_new_root(
    raw: &str,
    existing: &[RootConfig],
    keepout: &Keepout,
) -> Result<String, &'static str> {
    if existing.len() >= MAX_ROOTS {
        return Err(TOO_MANY);
    }
    let path = Path::new(raw);
    if raw.is_empty() || raw.contains('\0') {
        return Err(BAD_PATH);
    }
    if !path.is_absolute() {
        return Err(NOT_ABSOLUTE);
    }
    if path.components().any(|part| part == Component::ParentDir) {
        return Err(HAS_PARENT);
    }
    let canonical = path.canonicalize().map_err(|_| NOT_A_FOLDER)?;
    if !canonical.is_dir() {
        return Err(NOT_A_FOLDER);
    }
    if keepout
        .home
        .as_ref()
        .is_some_and(|home| home.starts_with(&canonical))
        || canonical.parent().is_none()
    {
        return Err(HOME_OR_ABOVE);
    }
    if canonical.starts_with(&keepout.app_dir) || keepout.app_dir.starts_with(&canonical) {
        return Err(APP_FOLDER);
    }
    for root in existing {
        if let Ok(other) = Path::new(&root.path).canonicalize() {
            if canonical.starts_with(&other) || other.starts_with(&canonical) {
                return Err(ALREADY_LISTED);
            }
        }
    }
    canonical.to_str().map(str::to_string).ok_or(BAD_PATH)
}

pub async fn list(state: &AppState) -> Vec<RootInfo> {
    describe(&state.config.read().await.tools.filesystem.roots)
}

pub async fn add(
    state: &Arc<AppState>,
    path: &str,
    access: RootAccess,
) -> Result<Vec<RootInfo>, String> {
    let keepout = Keepout::current();
    let mut next = state.config.read().await.tools.filesystem.roots.clone();
    let stored = check_new_root(path, &next, &keepout).map_err(str::to_string)?;
    info!(path = %stored, access = access.as_str(), "Added an allowed folder");
    next.push(RootConfig {
        path: stored,
        access,
    });
    apply(state, next, None).await
}

pub async fn set_access(
    state: &Arc<AppState>,
    path: &str,
    access: RootAccess,
) -> Result<Vec<RootInfo>, String> {
    let mut next = state.config.read().await.tools.filesystem.roots.clone();
    let root = next
        .iter_mut()
        .find(|root| root.path == path)
        .ok_or(UNKNOWN_FOLDER)?;
    root.access = access;
    info!(
        path,
        access = access.as_str(),
        "Changed the access of an allowed folder"
    );
    apply(state, next, None).await
}

pub async fn remove(state: &Arc<AppState>, path: &str) -> Result<Vec<RootInfo>, String> {
    let mut next = state.config.read().await.tools.filesystem.roots.clone();
    let position = next
        .iter()
        .position(|root| root.path == path)
        .ok_or(UNKNOWN_FOLDER)?;
    next.remove(position);
    info!(path, "Removed an allowed folder");
    let removed = Path::new(path).canonicalize().ok();
    apply(state, next, removed).await
}

/// Saves the list, rebuilds the file and command tools, and tells the Worker.
async fn apply(
    state: &Arc<AppState>,
    next: Vec<RootConfig>,
    removed: Option<PathBuf>,
) -> Result<Vec<RootInfo>, String> {
    let (roots, descriptors) = {
        let mut config = state.config.write().await;
        Config::update_filesystem_roots(&state.config_path, &next).map_err(|error| {
            warn!("Could not save the allowed folders: {error}");
            NOT_SAVED.to_string()
        })?;
        config.tools.filesystem.roots = next;
        let registry = state.providers.current().rebuilt(&config.tools);
        let descriptors = registry.descriptors().to_vec();
        state.providers.replace(registry);
        let supervisor = state.supervisor.lock().await.clone();
        if let Some(supervisor) = supervisor {
            supervisor.update_tools(descriptors.clone()).await;
        }
        (describe(&config.tools.filesystem.roots), descriptors)
    };
    info!(
        tools = descriptors.len(),
        "Rebuilt the file and command tools"
    );
    if let Some(directory) = removed {
        state
            .policy
            .authorizer
            .grant_store()
            .lock()
            .await
            .remove_paths_inside(&directory);
    }
    crate::tool_approval::deny_stale_approvals(state).await;
    Ok(roots)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keepout(home: &Path, app_dir: &Path) -> Keepout {
        Keepout {
            home: Some(home.canonicalize().unwrap()),
            app_dir: app_dir.canonicalize().unwrap(),
        }
    }

    struct Layout {
        _dir: tempfile::TempDir,
        home: PathBuf,
        app: PathBuf,
        docs: PathBuf,
    }

    fn layout() -> Layout {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let home = base.join("home");
        let app = home.join("app");
        let docs = home.join("docs");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::create_dir_all(&docs).unwrap();
        Layout {
            _dir: dir,
            home,
            app,
            docs,
        }
    }

    fn entry(path: &Path) -> RootConfig {
        RootConfig {
            path: path.display().to_string(),
            access: RootAccess::Read,
        }
    }

    #[test]
    fn a_folder_inside_home_is_accepted_with_its_resolved_path() {
        let l = layout();
        let stored = check_new_root(l.docs.to_str().unwrap(), &[], &keepout(&l.home, &l.app));
        assert_eq!(stored.unwrap(), l.docs.display().to_string());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_resolved_before_it_is_stored() {
        let l = layout();
        let link = l.home.join("link");
        std::os::unix::fs::symlink(&l.docs, &link).unwrap();
        let stored = check_new_root(link.to_str().unwrap(), &[], &keepout(&l.home, &l.app));
        assert_eq!(stored.unwrap(), l.docs.display().to_string());
    }

    #[test]
    fn home_and_the_folders_above_it_are_refused() {
        let l = layout();
        let keep = keepout(&l.home, &l.app);
        for path in [
            l.home.clone(),
            l.home.parent().unwrap().to_path_buf(),
            PathBuf::from("/"),
        ] {
            assert_eq!(
                check_new_root(path.to_str().unwrap(), &[], &keep),
                Err(HOME_OR_ABOVE),
                "{}",
                path.display()
            );
        }
    }

    #[test]
    fn the_app_folder_and_its_neighbors_in_the_chain_are_refused() {
        let l = layout();
        let inside = l.app.join("sub");
        std::fs::create_dir_all(&inside).unwrap();
        let keep = keepout(&l.home, &l.app);
        assert_eq!(
            check_new_root(l.app.to_str().unwrap(), &[], &keep),
            Err(APP_FOLDER)
        );
        assert_eq!(
            check_new_root(inside.to_str().unwrap(), &[], &keep),
            Err(APP_FOLDER)
        );
    }

    #[test]
    fn a_missing_path_a_file_and_a_relative_path_are_refused() {
        let l = layout();
        let keep = keepout(&l.home, &l.app);
        let file = l.docs.join("a.txt");
        std::fs::write(&file, "x").unwrap();
        assert_eq!(
            check_new_root(l.home.join("nope").to_str().unwrap(), &[], &keep),
            Err(NOT_A_FOLDER)
        );
        assert_eq!(
            check_new_root(file.to_str().unwrap(), &[], &keep),
            Err(NOT_A_FOLDER)
        );
        assert_eq!(check_new_root("docs", &[], &keep), Err(NOT_ABSOLUTE));
        assert_eq!(check_new_root("", &[], &keep), Err(BAD_PATH));
        assert_eq!(
            check_new_root(&format!("{}/../docs", l.docs.display()), &[], &keep),
            Err(HAS_PARENT)
        );
    }

    #[test]
    fn an_overlapping_folder_is_refused_in_both_directions() {
        let l = layout();
        let keep = keepout(&l.home, &l.app);
        let child = l.docs.join("child");
        std::fs::create_dir_all(&child).unwrap();
        assert_eq!(
            check_new_root(child.to_str().unwrap(), &[entry(&l.docs)], &keep),
            Err(ALREADY_LISTED)
        );
        assert_eq!(
            check_new_root(l.docs.to_str().unwrap(), &[entry(&child)], &keep),
            Err(ALREADY_LISTED)
        );
        assert_eq!(
            check_new_root(l.docs.to_str().unwrap(), &[entry(&l.docs)], &keep),
            Err(ALREADY_LISTED)
        );
    }

    #[test]
    fn the_list_has_a_limit() {
        let l = layout();
        let keep = keepout(&l.home, &l.app);
        let full: Vec<RootConfig> = (0..MAX_ROOTS)
            .map(|index| entry(&PathBuf::from(format!("/nowhere/{index}"))))
            .collect();
        assert_eq!(
            check_new_root(l.docs.to_str().unwrap(), &full, &keep),
            Err(TOO_MANY)
        );
    }

    #[test]
    fn an_unusable_folder_stays_in_the_list_with_a_reason() {
        let l = layout();
        let gone = l.home.join("gone");
        let listed = describe(&[
            entry(&l.docs),
            entry(&gone),
            RootConfig {
                path: "relative".to_string(),
                access: RootAccess::ReadWrite,
            },
        ]);
        assert!(listed[0].usable && listed[0].reason.is_none());
        assert!(!listed[1].usable);
        assert_eq!(
            listed[1].reason.as_deref(),
            Some("The folder does not exist")
        );
        assert_eq!(
            listed[2].reason.as_deref(),
            Some("The path is not absolute")
        );
        assert_eq!(listed[2].access, RootAccess::ReadWrite);
    }
}
