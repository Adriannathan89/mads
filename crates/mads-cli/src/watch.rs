use std::{
    ffi::OsStr,
    path::{Component, Path, PathBuf},
};

use notify::{Event, EventKind, RecursiveMode, Watcher};

use crate::{
    diagnostic::{CliError, MADS220},
    project::{CargoProject, ResolvedApplication},
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ChangeImpact {
    None,
    Restart,
    Rebuild,
}

impl ChangeImpact {
    pub(crate) fn merge(impacts: impl IntoIterator<Item = Self>) -> Self {
        impacts.into_iter().max().unwrap_or(Self::None)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WatchRoot {
    path: PathBuf,
    recursive: bool,
}

impl WatchRoot {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) const fn recursive(&self) -> bool {
        self.recursive
    }
}

pub(crate) struct WatchSet {
    workspace_root: PathBuf,
    package_root: PathBuf,
    local_roots: Vec<PathBuf>,
    excluded_package_roots: Vec<PathBuf>,
    notify_roots: Vec<WatchRoot>,
}

impl WatchSet {
    pub(crate) fn for_application(
        project: &CargoProject,
        application: &ResolvedApplication,
    ) -> Self {
        let workspace_root = canonical_path(application.package().workspace_root());
        let package_root = canonical_path(application.package().package_root());
        let local_roots = project.local_watch_roots(application);
        let excluded_package_roots = project
            .local_package_roots()
            .into_iter()
            .filter(|root| !local_roots.contains(root))
            .collect::<Vec<_>>();
        let mut notify_roots = local_roots
            .iter()
            .cloned()
            .map(|path| WatchRoot {
                path,
                recursive: true,
            })
            .collect::<Vec<_>>();

        if !local_roots
            .iter()
            .any(|root| workspace_root.starts_with(root))
        {
            notify_roots.push(WatchRoot {
                path: workspace_root.clone(),
                recursive: false,
            });
        }
        notify_roots.sort_by(|left, right| left.path.cmp(&right.path));

        Self {
            workspace_root,
            package_root,
            local_roots,
            excluded_package_roots,
            notify_roots,
        }
    }

    pub(crate) fn roots(&self) -> &[WatchRoot] {
        &self.notify_roots
    }

    pub(crate) fn classify(&self, path: impl AsRef<Path>) -> ChangeImpact {
        let path = path.as_ref();
        if is_ignored_path(path) || self.owner_is_excluded(path) {
            return ChangeImpact::None;
        }

        if path.parent() == Some(self.package_root.as_path())
            && matches!(path.file_name(), Some(name) if name == OsStr::new(".env") || name == OsStr::new("mads.toml"))
        {
            return ChangeImpact::Restart;
        }

        if self.is_rebuild_path(path) {
            ChangeImpact::Rebuild
        } else {
            ChangeImpact::None
        }
    }

    pub(crate) fn classify_event(&self, event: &Event) -> ChangeImpact {
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                ChangeImpact::merge(event.paths.iter().map(|path| self.classify(path)))
            }
            EventKind::Access(_) | EventKind::Other | EventKind::Any => ChangeImpact::None,
        }
    }

    fn owner_is_excluded(&self, path: &Path) -> bool {
        self.owning_root(path)
            .is_some_and(|(_, reachable)| !reachable)
    }

    fn owning_root(&self, path: &Path) -> Option<(&Path, bool)> {
        self.local_roots
            .iter()
            .map(|root| (root.as_path(), true))
            .chain(
                self.excluded_package_roots
                    .iter()
                    .map(|root| (root.as_path(), false)),
            )
            .filter(|(root, _)| path.starts_with(root))
            .max_by_key(|(root, _)| root.components().count())
    }

    fn is_rebuild_path(&self, path: &Path) -> bool {
        if path == self.workspace_root.join("Cargo.toml")
            || path == self.workspace_root.join("Cargo.lock")
        {
            return true;
        }

        let is_reachable_source = self
            .owning_root(path)
            .is_some_and(|(_, reachable)| reachable);
        let is_rust_source = path.extension() == Some(OsStr::new("rs"));
        let is_package_manifest = path.file_name() == Some(OsStr::new("Cargo.toml"))
            && self
                .owning_root(path)
                .is_some_and(|(root, reachable)| reachable && path.parent() == Some(root));
        let is_selected_migration = path.starts_with(self.package_root.join("migrations"));
        let is_selected_schema = path == self.package_root.join("src/schema.rs")
            || path.starts_with(self.package_root.join("src/schema"));

        (is_reachable_source && is_rust_source)
            || is_package_manifest
            || is_selected_migration
            || is_selected_schema
    }
}

pub(crate) struct WatchEvents {
    _watcher: notify::RecommendedWatcher,
    receiver: tokio::sync::mpsc::UnboundedReceiver<Result<Event, notify::Error>>,
}

