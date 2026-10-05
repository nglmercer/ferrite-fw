//! Official framework packages through Ferrite's real shared pipeline.
use ferrite::frameworks::{compiler_host::NodeCompilerHost, SveltePlugin, VuePlugin};
use ferrite::{server::DevServer, Ferrite, ModuleId, ModuleRequest};
use ferrite_test::TempProject;
use std::{sync::Arc, time::Duration};

async fn fixture() -> TempProject {
    let project = TempProject::new(&[
        ("Counter.vue", "<script setup lang='ts'>import { ref } from 'vue'; const count = ref<number>(0);</script><template><button @click='count++'>{{ count }}</button></template><style scoped>button { color: red; }</style><style module>.label { font-weight: bold; }</style>"),
        ("Counter.svelte", "<script lang='ts'>let count: number = $state(0);</script><button onclick={() => count++}>{count}</button><style>button { color: red; }</style>"),
        ("counter.svelte.ts", "let count: number = $state(0); export function increment() { return ++count; }"),
    ]);
    let npm = project.root.join(".ferrite/npm");
    let client =
        ferrite::npm::RegistryClient::new(ferrite::npm::DEFAULT_REGISTRY, npm.join("metadata"))
            .unwrap();
    let installer = ferrite::npm::Installer::new(client, npm);
    let manifest = ferrite::npm::JsPackageJson {
        dependencies: std::collections::HashMap::from([
            ("vue".into(), "3.5.22".into()),
            ("svelte".into(), "5.39.6".into()),
            ("typescript".into(), "5.9.3".into()),
        ]),
        ..Default::default()
    };
    manifest.write(&project.root.join("package.json")).unwrap();
    let mut lock = ferrite::npm::Lockfile::default();
    installer
        .install_manifest(&manifest, &mut lock, false)
        .await
        .unwrap();
    lock.write(&project.root.join("ferrite.lock")).unwrap();
    project
}

