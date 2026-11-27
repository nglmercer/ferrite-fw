//! Official framework islands preserve SSR nodes and hydrate on selected triggers.
#![cfg(feature = "napi-vm")]
use ferrite_e2e::{Browser, BrowserKind, LaunchOptions};
use std::time::Duration;
fn fixture() -> ferrite_test::TempProject {
    ferrite_test::TempProject::new(&[
        ("package.json",r#"{"type":"module","dependencies":{"react":"19.2.0","react-dom":"19.2.0","vue":"3.5.22","svelte":"5.39.6"}}"#),
        ("ferrite.toml",r#"base="/app/"
[framework]
enabled=["vue","svelte"]
compiler_host="node"
[ssr]
entry="src/server.js"
[ssr.islands]
React="src/react-client.js"
Vue="src/vue-client.js"
Svelte="src/svelte-client.js"
Probe="src/probe.js"
[[ssr.entries]]
route="/admin/*"
entry="src/admin.js"
shell="admin.html"
[runtime]
backend="napi-vm"
"#),
        ("index.html",r#"<html><head><title>HOME</title></head><body><!--ssr-outlet--><script>globalThis.saved=Array.from(document.querySelectorAll('[data-ferrite-island] button'));globalThis.islandErrors=0;document.addEventListener('ferrite:island-error',event=>{globalThis.islandErrors++;globalThis.lastIslandError=String(event.detail)});</script></body></html>"#),
        ("admin.html","<html><head><title>ADMIN</title></head><body><!--ssr-outlet--></body></html>"),
        ("src/admin.js","import './admin.css';export function render(){return '<h1>admin</h1>'}"),
        ("src/admin.css","h1 {color:rgb(0,0,128)}"),
        ("src/React.js","import React from 'react';export default function App({start}){const [count,set]=React.useState(start);return React.createElement('button',{id:'react-button',onClick:()=>set(count+1)},'React: '+count)}"),
        ("src/Vue.vue","<script setup>import {ref} from 'vue';const props=defineProps({start:Number});const count=ref(props.start);</script><template><button id='vue-button' @click='count++'>Vue: {{count}}</button></template><style scoped>button{color:rgb(128,0,0)}</style>"),
        ("src/Svelte.svelte","<script>let {start}=$props();let count=$state(start);</script><button id='svelte-button' onclick={()=>count++}>Svelte: {count}</button><style>button{color:rgb(0,128,0)}</style>"),
        ("src/react-client.js","import React from 'react';import {hydrateRoot} from 'react-dom/client';import App from './React.js';export function hydrate(el,props){const root=hydrateRoot(el,React.createElement(App,props));return ()=>root.unmount()}"),
        ("src/vue-client.js","import {createSSRApp} from 'vue';import App from './Vue.vue';export function hydrate(el,props){const app=createSSRApp(App,props);app.mount(el);return ()=>app.unmount()}"),
        ("src/svelte-client.js","import {hydrate as hydrateComponent,unmount} from 'svelte';import App from './Svelte.svelte';globalThis.svelteLoaded=true;export function hydrate(el,props){const app=hydrateComponent(App,{target:el,props});return ()=>unmount(app)}"),
        ("src/probe.js","export function hydrate(el,props){globalThis.probes=globalThis.probes||{};globalThis.probes[el.id]=(globalThis.probes[el.id]||0)+1;if(props.fail)throw new Error('retry probe');el.querySelector('button').title=props.label||'ok';return ()=>{globalThis.disposed=(globalThis.disposed||0)+1}}"),
        ("src/server.js",r#"import React from 'react';import {renderToString as reactRender} from 'react-dom/server.edge';import ReactApp from './React.js';import {createSSRApp} from 'vue';import {renderToString as vueRender} from 'vue/server-renderer';import VueApp from './Vue.vue';import {render as svelteRender} from 'svelte/server';import SvelteApp from './Svelte.svelte';
function island(name,html,props,mode,id,style=''){const json=JSON.stringify(props).replaceAll('&','&amp;').replaceAll('"','&quot;');return `<div id="${id}" data-ferrite-island="${name}" data-ferrite-hydrate="${mode}" data-props="${json}" style="${style}">${html}</div>`}
export async function render(){return '<p id="static">server only</p>'+island('React',reactRender(React.createElement(ReactApp,{start:2})),{start:2},'load','react')+island('Vue',await vueRender(createSSRApp(VueApp,{start:3})),{start:3},'interaction','vue')+island('Probe','<button>probe</button>',{fail:true},'manual','probe')+island('Svelte',svelteRender(SvelteApp,{props:{start:4}}).body,{start:4},'visible','svelte','margin-top:3000px');}
"#),
    ])
}
struct Serving(tokio::task::JoinHandle<()>);
impl Drop for Serving {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn exercise(browser: &Browser, url: &str) {
    let page = browser.new_page().await.unwrap();
    page.goto(url).await.unwrap();
    page.wait_for_function(
        "document.querySelector('#react')?.getAttribute('data-ferrite-state')==='ready'",
        Duration::from_secs(20),
    )
    .await
    .unwrap_or_else(|e| {
        panic!(
            "{e}: {:?} {:?}",
            page.page_errors(),
            page.console_messages()
        )
    });
    assert!(page.evaluate::<bool>("saved[0]===document.querySelector('#react-button') && !globalThis.svelteLoaded && document.querySelector('#vue').getAttribute('data-ferrite-state')==='pending'").await.unwrap());
    if let Err(error) = page.locator("#react-button").click().await {
        panic!("React click {error}; state={:?}; {:?}; {:?}",page.evaluate::<String>("JSON.stringify({rect:document.querySelector('#react-button').getBoundingClientRect().toJSON(),html:document.elementFromPoint(20,20)?.outerHTML,scroll:scrollY})").await,page.page_errors(),page.console_messages());
    }
    page.wait_for_function(
        "document.querySelector('#react-button').textContent==='React: 3'",
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    page.locator("#vue-button").click().await.unwrap();
    page.wait_for_function(
        "document.querySelector('#vue-button').textContent==='Vue: 4'",
        Duration::from_secs(15),
    )
    .await
    .unwrap_or_else(|e| {
        panic!(
            "Vue interaction {e}: {:?} {:?}",
            page.page_errors(),
            page.console_messages()
        )
    });
    assert!(page
        .evaluate::<bool>("saved[1]===document.querySelector('#vue-button')")
        .await
        .unwrap());
    page.evaluate::<bool>("document.querySelector('#probe').dispatchEvent(new CustomEvent('ferrite:hydrate',{bubbles:true}))").await.unwrap();
    page.wait_for_function(
        "document.querySelector('#probe').getAttribute('data-ferrite-state')==='error'",
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    page.evaluate::<bool>("(()=>{const el=document.querySelector('#probe');el.setAttribute('data-props',JSON.stringify({label:'&quot; <safe> & 😀'}));el.dispatchEvent(new CustomEvent('ferrite:hydrate',{bubbles:true}));return el.dispatchEvent(new CustomEvent('ferrite:hydrate',{bubbles:true}));})()").await.unwrap();
    page.wait_for_function(
        "document.querySelector('#probe').getAttribute('data-ferrite-state')==='ready'",
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    assert_eq!(page.evaluate::<u32>("probes.probe").await.unwrap(), 2);
    assert_eq!(
        page.evaluate::<String>("document.querySelector('#probe button').title")
            .await
            .unwrap(),
        "&quot; <safe> & 😀"
    );
    page.evaluate::<bool>("(()=>{const clone=document.querySelector('#probe').cloneNode(true);clone.id='idle';clone.setAttribute('data-ferrite-hydrate','idle');document.querySelector('#static').after(clone);return true})()").await.unwrap();
    page.wait_for_function(
        "document.querySelector('#idle')?.getAttribute('data-ferrite-state')==='ready' && globalThis.probes?.idle===1",
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    page.evaluate::<bool>("(()=>{document.querySelector('#idle').remove();return true})()")
        .await
        .unwrap();
    page.wait_for_function("globalThis.disposed===1", Duration::from_secs(5))
        .await
        .unwrap();
    page.evaluate::<bool>(
        "(()=>{document.querySelector('#svelte').scrollIntoView();return true})()",
    )
    .await
    .unwrap();
    let observed = page
        .wait_for_function(
            "document.querySelector('#svelte').getAttribute('data-ferrite-state')==='ready'",
            Duration::from_secs(15),
        )
        .await;
    if let Err(error) = observed {
        panic!("Svelte visible {error}; state={:?}; {:?}; {:?}",page.evaluate::<String>("JSON.stringify({html:document.querySelector('#svelte').outerHTML,error:globalThis.lastIslandError,loaded:globalThis.svelteLoaded})").await,page.page_errors(),page.console_messages());
    }
    assert!(page
        .evaluate::<bool>("saved[3]===document.querySelector('#svelte-button')")
        .await
        .unwrap());
    page.locator("#svelte-button").click().await.unwrap();
    page.wait_for_function(
        "document.querySelector('#svelte-button').textContent==='Svelte: 5'",
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
    assert_eq!(page.evaluate::<u32>("islandErrors").await.unwrap(), 1);
    page.close().await.unwrap();
    let admin = browser.new_page().await.unwrap();
    admin.goto(&format!("{url}admin")).await.unwrap();
    admin.wait_for_function("document.title==='ADMIN' && getComputedStyle(document.querySelector('h1')).color==='rgb(0, 0, 128)'",Duration::from_secs(10)).await.unwrap();
    assert!(admin.evaluate::<bool>("!document.querySelector('[data-ferrite-island]') && document.querySelectorAll('link[rel=stylesheet]').length===1").await.unwrap(),"unselected document CSS leaked into admin shell");
    admin.close().await.unwrap();
}
async fn matrix(kind: BrowserKind) {
    let p = fixture();
    let npm = p.root.join(".ferrite/npm");
    let installer = ferrite::npm::Installer::new(
        ferrite::npm::RegistryClient::new(ferrite::npm::DEFAULT_REGISTRY, npm.join("metadata"))
            .unwrap(),
        npm,
    );
    let mut lock = ferrite::npm::Lockfile::default();
    installer
        .install_project(&p.root, &mut lock, false)
        .await
        .unwrap();
    lock.write(&p.root.join("ferrite.lock")).unwrap();
    ferrite::npm::project_editor_dependencies(&p.root, &lock).unwrap();
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
    let (config, plugins) = ferrite::Config {
        root: Some(p.root.clone()),
        ..Default::default()
    }
    .resolve()
    .await
    .unwrap();
    let server = ferrite::DevServer::new(config.clone(), plugins)
        .await
        .unwrap();
    let shell = server.transform_index_html("/index.html").await.unwrap();
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
    let serving = Serving(tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap()
    }));
    eprintln!("islands {kind:?}: dev");
    exercise(&browser, &format!("http://127.0.0.1:{port}/app/")).await;
    drop(serving);
    let builder = ferrite::create_builder(ferrite::Config {
        root: Some(p.root.clone()),
        ..Default::default()
    })
    .await
    .unwrap();
    builder.build_app().await.unwrap();
    for prerender in [false, true] {
        if prerender {
            builder
                .prerender(&["/".into(), "/admin".into()])
                .await
                .unwrap();
        }
        std::fs::rename(p.root.join("src"), p.root.join("hidden-src")).unwrap();
        let mut c = builder.config.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        c.server.port = listener.local_addr().unwrap().port();
        drop(listener);
        let port = c.server.port;
        let serving = Serving(tokio::spawn(async move {
            ferrite::preview_with_plugins(&c, &[]).await.unwrap()
        }));
        let url = format!("http://127.0.0.1:{port}/app/");
        ferrite_e2e::wait_for_url(&url, Duration::from_secs(10))
            .await
            .unwrap();
        eprintln!("islands {kind:?}: preview prerender={prerender}");
        exercise(&browser, &url).await;
        drop(serving);
        std::fs::rename(p.root.join("hidden-src"), p.root.join("src")).unwrap();
    }
    browser.close().await.unwrap();
}
#[tokio::test]
#[ignore = "official React Vue Svelte packages, Node compiler and Chromium"]
async fn chromium_framework_islands_dev_preview_and_static() {
    matrix(BrowserKind::Chromium).await;
}
#[tokio::test]
#[ignore = "official React Vue Svelte packages, Node compiler and Firefox"]
async fn firefox_framework_islands_dev_preview_and_static() {
    matrix(BrowserKind::Firefox).await;
}

#[tokio::test]
#[ignore = "official framework packages, offline generated Cargo binary and both browsers"]
async fn standalone_framework_islands_without_node() {
    let p = fixture();
    let npm = p.root.join(".ferrite/npm");
    let installer = ferrite::npm::Installer::new(
        ferrite::npm::RegistryClient::new(ferrite::npm::DEFAULT_REGISTRY, npm.join("metadata"))
            .unwrap(),
        npm,
    );
    let mut lock = ferrite::npm::Lockfile::default();
    installer
        .install_project(&p.root, &mut lock, false)
        .await
        .unwrap();
    lock.write(&p.root.join("ferrite.lock")).unwrap();
    ferrite::npm::project_editor_dependencies(&p.root, &lock).unwrap();
    let mut builder = ferrite::create_builder(ferrite::Config {
        root: Some(p.root.clone()),
        ..Default::default()
    })
    .await
    .unwrap();
    builder.config.package.standalone = true;
    builder.config.package.ssr_sdk =
        Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ferrite"));
    builder.build_app().await.unwrap();
    let target = std::env::var_os("FERRITE_STANDALONE_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    let status = std::process::Command::new("cargo")
        .args(["build", "--offline", "--manifest-path"])
        .arg(p.root.join("dist/standalone/Cargo.toml"))
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .unwrap();
    assert!(status.success());
    let cwd = tempfile::tempdir().unwrap();
    let binary = cwd
        .path()
        .join(format!("server{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(
        target
            .join("debug")
            .join(format!("ferrite-app{}", std::env::consts::EXE_SUFFIX)),
        &binary,
    )
    .unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let child = std::process::Command::new(binary)
        .env("PATH", "")
        .env("HOST", "127.0.0.1")
        .env("PORT", port.to_string())
        .current_dir(cwd.path())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let _child = Child(child);
    let url = format!("http://127.0.0.1:{port}/app/");
    ferrite_e2e::wait_for_url(&url, Duration::from_secs(15))
        .await
        .unwrap();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let executable = match kind {
            BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
            BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
        }
        .expect("both browsers required");
        let browser = Browser::launch(
            LaunchOptions::default()
                .browser(kind)
                .executable(executable),
        )
        .await
        .unwrap();
        exercise(&browser, &url).await;
        browser.close().await.unwrap();
    }
}
