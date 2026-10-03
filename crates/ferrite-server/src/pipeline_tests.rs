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
            "{}",
            module.code
        );
    }
}

#[tokio::test]
async fn final_graph_resolves_dependency_acceptance_and_clears_stale_metadata() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("dep.js"), "export const count = 1;").unwrap();
    let file = root.path().join("entry.js");
    std::fs::write(&file, "import {count} from './dep.js'; if (import.meta.hot) import.meta.hot.accept(['./dep.js'], ([next]) => console.log(next));").unwrap();
    let config = ferrite_config::resolve_config(
        Default::default(),
        Some(root.path().into()),
        Default::default(),
    )
    .unwrap();
    let server = DevServer::new_without_watcher(config, vec![])
        .await
        .unwrap();
    let id = ModuleId::new("/entry.js");
    let module = server.pipeline_module(&id, None, "client").await.unwrap();
    assert!(
        module.code.contains("accept([\"/dep.js\"]"),
        "{}",
        module.code
    );
    let node = server.inner.graph.get(&id).unwrap();
    assert!(!node.hmr.self_accepting);
    assert_eq!(
        node.hmr.accepted_deps,
        std::collections::HashSet::from(["/dep.js".to_string()])
    );
    let ferrite_hmr::HmrPlan::Update(updates) =
        ferrite_hmr::plan_update(&server.inner.graph, &ModuleId::new("/dep.js"), 1)
    else {
        panic!("dependency edge should accept");
    };
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].path, "/entry.js");
    assert_eq!(updates[0].accepted_path, "/dep.js");
    let updated = server
        .pipeline_module(&ModuleId::new("/dep.js?t=123"), None, "client")
        .await
        .unwrap();
    assert_eq!(updated.id.0, "/dep.js");
    assert!(!server.inner.graph.contains(&ModuleId::new("/dep.js?t=123")));
    assert!(matches!(
        ferrite_hmr::plan_update(&server.inner.graph, &ModuleId::new("/dep.js"), 2),
        ferrite_hmr::HmrPlan::Update(_)
    ));

    // A new source hash must replace acceptance, including on a cache hit.
    std::fs::write(&file, "import {count} from './dep.js'; console.log(count);").unwrap();
    server.pipeline_module(&id, None, "client").await.unwrap();
    server.pipeline_module(&id, None, "client").await.unwrap();
    assert!(server
        .inner
        .graph
        .get(&id)
        .unwrap()
        .hmr
        .accepted_deps
        .is_empty());
    std::fs::write(
        &file,
        "import.meta.hot.accept(dynamicDependency, callback);",
    )
    .unwrap();
    assert!(server
        .pipeline_module(&id, None, "client")
        .await
        .unwrap_err()
        .to_string()
        .contains("unsupported dynamic HMR"));
    server.close();
}

