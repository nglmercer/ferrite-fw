//! Loading emitted server chunks without source compilation.

use ferrite_core::{FerriteError, ModuleType, Result};
use ferrite_runtime::{CompiledModule, CompiledModuleGraph};
use ferrite_transform::{JsCompiler, OxcCompiler, OxcOptions, ParseRequest};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Component, Path};

/// Load the single emitted SSR entry and its static/dynamic chunk dependencies.
/// External runtime imports require a separate host contract and fail explicitly.
/// Files and symlinks must remain inside the built server directory.
pub fn load_built_ssr_graph(server_dir: &Path) -> Result<CompiledModuleGraph> {
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
            std::fs::write(root.path().join("src/entry-server.ts"), "import { greeting } from './greeting.ts'; export function render(url: string) { return `<h1>${greeting} ${url}</h1>`; }").unwrap();
            std::fs::write(
                root.path().join("src/greeting.ts"),
                "export const greeting: string = 'built';",
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
            let builder = crate::Builder::new(config.clone(), Vec::new());
            let report = builder.build("ssr").await.unwrap();
            let graph = load_built_ssr_graph(&report.out_dir).unwrap();
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