#[tokio::test]
#[ignore = "requires real Node and registry access; executed explicitly"]
async fn official_components_resources_maps_cache_and_library_parity() {
    let project = fixture().await;
    let host = Arc::new(
        NodeCompilerHost::new(
            project.root.to_path_buf(),
            project.root.join("ferrite.lock"),
            None,
            Duration::from_secs(10),
        )
        .await
        .unwrap(),
    );
    let mut config = project.resolve_config();
    config.build.sourcemap = ferrite::config::SourceMapConfig::Bool(true);
    let plugins: Vec<Arc<dyn ferrite::plugin::Plugin>> = vec![
        Arc::new(VuePlugin::with_host(host.clone(), true)),
        Arc::new(SveltePlugin::with_host(host, true)),
    ];
    let server = DevServer::new_without_watcher(config.clone(), plugins.clone())
        .await
        .unwrap();
    let library = Ferrite::new(config, plugins).unwrap();
    for component in ["/Counter.vue", "/Counter.svelte", "/counter.svelte.ts"] {
        let module = server
            .pipeline_module(&ModuleId::new(component), None, "client")
            .await
            .unwrap();
        assert_eq!(module.module_type, ferrite::ModuleType::Js);
        assert!(module.map.is_some(), "{component}");
        assert!(!module.code.contains(": number"), "{component}");
        let cached = server
            .pipeline_module(&ModuleId::new(component), None, "client")
            .await
            .unwrap();
        assert_eq!(cached.code, module.code);
        let from_library = library
            .transform_request(ModuleRequest {
                specifier: component.into(),
                importer: None,
                environment: ferrite::EnvironmentKind::Client,
            })
            .await
            .unwrap();
        assert_eq!(
            module.code, from_library.code,
            "CLI/library compiler pipeline parity"
        );
    }
    let vue = server
        .pipeline_module(&ModuleId::new("/Counter.vue"), None, "client")
        .await
        .unwrap();
    assert!(vue
        .imports
        .iter()
        .any(|(_, id, _)| id.0 == "/Counter.vue?ferrite-style=0"));
    assert!(vue
        .imports
        .iter()
        .any(|(_, id, _)| id.0 == "/Counter.vue?ferrite-style=1"));
    let style = server
        .pipeline_module(
            &ModuleId::new("/Counter.vue?ferrite-style=0"),
            None,
            "client",
        )
        .await
        .unwrap();
    assert!(style.code.contains("data-v-"));
    assert!(style.code.contains("red"));
    assert!(style
        .dependencies
        .iter()
        .any(|dependency| dependency.ends_with("Counter.vue")));
    let second = server
        .pipeline_module(
            &ModuleId::new("/Counter.vue?ferrite-style=1"),
            None,
            "client",
        )
        .await
        .unwrap();
    assert!(second.code.contains("font-weight"));
    assert_ne!(style.id, second.id);
    let original = std::fs::read_to_string(project.root.join("Counter.vue")).unwrap();
    std::fs::write(
        project.root.join("Counter.vue"),
        original.replace("color: red", "color: blue"),
    )
    .unwrap();
    let updated = server
        .pipeline_module(
            &ModuleId::new("/Counter.vue?ferrite-style=0"),
            None,
            "client",
        )
        .await
        .unwrap();
    assert!(updated.code.contains("blue"));
    assert!(!updated.code.contains("color:red"));
    let raw = server
        .pipeline_module(&ModuleId::new("/Counter.vue?raw"), None, "client")
        .await
        .unwrap();
    assert!(
        raw.code.contains("<script setup"),
        "raw imports must return original component source"
    );
    let server_module = server
        .pipeline_module(&ModuleId::new("/Counter.vue"), None, "ssr")
        .await
        .unwrap();
    assert!(server_module.code.contains("ssrRender"));
    assert!(
        server_module
            .imports
            .iter()
            .any(|(_, id, _)| id.0 == "/Counter.vue?ferrite-style=0"),
        "SSR must preserve scoped style graph edges"
    );
    assert!(!server_module.code.contains("import.meta.hot"));
    let svelte_server = server
        .pipeline_module(&ModuleId::new("/Counter.svelte"), None, "ssr")
        .await
        .unwrap();
    assert!(
        svelte_server
            .imports
            .iter()
            .any(|(_, id, _)| id.0 == "/@npm/svelte@5.39.6/src/internal/server/index.js"),
        "{}",
        svelte_server.code
    );
    assert!(
        !svelte_server
            .imports
            .iter()
            .any(|(_, id, _)| id.0.contains("/internal/client/")),
        "{}",
        svelte_server.code
    );
    assert!(
        !svelte_server.code.contains("import.meta.hot"),
        "{}",
        svelte_server.code
    );
    assert!(
        !svelte_server.code.contains("document."),
        "SSR styles must not inject into a browser DOM"
    );
    assert!(
        svelte_server.map.is_some(),
        "server compiler maps must survive lowering"
    );
    let runes_server = server
        .pipeline_module(&ModuleId::new("/counter.svelte.ts"), None, "ssr")
        .await
        .unwrap();
    assert!(
        !runes_server.code.contains("$state("),
        "{}",
        runes_server.code
    );
    assert!(
        !runes_server.code.contains(": number"),
        "{}",
        runes_server.code
    );
    assert!(
        !runes_server.code.contains("import.meta.hot"),
        "{}",
        runes_server.code
    );

    std::fs::write(
        project.root.join("Counter.vue"),
        "<template><button>only one style</button></template><style>button{color:green}</style>",
    )
    .unwrap();
    let error = server
        .pipeline_module(
            &ModuleId::new("/Counter.vue?ferrite-style=1"),
            None,
            "client",
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("no longer exists"), "{error}");
    std::fs::write(
        project.root.join("Counter.vue"),
        "<template>\n<div>\n</template>\n",
    )
    .unwrap();
    let error = server
        .pipeline_module(&ModuleId::new("/Counter.vue"), None, "client")
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("Counter.vue:2:1"), "{error}");
    assert!(error.contains("missing end tag"), "{error}");
    std::fs::write(project.root.join("Counter.vue"), &original).unwrap();
    let types = project.root.join("props.ts");
    std::fs::write(&types, "export interface Props { value: string }").unwrap();
    std::fs::write(project.root.join("Typed.vue"), "<script setup lang='ts'>import type { Props } from './props'; defineProps<Props>();</script><template>{{ value }}</template>").unwrap();
    let typed = server
        .pipeline_module(&ModuleId::new("/Typed.vue"), None, "client")
        .await
        .unwrap();
    assert!(typed
        .dependencies
        .iter()
        .any(|path| path.ends_with("props.ts")));
    assert!(typed.code.contains("type: String"), "{}", typed.code);
    std::fs::write(&types, "export interface Props { value: number }").unwrap();
    let updated = server
        .pipeline_module(&ModuleId::new("/Typed.vue"), None, "client")
        .await
        .unwrap();
    assert!(updated.code.contains("type: Number"), "{}", updated.code);
    assert!(!updated.code.contains("type: String"), "{}", updated.code);
    let leaf = project.root.join("leaf.ts");
    std::fs::write(&leaf, "export interface Props { value: string }").unwrap();
    std::fs::write(&types, "export type { Props } from './leaf';").unwrap();
    let via_barrel = server
        .pipeline_module(&ModuleId::new("/Typed.vue"), None, "client")
        .await
        .unwrap();
    assert!(via_barrel
        .dependencies
        .iter()
        .any(|path| path.ends_with("leaf.ts")));
    assert!(
        via_barrel.code.contains("type: String"),
        "{}",
        via_barrel.code
    );
    std::fs::write(&leaf, "export interface Props { value: boolean }").unwrap();
    let updated_leaf = server
        .pipeline_module(&ModuleId::new("/Typed.vue"), None, "client")
        .await
        .unwrap();
    assert!(
        updated_leaf.code.contains("type: Boolean"),
        "{}",
        updated_leaf.code
    );
    assert!(
        !updated_leaf.code.contains("type: String"),
        "{}",
        updated_leaf.code
    );
    std::fs::write(&leaf, "export interface Props { value: ; }").unwrap();
    let invalid_type = server
        .pipeline_module(&ModuleId::new("/Typed.vue"), None, "client")
        .await
        .unwrap_err();
    assert!(
        invalid_type.to_string().contains("Unexpected token"),
        "{invalid_type}"
    );
    std::fs::write(&leaf, "export interface Props { value: number }").unwrap();
    let recovered_type = server
        .pipeline_module(&ModuleId::new("/Typed.vue"), None, "client")
        .await
        .unwrap();
    assert!(
        recovered_type.code.contains("type: Number"),
        "{}",
        recovered_type.code
    );
    assert!(
        !recovered_type.code.contains("type: Boolean"),
        "{}",
        recovered_type.code
    );
    // The same declarative opt-in is resolved by CLI and library entrypoints.
    std::fs::write(
        project.root.join("ferrite.toml"),
        "[framework]\nenabled=['vue','svelte']\ncompiler_host='node'\n[runtime]\nbackend='boa'\n",
    )
    .unwrap();
    let (configured, plugins) = ferrite::Config {
        root: Some(project.root.clone()),
        plugins: vec![
            Arc::new(VuePlugin::new(project.root.clone())),
            Arc::new(SveltePlugin::new(project.root.clone())),
        ],
        ..Default::default()
    }
    .resolve()
    .await
    .unwrap();
    assert_eq!(configured.runtime.backend, "boa");
    assert!(plugins
        .iter()
        .any(|plugin| plugin.name() == "ferrite:vue-official"));
    assert!(plugins
        .iter()
        .any(|plugin| plugin.name() == "ferrite:svelte-official"));
    assert!(!plugins.iter().any(|plugin| plugin.name() == "ferrite:vue"));
    let configured_server = DevServer::new_without_watcher(configured, plugins)
        .await
        .unwrap();
    for component in ["/Counter.vue", "/Counter.svelte"] {
        let module = configured_server
            .pipeline_module(&ModuleId::new(component), None, "client")
            .await
            .unwrap();
        assert_eq!(module.module_type, ferrite::ModuleType::Js);
    }
    std::fs::write(
        project.root.join("ferrite.toml"),
        "[framework]\nenabled = [",
    )
    .unwrap();
    let error = match (ferrite::Config {
        root: Some(project.root.clone()),
        ..Default::default()
    })
    .resolve()
    .await
    {
        Ok(_) => panic!("malformed explicit framework configuration was silently ignored"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("ferrite.toml"), "{error}");
}

// Test-only progress hooks locate production failures without changing output.
struct BuildProbe;
#[async_trait::async_trait]
impl ferrite::plugin::Plugin for BuildProbe {
    fn name(&self) -> &'static str {
        "acceptance:build-progress"
    }
    async fn transform(
        &self,
        _: &ferrite::plugin::PluginContext,
        request: ferrite::plugin::TransformRequest,
    ) -> ferrite::Result<Option<ferrite::plugin::TransformResult>> {
        eprintln!("production lowered {}", request.id);
        Ok(None)
    }
    async fn render_start(
        &self,
        _: &ferrite::plugin::PluginContext,
        _: ferrite::plugin::RenderStart,
    ) -> ferrite::Result<()> {
        eprintln!("production traversal and statement shaking finished");
        Ok(())
    }
}