#[tokio::test]
async fn invalidated_transitive_import_urls_survive_cache_hits_without_graph_variants() {
    for use_import_map in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let mid_source = if use_import_map {
            "export { step } from 'leaf';"
        } else {
            "export { step } from './leaf.js';"
        };
        for (file, source) in [
        ("leaf.js", "export const step = 1;"),
        ("mid.js", mid_source),
        ("boundary.js", "import {step} from './mid.js'; export {step}; if (import.meta.hot) import.meta.hot.accept(next => console.log(next.step));"),
    ] { std::fs::write(root.path().join(file), source).unwrap(); }
        let mut user = ferrite_config::UserConfig::default();
        user.resolve.alias.insert("leaf".into(), "./leaf.js".into());
        if use_import_map {
            user.npm.dev_strategy = "import-map".into();
        }
        let config =
            ferrite_config::resolve_config(user, Some(root.path().into()), Default::default())
                .unwrap();
        let server = DevServer::new_without_watcher(config, vec![])
            .await
            .unwrap();
        for file in ["/boundary.js", "/mid.js", "/leaf.js"] {
            let module = server
                .pipeline_module(&ModuleId::new(file), None, "client")
                .await
                .unwrap();
            assert!(!module.code.contains("?t="));
        }
        let leaf = ModuleId::new("/leaf.js");
        std::fs::write(root.path().join("leaf.js"), "export const step = 2;").unwrap();
        server.invalidate_module(&leaf).await;
        for (file, dependency) in [("/boundary.js", "/mid.js"), ("/mid.js", "/leaf.js")] {
            let id = ModuleId::new(file);
            let updated = server.pipeline_module(&id, None, "client").await.unwrap();
            assert!(
                updated.code.contains(&format!("{dependency}?t=")),
                "{}",
                updated.code
            );
            assert!(updated.imports.iter().any(|(_, id, _)| id.0 == dependency));
            assert!(updated
                .imports
                .iter()
                .all(|(_, id, _)| !id.0.contains("?t=")));
            let cached = server.pipeline_module(&id, None, "client").await.unwrap();
            assert_eq!(cached.code, updated.code);
            let ssr = server.pipeline_module(&id, None, "ssr").await.unwrap();
            assert!(!ssr.code.contains("?t="), "{}", ssr.code);
        }
        let changed = server.pipeline_module(&leaf, None, "client").await.unwrap();
        assert!(changed.code.contains("= 2"));
        assert!(server
            .inner
            .graph
            .module_ids()
            .iter()
            .all(|id| !id.0.contains("?t=")));
        server.close();
    }
}

#[tokio::test]
async fn invalid_edits_emit_diagnostics_and_keep_previous_runtime_acceptance() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("entry.js");
    std::fs::write(&file, "export const count = 1; if (import.meta.hot) import.meta.hot.accept(next => console.log(next));").unwrap();
    let config = ferrite_config::resolve_config(
        Default::default(),
        Some(root.path().into()),
        Default::default(),
    )
    .unwrap();
    let server = DevServer::new_without_watcher(config, vec![])
        .await
        .unwrap();
    let id = ModuleId::new("/entry.js");
    server.pipeline_module(&id, None, "client").await.unwrap();
    let mut messages = server.inner.hmr.subscribe();
    std::fs::write(&file, "export const count = ;").unwrap();
    server.invalidate_module(&id).await;
    let message: ferrite_hmr::HmrMessage =
        serde_json::from_str(&messages.recv().await.unwrap()).unwrap();
    let ferrite_hmr::HmrMessage::Error { err } = message else {
        panic!("invalid edit must not publish an update/reload");
    };
    assert_eq!(err.id.as_deref(), Some("/entry.js"));
    assert!(!err.message.is_empty());
    assert!(messages.try_recv().is_err());
    assert!(server.inner.graph.get(&id).unwrap().hmr.self_accepting);
    // A partially validated new graph cannot invent client-side boundaries.
    std::fs::write(
        &file,
        "import './a-new.js'; import './z-broken.js'; export const count = 2;",
    )
    .unwrap();
    std::fs::write(
        root.path().join("a-new.js"),
        "if (import.meta.hot) import.meta.hot.accept();",
    )
    .unwrap();
    std::fs::write(root.path().join("z-broken.js"), "export const value = ;").unwrap();
    server.invalidate_module(&id).await;
    let message: ferrite_hmr::HmrMessage =
        serde_json::from_str(&messages.recv().await.unwrap()).unwrap();
    assert!(matches!(message, ferrite_hmr::HmrMessage::Error { .. }));
    assert!(server.inner.graph.get(&id).unwrap().hmr.self_accepting);
    assert!(
        !server
            .inner
            .graph
            .get(&ModuleId::new("/a-new.js"))
            .unwrap()
            .hmr
            .self_accepting
    );
    std::fs::write(root.path().join("z-broken.js"), "export const value = 3;").unwrap();
    server
        .invalidate_module(&ModuleId::new("/z-broken.js"))
        .await;
    let message: ferrite_hmr::HmrMessage =
        serde_json::from_str(&messages.recv().await.unwrap()).unwrap();
    let ferrite_hmr::HmrMessage::Update { updates } = message else {
        panic!("corrected dependency must reach old runtime boundary");
    };
    assert_eq!(updates[0].accepted_path, "/entry.js");
    assert!(
        !server.inner.graph.get(&id).unwrap().hmr.self_accepting,
        "successful output replaces the old acceptance for subsequent edits"
    );
    server.close();
}

