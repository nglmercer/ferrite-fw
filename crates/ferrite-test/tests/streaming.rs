//! Stream ownership, backpressure and failure contracts through the real worker.
#![cfg(all(feature = "napi-vm", target_os = "linux"))]
use ferrite::runtime::process::{ProcessOptions, ProcessRuntime};
use ferrite::runtime::{CompiledModule, CompiledModuleGraph, JsRuntime, RuntimeEnvironment};
use futures::{StreamExt, TryStreamExt};
use std::{path::PathBuf, sync::Arc, time::Duration};
fn runtime(timeout: u64) -> Arc<ProcessRuntime> {
    let executable = std::env::var_os("FERRITE_CLI_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/ferrite")
        });
    assert!(executable.is_file(), "build the feature-enabled CLI first");
    Arc::new(ProcessRuntime::new(ProcessOptions {
        executable: Some(executable),
        concurrency: 1,
        engine: ferrite::runtime::napi_vm::NapiVmOptions {
            request_timeout_ms: timeout,
            fuel_budget: u64::MAX,
            loop_budget: u64::MAX,
            ..Default::default()
        },
        ..Default::default()
    }))
}
fn graph(code: &str) -> CompiledModuleGraph {
    CompiledModuleGraph {
        entry: "entry".into(),
        modules: vec![CompiledModule {
            id: "entry".into(),
            code: code.into(),
            url: None,
        }],
    }
}
async fn open(runtime: &ProcessRuntime, code: &str) -> ferrite::runtime::JsStreamResponse {
    runtime
        .invoke_module_graph_stream(
            graph(code),
            "render",
            vec![],
            RuntimeEnvironment {
                ssr: true,
                request_id: None,
            },
        )
        .await
        .unwrap()
}
async fn idle(runtime: &ProcessRuntime) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while runtime.active_children() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("worker must be killed and deregistered");
}
#[tokio::test]
#[ignore = "requires freshly built feature-enabled CLI worker"]
async fn stream_pull_unicode_metadata_and_large_chunk_boundaries() {
    let runtime = runtime(5000);
    let response = open(&runtime, "export function render(){let i=0;return {status:201,headers:[['x-stream','yes']],body:new ReadableStream({pull(c){i++;c.enqueue('🙂'+i);if(i===3)c.close();}})}}").await;
    assert_eq!(
        ferrite::runtime::js_value_to_json(&response.head).unwrap()["status"].as_f64(),
        Some(201.0)
    );
    let chunks: Vec<Vec<u8>> = response.body.try_collect().await.unwrap();
    assert_eq!(String::from_utf8(chunks.concat()).unwrap(), "🙂1🙂2🙂3");
    idle(&runtime).await;
    let response = open(&runtime, "export function render(){let sent=false;return {next(){if(sent)return {done:true};sent=true;return {done:false,value:new Uint8Array(131073).fill(97)}}}}").await;
    let chunks: Vec<Vec<u8>> = response.body.try_collect().await.unwrap();
    assert_eq!(
        chunks.iter().map(Vec::len).collect::<Vec<_>>(),
        [65536, 65536, 1]
    );
    assert!(chunks.concat().iter().all(|byte| *byte == 97));
    idle(&runtime).await;
}
#[tokio::test]
#[ignore = "requires freshly built feature-enabled CLI worker"]
async fn stream_byte_views_bounds_and_recovery() {
    let runtime = runtime(5000);
    let response = open(&runtime, "export function render(){let sent=false;const bytes=new Uint8Array([1,0,255,128,2]);return {next(){if(sent)return {done:true};sent=true;return {done:false,value:bytes.subarray(1,4)}}}}").await;
    assert_eq!(
        response
            .body
            .try_collect::<Vec<_>>()
            .await
            .unwrap()
            .concat(),
        [0, 255, 128]
    );
    idle(&runtime).await;
    for source in [
        "export function render(){return {next(){return {done:false,value:new Uint8Array(1048577)}}}}",
        "export function render(){return new ReadableStream({start(c){c.enqueue(new Uint8Array(600000));c.enqueue(new Uint8Array(600000));}})}",
        "export function render(){return new ReadableStream().getReader({mode:'byob'});}",
    ] {
        let result = runtime.invoke_module_graph_stream(graph(source), "render", vec![], RuntimeEnvironment {ssr:true,request_id:None}).await;
        match result {
            Err(_) => (),
            Ok(response) => assert!(response.body.try_collect::<Vec<_>>().await.is_err()),
        }
        idle(&runtime).await;
    }
    let response = open(&runtime, "export function render(){return new ReadableStream({start(c){c.enqueue('recovered');c.close();}})}").await;
    assert_eq!(
        response
            .body
            .try_collect::<Vec<_>>()
            .await
            .unwrap()
            .concat(),
        b"recovered"
    );
    idle(&runtime).await;
}

