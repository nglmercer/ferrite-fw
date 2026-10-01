//! Explicit attempt directory reservations. No automatic deletion on Drop.
use crate::{E2eError, E2eResult};
use cap_fs_ext::DirExt;
use cap_std::fs::Dir;
use std::{
    fs::{self, File, Metadata},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, Weak},
};

#[derive(Default)]
pub(crate) struct OwnedOutputs {
    reservations: Mutex<Vec<Reservation>>,
    protected: Mutex<std::collections::HashSet<PathBuf>>,
}
struct Reservation {
    container: PathBuf,
    directory: PathBuf,
    parent: PathBuf,
    parent_handle: File,
    container_handle: File,
    directory_handle: File,
}
fn open_directory(path: &Path) -> std::io::Result<File> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0x1 | 0x2); // do not allow concurrent directory rename/deletion
        options.custom_flags(0x02000000); // FILE_FLAG_BACKUP_SEMANTICS
    }
    options.open(path)
}
fn same_directory(left: &Metadata, right: &Metadata) -> bool {
    if !left.is_dir() || !right.is_dir() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    {
        left.created().ok() == right.created().ok()
    }
}
impl Reservation {
    fn verify(&self) -> E2eResult<()> {
        for (path, handle) in [
            (&self.parent, &self.parent_handle),
            (&self.container, &self.container_handle),
            (&self.directory, &self.directory_handle),
        ] {
            let actual = fs::symlink_metadata(path)?;
            if actual.file_type().is_symlink() || !same_directory(&actual, &handle.metadata()?) {
                return Err(E2eError::Config(format!(
                    "owned output directory identity changed: {}",
                    path.display()
                )));
            }
        }
        Ok(())
    }
}
static ACTIVE_OUTPUTS: OnceLock<Mutex<Vec<Weak<OwnedOutputs>>>> = OnceLock::new();

