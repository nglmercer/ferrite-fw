//! Vite compat: SWC engine and scope-hoisting builds.

use ferrite_test::TempProject;

#[tokio::test]
async fn emitted_source_map_urls_resolve_relative_to_each_chunk() {
    struct NestedChunks;
    #[async_trait::async_trait]
    impl ferrite::plugin::Plugin for NestedChunks {
        fn name(&self) -> &'static str {
            "nested-map-chunks"
        }
        async fn output_options(
            &self,
            _: &ferrite::plugin::PluginContext,
            options: &mut ferrite::plugin::OutputOptions,
        ) -> ferrite::Result<()> {
            options.chunk_pattern = "nested/chunks/[name]-[hash].js".into();
            Ok(())
        }
    }
    for (nested, hidden) in [(false, false), (true, false), (true, true)] {
        let project = TempProject::new(&[
            (
                "index.html",
                "<script type='module' src='/main.ts'></script>",
            ),
            (
                "main.ts",
                "const message: string = 'mapped-source';\nconsole.log(message);",
            ),
        ]);
        let mut config = project.resolve_config_mode("production");
        if hidden {
            config.build.sourcemap = ferrite::config::SourceMapConfig::Mode("hidden".into());
        }
        let plugins: Vec<std::sync::Arc<dyn ferrite::plugin::Plugin>> = if nested {
            vec![std::sync::Arc::new(NestedChunks)]
        } else {
            Vec::new()
        };
        let report = ferrite::Builder::new(config, plugins)
            .build("client")
            .await
            .unwrap();
        let directory = report
            .out_dir
            .join(if nested { "nested/chunks" } else { "assets" });
        let mut checked = 0;
        for entry in std::fs::read_dir(directory).unwrap() {
            let chunk = entry.unwrap().path();
            if !chunk.extension().is_some_and(|extension| extension == "js") {
                continue;
            }
            checked += 1;
            let code = std::fs::read_to_string(&chunk).unwrap();
            let map = if hidden {
                assert!(!code.contains("sourceMappingURL"));
                chunk.with_extension("js.map")
            } else {
                let url = code
                    .split("//# sourceMappingURL=")
                    .nth(1)
                    .expect("external maps need a URL")
                    .trim();
                assert!(
                    !url.contains("nested/") && !url.contains("assets/"),
                    "{url}"
                );
                chunk.parent().unwrap().join(url)
            };
            let map: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(map)
                    .expect("map URL must resolve from the chunk directory"),
            )
            .unwrap();
            assert_eq!(map["version"], 3);
            assert!(!map["sources"].as_array().unwrap().is_empty());
            assert!(!map["mappings"].as_str().unwrap().is_empty());
        }
        assert_eq!(checked, 1);
    }
}

#[tokio::test]
async fn configured_server_entry_overrides_discovery_and_fails_before_client_output() {
    let project = TempProject::new(&[
        (
            "index.html",
            "<script type='module' src='/main.js'></script>",
        ),
        ("main.js", "console.log('client');"),
        (
            "src/entry-server.js",
            "this is deliberately invalid JavaScript",
        ),
        (
            "src/renderer.ts",
            "export const server: boolean = import.meta.env.SSR;",
        ),
        ("src/custom.rs", "fn main() {}"),
    ]);
    let builder = |entry: &str| {
        let mut config = project.resolve_config_mode("production");
        config.ssr.entry = Some(entry.into());
        ferrite::Builder::new(config, Vec::new())
    };
    for entry in [
        "missing.js",
        "src",
        "../outside.js",
        "/absolute.js",
        "src/custom.rs",
    ] {
        let error = builder(entry).build_app().await.unwrap_err().to_string();
        assert!(
            error.contains("SSR entry") && error.contains(entry),
            "{error}"
        );
        assert!(!project.root.join("dist").exists());
    }
    let reports = builder("src/renderer.ts").build_app().await.unwrap();
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[1].env, "ssr");
    assert_eq!(reports[1].entries, ["/src/renderer.ts"]);
}

