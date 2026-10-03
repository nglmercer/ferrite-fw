//! Shared generation profiles and no-overwrite publication.
use ferrite_core::{FerriteError, Result};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub struct TemplateProfile {
    pub framework: &'static str,
    pub language: &'static str,
    pub rendering: &'static str,
    pub compiler_host: &'static str,
}
pub const TEMPLATES: &[TemplateProfile] = &[
    TemplateProfile {
        framework: "vanilla",
        language: "js",
        rendering: "client",
        compiler_host: "native",
    },
    TemplateProfile {
        framework: "vanilla",
        language: "ts",
        rendering: "client",
        compiler_host: "native",
    },
];

pub fn select(
    framework: &str,
    language: &str,
    rendering: &str,
) -> Result<&'static TemplateProfile> {
    crate::registry::descriptor(framework).map(|descriptor| descriptor.template_variants).unwrap_or(&[]).iter().find(|profile| profile.framework == framework && profile.language == language && profile.rendering == rendering)
        .ok_or_else(|| FerriteError::Config(format!(
            "unavailable generation profile {framework}/{language}/{rendering}; available: vanilla/js/client, vanilla/ts/client; SSR requires a validated renderer and hydration profile"
        )))
}

pub fn files(profile: &TemplateProfile, name: &str) -> Result<BTreeMap<String, String>> {
    // The public library API must validate, too; callers cannot invent profiles.
    let profile = select(profile.framework, profile.language, profile.rendering)?;
    let extension = profile.language;
    let mut files = BTreeMap::new();
    let main = if extension == "ts" {
        "const button = document.querySelector<HTMLButtonElement>('#counter');\nif (!button) throw new Error('missing #counter');\n"
    } else {
        "const button = document.querySelector('#counter');\nif (!(button instanceof HTMLButtonElement)) throw new Error('missing #counter');\n"
    };
    files.insert(format!("src/main.{extension}"), format!("import './style.css';\n{main}let count = 0;\nbutton.addEventListener('click', () => {{ count += 1; button.textContent = `count: ${{count}}`; }});\n"));
    files.insert("index.html".into(), format!("<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><link rel=\"icon\" href=\"/favicon.svg\"><title>Ferrite app</title></head><body><main id=\"app\"><h1>Hello Ferrite</h1><button id=\"counter\" type=\"button\">count: 0</button></main><script type=\"module\" src=\"/src/main.{extension}\"></script></body></html>\n"));
    files.insert("src/style.css".into(), "body { font-family: system-ui; margin: 3rem; } button { padding: .5rem 1rem; cursor: pointer; }\n".into());
    files.insert(
        "ferrite.toml".into(),
        "[server]\nport = 5173\n\n[build]\nentries = [\"index.html\"]\n".into(),
    );
    files.insert("public/favicon.svg".into(), "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 32 32\"><rect width=\"32\" height=\"32\" rx=\"6\" fill=\"#983b24\"/><path d=\"M10 8h14v4H14v4h8v4h-8v6h-4z\" fill=\"white\"/></svg>\n".into());
    files.insert("package.json".into(), format!("{}\n", serde_json::to_string_pretty(&serde_json::json!({
        "name": name, "private": true, "type": "module", "scripts": {"dev":"ferrite dev", "build":"ferrite build", "preview":"ferrite preview"}, "dependencies": {}, "devDependencies": {}
    }))?));
    files.insert(
        ".gitignore".into(),
        ".ferrite/\ndist/\nnode_modules/\n.env.local\n".into(),
    );
    files.insert(
        "src/ferrite-env.d.ts".into(),
        "declare module '*.css';\n".into(),
    );
    if extension == "ts" {
        files.insert("tsconfig.json".into(), "{\n  \"compilerOptions\": {\"target\": \"ES2022\", \"module\": \"ESNext\", \"moduleResolution\": \"Bundler\", \"lib\": [\"ES2022\", \"DOM\"], \"strict\": true, \"noEmit\": true},\n  \"include\": [\"src\"]\n}\n".into());
    }
    files.insert("README.md".into(), format!("# {name}\n\nRun `ferrite install`, then `ferrite dev`. Click the counter to verify interaction.\nRun `ferrite build` and `ferrite preview` to verify production behavior.\n\nThis is a client application; it has no SSR renderer. JavaScript/TypeScript\ntranspilation uses Ferrite's native compiler. TypeScript transpilation does not\nperform type checking; the tsconfig provides editor settings.\n"));
    Ok(files)
}

