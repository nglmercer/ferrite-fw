//! Stage baseline bytes without retaining a native owner. Only the awaiting
//! foreground assertion may install a staged file at the baseline path.
use std::{future::Future, path::PathBuf, sync::Arc};

use crate::{operation::Deadline, E2eError, E2eResult, SnapshotOptions, SnapshotUpdate};

fn expired() -> E2eError {
    E2eError::Expect("screenshot baseline update expired before it could commit".into())
}

fn within<T>(
    deadline: Deadline,
    future: impl Future<Output = E2eResult<T>>,
) -> impl Future<Output = E2eResult<T>> {
    let future = Box::pin(future);
    async move {
        if deadline.expired() {
            return Err(expired());
        }
        let result = deadline
            .run("screenshot baseline update", async { Ok(future.await) })
            .await;
        if deadline.expired() {
            return Err(expired());
        }
        match result {
            Ok(result) => result,
            Err(E2eError::Timeout(..)) => Err(expired()),
            Err(error) => Err(error),
        }
    }
}

pub(crate) async fn finish(
    deadline: Deadline,
    path: PathBuf,
    actual: Arc<Vec<u8>>,
    expected: Option<Arc<Vec<u8>>>,
    mode: SnapshotUpdate,
    options: &SnapshotOptions,
) -> E2eResult<()> {
    if deadline.expired() {
        return Err(expired());
    }
    let missing = expected.is_none();
    let tolerances = (
        options.threshold,
        options.max_diff_pixels,
        options.max_diff_ratio,
    );
    if missing && mode == SnapshotUpdate::None {
        return Err(E2eError::Expect(
            "no screenshot baseline (update=none)".into(),
        ));
    }
    let write = match mode {
        SnapshotUpdate::None => false,
        SnapshotUpdate::Missing => missing,
        SnapshotUpdate::All => true,
        SnapshotUpdate::Changed => match expected {
            None => true,
            Some(expected) => !matches(deadline, actual.clone(), expected, tolerances).await?,
        },
    };
    if !write {
        return Ok(());
    }
    let destination = path.clone();
    let staged_bytes = actual.clone();
    let no_clobber = mode == SnapshotUpdate::Missing;
    let staged = within(
        deadline,
        crate::snapshot_work::run(move |stop| {
            prepare(destination, &staged_bytes, || stop.check(), no_clobber)
        }),
    )
    .await?;
    let destination = match staged {
        Prepared::Existing(path) => path,
        Prepared::File(staged) => {
            let destination = staged.destination.clone();
            if install(deadline, staged, no_clobber)? {
                return Ok(());
            }
            destination
        }
    };
    // Two Missing assertions may concurrently generate the same baseline.
    // Never overwrite the winner; accept it only after bounded validation.
    let racing = within(deadline, crate::snapshot_work::read_baseline(destination)).await?;
    if let Some(expected) = racing {
        if matches(deadline, actual, expected, tolerances).await? {
            return Ok(());
        }
    }
    Err(E2eError::Expect("screenshot baseline appeared during capture; update=missing did not overwrite the competing baseline".into()))
}

async fn matches(
    deadline: Deadline,
    actual: Arc<Vec<u8>>,
    expected: Arc<Vec<u8>>,
    (threshold, pixels, ratio): (u8, u32, f32),
) -> E2eResult<bool> {
    // SnapshotOptions.capture can own mask locators. Only data/scalars enter
    // the worker, including the exceptional Missing-install race check.
    within(
        deadline,
        crate::snapshot_work::run(move |stop| {
            match crate::snapshot::compare_png_checked(&actual, &expected, threshold, || {
                stop.check()
            }) {
                Ok(diff) => Ok(diff.passed(&SnapshotOptions {
                    threshold,
                    max_diff_pixels: pixels,
                    max_diff_ratio: ratio,
                    ..Default::default()
                })),
                Err(error) if error.code() == "FERRITE_E2E_EXPECT" => Ok(false),
                Err(error) => Err(error),
            }
        }),
    )
    .await
}

#[derive(Debug)]
struct Staged {
    file: tempfile::NamedTempFile,
    destination: PathBuf,
}

#[derive(Debug)]
enum Prepared {
    File(Staged),
    Existing(PathBuf),
}

