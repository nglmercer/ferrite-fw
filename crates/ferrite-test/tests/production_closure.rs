//! Execute final files in real browsers: lazy CSS timing, retry and shared state.
use ferrite_e2e::{Browser, BrowserKind, LaunchOptions};
use std::time::Duration;
fn fixture() -> ferrite_test::TempProject {
    ferrite_test::TempProject::new(&[
        ("index.html", "<html><head></head><body><p id='probe'>probe</p><button id='both'>both</button><button id='fail'>fail</button><button id='direct'>direct</button><script type='module' src='/src/main.ts'></script></body></html>"),
        ("src/main.ts", "import './theme.js';\nimport './override.css';\nconst preserved = \"import('./a.js')\";\nglobalThis.originalMarker = preserved;\ndocument.querySelector('#both').onclick = () => Promise.all([import('./a.js'),import('./a.js'),import('./b.js')]).then(([a,again,b])=>{globalThis.result=[a.value,again.value,b.value];}).catch(error=>{globalThis.failure=String(error)});\ndocument.querySelector('#fail').onclick = () => import('./a.js').catch(error=>{globalThis.failure=String(error)});\ndocument.querySelector('#direct').onclick = () => import('./direct.css').then(()=>{globalThis.direct=true});\nglobalThis.ready=true;"),
        ("src/theme.js", "import './theme.css';"),
        ("src/theme.css", "@import './imported.css'; #probe{color:rgb(128,0,0)}"),
        ("src/imported.css", "#probe{color:rgb(255,0,255)}"),
        ("src/override.css", "#probe{color:rgb(0,128,0)}"),
        ("src/shared.ts", "globalThis.sharedEvaluations=(globalThis.sharedEvaluations||0)+1; let value=0;export function next(){return ++value}"),
        ("src/a.js", "import {next} from './shared.ts'; import './lazy.css'; globalThis.aEvaluations=(globalThis.aEvaluations||0)+1;globalThis.cssReady=getComputedStyle(document.querySelector('#probe')).color;export const value=next();"),
        ("src/b.js", "import {next} from './shared.ts'; import './lazy.css'; export const value=next();"),
        ("src/lazy.css", "#probe{color:rgb(0,0,128);background-image:url('./pixel.svg')}"),
        ("src/direct.css", "#probe{border-top:7px solid rgb(0,0,0)}"),
        ("src/pixel.svg", "<svg xmlns='http://www.w3.org/2000/svg' width='1' height='1'><rect width='1' height='1'/></svg>"),
    ])
}
struct NestedOutput;
#[async_trait::async_trait]
impl ferrite::plugin::Plugin for NestedOutput {
    fn name(&self) -> &'static str {
        "nested-production-output"
    }
    async fn output_options(
        &self,
        _: &ferrite::plugin::PluginContext,
        options: &mut ferrite::plugin::OutputOptions,
    ) -> ferrite::Result<()> {
        options.chunk_pattern = "chunks/nested/[name]-[hash].js".into();
        options.css_pattern = "styles/deep/[name]-[hash].css".into();
        Ok(())
    }
}
struct Serving(tokio::task::JoinHandle<ferrite::Result<()>>);
impl Drop for Serving {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn matrix(kind: BrowserKind) {
    let executable = match kind {
        BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
        BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
    }
    .expect("real browser required");
    let browser = Browser::launch(
        LaunchOptions::default()
            .browser(kind)
            .executable(executable),
    )
    .await
    .unwrap();
    for (minify, scope_hoist) in [(false, false), (true, false), (false, true), (true, true)] {
        let project = fixture();
        let mut config = project.resolve_config_mode("production");
        config.base = "/app/".into();
        config.build.minify = minify;
        config.build.scope_hoist = scope_hoist;
        config.build.sourcemap = ferrite::config::SourceMapConfig::Bool(true);
        let plugins: Vec<std::sync::Arc<dyn ferrite::plugin::Plugin>> = if scope_hoist && !minify {
            vec![std::sync::Arc::new(NestedOutput)]
        } else {
            vec![]
        };
        ferrite::Builder::new(config.clone(), plugins)
            .build("client")
            .await
            .unwrap();
        let manifest =
            ferrite::manifest::BuildManifest::read(&config.out_dir().join("manifest.json"))
                .unwrap();
        for entry in manifest.entries.values() {
            for file in std::iter::once(&entry.file)
                .chain(&entry.imports)
                .chain(&entry.dynamic_imports)
                .chain(&entry.css)
                .chain(&entry.assets)
            {
                assert!(
                    config.out_dir().join(file).is_file(),
                    "missing closure file {file}"
                );
            }
        }
        assert!(
            manifest.entries["/src/a.js"]
                .assets
                .iter()
                .any(|file| file.contains("pixel.")),
            "lazy CSS assets must be recorded in manifest"
        );
        assert!(
            manifest.entries["/src/main.ts"].assets.is_empty(),
            "initial asset closure must exclude lazy resources"
        );
        let lazy = config.out_dir().join(&manifest.entries["/src/a.js"].css[0]);
        let saved = lazy.with_extension("saved");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        config.server.port = listener.local_addr().unwrap().port();
        drop(listener);
        let serving_config = config.clone();
        let _serving = Serving(tokio::spawn(async move {
            ferrite::preview_with_plugins(&serving_config, &[]).await
        }));
        let url = format!("http://127.0.0.1:{}/app/", config.server.port);
        ferrite_e2e::wait_for_url(&url, Duration::from_secs(10))
            .await
            .unwrap();
        let page = browser.new_page().await.unwrap();
        page.goto(&url).await.unwrap();
        page.wait_for_function("globalThis.ready===true", Duration::from_secs(10))
            .await
            .unwrap();
        assert_eq!(
            page.evaluate::<String>("getComputedStyle(document.querySelector('#probe')).color")
                .await
                .unwrap(),
            "rgb(0, 128, 0)",
            "static CSS import/cascade order"
        );
        assert!(page.evaluate::<bool>("!globalThis.aEvaluations && !document.querySelector('link[href*=\"lazy-\"]') && !document.querySelector('link[href*=\"direct-\"]')").await.unwrap(),"lazy closure loaded eagerly");
        std::fs::rename(&lazy, &saved).unwrap();
        page.locator("#fail").click().await.unwrap();
        page.wait_for_function(
            "String(globalThis.failure).includes('Failed to load stylesheet')",
            Duration::from_secs(10),
        )
        .await
        .unwrap();
        assert!(page
            .evaluate::<bool>("!globalThis.aEvaluations")
            .await
            .unwrap());
        std::fs::rename(&saved, &lazy).unwrap();
        page.locator("#both").click().await.unwrap();
        page.wait_for_function("Array.isArray(globalThis.result)", Duration::from_secs(10))
            .await
            .unwrap();
        assert_eq!(
            page.evaluate::<String>("globalThis.cssReady")
                .await
                .unwrap(),
            "rgb(0, 0, 128)",
            "CSS must settle before module body"
        );
        assert_eq!(
            page.evaluate::<u32>("globalThis.sharedEvaluations")
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            page.evaluate::<u32>("globalThis.aEvaluations")
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            page.evaluate::<u32>("document.querySelectorAll('link[href*=\"lazy-\"]').length")
                .await
                .unwrap(),
            1
        );
        assert!(page
            .evaluate::<bool>("result[0]===result[1] && result[0]!==result[2]")
            .await
            .unwrap());
        assert!(page.evaluate::<bool>("getComputedStyle(document.querySelector('#probe')).backgroundImage.includes('/app/assets/pixel.')").await.unwrap());
        assert!(page.evaluate::<bool>("(async()=>{const image=getComputedStyle(document.querySelector('#probe')).backgroundImage;const response=await fetch(image.slice(5,-2));return response.ok && (await response.text()).includes('<svg');})()").await.unwrap(), "CSS asset closure must be served");
        page.locator("#direct").click().await.unwrap();
        page.wait_for_function("globalThis.direct===true", Duration::from_secs(10))
            .await
            .unwrap();
        assert_eq!(
            page.evaluate::<String>(
                "getComputedStyle(document.querySelector('#probe')).borderTopWidth"
            )
            .await
            .unwrap(),
            "7px"
        );
        assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
        page.close().await.unwrap();
    }
    browser.close().await.unwrap();
}
#[tokio::test]
#[ignore = "real Chromium, emitted production CSS/chunk graph"]
async fn chromium_lazy_css_shared_chunks_assets_and_retry() {
    matrix(BrowserKind::Chromium).await;
}
#[tokio::test]
#[ignore = "real Firefox, emitted production CSS/chunk graph"]
async fn firefox_lazy_css_shared_chunks_assets_and_retry() {
    matrix(BrowserKind::Firefox).await;
}

#[tokio::test]
async fn css_changes_rename_importer_cones_and_final_maps_retain_original_positions() {
    let project = fixture();
    let mut config = project.resolve_config_mode("production");
    config.build.sourcemap = ferrite::config::SourceMapConfig::Bool(true);
    config.build.minify = false;
    struct Banner;
    #[async_trait::async_trait]
    impl ferrite::plugin::Plugin for Banner {
        fn name(&self) -> &'static str {
            "production-map-banner"
        }
        async fn banner(
            &self,
            _: &ferrite::plugin::PluginContext,
            _: ferrite::plugin::RenderChunk,
        ) -> ferrite::Result<Option<String>> {
            Ok(Some("/* generated banner */\n/* second line */".into()))
        }
    }
    let build = ferrite::Builder::new(config.clone(), vec![std::sync::Arc::new(Banner)]);
    build.build("client").await.unwrap();
    let first =
        ferrite::manifest::BuildManifest::read(&config.out_dir().join("manifest.json")).unwrap();
    let entry = &first.entries["/src/main.ts"];
    let code = std::fs::read_to_string(config.out_dir().join(&entry.file)).unwrap();
    let offset = code.find("originalMarker").unwrap();
    let prefix = &code[..offset];
    let line = prefix.bytes().filter(|b| *b == b'\n').count() as u32;
    let col = prefix.rsplit('\n').next().unwrap().encode_utf16().count() as u32;
    let map_json =
        std::fs::read_to_string(config.out_dir().join(format!("{}.map", entry.file))).unwrap();
    let map = oxc_sourcemap::SourceMap::from_json_string(&map_json).unwrap();
    let token = map
        .lookup_token(&map.generate_lookup_table(), line, col)
        .unwrap();
    assert!(map
        .get_source(token.get_source_id().unwrap())
        .unwrap()
        .ends_with("/src/main.ts"));
    assert_eq!(
        token.get_src_line(),
        3,
        "final wrapper/import rewrites must retain original TS line"
    );
    std::fs::write(project.root.join("src/lazy.css"), "#probe {color:orange}").unwrap();
    build.build("client").await.unwrap();
    let second =
        ferrite::manifest::BuildManifest::read(&config.out_dir().join("manifest.json")).unwrap();
    assert_ne!(
        first.entries["/src/main.ts"].file, second.entries["/src/main.ts"].file,
        "lazy CSS changes must invalidate importer cache keys"
    );
}