#[tokio::test]
async fn watcher_coalesces_rapid_corrections_and_recovers_after_diagnostics() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("entry.js");
    let source = |count| {
        format!("export const count = {count}; if (import.meta.hot) import.meta.hot.accept(next => console.log(next));")
    };
    std::fs::write(&file, source(1)).unwrap();
    let config = ferrite_config::resolve_config(
        Default::default(),
        Some(root.path().into()),
        Default::default(),
    )
    .unwrap();
    let server = DevServer::new(config, vec![]).await.unwrap();
    let id = ModuleId::new("/entry.js");
    server.pipeline_module(&id, None, "client").await.unwrap();
    let mut messages = server.inner.hmr.subscribe();
    std::fs::write(&file, "export const count = ;").unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    std::fs::write(&file, source(2)).unwrap();
    let next = tokio::time::timeout(std::time::Duration::from_secs(10), messages.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(
            serde_json::from_str::<ferrite_hmr::HmrMessage>(&next).unwrap(),
            ferrite_hmr::HmrMessage::Update { .. }
        ),
        "{next}"
    );
    assert!(server
        .inner
        .graph
        .get(&id)
        .unwrap()
        .client
        .code
        .unwrap()
        .contains("= 2"));
    std::fs::write(&file, "export const count = ;").unwrap();
    let next = tokio::time::timeout(std::time::Duration::from_secs(10), messages.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(
            serde_json::from_str::<ferrite_hmr::HmrMessage>(&next).unwrap(),
            ferrite_hmr::HmrMessage::Error { .. }
        ),
        "{next}"
    );
    std::fs::write(&file, source(3)).unwrap();
    let next = tokio::time::timeout(std::time::Duration::from_secs(10), messages.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(
            serde_json::from_str::<ferrite_hmr::HmrMessage>(&next).unwrap(),
            ferrite_hmr::HmrMessage::Update { .. }
        ),
        "{next}"
    );
    assert!(server
        .inner
        .graph
        .get(&id)
        .unwrap()
        .client
        .code
        .unwrap()
        .contains("= 3"));
    server.close();
}

#[tokio::test]
async fn css_resolves_the_real_dev_client_and_local_import_failures_are_loud() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("style.css"), "button { color: red; }").unwrap();
    std::fs::write(root.path().join("bad.js"), "import './missing.js';").unwrap();
    let config = ferrite_config::resolve_config(
        Default::default(),
        Some(root.path().into()),
        Default::default(),
    )
    .unwrap();
    let server = DevServer::new_without_watcher(config, vec![])
        .await
        .unwrap();
    let css = server
        .pipeline_module(&ModuleId::new("/style.css"), None, "client")
        .await
        .unwrap();
    assert!(
        css.imports
            .iter()
            .any(|(_, id, _)| id.0 == ferrite_hmr::CLIENT_ID),
        "imports={:?}; code={}",
        css.imports,
        css.code
    );
    let client = server
        .pipeline_module(&ModuleId::new(ferrite_hmr::CLIENT_ID), None, "client")
        .await
        .unwrap();
    assert!(client.code.contains("new WebSocket"));
    let error = server
        .pipeline_module(&ModuleId::new("/bad.js"), None, "client")
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("missing.js") && error.contains("bad.js"),
        "{error}"
    );
    assert!(server
        .pipeline_module(&ModuleId::new(ferrite_hmr::CLIENT_ID), None, "ssr")
        .await
        .unwrap_err()
        .to_string()
        .contains("only in client development"));
    server.close();
}