async fn browser_interaction(kind: ferrite_e2e::BrowserKind) {
    use ferrite_e2e::{Browser, BrowserKind, LaunchOptions};
    let executable = match kind {
        BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
        BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
    }
    .expect("framework acceptance requires the selected real browser");
    let browser = Browser::launch(
        LaunchOptions::default()
            .browser(kind)
            .executable(executable),
    )
    .await
    .unwrap();
    let page = browser.new_page().await.unwrap();
    let project = fixture().await;
    std::fs::write(project.root.join("index.html"), "<!doctype html><html><head><link rel='icon' href='data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22/%3E'></head><body><div id='vue'></div><div id='svelte'></div><script type='module' src='/main.js'></script></body></html>").unwrap();
    std::fs::write(project.root.join("main.js"), "import { createApp } from 'vue'; import { mount } from 'svelte'; import VueCounter from './Counter.vue'; import SvelteCounter from './Counter.svelte'; createApp(VueCounter).mount('#vue'); mount(SvelteCounter, { target: document.querySelector('#svelte') });").unwrap();
    let host = Arc::new(
        NodeCompilerHost::new(
            project.root.clone(),
            project.root.join("ferrite.lock"),
            None,
            Duration::from_secs(10),
        )
        .await
        .unwrap(),
    );
    let plugins: Vec<Arc<dyn ferrite::plugin::Plugin>> = vec![
        Arc::new(VuePlugin::with_host(host.clone(), true)),
        Arc::new(SveltePlugin::with_host(host.clone(), true)),
    ];
    let server = DevServer::new(project.resolve_config(), plugins)
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let router = server.router();
    let serving = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    page.goto(&url).await.unwrap();
    page.wait_for_function("document.querySelector('#vue button')?.textContent === '0' && document.querySelector('#svelte button')?.textContent === '0'", Duration::from_secs(15)).await.unwrap_or_else(|error| panic!("{error}; {:?}; {:?}", page.page_errors(), page.console_messages()));
    eprintln!("{kind:?}: rendered and interactive checkpoint");
    for id in ["#vue button", "#svelte button"] {
        page.locator(id).click().await.unwrap();
        page.wait_for_function(
            &format!("document.querySelector({id:?})?.textContent === '1'"),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    }
    let color: String = page
        .evaluate("getComputedStyle(document.querySelector('#vue button')).color")
        .await
        .unwrap();
    assert_eq!(color, "rgb(255, 0, 0)");
    let source = project.read("Counter.vue");
    std::fs::write(
        project.root.join("Counter.vue"),
        source.replace("color: red", "color: blue"),
    )
    .unwrap();
    // This experimental profile deliberately reloads rather than claiming HMR
    // state preservation. Both runtimes must render and interact after invalidation.
    page.wait_for_function("getComputedStyle(document.querySelector('#vue button')).color === 'rgb(0, 0, 255)' && document.querySelector('#vue button').textContent === '0'", Duration::from_secs(15)).await.unwrap_or_else(|error| panic!("{error}; {:?}", page.page_errors()));
    assert_eq!(
        page.locator("#svelte button")
            .text_content()
            .await
            .unwrap()
            .as_deref(),
        Some("0")
    );
    assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
    assert!(
        page.console_messages()
            .iter()
            .all(|message| message.kind != "error"),
        "{:?}",
        page.console_messages()
    );
    let mut config = project.resolve_config_mode("production");
    config.build.scope_hoist = true;
    let plugins: Vec<Arc<dyn ferrite::plugin::Plugin>> = vec![
        Arc::new(VuePlugin::with_host(host.clone(), false)),
        Arc::new(SveltePlugin::with_host(host, false)),
        Arc::new(BuildProbe),
    ];
    eprintln!("{kind:?}: invalidation passed; starting production build");
    let report = ferrite::Builder::new(config, plugins)
        .build("client")
        .await
        .unwrap();
    eprintln!("{kind:?}: production build finished");
    let assets: Vec<_> = std::fs::read_dir(report.out_dir.join("assets"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert!(
        assets
            .iter()
            .any(|path| path.extension().is_some_and(|extension| extension == "css")),
        "framework CSS must be extracted in production"
    );
    for path in &assets {
        if path.extension().is_some_and(|extension| extension == "js") {
            let code = std::fs::read_to_string(path).unwrap();
            assert!(!code.contains("/@ferrite/client"));
            assert!(!code.contains("__ferrite_create_hot__"));
        }
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let out = report.out_dir;
    let preview = tokio::spawn(async move {
        ferrite::preview_dir(&out, port).await.unwrap();
    });
    let mut ready = false;
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(ready, "preview did not start");
    page.goto(&format!("http://127.0.0.1:{port}"))
        .await
        .unwrap();
    page.wait_for_function("document.querySelector('#vue button')?.textContent === '0' && document.querySelector('#svelte button')?.textContent === '0'", Duration::from_secs(15)).await.unwrap_or_else(|error| panic!("{error}; {:?}; {:?}", page.page_errors(), page.console_messages()));
    for id in ["#vue button", "#svelte button"] {
        page.locator(id).click().await.unwrap();
        page.wait_for_function(
            &format!("document.querySelector({id:?})?.textContent === '1'"),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    }
    let color: String = page
        .evaluate("getComputedStyle(document.querySelector('#vue button')).color")
        .await
        .unwrap();
    assert_eq!(color, "rgb(0, 0, 255)");
    assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
    assert!(
        page.console_messages()
            .iter()
            .all(|message| message.kind != "error"),
        "{:?}",
        page.console_messages()
    );
    serving.abort();
    server.close();
    preview.abort();
    browser.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires real Node, registry and Chromium; executed explicitly"]
async fn chromium_framework_dev_edit_build_preview_interaction() {
    browser_interaction(ferrite_e2e::BrowserKind::Chromium).await;
}
#[tokio::test]
#[ignore = "requires real Node, registry and Firefox; executed explicitly"]
async fn firefox_framework_dev_edit_build_preview_interaction() {
    browser_interaction(ferrite_e2e::BrowserKind::Firefox).await;
}

#[cfg(feature = "napi-vm")]
#[tokio::test]
#[ignore = "requires official packages and explicit Node compiler host; embedded SSR conformance"]
async fn official_svelte_server_renderer_executes_emitted_graph() {
    let project = fixture().await;
    std::fs::create_dir_all(project.root.join("src")).unwrap();
    std::fs::write(project.root.join("src/entry-server.ts"), "import { render as renderComponent } from 'svelte/server'; import Counter from '../Counter.svelte'; export function render() { return renderComponent(Counter).body; }").unwrap();
    let host = Arc::new(
        NodeCompilerHost::new(
            project.root.clone(),
            project.root.join("ferrite.lock"),
            None,
            Duration::from_secs(10),
        )
        .await
        .unwrap(),
    );
    let mut config = project.resolve_config_mode("production");
    config.runtime.backend = "napi-vm".into();
    config.build.minify = false;
    let builder =
        ferrite::Builder::new(config, vec![Arc::new(SveltePlugin::with_host(host, false))]);
    let report = builder.build("ssr").await.unwrap();
    let artifact = ferrite::SsrRendererArtifact::read(&report.out_dir).unwrap();
    if let Some(path) = std::env::var_os("FERRITE_SVELTE_SSR_ARTIFACT") {
        std::fs::write(path, serde_json::to_vec(&artifact).unwrap()).unwrap();
    }

    let adapter = artifact
        .into_adapter(
            Arc::new(ferrite::runtime::napi_vm::NapiVmRuntime::with_defaults()),
            "<!--ssr-outlet-->".into(),
            "/",
        )
        .unwrap();
    let response = adapter
        .render(
            ferrite::ssr::SsrHttpRequest {
                method: "GET".into(),
                uri: "/".into(),
                headers: vec![],
                body: vec![],
            },
            Default::default(),
        )
        .await
        .unwrap();
    let html = response.into_string().await.unwrap();
    assert!(html.contains("<button"), "{html}");
    assert!(html.contains(">0</button>"), "{html}");
    assert!(
        !html.contains("<script"),
        "renderer must return meaningful server HTML: {html}"
    );
}

#[cfg(feature = "napi-vm")]
#[tokio::test]
#[ignore = "requires official packages and explicit Node compiler host; embedded Vue SSR conformance"]
async fn official_vue_server_renderer_executes_emitted_graph() {
    vue_ssr_conformance(None).await;
}

#[cfg(feature = "napi-vm")]
async fn vue_ssr_conformance(kind: Option<ferrite_e2e::BrowserKind>) {
    let project = fixture().await;
    std::fs::create_dir_all(project.root.join("src")).unwrap();
    std::fs::write(project.root.join("src/entry-server.ts"), "import { createSSRApp } from 'vue'; import { renderToString } from 'vue/server-renderer'; import Counter from '../Counter.vue'; export async function render() { globalThis.__ferrite_vue_requests = (globalThis.__ferrite_vue_requests || 0) + 1; const html = await renderToString(createSSRApp(Counter)); return { html, headers: [['x-request-count', String(globalThis.__ferrite_vue_requests)]] }; }").unwrap();
    std::fs::write(project.root.join("index.html"), "<html><head><link rel=\"icon\" href=\"data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22/%3E\"></head><body><div id=\"app\"><!--ssr-outlet--></div><script>globalThis.__ssrButton = document.querySelector('#app button');</script><script type=\"module\" src=\"/src/main.js\"></script></body></html>").unwrap();
    std::fs::write(project.root.join("src/main.js"), "import { createSSRApp } from 'vue'; import Counter from '../Counter.vue'; createSSRApp(Counter).mount('#app'); globalThis.__hydrated = true;").unwrap();
    let host = Arc::new(
        NodeCompilerHost::new(
            project.root.clone(),
            project.root.join("ferrite.lock"),
            None,
            Duration::from_secs(10),
        )
        .await
        .unwrap(),
    );
    let mut config = project.resolve_config_mode("production");
    config.runtime.backend = "napi-vm".into();
    config.build.minify = false;
    let builder = ferrite::Builder::new(
        config.clone(),
        vec![Arc::new(VuePlugin::with_host(host, false))],
    );
    let reports = builder.build_app().await.unwrap();
    let report = reports.iter().find(|report| report.env == "ssr").unwrap();
    let artifact = ferrite::SsrRendererArtifact::read(&report.out_dir).unwrap();
    if let Some(path) = std::env::var_os("FERRITE_VUE_SSR_ARTIFACT") {
        std::fs::write(path, serde_json::to_vec(&artifact).unwrap()).unwrap();
    }
    let artifact_styles = artifact.stylesheets.clone();
    let adapter = artifact
        .into_adapter(
            Arc::new(ferrite::runtime::napi_vm::NapiVmRuntime::with_defaults()),
            "<!--ssr-outlet-->".into(),
            "/",
        )
        .unwrap();
    let response = adapter
        .render(
            ferrite::ssr::SsrHttpRequest {
                method: "GET".into(),
                uri: "/".into(),
                headers: vec![],
                body: vec![],
            },
            Default::default(),
        )
        .await
        .unwrap();
    assert!(
        response
            .headers
            .iter()
            .any(|(name, value)| name == "x-request-count" && value == "1"),
        "{:?}",
        response.headers
    );
    let html = response.into_string().await.unwrap();
    assert!(html.contains("<button"), "{html}");
    assert!(html.contains(">0</button>"), "{html}");
    assert!(
        html.contains("data-v-"),
        "scoped styles must match server markup: {html}"
    );
    assert!(
        !artifact_styles.is_empty(),
        "SSR component styles must be published"
    );
    let scope = html
        .split("data-v-")
        .nth(1)
        .unwrap()
        .split(|character: char| !character.is_ascii_alphanumeric())
        .next()
        .unwrap()
        .to_string();
    let reservation = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    config.server.port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let port = config.server.port;
    let serving = tokio::spawn(async move { ferrite::preview_with_plugins(&config, &[]).await });
    struct Preview(tokio::task::JoinHandle<ferrite::Result<()>>);
    impl Drop for Preview {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let preview = Preview(serving);
    async fn get(port: u16, path: &str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut socket = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        socket
            .write_all(format!("GET {path} HTTP/1.0\r\nHost: localhost\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).await.unwrap();
        response
    }
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_ok()
            {
                break;
            }
            assert!(!preview.0.is_finished(), "preview stopped before listening");
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let response = get(port, "/").await;
    assert!(response.starts_with("HTTP/1.0 200"), "{response}");
    assert!(response.contains(">0</button>"), "{response}");
    assert!(
        response.to_lowercase().contains("x-request-count: 1"),
        "{response}"
    );
    let mut css = String::new();
    for stylesheet in artifact_styles {
        let url = format!("/ssr-assets/{stylesheet}");
        assert!(response.contains(&url), "{response}");
        let style = get(port, &url).await;
        assert!(style.starts_with("HTTP/1.0 200"), "{style}");
        css.push_str(&style);
    }
    assert!(css.contains(&format!("data-v-{scope}")), "{css}");
    assert!(css.contains("red"), "{css}");
    let private = get(port, "/server/renderer.json").await;
    assert!(private.starts_with("HTTP/1.0 404"), "{private}");
    if let Some(kind) = kind {
        use ferrite_e2e::{Browser, BrowserKind, LaunchOptions};
        let executable = match kind {
            BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
            BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
        }
        .expect("SSR hydration conformance requires the selected browser");
        let browser = Browser::launch(
            LaunchOptions::default()
                .browser(kind)
                .executable(executable),
        )
        .await
        .unwrap();
        let page = browser.new_page().await.unwrap();
        page.goto(&format!("http://127.0.0.1:{port}/"))
            .await
            .unwrap();
        page.wait_for_function("globalThis.__hydrated === true", Duration::from_secs(15))
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "{error}; {:?}; {:?}",
                    page.page_errors(),
                    page.console_messages()
                )
            });
        assert_eq!(
            page.evaluate::<serde_json::Value>(
                "globalThis.__ssrButton === document.querySelector('#app button')"
            )
            .await
            .unwrap(),
            serde_json::json!(true),
            "hydration must retain the server-rendered button"
        );
        page.locator("#app button").click().await.unwrap();
        page.wait_for_function(
            "document.querySelector('#app button').textContent === '1'",
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert_eq!(
            page.evaluate::<serde_json::Value>(
                "getComputedStyle(document.querySelector('#app button')).color"
            )
            .await
            .unwrap(),
            serde_json::json!("rgb(255, 0, 0)")
        );
        assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
        assert!(
            page.console_messages()
                .iter()
                .all(|message| message.kind != "error"
                    && !message.text.to_lowercase().contains("hydration")),
            "{:?}",
            page.console_messages()
        );
        browser.close().await.unwrap();
    }
}

#[cfg(feature = "napi-vm")]
#[tokio::test]
#[ignore = "requires saved artifact from the official Vue renderer conformance case"]
async fn official_vue_artifact_renders_without_compiler_host() {
    let path = std::env::var_os("FERRITE_VUE_SSR_ARTIFACT").expect("set FERRITE_VUE_SSR_ARTIFACT to the artifact produced by official_vue_server_renderer_executes_emitted_graph");
    let artifact = ferrite::SsrRendererArtifact::from_bytes(&std::fs::read(path).unwrap()).unwrap();
    let adapter = artifact
        .into_adapter(
            Arc::new(ferrite::runtime::napi_vm::NapiVmRuntime::with_defaults()),
            "<!--ssr-outlet-->".into(),
            "/",
        )
        .unwrap();
    let mut requests = Vec::new();
    for url in ["/first", "/second", "/third"] {
        let adapter = adapter.clone();
        requests.push(tokio::spawn(async move {
            let response = adapter
                .render(
                    ferrite::ssr::SsrHttpRequest {
                        method: "GET".into(),
                        uri: url.into(),
                        headers: vec![],
                        body: vec![],
                    },
                    Default::default(),
                )
                .await
                .unwrap();
            assert!(
                response
                    .headers
                    .iter()
                    .any(|(name, value)| name == "x-request-count" && value == "1"),
                "{:?}",
                response.headers
            );
            let html = response.into_string().await.unwrap();
            assert!(html.contains("<button"), "{html}");
            assert!(html.contains(">0</button>"), "{html}");
            assert!(html.contains("data-v-"), "{html}");
        }));
    }
    for request in requests {
        request.await.unwrap();
    }
}

#[cfg(feature = "napi-vm")]
#[tokio::test]
#[ignore = "official packages, Node compiler host and Chromium; actual Vue SSR hydration"]
async fn official_vue_ssr_hydrates_in_chromium() {
    vue_ssr_conformance(Some(ferrite_e2e::BrowserKind::Chromium)).await;
}

#[cfg(feature = "napi-vm")]
#[tokio::test]
#[ignore = "official packages, Node compiler host and Firefox; actual Vue SSR hydration"]
async fn official_vue_ssr_hydrates_in_firefox() {
    vue_ssr_conformance(Some(ferrite_e2e::BrowserKind::Firefox)).await;
}

#[cfg(feature = "napi-vm")]
#[tokio::test]
#[ignore = "real registry/Node compiler host and both browsers; generated Vue SSR acceptance"]
async fn generated_vue_ssr_sources_install_build_and_render() {
    for language in ["js", "ts"] {
        let mut generated = ferrite::frameworks::scaffold::vue_ssr_files(
            "generated-ssr",
            language,
            "node",
            "napi-vm",
        )
        .unwrap();
        if language == "ts" {
            // Exercise the generated dependency graph, without injecting TypeScript.
            generated.insert(
                "src/props.ts".into(),
                "export interface Props { label?: string }\n".into(),
            );
            let component = generated.get_mut("src/App.vue").unwrap();
            *component = component.replace(
                "import { ref } from 'vue';",
                "import { ref } from 'vue';\nimport type { Props } from './props';\ndefineProps<Props>();",
            );
        }
        // Observability only: snapshot before module execution and signal mount.
        let index = generated.get_mut("index.html").unwrap();
        *index = index.replace("<script type=", "<script>globalThis.__ssrButton = document.querySelector('#app button');</script><script type=");
        generated
            .get_mut(&format!("src/main.{language}"))
            .unwrap()
            .push_str("\nglobalThis.__hydrated = true;\n");
        let inputs: Vec<_> = generated
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str()))
            .collect();
        let project = TempProject::new(&inputs);
        let manifest: ferrite::npm::JsPackageJson =
            serde_json::from_str(&generated["package.json"]).unwrap();
        let npm = project.root.join(".ferrite/npm");
        let client =
            ferrite::npm::RegistryClient::new(ferrite::npm::DEFAULT_REGISTRY, npm.join("metadata"))
                .unwrap();
        let installer = ferrite::npm::Installer::new(client, npm);
        let mut lock = ferrite::npm::Lockfile::default();
        installer
            .install_manifest(&manifest, &mut lock, false)
            .await
            .unwrap();
        lock.write(&project.root.join("ferrite.lock")).unwrap();
        let builder = ferrite::create_builder(ferrite::Config {
            root: Some(project.root.clone()),
            overrides: ferrite::CliOverrides {
                mode: Some("production".into()),
                ..Default::default()
            },
            ..Default::default()
        })
        .await
        .unwrap();
        assert_eq!(builder.config.runtime.backend, "napi-vm");
        let reports = builder.build_app().await.unwrap();
        assert_eq!(reports.len(), 2);
        let server = reports.iter().find(|report| report.env == "ssr").unwrap();
        let artifact = ferrite::SsrRendererArtifact::read(&server.out_dir).unwrap();
        assert!(
            !artifact.stylesheets.is_empty(),
            "{language}: missing component CSS"
        );
        let adapter = artifact
            .into_adapter(
                Arc::new(ferrite::runtime::napi_vm::NapiVmRuntime::with_defaults()),
                "<!--ssr-outlet-->".into(),
                "/",
            )
            .unwrap();
        let response = adapter
            .render(
                ferrite::ssr::SsrHttpRequest {
                    method: "GET".into(),
                    uri: "/".into(),
                    headers: vec![],
                    body: vec![],
                },
                Default::default(),
            )
            .await
            .unwrap();
        let html = response.into_string().await.unwrap();
        assert!(html.contains("Hello Ferrite + Vue"), "{language}: {html}");
        assert!(html.contains("count: 0</button>"), "{language}: {html}");
        assert!(html.contains("data-v-"), "{language}: {html}");
        let mut config = builder.config.clone();
        let reservation = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        config.server.port = reservation.local_addr().unwrap().port();
        drop(reservation);
        let port = config.server.port;
        let serving =
            tokio::spawn(async move { ferrite::preview_with_plugins(&config, &[]).await });
        struct Preview(tokio::task::JoinHandle<ferrite::Result<()>>);
        impl Drop for Preview {
            fn drop(&mut self) {
                self.0.abort();
            }
        }
        let preview = Preview(serving);
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if tokio::net::TcpStream::connect(("127.0.0.1", port))
                    .await
                    .is_ok()
                {
                    break;
                }
                assert!(!preview.0.is_finished());
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        for kind in [
            ferrite_e2e::BrowserKind::Chromium,
            ferrite_e2e::BrowserKind::Firefox,
        ] {
            let executable = match kind {
                ferrite_e2e::BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
                ferrite_e2e::BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
            }
            .expect("generated SSR acceptance requires both selected browsers");
            let browser = ferrite_e2e::Browser::launch(
                ferrite_e2e::LaunchOptions::default()
                    .browser(kind)
                    .executable(executable),
            )
            .await
            .unwrap();
            let page = browser.new_page().await.unwrap();
            page.goto(&format!("http://127.0.0.1:{port}/"))
                .await
                .unwrap();
            page.wait_for_function("globalThis.__hydrated === true", Duration::from_secs(15))
                .await
                .unwrap_or_else(|error| {
                    panic!(
                        "{language}/{kind:?}: {error}; {:?}; {:?}",
                        page.page_errors(),
                        page.console_messages()
                    )
                });
            assert!(page
                .evaluate::<bool>(
                    "globalThis.__ssrButton === document.querySelector('#app button')"
                )
                .await
                .unwrap());
            page.locator("#counter").click().await.unwrap();
            page.wait_for_function(
                "document.querySelector('#counter').textContent === 'count: 1'",
                Duration::from_secs(5),
            )
            .await
            .unwrap();
            assert_eq!(
                page.evaluate::<String>(
                    "getComputedStyle(document.querySelector('#counter')).color"
                )
                .await
                .unwrap(),
                "rgb(128, 0, 0)"
            );
            assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
            assert!(
                page.console_messages()
                    .iter()
                    .all(|message| message.kind != "error"
                        && !message.text.to_lowercase().contains("hydration")),
                "{:?}",
                page.console_messages()
            );
            browser.close().await.unwrap();
        }
    }
}

#[cfg(feature = "napi-vm")]
#[tokio::test]
#[ignore = "real registry and Node compiler host; generated Vue SSR development invalidation"]
async fn generated_vue_ssr_dev_graph_updates_and_recovers() {
    use ferrite::ssr::SsrAdapter;
    for language in ["js", "ts"] {
        let generated = ferrite::frameworks::scaffold::vue_ssr_files(
            "generated-dev",
            language,
            "node",
            "napi-vm",
        )
        .unwrap();
        let inputs: Vec<_> = generated
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str()))
            .collect();
        let project = TempProject::new(&inputs);
        let manifest: ferrite::npm::JsPackageJson =
            serde_json::from_str(&generated["package.json"]).unwrap();
        let npm = project.root.join(".ferrite/npm");
        let client =
            ferrite::npm::RegistryClient::new(ferrite::npm::DEFAULT_REGISTRY, npm.join("metadata"))
                .unwrap();
        let installer = ferrite::npm::Installer::new(client, npm);
        let mut lock = ferrite::npm::Lockfile::default();
        installer
            .install_manifest(&manifest, &mut lock, false)
            .await
            .unwrap();
        lock.write(&project.root.join("ferrite.lock")).unwrap();
        let (config, plugins) = ferrite::Config {
            root: Some(project.root.clone()),
            ..Default::default()
        }
        .resolve()
        .await
        .unwrap();
        assert!(!config.is_production);
        let server = DevServer::new_without_watcher(config.clone(), plugins)
            .await
            .unwrap();
        let shell = server.transform_index_html("/index.html").await.unwrap();
        assert!(shell.contains("/@ferrite/client"));
        server
            .set_ssr_adapter(
                ferrite::create_dev_ssr_adapter(&server, &config, &shell)
                    .await
                    .unwrap(),
            )
            .await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let router = server.router();
        struct Serving(tokio::task::JoinHandle<()>);
        impl Drop for Serving {
            fn drop(&mut self) {
                self.0.abort();
            }
        }
        let _serving = Serving(tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        }));
        async fn get(port: u16) -> String {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut socket = tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .unwrap();
            socket
                .write_all(b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n")
                .await
                .unwrap();
            let mut response = String::new();
            socket.read_to_string(&mut response).await.unwrap();
            response
        }
        let initial = get(port).await;
        assert!(initial.starts_with("HTTP/1.0 200"), "{initial}");
        assert!(initial.contains("Hello Ferrite + Vue"));
        assert!(initial.contains("count: 0"));
        assert!(
            initial.contains("App.vue?ferrite-style=0&amp;direct")
                || initial.contains("App.vue?ferrite-style=0&direct")
        );
        let entry = format!("/src/entry-server.{language}");
        let (graph, styles) = server.ssr_runtime_graph_with_styles(&entry).await.unwrap();
        let style_id = ModuleId::new("/src/App.vue?ferrite-style=0");
        assert!(styles
            .iter()
            .any(|url| url.contains("App.vue?ferrite-style=0")));
        let initial_style = server
            .pipeline_module(&style_id, None, "client")
            .await
            .unwrap();
        assert!(initial_style.code.contains("rgb(128, 0, 0)"));
        assert!(initial_style.code.contains("data-v-"));
        let runtime = Arc::new(ferrite::runtime::napi_vm::NapiVmRuntime::with_defaults());
        let mut adapter = ferrite::ssr::JsSsrAdapter::new_graph(runtime, graph).unwrap();
        async fn render(adapter: &ferrite::ssr::JsSsrAdapter) -> String {
            adapter
                .render(
                    ferrite::ssr::SsrHttpRequest {
                        method: "GET".into(),
                        uri: "/".into(),
                        headers: vec![],
                        body: vec![],
                    },
                    Default::default(),
                )
                .await
                .unwrap()
                .into_string()
                .await
                .unwrap()
        }
        let original = generated["src/App.vue"].clone();
        assert!(render(&adapter).await.contains("Hello Ferrite + Vue"));
        std::fs::write(
            project.root.join("src/App.vue"),
            original
                .replace("Hello Ferrite + Vue", "Hello updated Vue")
                .replace("rgb(128, 0, 0)", "rgb(0, 0, 128)"),
        )
        .unwrap();
        adapter
            .replace_graph(server.ssr_runtime_graph(&entry).await.unwrap())
            .unwrap();
        let updated = render(&adapter).await;
        assert!(updated.contains("Hello updated Vue"), "{updated}");
        assert!(!updated.contains("Hello Ferrite + Vue"));
        let html = get(port).await;
        assert!(html.starts_with("HTTP/1.0 200"), "{html}");
        assert!(html.contains("Hello updated Vue"));
        assert!(!html.contains("Hello Ferrite + Vue"));
        let updated_style = server
            .pipeline_module(&style_id, None, "client")
            .await
            .unwrap();
        assert!(updated_style.code.contains("rgb(0, 0, 128)"));
        assert!(!updated_style.code.contains("rgb(128, 0, 0)"));
        std::fs::write(
            project.root.join("src/App.vue"),
            "<script setup>const broken = ;</script>",
        )
        .unwrap();
        assert!(
            server.ssr_runtime_graph(&entry).await.is_err(),
            "broken component must not return stale graph"
        );
        let response = get(port).await;
        assert!(response.starts_with("HTTP/1.0 500"), "{response}");
        assert!(!response.contains("Hello updated Vue"));
        std::fs::write(project.root.join("src/App.vue"), &original).unwrap();
        adapter
            .replace_graph(server.ssr_runtime_graph(&entry).await.unwrap())
            .unwrap();
        assert!(render(&adapter).await.contains("Hello Ferrite + Vue"));
        let recovered_style = server
            .pipeline_module(&style_id, None, "client")
            .await
            .unwrap();
        assert_eq!(recovered_style.code, initial_style.code);
        let response = get(port).await;
        assert!(response.starts_with("HTTP/1.0 200"), "{response}");
        assert!(response.contains("Hello Ferrite + Vue"));
    }
}

#[cfg(feature = "napi-vm")]
async fn generated_vue_dev_browser(kind: ferrite_e2e::BrowserKind) {
    for language in ["js", "ts"] {
        let mut generated = ferrite::frameworks::scaffold::vue_ssr_files(
            "generated-dev-browser",
            language,
            "node",
            "napi-vm",
        )
        .unwrap();
        if language == "ts" {
            generated.insert(
                "src/leaf.ts".into(),
                "export interface Props { label?: string }\n".into(),
            );
            generated.insert(
                "src/props.ts".into(),
                "export type { Props } from './leaf';\n".into(),
            );
            let component = generated.get_mut("src/App.vue").unwrap();
            *component = component.replace(
                "import { ref } from 'vue';",
                "import { ref } from 'vue';\nimport type { Props } from './props';\ndefineProps<Props>();",
            );
        }
        let index = generated.get_mut("index.html").unwrap();
        *index = index.replace("<script type=", "<script>globalThis.__ssrButton = document.querySelector('#app button');</script><script type=");
        generated
            .get_mut(&format!("src/main.{language}"))
            .unwrap()
            .push_str("\nglobalThis.__hydrated = true;\n");
        let inputs: Vec<_> = generated
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str()))
            .collect();
        let project = TempProject::new(&inputs);
        let manifest: ferrite::npm::JsPackageJson =
            serde_json::from_str(&generated["package.json"]).unwrap();
        let npm = project.root.join(".ferrite/npm");
        let client =
            ferrite::npm::RegistryClient::new(ferrite::npm::DEFAULT_REGISTRY, npm.join("metadata"))
                .unwrap();
        let installer = ferrite::npm::Installer::new(client, npm);
        let mut lock = ferrite::npm::Lockfile::default();
        installer
            .install_manifest(&manifest, &mut lock, false)
            .await
            .unwrap();
        lock.write(&project.root.join("ferrite.lock")).unwrap();
        let (config, plugins) = ferrite::Config {
            root: Some(project.root.clone()),
            ..Default::default()
        }
        .resolve()
        .await
        .unwrap();
        let server = DevServer::new(config.clone(), plugins).await.unwrap();
        let shell = server.transform_index_html("/index.html").await.unwrap();
        server
            .set_ssr_adapter(
                ferrite::create_dev_ssr_adapter(&server, &config, &shell)
                    .await
                    .unwrap(),
            )
            .await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let router = server.router();
        struct Serving(tokio::task::JoinHandle<()>);
        impl Drop for Serving {
            fn drop(&mut self) {
                self.0.abort();
            }
        }
        let _serving = Serving(tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        }));
        let executable = match kind {
            ferrite_e2e::BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
            ferrite_e2e::BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
        }
        .expect("generated development SSR requires the selected real browser");
        let browser = ferrite_e2e::Browser::launch(
            ferrite_e2e::LaunchOptions::default()
                .browser(kind)
                .executable(executable),
        )
        .await
        .unwrap();
        let page = browser.new_page().await.unwrap();
        page.goto(&url).await.unwrap();
        page.wait_for_function("globalThis.__hydrated === true", Duration::from_secs(15))
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "{language}/{kind:?}: {error}; {:?}; {:?}",
                    page.page_errors(),
                    page.console_messages()
                )
            });
        assert!(page
            .evaluate::<bool>("globalThis.__ssrButton === document.querySelector('#counter')")
            .await
            .unwrap());
        assert_eq!(
            page.evaluate::<String>("getComputedStyle(document.querySelector('#counter')).color")
                .await
                .unwrap(),
            "rgb(128, 0, 0)"
        );
        let state = page.evaluate::<serde_json::Value>("({overlay:document.querySelector('#ferrite-error-overlay')?.textContent,button:document.querySelector('#counter')?.outerHTML,body:document.body.innerHTML})").await.unwrap();
        assert!(
            page.page_errors().is_empty(),
            "{state}; {:?}",
            page.page_errors()
        );
        assert!(
            page.console_messages()
                .iter()
                .all(|message| message.kind != "error"),
            "{state}; {:?}",
            page.console_messages()
        );
        page.locator("#counter")
            .click()
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "{language}/{kind:?}: {error}; {state}; {:?}; {:?}",
                    page.page_errors(),
                    page.console_messages()
                )
            });
        page.wait_for_function(
            "document.querySelector('#counter').textContent === 'count: 1'",
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
        assert!(
            page.console_messages()
                .iter()
                .all(|message| message.kind != "error"),
            "{:?}",
            page.console_messages()
        );
        if language == "ts" {
            assert!(page.evaluate::<bool>("fetch('/src/App.vue').then(r => r.text()).then(code => code.includes('type: String'))").await.unwrap());
            // Only the transitive type input changes: the component is untouched.
            std::fs::write(
                project.root.join("src/leaf.ts"),
                "export interface Props { label?: number }\n",
            )
            .unwrap();
            page.wait_for_function("globalThis.__hydrated === true && document.querySelector('#counter')?.textContent === 'count: 0'", Duration::from_secs(20)).await.unwrap();
            assert!(page.evaluate::<bool>("fetch('/src/App.vue').then(r => r.text()).then(code => code.includes('type: Number') && !code.includes('type: String'))").await.unwrap());
            assert!(page
                .evaluate::<bool>("globalThis.__ssrButton === document.querySelector('#counter')")
                .await
                .unwrap());
            page.locator("#counter").click().await.unwrap();
            page.wait_for_function(
                "document.querySelector('#counter').textContent === 'count: 1'",
                Duration::from_secs(5),
            )
            .await
            .unwrap();
        }
        let original = generated["src/App.vue"].clone();
        let changed = original
            .replace("Hello Ferrite + Vue", "Hello updated Vue")
            .replace("rgb(128, 0, 0)", "rgb(0, 0, 128)");
        std::fs::write(project.root.join("src/App.vue"), &changed).unwrap();
        // The existing experimental adapter declares full reload, so an edit
        // intentionally resets state. Do not claim component state preservation.
        page.wait_for_function("globalThis.__hydrated === true && document.querySelector('h1')?.textContent === 'Hello updated Vue' && document.querySelector('#counter')?.textContent === 'count: 0' && getComputedStyle(document.querySelector('#counter')).color === 'rgb(0, 0, 128)'", Duration::from_secs(20)).await.unwrap_or_else(|error| panic!("{language}/{kind:?}: {error}; {:?}; {:?}", page.page_errors(), page.console_messages()));
        assert!(page
            .evaluate::<bool>("globalThis.__ssrButton === document.querySelector('#counter')")
            .await
            .unwrap());
        page.locator("#counter").click().await.unwrap();
        page.wait_for_function(
            "document.querySelector('#counter').textContent === 'count: 1'",
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
        assert!(
            page.console_messages()
                .iter()
                .all(|message| message.kind != "error"),
            "{:?}",
            page.console_messages()
        );
        std::fs::write(
            project.root.join("src/App.vue"),
            "<script setup>const broken = ;</script>",
        )
        .unwrap();
        page.wait_for_function(
            "document.querySelector('#ferrite-error-overlay')?.textContent.includes('App.vue')",
            Duration::from_secs(15),
        )
        .await
        .unwrap_or_else(|error| {
            panic!(
                "{language}/{kind:?}: {error}; {:?}; {:?}",
                page.page_errors(),
                page.console_messages()
            )
        });
        assert_eq!(
            page.evaluate::<String>("document.querySelector('#counter').textContent")
                .await
                .unwrap(),
            "count: 1"
        );
        let expected_errors = page
            .console_messages()
            .iter()
            .filter(|message| message.kind == "error")
            .count();
        std::fs::write(project.root.join("src/App.vue"), &original).unwrap();
        page.wait_for_function("globalThis.__hydrated === true && !document.querySelector('#ferrite-error-overlay') && document.querySelector('h1')?.textContent === 'Hello Ferrite + Vue' && document.querySelector('#counter')?.textContent === 'count: 0' && getComputedStyle(document.querySelector('#counter')).color === 'rgb(128, 0, 0)'", Duration::from_secs(20)).await.unwrap_or_else(|error| panic!("{language}/{kind:?}: {error}; {:?}; {:?}", page.page_errors(), page.console_messages()));
        page.locator("#counter").click().await.unwrap();
        page.wait_for_function(
            "document.querySelector('#counter').textContent === 'count: 1'",
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
        assert_eq!(
            page.console_messages()
                .iter()
                .filter(|message| message.kind == "error")
                .count(),
            expected_errors,
            "{:?}",
            page.console_messages()
        );
        assert!(
            page.console_messages()
                .iter()
                .all(|message| !message.text.to_lowercase().contains("hydration")),
            "{:?}",
            page.console_messages()
        );
        browser.close().await.unwrap();
    }
}

#[cfg(feature = "napi-vm")]
#[tokio::test]
#[ignore = "real registry, Node compiler host and Chromium; generated SSR watcher acceptance"]
async fn generated_vue_dev_ssr_chromium() {
    generated_vue_dev_browser(ferrite_e2e::BrowserKind::Chromium).await;
}

#[cfg(feature = "napi-vm")]
#[tokio::test]
#[ignore = "real registry, Node compiler host and Firefox; generated SSR watcher acceptance"]
async fn generated_vue_dev_ssr_firefox() {
    generated_vue_dev_browser(ferrite_e2e::BrowserKind::Firefox).await;
}
