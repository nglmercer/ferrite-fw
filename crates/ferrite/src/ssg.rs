//! Deterministic, transactional static generation from the real SSR adapter.
use crate::{Builder, FerriteError, Result};
use ferrite_ssr::{SsrAdapter, SsrContext, SsrHttpRequest};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One materialized route, relative to the application base.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PrerenderedRoute {
    pub route: String,
    pub file: String,
    pub hash: String,
}

/// Stable generation report. Timing and machine paths do not enter the manifest.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PrerenderReport {
    pub version: u32,
    pub base: String,
    pub routes: Vec<PrerenderedRoute>,
}

fn route_files(routes: &[String]) -> Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    for route in routes {
        if !route.starts_with('/')
            || route.starts_with("//")
            || route.contains(['?', '#', '%', '\\', '\0'])
        {
            return Err(FerriteError::Build(format!("invalid prerender route `{route}`; use an absolute, unencoded path without query or fragment")));
        }
        let route = if route == "/" {
            "/"
        } else {
            route.trim_end_matches('/')
        };
        let mut segments = Vec::new();
        for segment in route
            .trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
        {
            let stem = segment
                .split('.')
                .next()
                .unwrap_or(segment)
                .to_ascii_lowercase();
            if segment == "."
                || segment == ".."
                || segment.ends_with(['.', ' '])
                || segment.contains([':', '*', '<', '>', '|', '"'])
                || [
                    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6",
                    "com7", "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7",
                    "lpt8", "lpt9",
                ]
                .contains(&stem.as_str())
            {
                return Err(FerriteError::Build(format!(
                    "unsafe or nonportable prerender path `{route}`"
                )));
            }
            segments.push(segment);
        }
        if segments.first().is_some_and(|s| {
            [
                "server",
                "assets",
                "ssr-assets",
                "standalone",
                ".ferrite",
                "prerender-manifest.json",
            ]
            .contains(s)
        }) || route.contains("//")
        {
            return Err(FerriteError::Build(format!(
                "reserved or ambiguous prerender route `{route}`"
            )));
        }
        let relative = segments.join("/");
        let file = if relative.is_empty() {
            "index.html".into()
        } else if relative.ends_with(".html") {
            relative
        } else {
            format!("{relative}/index.html")
        };
        if let Some(previous) = files.insert(route.to_string(), file.clone()) {
            if previous != file {
                return Err(FerriteError::Build("conflicting prerender routes".into()));
            }
        }
    }
    if files.is_empty() {
        return Err(FerriteError::Build(
            "prerender requires at least one route; set ssr.prerender or pass --route".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    for file in files.values() {
        if !seen.insert(file.to_ascii_lowercase()) {
            return Err(FerriteError::Build(format!(
                "prerender routes collide at `{file}`"
            )));
        }
    }
    Ok(files)
}

/// Serializes publication to one destination without overwriting another writer.
pub(crate) struct PublicationLock {
    path: PathBuf,
    file: Option<std::fs::File>,
}
impl PublicationLock {
    pub(crate) fn acquire(destination: &Path) -> Result<Self> {
        let parent = destination
            .parent()
            .ok_or_else(|| FerriteError::Build("output needs a parent directory".into()))?;
        std::fs::create_dir_all(parent)?;
        let canonical = parent
            .canonicalize()?
            .join(destination.file_name().ok_or_else(|| {
                FerriteError::Build("publication destination needs a filename".into())
            })?);
        let identity = canonical.to_string_lossy().into_owned();
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        let identity = identity.to_lowercase();
        let key = crate::Hash::of_str(&identity).0;
        let path = parent.join(format!(".ferrite-publish-{key}.lock"));
        let file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|error| FerriteError::Build(format!("cannot acquire publication lock {}: {error}; another writer may be active (remove a stale lock only after confirming no writer is running)", path.display())))?;
        Ok(Self {
            path,
            file: Some(file),
        })
    }
}
impl Drop for PublicationLock {
    fn drop(&mut self) {
        self.file.take();
        let _ = std::fs::remove_file(&self.path);
    }
}

pub(crate) fn publish(stage: &Path, destination: &Path) -> Result<()> {
    let backup = tempfile::tempdir_in(destination.parent().expect("validated parent"))?;
    let previous = backup.path().join("previous");
    let exists = match std::fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => true,
        Ok(_) => {
            return Err(FerriteError::Build(
                "publication destination must be an ordinary directory".into(),
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    if exists {
        std::fs::rename(destination, &previous)?;
    }
    if let Err(error) = std::fs::rename(stage, destination) {
        if exists {
            if let Err(restore) = std::fs::rename(&previous, destination) {
                let retained = backup.keep();
                return Err(FerriteError::Build(format!("publication failed: {error}; rollback failed: {restore}; previous output retained at {}", retained.display())));
            }
        }
        return Err(error.into());
    }
    Ok(())
}

fn copy_output(source: &Path, destination: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(source)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(FerriteError::Build(
            "prerender source must be an ordinary directory".into(),
        ));
    }
    for item in std::fs::read_dir(source)? {
        let item = item?;
        let metadata = std::fs::symlink_metadata(item.path())?;
        if metadata.file_type().is_symlink() {
            return Err(FerriteError::Build(format!(
                "prerender output cannot contain symlinks: {}",
                item.path().display()
            )));
        }
        let target = destination.join(item.file_name());
        if metadata.is_dir() {
            std::fs::create_dir(&target)?;
            copy_output(&item.path(), &target)?;
        } else if metadata.is_file() {
            std::fs::copy(item.path(), target)?;
        } else {
            return Err(FerriteError::Build(
                "prerender output must contain regular files".into(),
            ));
        }
    }
    Ok(())
}

async fn render_into(
    adapter: Arc<dyn SsrAdapter>,
    output: &Path,
    routes: BTreeMap<String, String>,
    base: &str,
) -> Result<PrerenderReport> {
    let base = crate::loader::with_trailing_slash(base);
    let mut report = PrerenderReport {
        version: 1,
        base: base.clone(),
        routes: Vec::new(),
    };
    for (route, file) in routes {
        let uri = if route == "/" {
            base.clone()
        } else {
            format!("{}{}", base, route.trim_start_matches('/'))
        };
        let response = adapter
            .render(
                SsrHttpRequest {
                    method: "GET".into(),
                    uri: uri.clone(),
                    headers: vec![("accept".into(), "text/html".into())],
                    body: Vec::new(),
                },
                SsrContext {
                    url: uri,
                    ..Default::default()
                },
            )
            .await?;
        if response.status != 200 {
            return Err(FerriteError::Build(format!("prerender `{route}` returned HTTP {}; all routes must render successfully before publication", response.status)));
        }
        if response
            .headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("set-cookie"))
        {
            return Err(FerriteError::Build(format!("prerender `{route}` sets cookies; static output cannot preserve request-specific cookies")));
        }
        if response.headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("content-type")
                && !matches!(
                    value
                        .split(';')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_ascii_lowercase()
                        .as_str(),
                    "text/html" | "application/xhtml+xml"
                )
        }) {
            return Err(FerriteError::Build(format!(
                "prerender `{route}` returned a non-HTML content type"
            )));
        }
        let html = response.into_string().await?;
        let target = output.join(&file);
        std::fs::create_dir_all(target.parent().expect("route parent"))?;
        std::fs::write(target, &html)?;
        report.routes.push(PrerenderedRoute {
            route,
            file,
            hash: crate::Hash::of_str(&html).0,
        });
    }
    std::fs::write(
        output.join("prerender-manifest.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(report)
}

fn remove_private_output(output: &Path) -> Result<()> {
    for name in ["server", "standalone"] {
        let path = output.join(name);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                std::fs::remove_dir_all(path)?
            }
            Ok(_) => std::fs::remove_file(path)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Pre-render configured routes using an explicitly supplied real renderer.
/// All output is staged; a failed route leaves the previous directory intact.
pub async fn prerender(
    adapter: Arc<dyn SsrAdapter>,
    output: &Path,
    routes: &[String],
    base: &str,
) -> Result<PrerenderReport> {
    let routes = route_files(routes)?;
    let output = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir()?.join(output)
    };
    let _lock = PublicationLock::acquire(&output)?;
    let staging = tempfile::tempdir_in(output.parent().expect("validated parent"))?;
    if output.exists() {
        copy_output(&output, staging.path())?;
    }
    let report = render_into(adapter, staging.path(), routes, base).await?;
    remove_private_output(staging.path())?;
    publish(staging.path(), &output)?;
    Ok(report)
}

impl Builder {
    /// Build client + SSR and publish deterministic public routes as one staged
    /// transaction. Private renderer code is removed from SSG output.
    pub async fn prerender(&self, routes: &[String]) -> Result<PrerenderReport> {
        let routes = route_files(routes)?;
        let output = self.config.out_dir();
        let _lock = PublicationLock::acquire(&output)?;
        let stage = tempfile::tempdir_in(output.parent().expect("validated parent"))?;
        let mut config = self.config.clone();
        config.build.out_dir = stage.path().to_string_lossy().into_owned();
        config.package.standalone = false;
        Self::new(config.clone(), self.plugins.clone())
            .build_app()
            .await?;
        let server = stage.path().join("server");
        let graph = crate::load_built_ssr_graph(&server)?;
        let styles = crate::ssr_build::built_ssr_styles(&server, &config.base)?;
        let shell = std::fs::read_to_string(stage.path().join("index.html"))?;
        let adapter = Arc::new(
            ferrite_ssr::JsSsrAdapter::from_resolved_graph(&config, graph)?.with_shell(shell),
        );
        let renderer = Arc::new(ferrite_ssr::FnAdapter::new(
            move |request, mut context: SsrContext| {
                let adapter = adapter.clone();
                let styles = styles.clone();
                async move {
                    context.preload.extend(styles);
                    adapter.render(request, context).await
                }
            },
        ));
        let report = render_into(renderer, stage.path(), routes, &config.base).await?;
        remove_private_output(stage.path())?;
        publish(stage.path(), &output)?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn deterministic_routes_preserve_assets_and_failure_rolls_back_every_page() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("dist");
        std::fs::create_dir(&out).unwrap();
        std::fs::write(out.join("asset.js"), "asset").unwrap();
        for name in ["server", "standalone"] {
            std::fs::create_dir(out.join(name)).unwrap();
            std::fs::write(out.join(name).join("private.json"), "private").unwrap();
        }
        let renderer = Arc::new(ferrite_ssr::FnAdapter::new(
            |request: SsrHttpRequest, _: SsrContext| async move {
                Ok(ferrite_ssr::SsrResponse::html(format!(
                    "<h1>{}</h1>",
                    request.uri
                )))
            },
        ));
        let routes = vec!["/z/".into(), "/".into(), "/a".into(), "/a/".into()];
        let a = prerender(renderer.clone(), &out, &routes, "/app/")
            .await
            .unwrap();
        assert!(!out.join("server").exists());
        assert!(!out.join("standalone").exists());
        let manifest = std::fs::read(out.join("prerender-manifest.json")).unwrap();
        assert_eq!(
            std::fs::read_to_string(out.join("a/index.html")).unwrap(),
            "<h1>/app/a</h1>"
        );
        assert_eq!(
            std::fs::read_to_string(out.join("asset.js")).unwrap(),
            "asset"
        );
        assert_eq!(
            a,
            prerender(renderer, &out, &routes, "/app/").await.unwrap()
        );
        assert_eq!(
            manifest,
            std::fs::read(out.join("prerender-manifest.json")).unwrap()
        );
        let bad = Arc::new(ferrite_ssr::FnAdapter::new(
            |request: SsrHttpRequest, _: SsrContext| async move {
                if request.uri.ends_with("/z") {
                    return Err(FerriteError::Ssr("failed route".into()));
                }
                Ok(ferrite_ssr::SsrResponse::html("changed"))
            },
        ));
        assert!(prerender(bad, &out, &routes, "/app/").await.is_err());
        assert_eq!(
            std::fs::read_to_string(out.join("a/index.html")).unwrap(),
            "<h1>/app/a</h1>"
        );
        assert_eq!(
            manifest,
            std::fs::read(out.join("prerender-manifest.json")).unwrap()
        );
    }
    #[test]
    fn unsafe_and_colliding_routes_fail_before_output_changes() {
        for route in [
            "relative",
            "/../x",
            "/%2e%2e/x",
            "/a?x",
            "/server/x",
            "/assets/x",
            "/a//b",
            "/CON",
            "/a\\b",
        ] {
            assert!(route_files(&[route.into()]).is_err(), "{route}");
        }
        assert!(route_files(&["/".into(), "/index.html".into()]).is_err());
        assert!(route_files(&["/About".into(), "/about".into()]).is_err());
    }
}