#[tokio::test]
async fn ssr_build_hooks_receive_server_resolver_conditions() {
    struct ResolverProbe;
    #[async_trait::async_trait]
    impl ferrite::plugin::Plugin for ResolverProbe {
        fn name(&self) -> &'static str {
            "ssr-resolver-probe"
        }
        async fn options(
            &self,
            ctx: &ferrite::plugin::PluginContext,
            _: &mut ferrite::plugin::BundleOptions,
        ) -> ferrite::Result<()> {
            assert!(ctx.environment.kind.is_ssr());
            assert_eq!(
                ctx.resolver.conditions,
                ctx.environment.kind.default_conditions()
            );
            assert!(ctx
                .resolver
                .conditions
                .iter()
                .any(|condition| condition == "node-compatible"));
            assert!(!ctx
                .resolver
                .conditions
                .iter()
                .any(|condition| condition == "browser"));
            Ok(())
        }
    }
    let project = TempProject::new(&[(
        "src/entry-server.js",
        "export const isServer = import.meta.env.SSR;",
    )]);
    let builder = ferrite::Builder::new(
        project.resolve_config_mode("production"),
        vec![std::sync::Arc::new(ResolverProbe)],
    );
    let report = builder.build("ssr").await.unwrap();
    assert_eq!(report.env, "ssr");
    assert!(report.out_dir.ends_with("dist/server"));
}

#[tokio::test]
async fn unknown_build_environments_fail_before_server_hooks_or_output() {
    struct UnexpectedHook;
    #[async_trait::async_trait]
    impl ferrite::plugin::Plugin for UnexpectedHook {
        fn name(&self) -> &'static str {
            "unexpected-build-hook"
        }
        async fn config_resolved(&self, _: &ferrite::ResolvedConfig) -> ferrite::Result<()> {
            panic!("invalid environment must fail before server hooks");
        }
    }
    let project = TempProject::new(&[
        (
            "index.html",
            "<script type='module' src='/main.js'></script>",
        ),
        ("main.js", "console.log('client');"),
    ]);
    let builder = ferrite::Builder::new(
        project.resolve_config_mode("production"),
        vec![std::sync::Arc::new(UnexpectedHook)],
    );
    for environment in ["staging", "worker", "SSR", ""] {
        let error = builder.build(environment).await.unwrap_err().to_string();
        assert!(error.contains("unsupported build environment"), "{error}");
        assert!(
            error.contains("client") && error.contains("ssr") && error.contains("mode"),
            "{error}"
        );
        assert!(!project.root.join("dist").exists());
    }
}

#[tokio::test]
async fn production_css_uses_shared_pre_and_post_transforms() {
    struct CssPlugin(bool);
    #[async_trait::async_trait]
    impl ferrite::plugin::Plugin for CssPlugin {
        fn name(&self) -> &'static str {
            if self.0 {
                "css-pre"
            } else {
                "css-post"
            }
        }
        fn enforce(&self) -> ferrite::plugin::Enforce {
            if self.0 {
                ferrite::plugin::Enforce::Pre
            } else {
                ferrite::plugin::Enforce::Post
            }
        }
        async fn transform(
            &self,
            _: &ferrite::plugin::PluginContext,
            request: ferrite::plugin::TransformRequest,
        ) -> ferrite::Result<Option<ferrite::plugin::TransformResult>> {
            if request.id != "/style.css" {
                return Ok(None);
            }
            let code = if self.0 {
                assert_eq!(request.module_type, ferrite::core::ModuleType::Css);
                request.code.replace("COLOR", "purple")
            } else {
                assert_eq!(request.module_type, ferrite::core::ModuleType::Js);
                format!(
                    "import {{ flag }} from './extra.js'; {}\nglobalThis.cssPost = flag;",
                    request.code
                )
            };
            Ok(Some(ferrite::plugin::TransformResult {
                code,
                map: None,
                dependencies: Vec::new(),
                module_type: None,
            }))
        }
    }
    let project = TempProject::new(&[
        (
            "index.html",
            "<script type='module' src='/main.js'></script>",
        ),
        ("main.js", "import './style.css'; console.log('ready');"),
        ("style.css", "body { color: COLOR; }"),
        ("extra.js", "export const flag = 'post-transform-retained';"),
    ]);
    let builder = ferrite::create_builder(
        ferrite::Config {
            root: Some(project.root.clone()),
            ..Default::default()
        }
        .plugin(CssPlugin(true))
        .plugin(CssPlugin(false)),
    )
    .await
    .unwrap();
    let report = builder.build("client").await.unwrap();
    let mut javascript = String::new();
    let mut css = String::new();
    for entry in std::fs::read_dir(report.out_dir.join("assets")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "js") {
            javascript.push_str(&std::fs::read_to_string(&path).unwrap());
        } else if path.extension().is_some_and(|extension| extension == "css") {
            css.push_str(&std::fs::read_to_string(&path).unwrap());
        }
    }
    assert!(css.contains("purple") && !css.contains("COLOR"), "{css}");
    assert!(
        javascript.contains("cssPost") && javascript.contains("post-transform-retained"),
        "{javascript}"
    );
    assert!(
        !javascript.contains("document.createElement"),
        "{javascript}"
    );
    assert!(!javascript.contains("/@ferrite/client"), "{javascript}");
}