/// Snapshot path resolution can run on an awaited spawned task without inheriting
/// task locals. Weak registration protects its baseline without retaining a run,
/// browser, attempt or directory handle after the actual owner drops.
pub(crate) fn protect_baseline(path: &Path) {
    let Some(active) = ACTIVE_OUTPUTS.get() else {
        return;
    };
    let outputs: Vec<_> = active
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .filter_map(Weak::upgrade)
        .collect();
    for outputs in outputs {
        outputs.protect(path);
    }
}
impl Drop for OwnedOutputs {
    fn drop(&mut self) {
        if let Some(active) = ACTIVE_OUTPUTS.get() {
            active
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|owner| owner.strong_count() != 0);
        }
    }
}
impl OwnedOutputs {
    pub(crate) fn register(self: Arc<Self>) -> Arc<Self> {
        let mut active = ACTIVE_OUTPUTS
            .get_or_init(Mutex::default)
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        active.retain(|owner| owner.strong_count() != 0);
        active.push(Arc::downgrade(&self));
        drop(active);
        self
    }
    /// Protect the lexical path and its current resolved target. Missing paths
    /// resolve against the nearest existing ancestor, preserving alias roots.
    pub(crate) fn protect(&self, path: &Path) {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            match std::env::current_dir() {
                Ok(root) => root.join(path),
                Err(_) => return,
            }
        };
        let mut normalized = PathBuf::new();
        for part in absolute.components() {
            match part {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    normalized.pop();
                }
                other => normalized.push(other.as_os_str()),
            }
        }
        let mut candidates = vec![normalized.clone()];
        let mut ancestor = normalized.as_path();
        while fs::canonicalize(ancestor).is_err() {
            let Some(parent) = ancestor.parent() else {
                return;
            };
            ancestor = parent;
        }
        if let Ok(resolved) = fs::canonicalize(ancestor) {
            if let Ok(suffix) = normalized.strip_prefix(ancestor) {
                candidates.push(resolved.join(suffix));
            }
        }
        let reservations = self.reservations.lock().unwrap_or_else(|e| e.into_inner());
        if candidates.iter().any(|candidate| {
            reservations
                .iter()
                .any(|reservation| candidate.starts_with(&reservation.directory))
        }) {
            self.protected
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .extend(candidates);
        }
    }

    pub(crate) fn is_protected(&self, path: &Path) -> bool {
        self.protected
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .any(|baseline| path.starts_with(baseline))
    }

    /// Clean only a directory actually reserved by this registry. Baselines and
    /// their parent directories survive; links are unlinked, never traversed.
    pub(crate) fn cleanup(&self, directory: &Path) -> E2eResult<usize> {
        let reservations = self.reservations.lock().unwrap_or_else(|e| e.into_inner());
        let reservation = reservations
            .iter()
            .find(|r| r.directory == directory)
            .ok_or_else(|| {
                E2eError::Config(format!("unowned attempt output: {}", directory.display()))
            })?;
        reservation.verify()?;
        let protected = self.protected.lock().unwrap_or_else(|e| e.into_inner());
        let protected: Vec<_> = protected
            .iter()
            .filter_map(|path| path.strip_prefix(directory).ok().map(Path::to_path_buf))
            .collect();
        let dir = Dir::from_std_file(reservation.directory_handle.try_clone()?);
        remove_contents(&dir, Path::new(""), &protected, 0)?;
        if protected.is_empty() {
            reservation.verify()?;
            let container = Dir::from_std_file(reservation.container_handle.try_clone()?);
            container.remove_dir(directory.file_name().unwrap())?;
            let parent = Dir::from_std_file(reservation.parent_handle.try_clone()?);
            // A caller-created sibling within the private container is not ours.
            match parent.remove_dir(reservation.container.file_name().unwrap()) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(protected.len())
    }

    /// Create a unique container and a familiar attempt leaf. Separate attempts,
    /// duplicate test names, repeated runs and overlapping project roots never
    /// acquire ownership by inferring a filename or reusing an existing directory.
    pub(crate) fn reserve(&self, root: &Path, leaf: &str) -> E2eResult<PathBuf> {
        if leaf.is_empty()
            || Path::new(leaf).components().count() != 1
            || !matches!(
                Path::new(leaf).components().next(),
                Some(std::path::Component::Normal(_))
            )
        {
            return Err(E2eError::Config(
                "owned output leaf must be one normal path component".into(),
            ));
        }
        fs::create_dir_all(root)?;
        let parent = fs::canonicalize(root)?;
        let parent_handle = open_directory(&parent)?;
        let temporary = tempfile::Builder::new()
            .prefix(".ferrite-attempt-")
            .tempdir_in(&parent)?;
        let container = temporary.path().to_path_buf();
        let container_handle = open_directory(&container)?;
        let directory = container.join(leaf);
        fs::create_dir(&directory)?;
        let directory_handle = open_directory(&directory)?;
        let reservation = Reservation {
            container,
            directory: directory.clone(),
            parent,
            parent_handle,
            container_handle,
            directory_handle,
        };
        reservation.verify()?;
        // Ownership is retained by the run registry. Drop must not delete outputs
        // before reporters, export or future retention classification completes.
        let _ = temporary.keep();
        self.reservations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(reservation);
        Ok(directory)
    }
}
fn remove_contents(
    dir: &Dir,
    relative: &Path,
    protected: &[PathBuf],
    depth: usize,
) -> E2eResult<()> {
    if depth > 128 {
        return Err(E2eError::Config(
            "owned output cleanup nesting limit exceeded".into(),
        ));
    }
    for entry in dir.entries()? {
        let entry = entry?;
        let name = entry.file_name();
        let path = relative.join(&name);
        if protected.iter().any(|baseline| path.starts_with(baseline)) {
            continue;
        }
        let metadata = dir.symlink_metadata(&name)?;
        if metadata.is_dir() {
            let child = dir.open_dir_nofollow(&name)?;
            remove_contents(&child, &path, protected, depth + 1)?;
            if !protected.iter().any(|baseline| baseline.starts_with(&path)) {
                dir.remove_dir(&name)?;
            }
        } else if !protected.iter().any(|baseline| baseline.starts_with(&path)) {
            // A symlink may be an ancestor of a caller baseline. Preserve the
            // link itself as well as its target; cleanup never traverses it.
            dir.remove_file_or_symlink(&name)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_preserves_baselines_and_other_reservations() {
        let root = tempfile::tempdir().unwrap();
        let outputs = OwnedOutputs::default();
        let one = outputs.reserve(root.path(), "one").unwrap();
        let other = outputs.reserve(root.path(), "one").unwrap();
        fs::create_dir(one.join("nested")).unwrap();
        fs::write(one.join("nested/baseline.snap"), "baseline").unwrap();
        fs::write(one.join("nested/artifact"), "delete").unwrap();
        fs::write(other.join("caller"), "preserve").unwrap();
        outputs.protect(&one.join("nested/baseline.snap"));
        assert_eq!(outputs.cleanup(&one).unwrap(), 1);
        assert_eq!(
            fs::read_to_string(one.join("nested/baseline.snap")).unwrap(),
            "baseline"
        );
        assert!(!one.join("nested/artifact").exists());
        assert!(other.join("caller").exists());
        assert!(outputs.cleanup(root.path()).is_err());
        outputs.cleanup(&other).unwrap();
        assert!(!other.exists());
    }
    #[cfg(unix)]
    #[test]
    fn cleanup_preserves_symlink_ancestors_of_caller_baselines() {
        let root = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let outputs = OwnedOutputs::default();
        let owned = outputs.reserve(root.path(), "one").unwrap();
        std::os::unix::fs::symlink(external.path(), owned.join("baselines")).unwrap();
        fs::write(external.path().join("reference.snap"), "baseline").unwrap();
        fs::write(owned.join("artifact"), "delete").unwrap();
        outputs.protect(&owned.join("baselines/reference.snap"));
        outputs.cleanup(&owned).unwrap();
        assert!(owned.join("baselines/reference.snap").is_file());
        assert!(!owned.join("artifact").exists());
    }
    #[cfg(unix)]
    #[test]
    fn cleanup_unlinks_external_links_and_refuses_replaced_owned_roots() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        fs::write(external.path().join("caller"), "preserve").unwrap();
        let outputs = OwnedOutputs::default();
        let one = outputs.reserve(root.path(), "one").unwrap();
        symlink(external.path(), one.join("outside")).unwrap();
        outputs.cleanup(&one).unwrap();
        assert!(external.path().join("caller").is_file());
        let replaced = outputs.reserve(root.path(), "replaced").unwrap();
        fs::rename(&replaced, replaced.with_file_name("original")).unwrap();
        symlink(external.path(), &replaced).unwrap();
        assert!(outputs.cleanup(&replaced).is_err());
        assert!(external.path().join("caller").is_file());
    }
    #[test]
    fn repeated_names_and_runs_reserve_distinct_persistent_directories() {
        let root = tempfile::tempdir().unwrap();
        let first = OwnedOutputs::default();
        let second = OwnedOutputs::default();
        let paths = [
            first.reserve(root.path(), "same-attempt1").unwrap(),
            first.reserve(root.path(), "same-attempt1").unwrap(),
            second.reserve(root.path(), "same-attempt1").unwrap(),
        ];
        assert!(paths
            .iter()
            .all(|p| p.file_name().unwrap() == "same-attempt1"));
        assert_ne!(paths[0], paths[1]);
        assert_ne!(paths[1], paths[2]);
        for (index, path) in paths.iter().enumerate() {
            fs::write(path.join("marker"), index.to_string()).unwrap();
        }
        drop(first);
        drop(second);
        for (index, path) in paths.iter().enumerate() {
            assert_eq!(
                fs::read_to_string(path.join("marker")).unwrap(),
                index.to_string()
            );
        }
    }
    #[test]
    fn reservations_reject_escape_and_detect_replacement() {
        let root = tempfile::tempdir().unwrap();
        let outputs = OwnedOutputs::default();
        for leaf in ["", "..", "../outside", "/absolute", "a/b"] {
            assert!(outputs.reserve(root.path(), leaf).is_err());
        }
        let path = outputs.reserve(root.path(), "one").unwrap();
        fs::rename(&path, path.with_file_name("moved")).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(outputs.reservations.lock().unwrap()[0].verify().is_err());
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_roots_are_canonicalized_and_replaced_leaf_links_are_rejected() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(root.path(), outside.path().join("alias")).unwrap();
        let outputs = OwnedOutputs::default();
        let path = outputs
            .reserve(&outside.path().join("alias"), "one")
            .unwrap();
        assert!(path.starts_with(fs::canonicalize(root.path()).unwrap()));
        fs::remove_dir(&path).unwrap();
        symlink(outside.path(), &path).unwrap();
        fs::write(outside.path().join("caller"), "preserve").unwrap();
        assert!(outputs.reservations.lock().unwrap()[0].verify().is_err());
        drop(outputs);
        assert_eq!(
            fs::read_to_string(outside.path().join("caller")).unwrap(),
            "preserve"
        );
    }
}