/// Anchored parent directory; publication never replaces an existing destination.
pub struct CreationTarget {
    pub path: PathBuf,
    name: std::ffi::OsString,
    parent: std::fs::File,
}
impl CreationTarget {
    pub fn new(path: &Path) -> Result<Self> {
        if path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(FerriteError::Config(
                "app directory must not contain '..'".into(),
            ));
        }
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        let name = absolute
            .file_name()
            .ok_or_else(|| FerriteError::Config("app directory needs a final name".into()))?
            .to_os_string();
        let parent_path = absolute
            .parent()
            .ok_or_else(|| FerriteError::Config("app directory needs a parent".into()))?;
        let mut current = PathBuf::new();
        for part in parent_path.components() {
            current.push(part.as_os_str());
            if std::fs::symlink_metadata(&current)?
                .file_type()
                .is_symlink()
            {
                return Err(FerriteError::Config(format!(
                    "refusing symlink ancestor `{}`",
                    current.display()
                )));
            }
        }
        match std::fs::symlink_metadata(&absolute) {
            Ok(_) => {
                return Err(FerriteError::Config(format!(
                    "`{}` already exists",
                    absolute.display()
                )))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let parent = {
            use rustix::fs::{open, openat, Mode, OFlags};
            let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
            let mut fd = open("/", flags, Mode::empty()).map_err(std::io::Error::from)?;
            for component in parent_path.components() {
                if let Component::Normal(name) = component {
                    fd = openat(&fd, name, flags, Mode::empty()).map_err(std::io::Error::from)?;
                }
            }
            std::fs::File::from(fd)
        };
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let parent = std::fs::File::open(parent_path)?;
        Ok(Self {
            path: absolute,
            name,
            parent,
        })
    }
    fn verify_parent(&self) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let actual = std::fs::symlink_metadata(self.path.parent().unwrap())?;
            let held = self.parent.metadata()?;
            if actual.file_type().is_symlink()
                || actual.dev() != held.dev()
                || actual.ino() != held.ino()
            {
                return Err(FerriteError::Config("app parent directory changed during generation; retry with a stable destination".into()));
            }
        }
        Ok(())
    }
    pub fn stage(&self) -> Result<tempfile::TempDir> {
        self.verify_parent()?;
        #[cfg(target_os = "linux")]
        let parent = {
            use std::os::fd::AsRawFd;
            PathBuf::from(format!("/proc/self/fd/{}", self.parent.as_raw_fd()))
        };
        #[cfg(not(target_os = "linux"))]
        let parent = self.path.parent().unwrap().to_path_buf();
        Ok(tempfile::Builder::new()
            .prefix(".ferrite-create-")
            .tempdir_in(parent)?)
    }
    pub fn publish(&self, stage: &tempfile::TempDir) -> Result<()> {
        self.verify_parent()?;
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            rustix::fs::renameat_with(
                &self.parent,
                stage.path().file_name().unwrap(),
                &self.parent,
                &self.name,
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(|error| {
                FerriteError::Other(format!(
                    "cannot publish `{}` without overwriting: {error}",
                    self.path.display()
                ))
            })?;
            Ok(())
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = stage;
            Err(FerriteError::Config(
                "atomic no-overwrite project publication is unavailable on this platform".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profiles_fail_before_writing() {
        for (framework, language, rendering) in [
            ("vanila", "ts", "client"),
            ("vue", "ts", "client"),
            ("vanilla", "rust", "client"),
            ("vanilla", "ts", "ssr"),
        ] {
            assert!(select(framework, language, rendering).is_err());
        }
        for language in ["js", "ts"] {
            let profile = select("vanilla", language, "client").unwrap();
            let files = files(profile, "example").unwrap();
            assert!(files.contains_key(&format!("src/main.{language}")));
            assert_eq!(files.contains_key("tsconfig.json"), language == "ts");
            assert!(files["index.html"].contains("id=\"counter\""));
            assert!(files[&format!("src/main.{language}")].contains("addEventListener"));
        }
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn publication_is_atomic_and_never_overwrites_a_raced_destination() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("app");
        let target = CreationTarget::new(&destination).unwrap();
        let stage = target.stage().unwrap();
        std::fs::write(stage.path().join("ready"), "complete").unwrap();
        assert!(!destination.exists());
        std::fs::create_dir(&destination).unwrap();
        assert!(target.publish(&stage).is_err());
        assert!(std::fs::read_dir(&destination).unwrap().next().is_none());
        assert!(stage.path().join("ready").exists());
        let other = root.path().join("complete");
        let target = CreationTarget::new(&other).unwrap();
        let stage = target.stage().unwrap();
        std::fs::write(stage.path().join("ready"), "complete").unwrap();
        target.publish(&stage).unwrap();
        assert_eq!(
            std::fs::read_to_string(other.join("ready")).unwrap(),
            "complete"
        );
        assert!(CreationTarget::new(&other).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn replacing_the_parent_with_a_symlink_cannot_redirect_staged_writes() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let parent = root.path().join("parent");
        std::fs::create_dir(&parent).unwrap();
        let target = CreationTarget::new(&parent.join("app")).unwrap();
        let stage = target.stage().unwrap();
        std::fs::rename(&parent, root.path().join("moved")).unwrap();
        symlink(outside.path(), &parent).unwrap();
        std::fs::write(stage.path().join("anchored"), "safe").unwrap();
        assert!(target.publish(&stage).is_err());
        assert!(target.stage().is_err());
        assert!(std::fs::read_dir(outside.path()).unwrap().next().is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unsafe_paths_and_symlinks_are_rejected_and_abandoned_staging_is_removed() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), root.path().join("linked")).unwrap();
        assert!(CreationTarget::new(&root.path().join("linked/app")).is_err());
        symlink(outside.path().join("absent"), root.path().join("dangling")).unwrap();
        assert!(CreationTarget::new(&root.path().join("dangling")).is_err());
        assert!(CreationTarget::new(&root.path().join("../escape")).is_err());
        let target = CreationTarget::new(&root.path().join("app")).unwrap();
        let stage = target.stage().unwrap();
        let path = stage.path().to_path_buf();
        std::fs::write(path.join("partial"), "partial").unwrap();
        drop(stage);
        assert!(!path.exists());
        assert!(!target.path.exists());
        assert!(std::fs::read_dir(outside.path()).unwrap().next().is_none());
    }
}
