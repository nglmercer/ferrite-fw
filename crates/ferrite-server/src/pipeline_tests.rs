use crate::DevServer;
use ferrite_core::{ModuleId, ModuleType, Result, SourceMap};
use ferrite_plugin::{
    Enforce, LoadRequest, LoadResult, Plugin, PluginContext, TransformRequest, TransformResult,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct CompilerFixture {
    dependency: PathBuf,
    watch: PathBuf,
    calls: Arc<Mutex<usize>>,
}

#[async_trait::async_trait]
impl Plugin for CompilerFixture {
    fn name(&self) -> &'static str {
        "fixture-compiler"
    }
    fn enforce(&self) -> Enforce {
        Enforce::Pre
    }

    async fn load(&self, _: &PluginContext, request: LoadRequest) -> Result<Option<LoadResult>> {
        if request.id != "/src/main.ts" {
            return Ok(None);
        }
        Ok(Some(LoadResult {
            code: "export const value: number = SOURCE;".into(),
            module_type: ModuleType::Ts,
            dependencies: vec![self.dependency.to_string_lossy().into_owned()],
            side_effects: Some(false),
            map: ferrite_transform::apply_text_edits(
                "/src/original.component",
                "export const value: number = SOURCE;",
                &[],
                true,
            )?
            .1
            .map(SourceMap::external),
        }))
    }

    async fn transform(
        &self,
        ctx: &PluginContext,
        request: TransformRequest,
    ) -> Result<Option<TransformResult>> {
        if request.id != "/src/main.ts" {
            return Ok(None);
        }
        assert_eq!(request.module_type, ModuleType::Ts);
        assert!(
            request.code.contains(": number"),
            "pre transform must see TS"
        );
        *self.calls.lock().unwrap() += 1;
        ctx.add_watch_file(&self.watch.to_string_lossy());
        let watched: i32 = std::fs::read_to_string(&self.watch)
            .unwrap()
            .parse()
            .unwrap();
        let value = std::fs::read_to_string(&self.dependency).unwrap_or_else(|_| "0".into());
        let value = (value.parse::<i32>().unwrap() + watched).to_string();
        let offset = request.code.find("SOURCE").unwrap();
        let (code, map) = ferrite_transform::apply_text_edits(
            &request.id,
            &request.code,
            &[(offset, offset + 6, value)],
            true,
        )?;
        Ok(Some(TransformResult {
            code,
            map: map.map(SourceMap::external),
            dependencies: vec![],
            module_type: None,
        }))
    }
}

struct PostFixture;
#[async_trait::async_trait]
impl Plugin for PostFixture {
    fn name(&self) -> &'static str {
        "fixture-post"
    }
    async fn transform(
        &self,
        _: &PluginContext,
        request: TransformRequest,
    ) -> Result<Option<TransformResult>> {
        if request.id != "/src/main.ts" {
            return Ok(None);
        }
        assert_eq!(request.module_type, ModuleType::Js);
        assert!(
            !request.code.contains(": number"),
            "post transform must see lowered JS"
        );
        let addition =
            "\nexport const added = true;\nif (import.meta.hot) import.meta.hot.accept();\n";
        let (code, map) = ferrite_transform::apply_text_edits(
            &request.id,
            &request.code,
            &[(request.code.len(), request.code.len(), addition.into())],
            true,
        )?;
        Ok(Some(TransformResult {
            code,
            map: map.map(SourceMap::external),
            dependencies: vec![],
            module_type: None,
        }))
    }
}