#[tokio::test]
#[ignore = "requires freshly built feature-enabled CLI worker"]
async fn stream_drop_and_unpolled_deadline_release_admission() {
    let runtime = runtime(1000);
    let stream = open(
        &runtime,
        "export function render(){return {next(){return {done:false,value:'x'}}}}",
    )
    .await;
    assert_eq!(runtime.active_children(), 1);
    // The body is deliberately never polled. Its independent owner must still
    // enforce the request deadline and free the concurrency slot.
    idle(&runtime).await;
    drop(stream);
    let stream = open(&runtime, "export function render(){return new ReadableStream({start(c){console.log('not protocol');c.enqueue('ok');c.close();}})}").await;
    let chunks: Vec<Vec<u8>> = stream.body.try_collect().await.unwrap();
    assert_eq!(chunks.concat(), b"ok");
    idle(&runtime).await;
    let stream = open(
        &runtime,
        "export function render(){return {next(){return {done:false,value:'x'}}}}",
    )
    .await;
    drop(stream);
    idle(&runtime).await;
}
#[tokio::test]
#[ignore = "requires freshly built feature-enabled CLI worker"]
async fn stream_shell_errors_terminate_without_suffix_and_recover() {
    use ferrite::ssr::SsrAdapter;
    let runtime = runtime(5000);
    let adapter = ferrite::ssr::JsSsrAdapter::new_graph(runtime.clone(), graph("export function render(){let i=0;return new ReadableStream({pull(c){if(i++===0)c.enqueue('prefix');else throw new Error('producer failed');}})}")).unwrap().with_streaming(true).with_shell("<head></head><main><!--ssr-outlet--></main>SUFFIX");
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
    let ferrite::ssr::RenderBody::Stream(mut body) = response.body else {
        panic!("stream buffered")
    };
    let mut output = Vec::new();
    let mut failed = false;
    while let Some(chunk) = body.next().await {
        match chunk {
            Ok(bytes) => output.extend(bytes),
            Err(error) => {
                assert!(error.to_string().contains("producer failed"), "{error}");
                failed = true;
                break;
            }
        }
    }
    assert!(failed);
    assert!(!String::from_utf8(output).unwrap().contains("SUFFIX"));
    assert!(body.next().await.is_none());
    idle(&runtime).await;
    let response = open(&runtime, "export function render(){return new ReadableStream({start(c){c.enqueue('recovered');c.close();}})}").await;
    assert_eq!(
        response
            .body
            .try_collect::<Vec<_>>()
            .await
            .unwrap()
            .concat(),
        b"recovered"
    );
}

