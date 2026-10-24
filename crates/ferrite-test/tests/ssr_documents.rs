//! Selected CSS and independent shells use emitted metadata in every host.
#![cfg(feature = "napi-vm")]
use ferrite::ssr::{SsrAdapter, SsrHttpRequest};
use std::sync::Arc;
fn fixture() -> ferrite_test::TempProject {
    ferrite_test::TempProject::new(&[
        ("index.html","<html><head><title>HOME</title></head><body><!--ssr-outlet--><script type='module' src='/src/main.js'></script></body></html>"),
        ("admin.html","<html><head><title>ADMIN</title></head><body class='admin'><!--ssr-outlet--><script type='module' src='/src/admin-client.js'></script></body></html>"),
        ("src/main.js","globalThis.client='home'"),("src/admin-client.js","globalThis.client='admin'"),
        ("src/home.ts","import './home.css';export function render(url,req){return `<h1>home ${req.method} ${url}</h1>`}"),
        ("src/admin.ts","import './admin.css';export function render(){return '<h1>admin</h1>'}"),
        ("src/home.css","h1{color:red}"),("src/admin.css","h1{color:blue}"),
    ])
}
fn config(p: &ferrite_test::TempProject) -> ferrite::ResolvedConfig {
    let mut c = p.resolve_config_mode("production");
    c.runtime.backend = "napi-vm".into();
    c.base = "/app/".into();
    c.ssr.entry = Some("src/home.ts".into());
    c.ssr.entries = vec![ferrite::config::SsrRouteEntry {
        route: "/admin/*".into(),
        entry: "src/admin.ts".into(),
        shell: Some("admin.html".into()),
    }];
    c
}
async fn html(adapter: &dyn SsrAdapter, path: &str) -> String {
    adapter
        .render(
            SsrHttpRequest {
                method: "POST".into(),
                uri: path.into(),
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
#[tokio::test]
async fn selected_documents_isolate_css_revalidate_shells_and_prerender() {
    let p = fixture();
    let c = config(&p);
    let server = ferrite::DevServer::new_without_watcher(c.clone(), vec![])
        .await
        .unwrap();
    let adapter = ferrite::create_dev_ssr_adapter(
        &server,
        &c,
        &std::fs::read_to_string(p.root.join("index.html")).unwrap(),
    )
    .await
    .unwrap();
    server.set_ssr_adapter(adapter.clone()).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = server.router();
    let serving = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    socket
        .write_all(
            b"POST /app/admin/users?x=1 HTTP/1.0\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n",
        )
        .await
        .unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).await.unwrap();
    serving.abort();
    assert!(
        response.starts_with("HTTP/1.0 200")
            && response.contains("ADMIN")
            && response.contains("admin.css")
            && !response.contains("home.css"),
        "{response}"
    );
    let admin = html(adapter.as_ref(), "/app/admin/users?x=1").await;
    assert!(
        admin.contains("ADMIN") && admin.contains("admin.css") && !admin.contains("home.css"),
        "{admin}"
    );
    let home = html(adapter.as_ref(), "/app/").await;
    assert!(
        home.contains("HOME") && home.contains("home.css") && !home.contains("admin.css"),
        "{home}"
    );
    std::fs::write(
        p.root.join("admin.html"),
        "<head><title>UPDATED</title></head><main><!--ssr-outlet--></main>",
    )
    .unwrap();
    assert!(html(adapter.as_ref(), "/app/admin")
        .await
        .contains("UPDATED"));
    let builder = ferrite::Builder::new(c.clone(), vec![]);
    let reports = builder.build_app().await.unwrap();
    let artifact = ferrite::SsrRendererArtifact::read(&reports[1].out_dir).unwrap();
    assert_eq!(artifact.version, 2);
    assert_eq!(artifact.documents.len(), 2);
    for doc in &artifact.documents {
        assert!(!doc
            .graph
            .modules
            .iter()
            .any(|module| module.code.contains("ssr-route-dispatch")));
    }
    let built = artifact
        .clone()
        .into_adapter(
            Arc::new(ferrite::runtime::napi_vm::NapiVmRuntime::with_defaults()),
            std::fs::read_to_string(p.root.join("dist/index.html")).unwrap(),
            "/app/",
        )
        .unwrap();
    let admin = html(built.as_ref(), "/app/admin").await;
    let home = html(built.as_ref(), "/app/").await;
    let admin_css = &artifact
        .documents
        .iter()
        .find(|d| d.route.is_some())
        .unwrap()
        .stylesheets[0];
    let home_css = &artifact
        .documents
        .iter()
        .find(|d| d.route.is_none())
        .unwrap()
        .stylesheets[0];
    assert!(
        admin.contains("UPDATED") && admin.contains(admin_css) && !admin.contains(home_css),
        "{admin}"
    );
    assert!(
        home.contains(home_css) && !home.contains(admin_css),
        "{home}"
    );
    builder
        .prerender(&["/".into(), "/admin/users".into()])
        .await
        .unwrap();
    let static_admin = std::fs::read_to_string(p.root.join("dist/admin/users/index.html")).unwrap();
    assert!(static_admin.contains("UPDATED") && !static_admin.contains(home_css));
}
#[tokio::test]
async fn document_artifacts_reject_duplicate_routes_and_invalid_shells() {
    let p = fixture();
    let c = config(&p);
    let report = ferrite::Builder::new(c, vec![]).build_app().await.unwrap();
    let mut artifact = ferrite::SsrRendererArtifact::read(&report[1].out_dir).unwrap();
    artifact.documents.push(artifact.documents[0].clone());
    assert!(artifact.validate().is_err());
    artifact.documents.pop();
    artifact.documents[0].shell = Some("no outlet".into());
    assert!(artifact.validate().is_err());
}

#[cfg(target_os = "linux")]
#[tokio::test]
#[ignore = "fresh feature-enabled CLI, process-streamed static generation"]
async fn cli_streamed_documents_prerender_without_node() {
    let p = fixture();
    std::fs::write(p.root.join("ferrite.toml"),"base=\"/app/\"\n[ssr]\nentry=\"src/home.ts\"\nstreaming=true\nprerender=[\"/\",\"/admin\"]\n[[ssr.entries]]\nroute=\"/admin\"\nentry=\"src/admin.ts\"\nshell=\"admin.html\"\n[runtime]\nbackend=\"napi-vm-process\"\n").unwrap();
    for name in ["home", "admin"] {
        std::fs::write(p.root.join(format!("src/{name}.ts")),format!("import './{name}.css';export function render(){{return new ReadableStream({{start(c){{c.enqueue('<h1>{name}</h1>');c.close();}}}})}}")).unwrap();
    }
    let cli = std::env::var_os("FERRITE_CLI_PATH").expect("fresh CLI required");
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        tokio::process::Command::new(cli)
            .arg("prerender")
            .current_dir(&p.root)
            .env("PATH", "")
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let home = std::fs::read_to_string(p.root.join("dist/index.html")).unwrap();
    let admin = std::fs::read_to_string(p.root.join("dist/admin/index.html")).unwrap();
    assert!(
        home.contains("HOME") && home.contains("<h1>home</h1>") && !home.contains("admin-"),
        "{home}"
    );
    assert!(
        admin.contains("ADMIN") && admin.contains("<h1>admin</h1>") && !admin.contains("home-"),
        "{admin}"
    );
    assert!(!p.root.join("dist/server").exists());
}

#[tokio::test]
async fn legacy_manifest_only_renderer_remains_readable_and_executable() {
    let p = fixture();
    let mut c = config(&p);
    c.ssr.entries.clear();
    let reports = ferrite::Builder::new(c, vec![]).build_app().await.unwrap();
    std::fs::remove_file(reports[1].out_dir.join("renderer.json")).unwrap();
    let legacy = ferrite::SsrRendererArtifact::read(&reports[1].out_dir).unwrap();
    assert_eq!(legacy.version, 1);
    assert!(legacy.documents.is_empty());
    let adapter = legacy
        .into_adapter(
            Arc::new(ferrite::runtime::napi_vm::NapiVmRuntime::with_defaults()),
            std::fs::read_to_string(p.root.join("dist/index.html")).unwrap(),
            "/app/",
        )
        .unwrap();
    assert!(html(adapter.as_ref(), "/app/legacy?x=1")
        .await
        .contains("home POST /app/legacy?x=1"));
}