impl WatchEvents {
    pub(crate) fn start(watch_set: &WatchSet) -> Result<Self, CliError> {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        })
        .map_err(watch_error)?;

        for root in watch_set.roots() {
            let mode = if root.recursive() {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };
            watcher.watch(root.path(), mode).map_err(watch_error)?;
        }

        Ok(Self {
            _watcher: watcher,
            receiver,
        })
    }

    pub(crate) async fn recv(&mut self) -> Result<Event, CliError> {
        match self.receiver.recv().await {
            Some(Ok(event)) => Ok(event),
            Some(Err(error)) => Err(watch_error(error)),
            None => Err(CliError::new(
                MADS220,
                "File watcher failed",
                "the watch event channel closed unexpectedly",
            )),
        }
    }
}

fn is_ignored_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(component, Component::Normal(name) if name == OsStr::new("target") || name == OsStr::new(".git"))
    }) || path.file_name().is_some_and(|name| {
        name == OsStr::new(".DS_Store")
            || ["~", ".swp", ".swx", ".tmp"]
                .iter()
                .any(|suffix| name.to_string_lossy().ends_with(suffix))
    })
}

fn canonical_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn watch_error(error: notify::Error) -> CliError {
    CliError::new(
        MADS220,
        "File watcher failed",
        "could not watch project files",
    )
    .with_source(error)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{ChangeImpact, WatchSet};

    #[test]
    fn classifies_relevant_paths_and_ignores_generated_and_editor_paths() {
        let workspace = tempfile::tempdir().unwrap();
        let workspace_root = workspace.path().to_path_buf();
        let root = workspace_root.join("app");

        for directory in [
            root.join("src/schema"),
            root.join("migrations/1"),
            workspace_root.join("target/debug"),
            workspace_root.join(".git"),
        ] {
            fs::create_dir_all(directory).unwrap();
        }

        let watch_set = WatchSet {
            workspace_root: workspace_root.clone(),
            package_root: root.clone(),
            local_roots: vec![root.clone()],
            excluded_package_roots: Vec::new(),
            notify_roots: Vec::new(),
        };

        assert_eq!(
            watch_set.classify(root.join("src/main.rs")),
            ChangeImpact::Rebuild
        );
        assert_eq!(
            watch_set.classify(root.join("src/schema/user.rs")),
            ChangeImpact::Rebuild
        );
        assert_eq!(
            watch_set.classify(root.join("migrations/1/up.sql")),
            ChangeImpact::Rebuild
        );
        assert_eq!(
            watch_set.classify(root.join("Cargo.toml")),
            ChangeImpact::Rebuild
        );
        assert_eq!(
            watch_set.classify(workspace_root.join("Cargo.lock")),
            ChangeImpact::Rebuild
        );
        assert_eq!(
            watch_set.classify(root.join("mads.toml")),
            ChangeImpact::Restart
        );
        assert_eq!(watch_set.classify(root.join(".env")), ChangeImpact::Restart);
        assert_eq!(
            watch_set.classify(workspace_root.join("target/debug/app")),
            ChangeImpact::None
        );
        assert_eq!(
            watch_set.classify(workspace_root.join(".git/index")),
            ChangeImpact::None
        );
        assert_eq!(
            watch_set.classify(root.join("src/main.rs~")),
            ChangeImpact::None
        );
    }

    #[test]
    fn rebuild_dominates_restart_in_a_change_batch() {
        assert_eq!(
            ChangeImpact::merge([ChangeImpact::Restart, ChangeImpact::Rebuild]),
            ChangeImpact::Rebuild,
        );
    }

    #[test]
    fn excluded_nested_package_owns_its_source_path() {
        let workspace = tempfile::tempdir().unwrap();
        let workspace_root = workspace.path().to_path_buf();
        let root = workspace_root.join("app");
        let excluded = root.join("examples/tool");
        let watch_set = WatchSet {
            workspace_root,
            package_root: root.clone(),
            local_roots: vec![root],
            excluded_package_roots: vec![excluded.clone()],
            notify_roots: Vec::new(),
        };

        assert_eq!(
            watch_set.classify(excluded.join("src/main.rs")),
            ChangeImpact::None
        );
    }

    #[test]
    fn classifies_both_rename_paths_and_ignores_access_events() {
        let workspace = tempfile::tempdir().unwrap();
        let workspace_root = workspace.path().to_path_buf();
        let root = workspace_root.join("app");
        let watch_set = WatchSet {
            workspace_root,
            package_root: root.clone(),
            local_roots: vec![root.clone()],
            excluded_package_roots: Vec::new(),
            notify_roots: Vec::new(),
        };

        let rename = notify::Event {
            kind: notify::EventKind::Modify(notify::event::ModifyKind::Name(
                notify::event::RenameMode::Both,
            )),
            paths: vec![root.join(".env"), root.join("src/main.rs")],
            attrs: notify::event::EventAttributes::default(),
        };
        assert_eq!(watch_set.classify_event(&rename), ChangeImpact::Rebuild);

        let access = notify::Event::new(notify::EventKind::Access(notify::event::AccessKind::Any));
        assert_eq!(watch_set.classify_event(&access), ChangeImpact::None);
    }
}