#[tokio::test]
#[ignore = "requires freshly built feature-enabled CLI worker"]
async fn real_http_stream_head_disconnect_and_recovery() {
    use ferrite::ssr::SsrAdapter;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let runtime = runtime(5000);
    let renderer = Arc::new(ferrite::ssr::JsSsrAdapter::new_graph(runtime.clone(), graph("export function render(url,request){let i=0;return {status:202,headers:[['x-stream','yes']],body:{next(){if(request.method==='HEAD')throw new Error('HEAD polled');if(url==='/never')return new Promise(resolve=>setTimeout(()=>resolve({done:false,value:'late'}),10000));return ++i===1?{done:false,value:'<h1>streamed</h1>'}:{done:true}}}}}")).unwrap().with_streaming(true).with_shell("<html><head></head><main><!--ssr-outlet--></main></html>"));
    let app =
        axum::Router::new().fallback(move |method: axum::http::Method, uri: axum::http::Uri| {
            let renderer = renderer.clone();
            async move {
                let response = renderer
                    .render(
                        ferrite::ssr::SsrHttpRequest {
                            method: method.to_string(),
                            uri: uri.to_string(),
                            headers: vec![],
                            body: vec![],
                        },
                        Default::default(),
                    )
                    .await
                    .unwrap();
                ferrite::server::ssr_http_response(response, method == axum::http::Method::HEAD)
            }
        });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    struct Server(tokio::task::JoinHandle<()>);
    impl Drop for Server {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _server = Server(tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    }));
    async fn request(address: std::net::SocketAddr, method: &str) -> String {
        let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
        socket
            .write_all(format!("{method} / HTTP/1.0\r\nHost: localhost\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut text = String::new();
        tokio::time::timeout(Duration::from_secs(5), socket.read_to_string(&mut text))
            .await
            .unwrap()
            .unwrap();
        text
    }
    let head = request(address, "HEAD").await;
    assert!(head.starts_with("HTTP/1.0 202"));
    assert!(head.split_once("\r\n\r\n").unwrap().1.is_empty());
    idle(&runtime).await;
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    socket
        .write_all(b"GET /never HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    let mut first = [0; 4096];
    assert!(
        tokio::time::timeout(Duration::from_secs(2), socket.read(&mut first))
            .await
            .unwrap()
            .unwrap()
            > 0
    );
    assert_eq!(runtime.active_children(), 1);
    drop(socket);
    idle(&runtime).await;
    let response = request(address, "GET").await;
    assert!(
        response.starts_with("HTTP/1.0 202") && response.to_lowercase().contains("x-stream: yes"),
        "{response}"
    );
    assert!(
        response.contains("<main><h1>streamed</h1></main>"),
        "{response}"
    );
    idle(&runtime).await;
}

#[tokio::test]
#[ignore = "official packages, Node compiler, feature-enabled CLI and both real browsers"]
async fn official_react_and_vue_renderers_execute_streamed_production_graphs() {
    for framework in ["react", "vue"] {
        let mut files = ferrite::frameworks::scaffold::files(
            ferrite::frameworks::scaffold::select(framework, "js", "ssr").unwrap(),
            "streamed-app",
        )
        .unwrap();
        let (path, entry) = if framework == "react" {
            ("src/entry-server.jsx","import React from 'react'; import {renderToReadableStream} from 'react-dom/server.edge'; import App from './App.jsx'; export function render(){return renderToReadableStream(React.createElement(App));}")
        } else {
            ("src/entry-server.js","import {createSSRApp} from 'vue'; import {renderToWebStream} from 'vue/server-renderer'; import App from './App.vue'; export function render(){return renderToWebStream(createSSRApp(App));}")
        };
        files.insert(path.into(), entry.into());
        let config = files.get_mut("ferrite.toml").unwrap();
        *config = config.replace("[ssr]", "[ssr]\nstreaming = true").replace(
            "backend = \"napi-vm\"",
            "backend = \"napi-vm-process\"\nrequest_timeout_ms = 15000",
        );
        let html = files.get_mut("index.html").unwrap();
        *html = html.replace("<script type=", "<script>globalThis.__ssrButton=document.querySelector('#counter');</script><script type=");
        files
            .get_mut(if framework == "react" {
                "src/main.jsx"
            } else {
                "src/main.js"
            })
            .unwrap()
            .push_str("\nglobalThis.__hydrated=true;\n");

        let inputs: Vec<_> = files
            .iter()
            .map(|(path, code)| (path.as_str(), code.as_str()))
            .collect();
        let project = ferrite_test::TempProject::new(&inputs);
        let npm = project.root.join(".ferrite/npm");
        let installer = ferrite::npm::Installer::new(
            ferrite::npm::RegistryClient::new(ferrite::npm::DEFAULT_REGISTRY, npm.join("metadata"))
                .unwrap(),
            npm,
        );
        let mut lock = ferrite::npm::Lockfile::default();
        installer
            .install_project(&project.root, &mut lock, false)
            .await
            .unwrap();
        lock.write(&project.root.join("ferrite.lock")).unwrap();
        ferrite::npm::project_editor_dependencies(&project.root, &lock).unwrap();
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
        let reports = builder.build_app().await.unwrap();
        let server = reports.iter().find(|report| report.env == "ssr").unwrap();
        let artifact = ferrite::SsrRendererArtifact::read(&server.out_dir).unwrap();
        let runtime = runtime(15000);
        let adapter = artifact
            .into_streaming_adapter(
                runtime.clone(),
                "<html><head></head><main><!--ssr-outlet--></main></html>".into(),
                "/",
                true,
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
        assert!(
            html.contains(if framework == "react" {
                "Hello Ferrite + React"
            } else {
                "Hello Ferrite + Vue"
            }),
            "{framework}: {html}"
        );
        assert!(
            html.contains("<button") && html.ends_with("</main></html>"),
            "{html}"
        );
        idle(&runtime).await;
        preview_hydration(&project.root).await;
    }
}

#[tokio::test]
#[ignore = "requires freshly built feature-enabled CLI worker"]
async fn progressive_bytes_arrive_before_unresolved_async_start() {
    let runtime = runtime(2000);
    let mut response = open(&runtime, "export function render(){return new ReadableStream({async start(c){c.enqueue('first');await new Promise(resolve=>setTimeout(resolve,10000));c.enqueue('late');c.close();}})}").await;
    assert_eq!(response.body.next().await.unwrap().unwrap(), b"first");
    let error = response.body.next().await.unwrap().unwrap_err();
    assert!(!error.to_string().is_empty());
    idle(&runtime).await;
}

async fn preview_hydration(root: &std::path::Path) {
    use ferrite_e2e::{Browser, BrowserKind, LaunchOptions};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    drop(listener);
    let mut child = tokio::process::Command::new(
        std::env::var_os("FERRITE_CLI_PATH").expect("fresh CLI required"),
    )
    .args(["preview", "--port", &port])
    .current_dir(root)
    .env("PATH", "")
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::inherit())
    .kill_on_drop(true)
    .spawn()
    .unwrap();
    let url = format!("http://127.0.0.1:{port}/");
    ferrite_e2e::wait_for_url(&url, Duration::from_secs(15))
        .await
        .unwrap();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let executable = match kind {
            BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
            BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
        }
        .expect("both real browsers required");
        let browser = Browser::launch(
            LaunchOptions::default()
                .browser(kind)
                .executable(executable),
        )
        .await
        .unwrap();
        let page = browser.new_page().await.unwrap();
        page.goto(&url).await.unwrap();
        page.wait_for_function("globalThis.__hydrated===true", Duration::from_secs(15))
            .await
            .unwrap();
        assert!(
            page.evaluate::<bool>("globalThis.__ssrButton===document.querySelector('#counter')")
                .await
                .unwrap(),
            "stream hydration replaced SSR button"
        );
        page.locator("#counter").click().await.unwrap();
        page.wait_for_function(
            "document.querySelector('#counter').textContent==='count: 1'",
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
        browser.close().await.unwrap();
    }
    child.kill().await.unwrap();
    child.wait().await.unwrap();
}