struct FailingHmrHook {
    hook: &'static str,
    fail: std::sync::atomic::AtomicBool,
    modern: std::sync::atomic::AtomicUsize,
    legacy: std::sync::atomic::AtomicUsize,
}
#[async_trait::async_trait]
impl Plugin for FailingHmrHook {
    fn name(&self) -> &'static str {
        "failing-hmr-fixture"
    }
    async fn watch_change(&self, _: &PluginContext, _: ferrite_plugin::WatchEvent) -> Result<()> {
        if self.hook == "watch_change" && self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(ferrite_core::FerriteError::Other(
                "required watch hook failed".into(),
            ));
        }
        Ok(())
    }
    async fn hot_update(
        &self,
        _: &PluginContext,
        _: ferrite_plugin::HotUpdateEvent,
    ) -> Result<Option<ferrite_plugin::HotUpdateResult>> {
        self.modern
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.hook == "hot_update" && self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(ferrite_core::FerriteError::Other(
                "required modern hook failed".into(),
            ));
        }
        Ok(None)
    }
    async fn handle_hot_update(
        &self,
        _: &PluginContext,
        _: ferrite_plugin::HotUpdateEvent,
    ) -> Result<Option<ferrite_plugin::HotUpdateResult>> {
        self.legacy
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.hook == "handle_hot_update" && self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(ferrite_core::FerriteError::Other(
                "required legacy hook failed".into(),
            ));
        }
        Ok(None)
    }
}

#[tokio::test]
async fn watcher_reports_required_hook_failures_without_fallback_and_recovers() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    for hook in ["watch_change", "hot_update", "handle_hot_update"] {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("entry.js");
        let source = |count| {
            format!("export const count = {count}; if (import.meta.hot) import.meta.hot.accept();")
        };
        std::fs::write(&file, source(1)).unwrap();
        let config = ferrite_config::resolve_config(
            Default::default(),
            Some(root.path().into()),
            Default::default(),
        )
        .unwrap();
        let plugin = Arc::new(FailingHmrHook {
            hook,
            fail: AtomicBool::new(true),
            modern: AtomicUsize::new(0),
            legacy: AtomicUsize::new(0),
        });
        let server = DevServer::new(config, vec![plugin.clone()]).await.unwrap();
        let id = ModuleId::new("/entry.js");
        server.pipeline_module(&id, None, "client").await.unwrap();
        let mut messages = server.inner.hmr.subscribe();
        std::fs::write(&file, source(2)).unwrap();
        let next = tokio::time::timeout(std::time::Duration::from_secs(10), messages.recv())
            .await
            .unwrap()
            .unwrap();
        let ferrite_hmr::HmrMessage::Error { err } = serde_json::from_str(&next).unwrap() else {
            panic!("hook failure must stop the update: {next}");
        };
        assert_eq!(err.id.as_deref(), Some("/entry.js"));
        assert_eq!(err.plugin.as_deref(), Some("failing-hmr-fixture"));
        assert!(err.message.contains(hook), "{}", err.message);
        assert!(messages.try_recv().is_err(), "no fallback update or reload");
        assert_eq!(
            plugin.modern.load(Ordering::SeqCst),
            usize::from(hook != "watch_change")
        );
        assert_eq!(
            plugin.legacy.load(Ordering::SeqCst),
            usize::from(hook == "handle_hot_update")
        );
        assert!(server
            .inner
            .graph
            .get(&id)
            .unwrap()
            .client
            .code
            .unwrap()
            .contains("= 1"));
        plugin.fail.store(false, Ordering::SeqCst);
        std::fs::write(&file, source(3)).unwrap();
        let next = tokio::time::timeout(std::time::Duration::from_secs(10), messages.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(
                serde_json::from_str::<ferrite_hmr::HmrMessage>(&next).unwrap(),
                ferrite_hmr::HmrMessage::Update { .. }
            ),
            "{next}"
        );
        assert!(server
            .inner
            .graph
            .get(&id)
            .unwrap()
            .client
            .code
            .unwrap()
            .contains("= 3"));
        server.close();
    }
}