fn destination(
    mut path: PathBuf,
    check: &mut impl FnMut() -> E2eResult<()>,
) -> E2eResult<(PathBuf, Option<std::fs::Permissions>)> {
    // Preserve terminal symlink aliases, including links to missing targets.
    // Parent aliases retain normal OS path semantics. Resolve before staging
    // so both the temporary file and installation use the target directory.
    for _ in 0..64 {
        check()?;
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_symlink() => {
                let target = std::fs::read_link(&path)?;
                path = if target.is_absolute() {
                    target
                } else {
                    path.parent()
                        .unwrap_or_else(|| std::path::Path::new(""))
                        .join(target)
                };
            }
            Ok(metadata) if metadata.is_file() => return Ok((path, Some(metadata.permissions()))),
            Ok(_) => {
                return Err(E2eError::Config(
                    "baseline update destination must resolve to a regular file".into(),
                ))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((path, None)),
            Err(error) => return Err(error.into()),
        }
    }
    Err(E2eError::Config(
        "baseline update exceeds the 64 symlink resolution limit".into(),
    ))
}

fn prepare(
    path: PathBuf,
    bytes: &[u8],
    mut check: impl FnMut() -> E2eResult<()>,
    no_clobber: bool,
) -> E2eResult<Prepared> {
    use std::io::Write;
    check()?;
    let (path, permissions) = destination(path, &mut check)?;
    if no_clobber && permissions.is_some() {
        // A readable matching winner needs no write access to its file or
        // directory, and must not inherit permissions into a throwaway temp.
        return Ok(Prepared::Existing(path));
    }
    if permissions
        .as_ref()
        .is_some_and(std::fs::Permissions::readonly)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "screenshot baseline is read-only",
        )
        .into());
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    check()?;
    std::fs::create_dir_all(parent)?;
    check()?;
    let mut builder = tempfile::Builder::new();
    builder.prefix(".ferrite-snapshot-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o666));
    }
    let mut file = builder.tempfile_in(parent)?;
    for chunk in bytes.chunks(65_536) {
        check()?;
        file.write_all(chunk)?;
    }
    check()?;
    file.flush()?;
    check()?;
    if let Some(permissions) = permissions {
        file.as_file().set_permissions(permissions)?;
    }
    check()?;
    Ok(Prepared::File(Staged {
        file,
        destination: path,
    }))
}

