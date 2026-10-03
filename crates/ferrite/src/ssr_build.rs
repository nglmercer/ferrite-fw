//! Loading emitted server chunks without source compilation.

use ferrite_core::{FerriteError, ModuleType, Result};
use ferrite_runtime::{CompiledModule, CompiledModuleGraph};
use ferrite_transform::{JsCompiler, OxcCompiler, OxcOptions, ParseRequest};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Component, Path};

/// Versioned renderer payload for embedding without a source compiler.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SsrRendererArtifact {
    pub version: u32,
    pub graph: CompiledModuleGraph,
    /// Extracted stylesheet output paths in dependency order.
    pub stylesheets: Vec<String>,
}

impl SsrRendererArtifact {
    pub const VERSION: u32 = 1;
    pub const FILE_NAME: &'static str = "renderer.json";

    /// Validate before executing or embedding an artifact.
    pub fn validate(&self) -> Result<()> {
        if self.version != Self::VERSION {
            return Err(FerriteError::Ssr(format!("unsupported SSR renderer artifact version {}; expected {}; rebuild with this Ferrite version", self.version, Self::VERSION)));
        }
        self.graph.validate()?;
        let ids: HashSet<_> = self
            .graph
            .modules
            .iter()
            .map(|module| module.id.as_str())
            .collect();
        let compiler = OxcCompiler::new(OxcOptions::default());
        for module in &self.graph.modules {
            let parsed = compiler.parse(ParseRequest {
                id: module.id.clone(),
                code: module.code.clone(),
                module_type: ModuleType::Js,
            })?;
            for import in parsed.imports {
                if !ids.contains(import.specifier.as_str()) {
                    return Err(FerriteError::Ssr(format!("SSR renderer artifact module `{}` requires missing compiled dependency `{}`; rebuild the complete renderer graph", module.id, import.specifier)));
                }
            }
        }
        for stylesheet in &self.stylesheets {
            output_id(stylesheet)?;
        }
        Ok(())
    }

    /// Decode an embedded artifact without filesystem dependencies.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let artifact: Self = serde_json::from_slice(bytes).map_err(|error| {
            FerriteError::Ssr(format!(
                "invalid SSR renderer artifact: {error}; rebuild the SSR output"
            ))
        })?;
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn read(server_dir: &Path) -> Result<Self> {
        Self::from_bytes(&std::fs::read(server_dir.join(Self::FILE_NAME))?)
    }
}

/// Materialize a validated renderer payload from final emitted chunks.
/// This uses build output, including final bundle hooks, rather than sources.
pub fn write_built_ssr_artifact(server_dir: &Path) -> Result<SsrRendererArtifact> {
    let mut graph = load_emitted_ssr_graph(server_dir)?;
    // Portable chunk URLs replace build-machine absolute source paths.
    for module in &mut graph.modules {
        module.url = Some(module.id.clone());
    }
    let manifest = ferrite_manifest::BuildManifest::read(&server_dir.join("manifest.json"))?;
    let entry = manifest
        .entries
        .values()
        .find(|entry| entry.is_entry == Some(true))
        .expect("validated single entry");
    let artifact = SsrRendererArtifact {
        version: SsrRendererArtifact::VERSION,
        graph,
        stylesheets: entry.css.clone(),
    };
    artifact.validate()?;
    std::fs::write(
        server_dir.join(SsrRendererArtifact::FILE_NAME),
        serde_json::to_vec_pretty(&artifact).map_err(FerriteError::Json)?,
    )?;
    Ok(artifact)
}

/// Load the single emitted SSR entry and its static/dynamic chunk dependencies.
/// External runtime imports require a separate host contract and fail explicitly.
/// Files and symlinks must remain inside the built server directory.
pub fn load_built_ssr_graph(server_dir: &Path) -> Result<CompiledModuleGraph> {
    if server_dir.join(SsrRendererArtifact::FILE_NAME).exists() {
        return Ok(SsrRendererArtifact::read(server_dir)?.graph);
    }
    load_emitted_ssr_graph(server_dir)
}