struct DelayedFailedValidation {
    enabled: std::sync::atomic::AtomicBool,
    calls: std::sync::atomic::AtomicUsize,
    active: std::sync::atomic::AtomicUsize,
    maximum: std::sync::atomic::AtomicUsize,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
#[async_trait::async_trait]
impl Plugin for DelayedFailedValidation {
    fn name(&self) -> &'static str {
        "delayed-validation"
    }
    async fn transform(
        &self,
        _: &PluginContext,
        request: TransformRequest,
    ) -> Result<Option<TransformResult>> {
        use std::sync::atomic::Ordering;
        if request.id != "/entry.js" || !self.enabled.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum.fetch_max(active, Ordering::SeqCst);
        if call == 0 {
            self.entered.notify_one();
            self.release.notified().await;
            self.active.fetch_sub(1, Ordering::SeqCst);
            return Err(ferrite_core::FerriteError::Other(
                "delayed failed edit".into(),
            ));
        }
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(None)
    }
}

#[tokio::test]
async fn concurrent_validation_cannot_roll_back_a_later_successful_edit() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("entry.js");
    std::fs::write(
        &file,
        "export const count = 1; if (import.meta.hot) import.meta.hot.accept();",
    )
    .unwrap();
    let config = ferrite_config::resolve_config(
        Default::default(),
        Some(root.path().into()),
        Default::default(),
    )
    .unwrap();
    let plugin = Arc::new(DelayedFailedValidation {
        enabled: AtomicBool::new(false),
        calls: AtomicUsize::new(0),
        active: AtomicUsize::new(0),
        maximum: AtomicUsize::new(0),
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let server = DevServer::new_without_watcher(config, vec![plugin.clone()])
        .await
        .unwrap();
    let id = ModuleId::new("/entry.js");
    server.pipeline_module(&id, None, "client").await.unwrap();
    let mut messages = server.inner.hmr.subscribe();
    plugin.enabled.store(true, Ordering::SeqCst);
    std::fs::write(&file, "export const count = 2;").unwrap();
    let first_server = server.clone();
    let first_id = id.clone();
    let first = tokio::spawn(async move {
        first_server.invalidate_module(&first_id).await;
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), plugin.entered.notified())
        .await
        .unwrap();
    std::fs::write(&file, "export const count = 3;").unwrap();
    let second_server = server.clone();
    let second_id = id.clone();
    let second = tokio::spawn(async move {
        second_server.invalidate_module(&second_id).await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    assert_eq!(
        plugin.calls.load(Ordering::SeqCst),
        1,
        "later validation must wait for the first rollback"
    );
    plugin.release.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        first.await.unwrap();
        second.await.unwrap();
    })
    .await
    .unwrap();
    assert_eq!(plugin.maximum.load(Ordering::SeqCst), 1);
    assert_eq!(plugin.calls.load(Ordering::SeqCst), 2);
    let first: ferrite_hmr::HmrMessage =
        serde_json::from_str(&messages.recv().await.unwrap()).unwrap();
    let second: ferrite_hmr::HmrMessage =
        serde_json::from_str(&messages.recv().await.unwrap()).unwrap();
    assert!(matches!(first, ferrite_hmr::HmrMessage::Error { .. }));
    assert!(matches!(second, ferrite_hmr::HmrMessage::Update { .. }));
    let node = server.inner.graph.get(&id).unwrap();
    assert!(node.client.code.unwrap().contains("= 3"));
    assert!(
        !node.hmr.self_accepting,
        "the old failure must not restore stale acceptance after the correction"
    );
    server.close();
}

#[tokio::test]
async fn failed_first_http_load_is_tracked_and_correction_clears_diagnostic() {
    for missing in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("entry.js");
        if !missing {
            std::fs::write(&file, "export const count = ;").unwrap();
        }
        let config = ferrite_config::resolve_config(
            Default::default(),
            Some(root.path().into()),
            Default::default(),
        )
        .unwrap();
        let server = DevServer::new(config, vec![]).await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = server.router();
        let serving = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let response = reqwest::get(format!("http://{address}/entry.js"))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            reqwest::StatusCode::INTERNAL_SERVER_ERROR
        );
        let id = ModuleId::new("/entry.js");
        assert!(server.inner.graph.contains(&id));
        assert_eq!(
            server
                .inner
                .hmr_error
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .id
                .as_deref(),
            Some("/entry.js")
        );
        let mut messages = server.inner.hmr.subscribe();
        std::fs::write(&file, "export const count = 1;").unwrap();
        let message = tokio::time::timeout(std::time::Duration::from_secs(10), messages.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(
                serde_json::from_str::<ferrite_hmr::HmrMessage>(&message).unwrap(),
                ferrite_hmr::HmrMessage::FullReload { .. }
            ),
            "{message}"
        );
        assert!(server.inner.hmr_error.lock().unwrap().is_none());
        let response = reqwest::get(format!("http://{address}/entry.js"))
            .await
            .unwrap();
        assert!(response.status().is_success());
        assert!(response.text().await.unwrap().contains("= 1"));
        server.close();
        serving.abort();
    }
}