fn install(deadline: Deadline, staged: Staged, no_clobber: bool) -> E2eResult<bool> {
    if deadline.expired() {
        return Err(expired());
    }
    // This foreground atomic filesystem operation has no await point. Workers
    // can finish temporary writes after cancellation, but cannot install them.
    let result = if no_clobber {
        staged.file.persist_noclobber(&staged.destination)
    } else {
        staged.file.persist(&staged.destination)
    };
    match result {
        Ok(_) => Ok(true),
        Err(error) if no_clobber && error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            Ok(false)
        }
        Err(error) => Err(error.error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Cursor, time::Duration};

    fn stage(
        path: PathBuf,
        bytes: &[u8],
        check: impl FnMut() -> E2eResult<()>,
    ) -> E2eResult<Staged> {
        match prepare(path, bytes, check, false)? {
            Prepared::File(file) => Ok(file),
            Prepared::Existing(_) => unreachable!("replacement preparation always stages a file"),
        }
    }

    fn png(width: u32, green: bool) -> Arc<Vec<u8>> {
        let mut image = image::RgbaImage::from_pixel(width, 1, image::Rgba([255, 0, 0, 255]));
        if green {
            image.put_pixel(0, 0, image::Rgba([0, 255, 0, 255]));
        }
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        Arc::new(bytes.into_inner())
    }

    async fn update(
        path: PathBuf,
        actual: Arc<Vec<u8>>,
        expected: Option<Arc<Vec<u8>>>,
        mode: SnapshotUpdate,
        options: &SnapshotOptions,
    ) -> E2eResult<()> {
        finish(
            Deadline::new(Duration::from_secs(2)),
            path,
            actual,
            expected,
            mode,
            options,
        )
        .await
    }

    #[tokio::test]
    async fn update_modes_use_frozen_expected_bytes_and_preserve_matching_baselines() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("nested/baseline.png");
        let red = png(2, false);
        let green = png(2, true);
        let options = SnapshotOptions::default();
        assert_eq!(
            update(
                path.clone(),
                red.clone(),
                None,
                SnapshotUpdate::None,
                &options
            )
            .await
            .unwrap_err()
            .code(),
            "FERRITE_E2E_EXPECT"
        );
        assert!(!path.exists());
        update(
            path.clone(),
            red.clone(),
            None,
            SnapshotUpdate::Missing,
            &options,
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), *red);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let file_permissions = std::fs::metadata(&path).unwrap().permissions();
            let parent = path.parent().unwrap();
            let directory_permissions = std::fs::metadata(parent).unwrap().permissions();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o555)).unwrap();
            let result = update(
                path.clone(),
                red.clone(),
                None,
                SnapshotUpdate::Missing,
                &options,
            )
            .await;
            std::fs::set_permissions(parent, directory_permissions).unwrap();
            std::fs::set_permissions(&path, file_permissions).unwrap();
            result.unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), *red);
        }

        // Changing the disk contents after the initial read must not introduce
        // a second baseline read, even for Changed's tolerance decision.
        std::fs::write(&path, b"external change after initial read").unwrap();
        let before = std::fs::metadata(&path).unwrap();
        for mode in [
            SnapshotUpdate::None,
            SnapshotUpdate::Missing,
            SnapshotUpdate::Changed,
        ] {
            update(path.clone(), red.clone(), Some(red.clone()), mode, &options)
                .await
                .unwrap();
            assert_eq!(
                std::fs::read(&path).unwrap(),
                b"external change after initial read"
            );
            assert_eq!(
                std::fs::metadata(&path).unwrap().modified().unwrap(),
                before.modified().unwrap()
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                assert_eq!(std::fs::metadata(&path).unwrap().ino(), before.ino());
            }
        }
        update(
            path.clone(),
            green.clone(),
            Some(red.clone()),
            SnapshotUpdate::All,
            &options,
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), *green);

        std::fs::write(&path, &*red).unwrap();
        let mut old_reader = std::fs::File::open(&path).unwrap();
        let tolerant = SnapshotOptions {
            max_diff_pixels: 1,
            max_diff_ratio: 0.5,
            ..Default::default()
        };
        update(
            path.clone(),
            green.clone(),
            Some(red.clone()),
            SnapshotUpdate::Changed,
            &tolerant,
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), *red);
        update(
            path.clone(),
            green.clone(),
            Some(red),
            SnapshotUpdate::Changed,
            &options,
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), *green);
        use std::io::Read;
        let mut old_bytes = Vec::new();
        old_reader.read_to_end(&mut old_bytes).unwrap();
        assert_eq!(old_bytes, *png(2, false));
        let small = png(1, false);
        update(
            path.clone(),
            small.clone(),
            Some(green),
            SnapshotUpdate::Changed,
            &options,
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), *small);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
    }

    #[tokio::test]
    async fn corrupt_changed_baselines_and_invalid_write_destinations_are_preserved() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("baseline.png");
        let corrupt = Arc::new(b"invalid baseline".to_vec());
        std::fs::write(&path, &*corrupt).unwrap();
        let error = update(
            path.clone(),
            png(2, false),
            Some(corrupt.clone()),
            SnapshotUpdate::Changed,
            &SnapshotOptions::default(),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code(), "FERRITE_E2E_CONFIG");
        assert_eq!(std::fs::read(&path).unwrap(), *corrupt);
        update(
            path.clone(),
            png(2, false),
            Some(corrupt),
            SnapshotUpdate::All,
            &SnapshotOptions::default(),
        )
        .await
        .unwrap();
        let occupied = root.path().join("directory");
        std::fs::create_dir(&occupied).unwrap();
        assert_eq!(
            stage(occupied.clone(), b"bytes", || Ok(()))
                .unwrap_err()
                .code(),
            "FERRITE_E2E_CONFIG"
        );
        assert!(occupied.is_dir());
        #[cfg(unix)]
        {
            let link = root.path().join("link");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            let file = stage(link.clone(), b"through alias", || Ok(())).unwrap();
            install(Deadline::new(Duration::ZERO), file, false).unwrap();
            assert!(std::fs::symlink_metadata(&link).unwrap().is_symlink());
            assert_eq!(std::fs::read(&path).unwrap(), b"through alias");
            let dangling = root.path().join("dangling");
            std::os::unix::fs::symlink("future/nested/baseline.png", &dangling).unwrap();
            let file = stage(dangling.clone(), b"new target", || Ok(())).unwrap();
            install(Deadline::new(Duration::ZERO), file, true).unwrap();
            assert!(std::fs::symlink_metadata(&dangling).unwrap().is_symlink());
            assert_eq!(std::fs::read(dangling).unwrap(), b"new target");
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
            let file = stage(path.clone(), b"replacement", || Ok(())).unwrap();
            install(Deadline::new(Duration::ZERO), file, false).unwrap();
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o640
            );
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o440)).unwrap();
            assert_eq!(
                stage(path.clone(), b"bytes", || Ok(())).unwrap_err().code(),
                "FERRITE_E2E_IO"
            );
            assert_eq!(std::fs::read(&path).unwrap(), b"replacement");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn expired_install_and_missing_destination_races_never_replace_baselines() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("baseline.png");
        std::fs::write(&path, b"original").unwrap();
        let file = stage(path.clone(), b"replacement", || Ok(())).unwrap();
        let temporary = file.file.path().to_owned();
        let deadline = Deadline::new(Duration::from_millis(10));
        tokio::time::advance(Duration::from_millis(11)).await;
        assert_eq!(
            install(deadline, file, false).unwrap_err().code(),
            "FERRITE_E2E_EXPECT"
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        assert!(!temporary.exists());
        let file = stage(path.clone(), b"replacement", || Ok(())).unwrap();
        let temporary = file.file.path().to_owned();
        assert!(!install(Deadline::new(Duration::ZERO), file, true).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        assert!(!temporary.exists());
    }

    #[tokio::test]
    async fn missing_install_races_accept_matching_winners_and_preserve_other_contents() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("baseline.png");
        let red = png(2, false);
        let green = png(2, true);
        let options = SnapshotOptions::default();
        std::fs::write(&path, &*red).unwrap();
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        update(
            path.clone(),
            red.clone(),
            None,
            SnapshotUpdate::Missing,
            &options,
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            before
        );
        assert_eq!(std::fs::read(&path).unwrap(), *red);
        std::fs::write(&path, &*green).unwrap();
        let error = update(
            path.clone(),
            red.clone(),
            None,
            SnapshotUpdate::Missing,
            &options,
        )
        .await
        .unwrap_err();
        assert_eq!(error.code(), "FERRITE_E2E_EXPECT");
        assert!(error.to_string().contains("appeared during capture"));
        assert_eq!(std::fs::read(&path).unwrap(), *green);
        std::fs::write(&path, b"corrupt winner").unwrap();
        assert_eq!(
            update(path.clone(), red, None, SnapshotUpdate::Missing, &options)
                .await
                .unwrap_err()
                .code(),
            "FERRITE_E2E_CONFIG"
        );
        assert_eq!(std::fs::read(path).unwrap(), b"corrupt winner");
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn completed_stage_is_discarded_when_owner_cancels_before_foreground_install() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("baseline.png");
        std::fs::write(&path, b"original").unwrap();
        let token = crate::CancellationToken::new();
        let cancel = token.clone();
        let destination = path.clone();
        let error = token
            .run(async {
                let file = within(
                    Deadline::new(Duration::ZERO),
                    crate::snapshot_work::run(move |stop| {
                        let file = stage(destination, b"replacement", || stop.check())?;
                        cancel.cancel();
                        Ok(file)
                    }),
                )
                .await?;
                install(Deadline::new(Duration::ZERO), file, false)
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), "FERRITE_E2E_CANCELLED");
        tokio::time::timeout(Duration::from_secs(1), async {
            while std::fs::read_dir(root.path()).unwrap().count() != 1 {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"original");
    }

    #[tokio::test]
    async fn interrupted_real_chunk_write_discards_temporary_file_without_late_install() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("baseline.png");
        std::fs::write(&path, b"original").unwrap();
        let destination = path.clone();
        let (entered, entry) = tokio::sync::oneshot::channel();
        let (resume, resumed) = std::sync::mpsc::channel();
        let (finished, done) = tokio::sync::oneshot::channel();
        let mut job = Box::pin(crate::snapshot_work::run(move |stop| {
            let bytes = vec![7; 200_000];
            let mut entered = Some(entered);
            let mut checks = 0;
            let result = stage(destination, &bytes, || {
                checks += 1;
                if checks == 6 {
                    entered.take().unwrap().send(()).unwrap();
                    resumed.recv().unwrap();
                }
                stop.check()
            });
            finished.send(result.as_ref().unwrap_err().code()).unwrap();
            result
        }));
        tokio::select! {
            result=&mut job=>panic!("staging returned before its chunk barrier: {result:?}"),
            entered=entry=>entered.unwrap(),
        }
        let temporary = std::fs::read_dir(root.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|entry| entry != &path)
            .unwrap();
        assert_eq!(std::fs::metadata(&temporary).unwrap().len(), 65_536);
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        let error = within(Deadline::new(Duration::from_millis(25)), job)
            .await
            .unwrap_err();
        assert_eq!(error.code(), "FERRITE_E2E_EXPECT");
        resume.send(()).unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), done)
                .await
                .unwrap()
                .unwrap(),
            "FERRITE_E2E_CANCELLED"
        );
        assert!(!temporary.exists());
        assert_eq!(std::fs::read(path).unwrap(), b"original");
    }
}