fn load_emitted_ssr_graph(server_dir: &Path) -> Result<CompiledModuleGraph> {
    let root = server_dir.canonicalize()?;
    let manifest = ferrite_manifest::BuildManifest::read(&root.join("manifest.json"))?;
    let entries: Vec<_> = manifest
        .entries
        .values()
        .filter(|entry| entry.is_entry == Some(true))
        .collect();
    if entries.len() != 1 {
        return Err(FerriteError::Ssr(format!(
            "built SSR requires exactly one renderer entry; manifest contains {}",
            entries.len()
        )));
    }
    let entry = output_id(&entries[0].file)?;
    let compiler = OxcCompiler::new(OxcOptions::default());
    let mut queue = VecDeque::from([entry.clone()]);
    let mut seen = HashSet::new();
    let mut modules = Vec::new();
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let file = root
            .join(id.trim_start_matches('/'))
            .canonicalize()
            .map_err(|error| {
                FerriteError::Ssr(format!("cannot load built SSR chunk `{id}`: {error}"))
            })?;
        if !file.starts_with(&root) || !file.is_file() {
            return Err(FerriteError::Ssr(format!(
                "built SSR chunk `{id}` escapes the server output directory or is not a file"
            )));
        }
        let code = std::fs::read_to_string(&file)?;
        let parsed = compiler.parse(ParseRequest {
            id: id.clone(),
            code: code.clone(),
            module_type: ModuleType::Js,
        })?;
        let mut mapping = HashMap::new();
        for import in parsed.imports {
            if !import.specifier.starts_with('.') {
                return Err(FerriteError::Ssr(format!("built SSR chunk `{id}` imports external `{}`; this embedded graph loader requires bundled dependencies; configure SSR bundling or select a host with explicit external support", import.specifier)));
            }
            // Normalize on disk first, so ../ cannot silently clamp to the root.
            let parent = Path::new(id.trim_start_matches('/'))
                .parent()
                .unwrap_or(Path::new(""));
            let joined = ferrite_core::normalize_path(&parent.join(&import.specifier));
            let dependency = output_id(&joined)?;
            mapping.insert(import.specifier, dependency.clone());
            queue.push_back(dependency);
        }
        let (code, _) = ferrite_transform::rewrite_specifiers(&code, &ModuleType::Js, &mapping)?;
        modules.push(CompiledModule {
            id,
            code,
            url: Some(file.to_string_lossy().into_owned()),
        });
    }
    let graph = CompiledModuleGraph { entry, modules };
    graph.validate()?;
    Ok(graph)
}

/// Ordered extracted styles for the single emitted renderer entry.
pub(crate) fn built_ssr_styles(server_dir: &Path, base: &str) -> Result<Vec<String>> {
    let stylesheets = if server_dir.join(SsrRendererArtifact::FILE_NAME).exists() {
        SsrRendererArtifact::read(server_dir)?.stylesheets
    } else {
        let manifest = ferrite_manifest::BuildManifest::read(&server_dir.join("manifest.json"))?;
        let entries: Vec<_> = manifest
            .entries
            .values()
            .filter(|entry| entry.is_entry == Some(true))
            .collect();
        if entries.len() != 1 {
            return Err(FerriteError::Ssr(
                "built SSR stylesheet selection requires one renderer entry".into(),
            ));
        }
        entries[0].css.clone()
    };
    let mut seen = HashSet::new();
    stylesheets
        .iter()
        .filter(|file| seen.insert((*file).clone()))
        .map(|file| {
            output_id(file)?;
            let public_file = server_dir
                .parent()
                .ok_or_else(|| FerriteError::Ssr("server output has no parent".into()))?
                .join("ssr-assets")
                .join(file);
            if !public_file.is_file() {
                return Err(FerriteError::Ssr(format!(
                    "missing published SSR stylesheet `{file}`; rebuild SSR output"
                )));
            }
            Ok(format!(
                "{}ssr-assets/{file}",
                crate::loader::with_trailing_slash(base)
            ))
        })
        .collect()
}

fn output_id(file: &str) -> Result<String> {
    if file.contains(['?', '#', '\\'])
        || Path::new(file)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || file.is_empty()
    {
        return Err(FerriteError::Ssr(format!(
            "unsafe built SSR output path `{file}`"
        )));
    }
    Ok(format!("/{file}"))
}

#[cfg(all(test, feature = "napi-vm"))]
mod tests {
    use super::*;
    use ferrite_ssr::SsrAdapter;