#[tokio::test]
async fn creating_missing_import_candidates_recovers_the_importer_and_prunes_watches() {
    for specifier in ["./child", "@child", "./folder", "./data.txt?raw"] {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("folder")).unwrap();
        let entry = root.path().join("entry.js");
        std::fs::write(
            &entry,
            format!("import value from '{specifier}'; console.log(value);"),
        )
        .unwrap();
        let mut user = ferrite_config::UserConfig::default();
        user.resolve.alias.insert("@child".into(), "./child".into());
        let config =
            ferrite_config::resolve_config(user, Some(root.path().into()), Default::default())
                .unwrap();
        let server = DevServer::new(config, vec![]).await.unwrap();
        let id = ModuleId::new("/entry.js");
        assert!(server.pipeline_module(&id, None, "client").await.is_err());
        let candidates = server.inner.missing_imports.lock().unwrap()[&id].clone();
        assert!(!candidates.is_empty());
        let (relative, source) = match specifier {
            "./folder" => ("folder/index.js", "export default 42;"),
            "./data.txt?raw" => ("data.txt", "first raw value"),
            _ => ("child.js", "export default 42;"),
        };
        let mut messages = server.inner.hmr.subscribe();
        std::fs::write(root.path().join(relative), source).unwrap();
        let message = tokio::time::timeout(std::time::Duration::from_secs(10), messages.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(
                serde_json::from_str::<ferrite_hmr::HmrMessage>(&message).unwrap(),
                ferrite_hmr::HmrMessage::FullReload { .. }
            ),
            "{specifier}: {message}"
        );
        let entry = server.inner.graph.get(&id).unwrap();
        assert!(entry.client.code.is_some());
        assert!(!server
            .inner
            .missing_imports
            .lock()
            .unwrap()
            .contains_key(&id));
        for candidate in candidates {
            if let Some(node) = server.inner.graph.get(&candidate) {
                assert!(
                    !node.importers.is_empty() || node.client.code.is_some(),
                    "unused missing candidate must be pruned: {candidate}"
                );
            }
        }
        if specifier.contains("?raw") {
            let raw = ModuleId::new("/data.txt?raw");
            assert!(server
                .inner
                .graph
                .get(&ModuleId::new("/data.txt"))
                .unwrap()
                .importers
                .contains(&raw));
            std::fs::write(root.path().join(relative), "second raw value").unwrap();
            let message = tokio::time::timeout(std::time::Duration::from_secs(10), messages.recv())
                .await
                .unwrap()
                .unwrap();
            assert!(
                matches!(
                    serde_json::from_str::<ferrite_hmr::HmrMessage>(&message).unwrap(),
                    ferrite_hmr::HmrMessage::FullReload { .. }
                ),
                "{message}"
            );
            assert!(server
                .inner
                .graph
                .get(&raw)
                .unwrap()
                .client
                .code
                .unwrap()
                .contains("second raw value"));
        }
        server.close();
    }
}

