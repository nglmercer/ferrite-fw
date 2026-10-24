//! Real HTTP disconnects propagate into process-runtime cancellation.
#[cfg(all(feature = "napi-vm", target_os = "linux"))]
#[tokio::test]
#[ignore = "requires the freshly built feature-enabled Ferrite CLI worker"]
async fn http_disconnect_kills_render_worker_and_next_request_recovers() {
    use ferrite::ssr::SsrAdapter;
    use std::{path::PathBuf, sync::Arc, time::Duration};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let executable = std::env::var_os("FERRITE_CLI_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/ferrite")
        });
    assert!(
        executable.is_file(),
        "build ferrite-cli --features napi-vm first"
    );
    let runtime = Arc::new(ferrite::runtime::process::ProcessRuntime::new(
        ferrite::runtime::process::ProcessOptions {
            executable: Some(executable),
            engine: ferrite::runtime::napi_vm::NapiVmOptions {
                request_timeout_ms: 10_000,
                fuel_budget: u64::MAX,
                loop_budget: u64::MAX,
                ..Default::default()
            },
            ..Default::default()
        },
    ));
    let renderer = Arc::new(ferrite::ssr::JsSsrAdapter::new_graph(runtime.clone(), ferrite::runtime::CompiledModuleGraph {
        entry: "entry".into(), modules: vec![ferrite::runtime::CompiledModule { id: "entry".into(), code: "export function render(url) { if (url === '/slow') { while(true){} } return '<h1>recovered</h1>'; }".into(), url: None }],
    }).unwrap());
    let app = axum::Router::new().fallback(move |uri: axum::http::Uri| {
        let renderer = renderer.clone();
        async move {
            ferrite::server::ssr_http_response(
                renderer
                    .render(
                        ferrite::ssr::SsrHttpRequest {
                            method: "GET".into(),
                            uri: uri.to_string(),
                            headers: vec![],
                            body: vec![],
                        },
                        Default::default(),
                    )
                    .await
                    .unwrap(),
                false,
            )
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    struct Serving(tokio::task::JoinHandle<()>);
    impl Drop for Serving {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _serving = Serving(tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    }));
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    socket
        .write_all(b"GET /slow HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while runtime.active_children() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    drop(socket);
    tokio::time::timeout(Duration::from_secs(2), async {
        while runtime.active_children() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("disconnect must cancel before the ten-second guest deadline");
    let mut next = tokio::net::TcpStream::connect(address).await.unwrap();
    next.write_all(b"GET /ok HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    let mut response = String::new();
    tokio::time::timeout(Duration::from_secs(5), next.read_to_string(&mut response))
        .await
        .unwrap()
        .unwrap();
    assert!(
        response.starts_with("HTTP/1.0 200") && response.contains("<h1>recovered</h1>"),
        "{response}"
    );
}