#[tokio::test]
async fn missing_relative_css_inputs_fail_build_and_recover_when_created() {
    for (css, missing, contents, diagnostic) in [
        (
            "body { background: url('./missing.svg'); }",
            "missing.svg",
            "<svg xmlns='http://www.w3.org/2000/svg'></svg>",
            "CSS asset",
        ),
        (
            "@import './missing.css'; body { color: blue; }",
            "missing.css",
            "h1 { color: red; }",
            "CSS @import",
        ),
    ] {
        let project = TempProject::new(&[
            (
                "index.html",
                "<script type='module' src='/main.js'></script>",
            ),
            ("main.js", "import './style.css'; console.log('ready');"),
            ("style.css", css),
        ]);
        let builder = ferrite::create_builder(ferrite::Config {
            root: Some(project.root.clone()),
            ..Default::default()
        })
        .await
        .unwrap();
        let error = builder.build("client").await.unwrap_err().to_string();
        assert!(error.contains(diagnostic), "{error}");
        assert!(error.contains(missing), "{error}");
        assert!(error.contains("style.css"), "{error}");
        assert!(!project.root.join("dist").exists());
        std::fs::write(project.root.join(missing), contents).unwrap();
        let report = builder.build("client").await.unwrap();
        let outputs: Vec<_> = std::fs::read_dir(report.out_dir.join("assets"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        let emitted = outputs
            .iter()
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("missing")
            })
            .expect("the recovered CSS input must be emitted");
        let output = std::fs::read_to_string(emitted).unwrap();
        if missing.ends_with(".svg") {
            assert_eq!(output, contents);
        } else {
            // CSS formatting/minification can change whitespace.
            assert!(output.contains("h1") && output.contains("red"), "{output}");
        }
        let stylesheet = outputs
            .iter()
            .find(|path| {
                path.extension().is_some_and(|extension| extension == "css")
                    && path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("style-")
            })
            .expect("the importing stylesheet must be emitted");
        let stylesheet = std::fs::read_to_string(stylesheet).unwrap();
        assert!(
            stylesheet.contains(&emitted.file_name().unwrap().to_string_lossy().to_string()),
            "{stylesheet}"
        );
        assert!(
            !stylesheet.contains(&format!("./{missing}")),
            "{stylesheet}"
        );
    }
}