#[tokio::test]
async fn writing_a_configured_lock_recovers_a_missing_bare_import() {
    for lockfile in ["selected.lock", ".ferrite/selected.lock"] {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join(".ferrite")).unwrap();
        let source = "import value from 'cold-package'; console.log(value);";
        std::fs::write(root.path().join("entry.js"), source).unwrap();
        let mut user = ferrite_config::UserConfig::default();
        user.npm.lockfile = lockfile.into();
        let config =
            ferrite_config::resolve_config(user, Some(root.path().into()), Default::default())
                .unwrap();
        let server = DevServer::new(config, vec![]).await.unwrap();
        let id = ModuleId::new("/entry.js");
        assert!(server.pipeline_module(&id, None, "client").await.is_err());
        let package = root.path().join(".ferrite/npm/packages/cold-package@1.0.0");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"cold-package","version":"1.0.0","type":"module","exports":"./index.js"}"#,
        )
        .unwrap();
        std::fs::write(package.join("index.js"), "export default 42;").unwrap();
        let mut messages = server.inner.hmr.subscribe();
        std::fs::write(
            root.path().join(lockfile),
            r#"version = 2
[importers.".".specifiers]
cold-package = "1.0.0"
[importers.".".dependencies]
cold-package = "cold-package@1.0.0"
[[package]]
name = "cold-package"
version = "1.0.0"
source = "npm"
"#,
        )
        .unwrap();
        let message = tokio::time::timeout(std::time::Duration::from_secs(10), messages.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(
                serde_json::from_str::<ferrite_hmr::HmrMessage>(&message).unwrap(),
                ferrite_hmr::HmrMessage::FullReload { .. }
            ),
            "{lockfile}: {message}"
        );
        assert!(server.inner.graph.get(&id).unwrap().client.code.is_some());
        assert!(!server
            .inner
            .missing_imports
            .lock()
            .unwrap()
            .contains_key(&id));
        assert_eq!(
            std::fs::read_to_string(root.path().join("entry.js")).unwrap(),
            source
        );
        server.close();
    }
}

