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
    assert!(!server_module.code.contains("ferrite-style="));
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
        !svelte_server.code.contains("ferrite-style="),
        "{}",
        svelte_server.code
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
    let project = fixture().await;
    std::fs::create_dir_all(project.root.join("src")).unwrap();
    std::fs::write(project.root.join("src/entry-server.ts"), "import { createSSRApp } from 'vue'; import { renderToString } from 'vue/server-renderer'; import Counter from '../Counter.vue'; export async function render() { globalThis.__ferrite_vue_requests = (globalThis.__ferrite_vue_requests || 0) + 1; const html = await renderToString(createSSRApp(Counter)); return { html, headers: [['x-request-count', String(globalThis.__ferrite_vue_requests)]] }; }").unwrap();
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
    let builder = ferrite::Builder::new(config, vec![Arc::new(VuePlugin::with_host(host, false))]);
    let report = builder.build("ssr").await.unwrap();
    let artifact = ferrite::SsrRendererArtifact::read(&report.out_dir).unwrap();
    if let Some(path) = std::env::var_os("FERRITE_VUE_SSR_ARTIFACT") {
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
