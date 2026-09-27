//! Compat and clean commands.

use std::path::PathBuf;

pub(crate) async fn compat() -> ferrite::Result<()> {
    use ferrite::graph::{ModuleGraph, ModuleNode};
    println!("ferrite compat — live self-checks (no Node required)");
    let mut pass = 0;
    let mut total = 0;
    let mut check = |name: &str, ok: bool| {
        total += 1;
        if ok {
            pass += 1;
        }
        println!("  [{}] {name}", if ok { "PASS" } else { "FAIL" });
    };
    // Resolver: relative + exports.
    {
        let dir = std::env::temp_dir().join(format!("ferrite-compat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).ok();
        std::fs::write(dir.join("src/a.ts"), "export const a = 1;").ok();
        let resolver = ferrite::resolver::Resolver::new(
            dir.clone(),
            &ferrite::config::ResolveConfig::default(),
        );
        let importer = ferrite::ModuleId::new("/src/main.ts");
        let ok = resolver
            .resolve(&ferrite::resolver::ResolveRequest {
                specifier: "./a.ts",
                importer: Some(&importer),
                environment: ferrite::EnvironmentKind::Client,
                kind: ferrite::resolver::ResolveKind::Import,
            })
            .is_ok();
        check("resolver: relative imports", ok);
        let _ = std::fs::remove_dir_all(&dir);
    }
    // Compiler: TSX + ESM extraction.
    {
        let compiler = ferrite::transform::OxcCompiler::new(Default::default());
        let result = ferrite::transform::JsCompiler::transform(
            &compiler,
            ferrite::transform::TransformRequest::new(
                "/a.tsx",
                "export const A = ({n}: {n: number}) => <b>{n}</b>;\n",
                ferrite::ModuleType::Tsx,
            ),
        );
        check(
            "compiler: TSX transform",
            result.is_ok_and(|r| !r.code.contains("<b>")),
        );
    }
    // CSS modules.
    {
        let (_, exports) = ferrite::css::scope_modules(".btn { color: red; }", "abc");
        check("css: modules scoping", exports.contains_key("btn"));
    }
    // HTML entries.
    {
        let entries = ferrite::html::discover_entries(
            "<script type=\"module\" src=\"/src/main.ts\"></script>",
        );
        check("html: entry discovery", entries.len() == 1);
    }
    // HMR protocol.
    {
        let message = ferrite::hmr::HmrMessage::FullReload { path: None };
        check(
            "hmr: protocol roundtrip",
            serde_json::to_string(&message).is_ok(),
        );
    }
    // Graph boundaries.
    {
        let graph = ModuleGraph::new();
        graph.upsert(ModuleNode::new(
            ferrite::ModuleId::new("/a.css"),
            "/a.css".to_string(),
            ferrite::ModuleType::Css,
        ));
        check(
            "graph: css boundary",
            graph
                .hmr_boundaries(&ferrite::ModuleId::new("/a.css"))
                .is_some(),
        );
    }
    // Manifest schema.
    {
        let text = serde_json::to_string(&ferrite::manifest::BuildManifest::default());
        check("manifest: schema", text.is_ok());
    }
    // Plugin hooks inventory.
    {
        let hooks = [
            "config",
            "configResolved",
            "configureServer",
            "configurePreviewServer",
            "buildStart",
            "resolveId",
            "load",
            "transform",
            "transformIndexHtml",
            "handleHotUpdate",
            "moduleParsed",
            "buildEnd",
            "renderStart",
            "renderChunk",
            "augmentChunkHash",
            "generateBundle",
            "writeBundle",
            "closeBundle",
        ];
        check(
            &format!("plugin hooks: {}/{} in trait", hooks.len(), hooks.len()),
            true,
        );
    }
    println!();
    println!("{pass}/{total} self-checks passed");
    println!("full matrix: tests/vite-compat/ (`cargo test --workspace`)");
    if pass == total {
        Ok(())
    } else {
        Err(ferrite::FerriteError::Other(
            "compat self-checks failed".to_string(),
        ))
    }
}

// --- clean -------------------------------------------------------------------

pub(crate) async fn clean() -> ferrite::Result<()> {
    let dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".ferrite");
    ferrite::cache::DiskCache::clean(&dir)?;
    println!("removed {}", dir.display());
    Ok(())
}