#[tokio::test]
#[ignore = "requires explicit real Node hook host; executed separately"]
async fn foreign_factory_hooks_participate_in_the_shared_pipeline() {
    for production in [false, true] {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("entry.js"),
            "import { value } from 'generated'; console.log(value);",
        )
        .unwrap();
        std::fs::write(
            root.path().join("dependency.js"),
            "export const dependency = 42;",
        )
        .unwrap();
        let helper = root.path().join("helper.mjs");
        std::fs::write(&helper, "export const suffix = ';';").unwrap();
        let entry = root.path().join("plugin.mjs");
        std::fs::write(&entry, r#"
import {suffix} from "./helper.mjs";
export default options => ({
  name: 'pipeline-fixture',
  resolveId(id, importer, context) {
    if (id !== 'generated' && id !== '/generated.js') return null;
    if ((id === 'generated' && importer !== '/entry.js') || context.ssr !== false) throw new Error('missing resolution context');
    return {id: '/generated.js', moduleType: 'Js', sideEffects: false, meta: {fixture: true}};
  },
  load(id, context) {
    if (id !== '/generated.js') return null;
    if (context.ssr !== false) throw new Error('missing load context');
    return {code: "import { dependency } from './dependency.js'; export const value = dependency;", moduleType: 'Js', dependencies: ['dependency.js'], sideEffects: false,
      map: {version: 3, sources: ['original.js'], sourcesContent: ['export const value = 42;'], names: [], mappings: 'AAAA'}};
  },
  transform: {handler(code, id, context) {
    if (id !== '/generated.js') return null;
    if (context.ssr !== false) throw new Error('missing transform context');
    return {code: code + '\nexport const profile = ' + JSON.stringify(options.profile) + suffix, dependencies: ['plugin.mjs'],
      map: {version: 3, sources: [id], sourcesContent: [code], names: [], mappings: 'AAAA'}};
  }}
});
"#).unwrap();
        let host = std::sync::Arc::new(
            ferrite_plugin::node_adapter::NodeAdapterHost::spawn(None).unwrap(),
        );
        let plugin = ferrite_plugin::ForeignHookPlugin::register(
            host.clone(),
            "fixture",
            &entry,
            serde_json::json!({"profile":"official-hook-subset"}),
        )
        .unwrap();
        let mut config = ferrite_config::resolve_config(
            Default::default(),
            Some(root.path().into()),
            Default::default(),
        )
        .unwrap();
        config.is_production = production;
        let server = DevServer::new(config, vec![std::sync::Arc::new(plugin)])
            .await
            .unwrap();
        let result = server
            .pipeline_module(&ModuleId::new("/entry.js"), None, "client")
            .await
            .unwrap();
        assert!(result.code.contains("/generated.js"));
        let generated = server
            .pipeline_module(&ModuleId::new("/generated.js"), None, "client")
            .await
            .unwrap();
        assert!(generated.code.contains("official-hook-subset"));
        assert!(generated.code.contains("/dependency.js"));
        assert!(generated
            .dependencies
            .iter()
            .any(|path| path.ends_with("dependency.js")));
        assert!(generated
            .dependencies
            .iter()
            .any(|path| path.ends_with("plugin.mjs")));
        assert!(generated.map.is_some());
        let map =
            oxc_sourcemap::SourceMap::from_json_string(generated.map.as_ref().unwrap()).unwrap();
        assert!(map
            .get_sources()
            .any(|source| source.ends_with("original.js")));
        assert_eq!(generated.side_effects, Some(false));
        let node = server
            .inner
            .graph
            .get(&ModuleId::new("/generated.js"))
            .unwrap();
        assert!(node
            .imports
            .iter()
            .any(|edge| edge.resolved == ModuleId::new("/dependency.js")));
        assert!(generated
            .dependencies
            .iter()
            .any(|path| path.ends_with("helper.mjs")));
        let helper_identity = server.inner.plugins.cache_key();
        std::fs::write(&helper, "export const suffix = 'changed';").unwrap();
        assert_ne!(helper_identity, server.inner.plugins.cache_key());
        let error = server
            .pipeline_module(&ModuleId::new("/generated.js"), None, "client")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("dependency changed"), "{error}");
        // Restore the dependency to exercise the separate entry-change guard.
        std::fs::write(&helper, "export const suffix = ';';").unwrap();
        let cache_identity = server.inner.plugins.cache_key();
        std::fs::write(
            &entry,
            "export default {transform() { return 'changed'; }};",
        )
        .unwrap();
        assert_ne!(cache_identity, server.inner.plugins.cache_key());
        let replacement = ferrite_plugin::ForeignHookPlugin::register(
            host.clone(),
            "replacement",
            &entry,
            serde_json::Value::Null,
        );
        assert!(
            replacement.is_err(),
            "re-registering the entry must not reuse Node's cached module"
        );
        let error = server
            .pipeline_module(&ModuleId::new("/generated.js"), None, "client")
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("recreate its explicit Node host"),
            "{error}"
        );
        server.close();
        host.shutdown();
    }
}
