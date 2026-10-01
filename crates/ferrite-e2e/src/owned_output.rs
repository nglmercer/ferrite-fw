//! Explicit attempt directory reservations. No automatic deletion on Drop.
use crate::{E2eError, E2eResult};
use std::{
    fs::{self, File, Metadata},
    path::{Path, PathBuf},
    sync::Mutex,
};

#[derive(Default)]
pub(crate) struct OwnedOutputs {
    reservations: Mutex<Vec<Reservation>>,
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
impl OwnedOutputs {
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
#[cfg(test)]
mod tests {
    use super::*;
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