#[tokio::test]
async fn shared_pipeline_orders_transforms_and_invalidates_compiler_dependencies() {
    let profiles = if cfg!(feature = "swc") {
        vec![(false, "oxc"), (true, "oxc"), (false, "swc"), (true, "swc")]
    } else {
        vec![(false, "oxc"), (true, "oxc")]
    };
    for (production, engine) in profiles {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.ts"), "fixture source").unwrap();
        let dependency = dir.path().join("input.txt");
        std::fs::write(&dependency, "41").unwrap();
        let watch = dir.path().join("watch.txt");
        std::fs::write(&watch, "0").unwrap();
        let mut user = ferrite_config::UserConfig::default();
        user.compiler.engine = engine.into();
        let config = ferrite_config::resolve_config(
            user,
            Some(dir.path().into()),
            ferrite_config::CliOverrides {
                mode: Some(
                    if production {
                        "production"
                    } else {
                        "development"
                    }
                    .into(),
                ),
                ..Default::default()
            },
        )
        .unwrap();
        let calls = Arc::new(Mutex::new(0));
        let server = DevServer::new_without_watcher(
            config,
            vec![
                Arc::new(CompilerFixture {
                    dependency: dependency.clone(),
                    watch: watch.clone(),
                    calls: calls.clone(),
                }),
                Arc::new(PostFixture),
            ],
        )
        .await
        .unwrap();
        let id = ModuleId::new("/src/main.ts");
        let first = server.pipeline_module(&id, None, "client").await.unwrap();
        assert!(first.code.contains("41"));
        assert!(first
            .shake
            .as_ref()
            .unwrap()
            .exports
            .iter()
            .any(|export| export.exported == "added"));
        assert!(first.uses_import_meta_hot);
        assert_eq!(first.side_effects, Some(false));
        assert!(first
            .dependencies
            .contains(&dependency.to_string_lossy().into_owned()));
        assert!(first
            .dependencies
            .contains(&watch.to_string_lossy().into_owned()));
        assert_eq!(first.code.contains("/@ferrite/client"), !production);
        let map_json = first.map.as_ref().expect("all compiler maps preserved");
        let map = oxc_sourcemap::SourceMap::from_json_string(map_json).unwrap();
        assert_eq!(
            map.get_sources().collect::<Vec<_>>(),
            vec!["/src/original.component"]
        );
        assert!(map
            .get_tokens()
            .any(|token| token.get_source_id().is_some()));
        assert!(server
            .inner
            .graph
            .get(&ModuleId::new("/input.txt"))
            .unwrap()
            .importers
            .contains(&id));
        server.pipeline_module(&id, None, "client").await.unwrap();
        assert_eq!(*calls.lock().unwrap(), 1, "valid cache must be reusable");
        std::fs::write(&dependency, "42").unwrap();
        let changed = server.pipeline_module(&id, None, "client").await.unwrap();
        assert!(changed.code.contains("42"));
        assert!(!changed.code.contains("41"));
        assert_eq!(*calls.lock().unwrap(), 2);
        std::fs::write(&watch, "1").unwrap();
        let watched = server.pipeline_module(&id, None, "client").await.unwrap();
        assert!(watched.code.contains("43"));
        assert_eq!(*calls.lock().unwrap(), 3);
        std::fs::remove_file(&dependency).unwrap();
        let deleted = server.pipeline_module(&id, None, "client").await.unwrap();
        assert!(deleted.code.contains("= 1"));
        assert_eq!(*calls.lock().unwrap(), 4);
        let inline = server
            .ssr_transform("export const value: number = SOURCE;", "/src/main.ts")
            .await
            .unwrap();
        assert!(inline.code.contains("= 1"));
        assert!(inline.code.contains("added"));
        assert!(!inline.code.contains(": number"));
        assert!(!inline.code.contains("/@ferrite/client"));
        assert!(inline.map.is_some());
        server.close();
    }
}

#[tokio::test]
async fn components_are_not_asset_shimmed_and_missing_compilers_fail_loudly() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src/main.js"),
        "import App from './App.vue'; import Other from './Other.svelte'; console.log(App, Other);",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/App.vue"),
        "<template><button>Vue</button></template>",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("src/Other.svelte"),
        "<button>Svelte</button>",
    )
    .unwrap();
    let config = ferrite_config::resolve_config(
        Default::default(),
        Some(dir.path().into()),
        Default::default(),
    )
    .unwrap();
    let server = DevServer::new_without_watcher(
        config,
        vec![
            Arc::new(ferrite_frameworks::VuePlugin::new(dir.path().into())),
            Arc::new(ferrite_frameworks::SveltePlugin::new(dir.path().into())),
        ],
    )
    .await
    .unwrap();
    let entry = server
        .pipeline_module(&ModuleId::new("/src/main.js"), None, "client")
        .await
        .unwrap();
    assert!(!entry.code.contains("asset-shim"), "{}", entry.code);
    for (_, id, _) in entry.imports {
        let error = server
            .pipeline_module(&id, None, "client")
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("validated compiler host"), "{error}");
    }
    server.close();
}

#[tokio::test]
async fn final_graph_requires_self_accept_call_not_context_read() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("read-hot.js"),
        "console.log(import.meta.hot.data); import.meta.hot.dispose(() => {});",
    )
    .unwrap();
    std::fs::write(
        root.path().join("accept-hot.js"),
        "if (import.meta.hot) import.meta.hot.accept(next => console.log(next));",
    )
    .unwrap();
    let config = ferrite_config::resolve_config(
        Default::default(),
        Some(root.path().into()),
        Default::default(),
    )
    .unwrap();
    let server = DevServer::new_without_watcher(config, vec![])
        .await
        .unwrap();
    for (path, expected) in [("/read-hot.js", false), ("/accept-hot.js", true)] {
        let id = ModuleId::new(path);
        let module = server.pipeline_module(&id, None, "client").await.unwrap();
        assert!(module.uses_import_meta_hot);
        assert_eq!(
            server.inner.graph.get(&id).unwrap().hmr.self_accepting,
            expected,
            "{}", module.code
        );
    }
}