#[tokio::test]
async fn code_replacement_without_composable_map_drops_stale_original_positions() {
    struct Replace;
    #[async_trait::async_trait]
    impl ferrite::plugin::Plugin for Replace {
        fn name(&self) -> &'static str {
            "unmapped-code-replacement"
        }
        async fn render_chunk(
            &self,
            _: &ferrite::plugin::PluginContext,
            chunk: ferrite::plugin::RenderChunk,
        ) -> ferrite::Result<Option<ferrite::plugin::RenderChunkResult>> {
            Ok(Some(ferrite::plugin::RenderChunkResult {
                code: Some(format!("/* replacement */\n\n{}", chunk.code)),
                map: None,
            }))
        }
    }
    let project = fixture();
    let mut config = project.resolve_config_mode("production");
    config.build.sourcemap = ferrite::config::SourceMapConfig::Bool(true);
    ferrite::Builder::new(config.clone(), vec![std::sync::Arc::new(Replace)])
        .build("client")
        .await
        .unwrap();
    let manifest =
        ferrite::manifest::BuildManifest::read(&config.out_dir().join("manifest.json")).unwrap();
    for entry in manifest.entries.values() {
        assert!(
            !config
                .out_dir()
                .join(format!("{}.map", entry.file))
                .exists(),
            "unmapped replacement retained stale original map"
        );
    }
}