    #[tokio::test]
    async fn emitted_server_graph_renders_without_source_files() {
        for scope_hoist in [false, true] {
            let root = tempfile::tempdir().unwrap();
            std::fs::create_dir(root.path().join("src")).unwrap();
            std::fs::write(root.path().join("src/entry-server.ts"), "import './server.css'; import { greeting } from './greeting.ts'; export function render(url: string) { return `<h1>${greeting} ${url}</h1>`; }").unwrap();
            std::fs::write(
                root.path().join("src/greeting.ts"),
                "export const greeting: string = 'built';",
            )
            .unwrap();
            std::fs::write(
                root.path().join("src/server.css"),
                ".server-only { color: purple; background-image: url('./server.svg'); }",
            )
            .unwrap();
            std::fs::write(
                root.path().join("src/server.svg"),
                "<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"3\"/></svg>",
            )
            .unwrap();
            let mut config = ferrite_config::resolve_config(
                Default::default(),
                Some(root.path().into()),
                Default::default(),
            )
            .unwrap();
            config.runtime.backend = "napi-vm".into();
            config.build.scope_hoist = scope_hoist;
            config.base = "/app/".into();
            std::fs::write(root.path().join("index.html"), "<html><head></head><body><main><!--ssr-outlet--></main><script type=\"module\" src=\"/src/main.js\"></script></body></html>").unwrap();
            std::fs::write(
                root.path().join("src/main.js"),
                "globalThis.clientLoaded = true;",
            )
            .unwrap();
            let builder = crate::Builder::new(config.clone(), Vec::new());
            builder.build("client").await.unwrap();
            let report = builder.build("ssr").await.unwrap();
            let artifact = SsrRendererArtifact::read(&report.out_dir).unwrap();
            let embedded = serde_json::to_vec(&artifact).unwrap();
            let graph = SsrRendererArtifact::from_bytes(&embedded).unwrap().graph;
            assert_eq!(
                graph.entry,
                load_built_ssr_graph(&report.out_dir).unwrap().entry
            );
            // The versioned payload contains the final executable chunk graph.
            for module in &graph.modules {
                std::fs::remove_file(report.out_dir.join(module.id.trim_start_matches('/')))
                    .unwrap();
            }
            std::fs::remove_file(report.out_dir.join("manifest.json")).unwrap();
            // Alter source after the build: only emitted code may be executed.
            std::fs::write(
                root.path().join("src/greeting.ts"),
                "throw new Error('source must not execute');",
            )
            .unwrap();
            let adapter = ferrite_ssr::JsSsrAdapter::from_resolved_graph(&config, graph).unwrap();
            let response = adapter
                .render(
                    ferrite_ssr::SsrHttpRequest {
                        method: "GET".into(),
                        uri: "/built".into(),
                        headers: Vec::new(),
                        body: Vec::new(),
                    },
                    Default::default(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.into_string().await.unwrap(),
                "<h1>built /built</h1>"
            );
            let reservation = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            config.server.port = reservation.local_addr().unwrap().port();
            drop(reservation);
            let url = format!("http://127.0.0.1:{}/app/", config.server.port);
            let serving =
                tokio::spawn(async move { crate::preview_with_plugins(&config, &[]).await });
            let client = reqwest::Client::new();
            let response = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    if let Ok(response) = client.get(&url).send().await {
                        break response;
                    }
                    if serving.is_finished() {
                        panic!("preview exited before responding");
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("preview must respond");
            assert_eq!(response.status(), 200);
            let html = response.text().await.unwrap();
            assert!(html.contains("<main><h1>built /app/</h1></main>"), "{html}");
            let stylesheet = html
                .split("href=\"")
                .skip(1)
                .map(|part| part.split('"').next().unwrap())
                .find(|href| href.starts_with("/app/ssr-assets/") && href.ends_with(".css"))
                .expect("SSR stylesheet link");
            let origin = url.trim_end_matches("/app/");
            let response = client
                .get(format!("{origin}{stylesheet}"))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            let css = response.text().await.unwrap();
            assert!(
                css.contains("server-only") && css.contains("purple"),
                "{css}"
            );
            let asset = css
                .split("url(")
                .nth(1)
                .expect("CSS asset URL")
                .split(')')
                .next()
                .unwrap()
                .trim_matches(['\'', '"']);
            assert!(asset.starts_with("/app/ssr-assets/"), "{css}");
            let response = client.get(format!("{origin}{asset}")).send().await.unwrap();
            assert_eq!(response.status(), 200);
            assert!(response.text().await.unwrap().contains("<circle"));
            let response = client
                .post(format!("{url}submit"))
                .body("payload")
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            assert!(response
                .text()
                .await
                .unwrap()
                .contains("<h1>built /app/submit</h1>"));
            let response = client
                .get(format!("{url}server/manifest.json"))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 404);
            let response = client
                .get(url.replace("/app/", "/outside/"))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 404);
            serving.abort();
            let _ = serving.await;
        }
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;

    #[test]
    fn built_graph_rejects_missing_chunks_external_imports_and_unsafe_paths() {
        for (file, code, expected) in [
            ("assets/server.js", "import './missing.js';", "missing.js"),
            (
                "assets/server.js",
                "import 'node:fs';",
                "external `node:fs`",
            ),
            (
                "assets/server.js",
                "import '../../outside.js';",
                "unsafe built SSR",
            ),
            (
                "../outside.js",
                "export const value = 1;",
                "unsafe built SSR",
            ),
            ("/outside.js", "export const value = 1;", "unsafe built SSR"),
        ] {
            let root = tempfile::tempdir().unwrap();
            std::fs::create_dir(root.path().join("assets")).unwrap();
            std::fs::write(root.path().join("assets/server.js"), code).unwrap();
            std::fs::write(
                root.path().join("manifest.json"),
                serde_json::json!({"entry": {"file": file, "isEntry": true}}).to_string(),
            )
            .unwrap();
            let error = load_built_ssr_graph(root.path()).unwrap_err();
            assert!(error.to_string().contains(expected), "{file}: {error}");
        }
    }

    #[test]
    fn published_styles_preserve_entry_order_and_require_files() {
        let root = tempfile::tempdir().unwrap();
        let server = root.path().join("server");
        let assets = root.path().join("ssr-assets");
        std::fs::create_dir(&server).unwrap();
        std::fs::create_dir(&assets).unwrap();
        std::fs::write(server.join("manifest.json"), serde_json::json!({"entry": {"file": "server.js", "isEntry": true, "css": ["b.css", "a.css", "b.css"]}}).to_string()).unwrap();
        std::fs::write(assets.join("b.css"), ".b {}").unwrap();
        let error = built_ssr_styles(&server, "/app/").unwrap_err();
        assert!(
            error.to_string().contains("a.css") && error.to_string().contains("rebuild"),
            "{error}"
        );
        std::fs::write(assets.join("a.css"), ".a {}").unwrap();
        assert_eq!(
            built_ssr_styles(&server, "/app/").unwrap(),
            ["/app/ssr-assets/b.css", "/app/ssr-assets/a.css"]
        );
    }

    #[test]
    fn renderer_artifact_rejects_unknown_versions_and_incomplete_graphs() {
        let artifact = SsrRendererArtifact {
            version: 1,
            graph: CompiledModuleGraph {
                entry: "/server.js".into(),
                modules: vec![CompiledModule {
                    id: "/server.js".into(),
                    code: "export function render() { return 'ok'; }".into(),
                    url: None,
                }],
            },
            stylesheets: vec!["assets/style.css".into()],
        };
        let bytes = serde_json::to_vec(&artifact).unwrap();
        assert!(SsrRendererArtifact::from_bytes(&bytes).is_ok());
        let mut invalid = artifact.clone();
        invalid.version = 2;
        assert!(invalid
            .validate()
            .unwrap_err()
            .to_string()
            .contains("version 2"));
        invalid = artifact;
        invalid.graph.modules[0].code =
            "import '/missing.js'; export function render() { return 'ok'; }".into();
        assert!(invalid
            .validate()
            .unwrap_err()
            .to_string()
            .contains("missing compiled dependency"));
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join(SsrRendererArtifact::FILE_NAME), b"{broken").unwrap();
        let error = load_built_ssr_graph(root.path()).unwrap_err();
        assert!(
            !error.to_string().contains("manifest.json"),
            "artifact failures must not fall back: {error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn built_graph_rejects_symlinks_outside_output() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(
            outside.path().join("server.js"),
            "export function render() { return 'outside'; }",
        )
        .unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("server.js"),
            root.path().join("server.js"),
        )
        .unwrap();
        std::fs::write(
            root.path().join("manifest.json"),
            "{\"entry\":{\"file\":\"server.js\",\"isEntry\":true}}",
        )
        .unwrap();
        let error = load_built_ssr_graph(root.path()).unwrap_err();
        assert!(error.to_string().contains("escapes"), "{error}");
    }
}