#[tokio::test]
async fn named_build_modes_load_their_env_and_remove_development_hooks() {
    let project = TempProject::new(&[
        ("ferrite.toml", "mode = 'staging'\n[build]\nminify = false\n"),
        (".env.staging", "FERRITE_PROFILE=named-staging\n"),
        ("index.html", "<script type='module' src='/main.js'></script>"),
        ("main.js", "import './style.css'; globalThis.profile = {mode: import.meta.env.MODE, prod: import.meta.env.PROD, dev: import.meta.env.DEV, value: import.meta.env.FERRITE_PROFILE, nodeEnv: process.env.NODE_ENV}; if (import.meta.hot) import.meta.hot.accept();"),
        ("style.css", "body { color: blue; }"),
    ]);
    let builder = ferrite::create_builder(ferrite::Config {
        root: Some(project.root.clone()),
        ..Default::default()
    })
    .await
    .unwrap();
    assert_eq!(builder.config.mode, "staging");
    assert!(builder.config.is_production);
    let report = builder.build("client").await.unwrap();
    let mut code = String::new();
    for entry in std::fs::read_dir(report.out_dir.join("assets")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "js") {
            code.push_str(&std::fs::read_to_string(path).unwrap());
        }
    }
    assert!(code.contains("named-staging"), "{code}");
    assert!(code.contains("staging"), "{code}");
    assert!(code.contains("production"), "{code}");
    assert!(!code.contains("/@ferrite/client"), "{code}");
    assert!(!code.contains("import.meta.hot"), "{code}");
    assert!(!code.contains("import.meta.env"), "{code}");
}

/// Production build through the SWC engine: TS strip, JSX, minify,
/// and shake re-minification all run on SWC.
#[cfg(feature = "swc")]
#[tokio::test]
async fn production_build_with_swc_engine() {
    let project = TempProject::new(&[
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <script type=\"module\" src=\"/src/main.tsx\"></script></body></html>",
        ),
        (
            "src/main.tsx",
            "import { keepme } from \"./lib\";\n\
             export const el = <div>{keepme}</div>;\n\
             console.log(el);\n",
        ),
        (
            "src/lib.ts",
            "export const keepme: number = 1;\nexport const dropme: number = 2;\n",
        ),
    ]);
    let mut user = ferrite::UserConfig::default();
    user.compiler.engine = "swc".to_string();
    let config = ferrite::resolve_config(
        user,
        Some(project.root.clone()),
        ferrite::CliOverrides {
            mode: Some("production".to_string()),
            ..Default::default()
        },
    )
    .expect("resolve");
    assert_eq!(config.compiler.engine, "swc");
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    let mut blob = String::new();
    for entry in std::fs::read_dir(report.out_dir.join("assets")).expect("assets") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("js") {
            continue;
        }
        blob.push_str(&std::fs::read_to_string(&path).expect("chunk"));
    }
    // JSX compiled, TS stripped, unused export shaken — all via SWC.
    assert!(
        blob.contains("jsx-runtime") || blob.contains("jsxDEV"),
        "{blob}"
    );
    assert!(!blob.contains(": number"), "{blob}");
    assert!(!blob.contains("dropme"), "{blob}");
}

#[tokio::test]
async fn production_build_scope_hoists_entry_closure() {
    let project = TempProject::new(&[
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <script type=\"module\" src=\"/src/main.ts\"></script></body></html>",
        ),
        (
            "src/main.ts",
            "import { greet } from \"./greet\";\nconsole.log(greet(\"hoist\"));\n",
        ),
        (
            "src/greet.ts",
            "import { punct } from \"./punct\";\nexport function greet(n: string): string { return `hi ${n}${punct}`; }\n",
        ),
        ("src/punct.ts", "export const punct = \"!\";\n"),
    ]);
    let mut user = ferrite::UserConfig::default();
    user.build.scope_hoist = true;
    let config = ferrite::resolve_config(
        user,
        Some(project.root.clone()),
        ferrite::CliOverrides {
            mode: Some("production".to_string()),
            ..Default::default()
        },
    )
    .expect("resolve");
    assert!(config.build.scope_hoist);
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    let js: Vec<std::path::PathBuf> = std::fs::read_dir(report.out_dir.join("assets"))
        .expect("assets")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("js"))
        .collect();
    assert_eq!(js.len(), 1, "{js:?}");
    let code = std::fs::read_to_string(&js[0]).expect("chunk");
    // One scope: no surviving inside imports, prefixed bindings, kept log.
    assert!(!code.contains("from\"./"), "{code}");
    assert!(!code.contains("from \"./"), "{code}");
    assert!(code.contains("$f"), "{code}");
    assert!(code.contains("console.log"), "{code}");
}
