//! Vite compat: standalone packaging.

use ferrite_test::TempProject;

// --- standalone packaging -------------------------------------------------------------------

fn standalone_config(project: &TempProject, target: Option<&str>) -> ferrite::ResolvedConfig {
    ferrite::resolve_config(
        ferrite::UserConfig::default(),
        Some(project.root.clone()),
        ferrite::CliOverrides {
            mode: Some("production".to_string()),
            standalone: Some(true),
            target: target.map(str::to_string),
            ..Default::default()
        },
    )
    .expect("resolve")
}

fn standalone_files() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <script type=\"module\" src=\"/src/main.ts\"></script></body></html>",
        ),
        (
            "src/main.ts",
            "import { greet } from \"./greet\";\ndocument.body.textContent = greet(\"ferrite\");\n",
        ),
        (
            "src/greet.ts",
            "export function greet(name: string): string {\n  return `hello ${name}`;\n}\n",
        ),
    ]
}

#[tokio::test]
async fn standalone_build_embeds_scaffold() {
    let project = TempProject::new(&standalone_files());
    std::fs::create_dir_all(project.root.join("src")).unwrap();
    std::fs::write(
        project.root.join("src/entry-server.js"),
        "export function render() { return '<h1>server</h1>'; }",
    )
    .unwrap();
    let config = standalone_config(&project, None);
    assert!(config.package.target.is_none());
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    let scaffold = report.out_dir.join("standalone");
    assert!(scaffold.join("Cargo.toml").is_file());
    assert!(scaffold.join("src/main.rs").is_file());
    let assets = std::fs::read_to_string(scaffold.join("src/assets.rs")).expect("assets.rs");
    assert!(assets.contains("/index.html"), "{assets}");
    assert!(assets.contains("include_bytes!"), "{assets}");
    let main = std::fs::read_to_string(scaffold.join("src/main.rs")).expect("main.rs");
    assert!(main.contains("fallback(handler)"), "{main}");
    assert!(report.standalone_binary.is_none());
}

/// Compiles the generated scaffold for the host triple and boots the
/// binary: the single-binary claim, end to end. Slow (real cargo build).
#[tokio::test]
#[ignore = "slow: real cargo build + boot of the standalone binary"]
async fn standalone_build_compiles_and_serves() {
    let host = rustc_host_triple();
    let project = TempProject::new(&standalone_files());
    let config = standalone_config(&project, Some(&host));
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    let binary = report.standalone_binary.expect("prebuilt binary");
    assert!(binary.is_file(), "{}", binary.display());
    // Boot from an empty cwd: the binary must serve embedded bytes with no
    // sibling dist/.
    let serve_dir = tempfile::tempdir().expect("tempdir");
    let port = free_port();
    let child = std::process::Command::new(&binary)
        .env("PORT", port.to_string())
        .env("HOST", "127.0.0.1")
        .current_dir(serve_dir.path())
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("spawn");
    struct Running(std::process::Child);
    impl Drop for Running {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let _running = Running(child);
    wait_until_ready(port);
    let body = http_get(port, "/index.html");
    assert!(body.contains("200"), "{body}");
    assert!(body.contains("<script"), "{body}");
    let head = http_request(port, "/index.html", "HEAD");
    assert!(head.starts_with("HTTP/1.0 200"), "{head}");
    assert_eq!(head.split_once("\r\n\r\n").unwrap().1, "");
    let get_size = body.split_once("\r\n\r\n").unwrap().1.len();
    assert!(
        head.to_lowercase()
            .contains(&format!("content-length: {get_size}\r\n")),
        "{head}"
    );

    let post = http_request(port, "/index.html", "POST");
    assert!(post.starts_with("HTTP/1.0 405"), "{post}");
    assert!(post.to_lowercase().contains("allow: get, head"), "{post}");
    let missing = http_get(port, "/does-not-exist");
    assert!(missing.contains("404"), "{missing}");
}

/// Cross-compiles the scaffold for musl: the cross-target claim.
/// Slow (real `cargo build --target`).
#[tokio::test]
#[ignore = "slow: real musl cross build"]
async fn standalone_build_cross_compiles_musl() {
    let project = TempProject::new(&standalone_files());
    let config = standalone_config(&project, Some("x86_64-unknown-linux-musl"));
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    let binary = report.standalone_binary.expect("prebuilt binary");
    assert!(binary.is_file(), "{}", binary.display());
    assert!(binary
        .file_name()
        .unwrap()
        .to_string_lossy()
        .contains("x86_64-unknown-linux-musl"));
}

fn rustc_host_triple() -> String {
    let output = std::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .expect("rustc");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::to_string))
        .expect("host triple")
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.local_addr().expect("addr").port()
}

fn wait_until_ready(port: u16) {
    for _ in 0..100 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("server on {port} never came up");
}

fn http_get(port: u16, path: &str) -> String {
    http_request(port, path, "GET")
}

fn http_request(port: u16, path: &str, method: &str) -> String {
    use std::io::{Read as _, Write as _};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
    write!(
        stream,
        "{method} {path} HTTP/1.0\r\nHost: x\r\nContent-Length: 0\r\n\r\n"
    )
    .expect("write");
    let mut body = String::new();
    stream.read_to_string(&mut body).expect("read");
    body
}

#[tokio::test]
async fn ssr_standalone_rejects_static_shell_packaging_before_writing_output() {
    let project = TempProject::new(&[
        ("index.html", "<main><!--ssr-outlet--></main>"),
        (
            "src/entry-server.js",
            "export function render() { return '<h1>actual</h1>'; }",
        ),
    ]);
    let config = standalone_config(&project, None);
    let output = config.out_dir();
    let builder = ferrite::Builder::new(config, Vec::new());
    let error = builder.build("ssr").await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("SSR standalone packaging requires both client and server output"),
        "{error}"
    );
    assert!(
        !output.exists(),
        "unsupported packaging must not write output"
    );
    let error = builder.build_app().await.unwrap_err();
    assert!(error.to_string().contains("package.ssr_sdk"), "{error}");
    assert!(!output.exists());
}

#[test]
fn existing_ssr_output_is_not_embedded_or_packaged_as_static() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("server")).unwrap();
    std::fs::write(root.path().join("server/manifest.json"), "{}").unwrap();
    std::fs::write(
        root.path().join("server/private.js"),
        "const secret = 'server-only';",
    )
    .unwrap();
    std::fs::write(root.path().join("index.html"), "shell").unwrap();
    let assets = ferrite::package::collect_assets(root.path()).unwrap();
    assert_eq!(
        assets.keys().map(String::as_str).collect::<Vec<_>>(),
        ["/index.html"]
    );
    std::fs::rename(
        root.path().join("server/manifest.json"),
        root.path().join("server/renderer.json"),
    )
    .unwrap();
    let assets = ferrite::package::collect_assets(root.path()).unwrap();
    assert_eq!(
        assets.keys().map(String::as_str).collect::<Vec<_>>(),
        ["/index.html"]
    );
    let error = ferrite::package::write_standalone(
        root.path(),
        &ferrite::package::StandaloneOptions {
            embed_assets: true,
            compress_assets: true,
            target: None,
            cargo: None,
        },
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("SSR standalone packaging is unavailable"),
        "{error}"
    );
    assert!(!root.path().join("standalone").exists());
}

/// Build the real production graph, compile its generated application, then
/// execute the embedded renderer with no source directory or command PATH.
#[cfg(feature = "napi-vm")]
#[tokio::test]
#[ignore = "slow: generated SSR Cargo build and HTTP server"]
async fn standalone_ssr_build_compiles_and_renders_without_node() {
    let project = TempProject::new(&[
        ("index.html", "<html><head></head><body><!--ssr-outlet--><script type=\"module\" src=\"/src/main.js\"></script></body></html>"),
        ("src/main.js", "globalThis.clientLoaded = true;"),
        ("src/entry-server.ts", "export function render(url: string, request: any) { return { html: `<h1>embedded ${request.method} ${url}</h1>`, status: 202, headers: [['x-renderer', 'compiled']] }; }"),
    ]);
    let mut config = standalone_config(&project, None);
    config.runtime.backend = "napi-vm".into();
    config.package.ssr_sdk =
        Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ferrite"));
    config.base = "/app/".into();
    let reports = ferrite::Builder::new(config, vec![])
        .build_app()
        .await
        .unwrap();
    assert_eq!(reports.len(), 2);
    let scaffold = reports[0].out_dir.join("standalone");
    let target = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    let status = std::process::Command::new("cargo")
        .args(["build", "--offline", "--manifest-path"])
        .arg(scaffold.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .expect("build generated application");
    assert!(status.success(), "generated SSR application did not build");
    let binary = target
        .join("debug")
        .join(format!("ferrite-app{}", std::env::consts::EXE_SUFFIX));
    // Alter sources: request handling must use only the embedded emitted graph.
    std::fs::write(
        project.root.join("src/entry-server.ts"),
        "throw new Error('source must not execute');",
    )
    .unwrap();
    let cwd = tempfile::tempdir().unwrap();
    let port = free_port();
    let child = std::process::Command::new(binary)
        .env("PATH", "")
        .env("HOST", "127.0.0.1")
        .env("PORT", port.to_string())
        .current_dir(cwd.path())
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("start embedded renderer");
    struct Running(std::process::Child);
    impl Drop for Running {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let _running = Running(child);
    wait_until_ready(port);
    let response = http_get(port, "/app/");
    assert!(response.starts_with("HTTP/1.0 202"), "{response}");
    assert!(
        response.to_lowercase().contains("x-renderer: compiled"),
        "{response}"
    );
    assert!(
        response.contains("<h1>embedded GET /app/</h1>"),
        "{response}"
    );
    assert!(response.contains("<script"), "{response}");
    let post = http_request(port, "/app/submitted", "POST");
    assert!(post.starts_with("HTTP/1.0 202"), "{post}");
    assert!(
        post.contains("<h1>embedded POST /app/submitted</h1>"),
        "{post}"
    );
    let head = http_request(port, "/app/", "HEAD");
    assert!(head.starts_with("HTTP/1.0 202"), "{head}");
    assert_eq!(head.split_once("\r\n\r\n").unwrap().1, "");
    let script = response
        .split("src=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let asset = http_get(port, script);
    assert!(asset.starts_with("HTTP/1.0 200"), "{asset}");
    assert!(asset.contains("clientLoaded"), "{asset}");
    let asset_head = http_request(port, script, "HEAD");
    assert!(asset_head.starts_with("HTTP/1.0 200"), "{asset_head}");
    assert_eq!(asset_head.split_once("\r\n\r\n").unwrap().1, "");
    let size = asset.split_once("\r\n\r\n").unwrap().1.len();
    assert!(
        asset_head
            .to_lowercase()
            .contains(&format!("content-length: {size}\r\n")),
        "{asset_head}"
    );
    let asset_post = http_request(port, script, "POST");
    assert!(asset_post.starts_with("HTTP/1.0 405"), "{asset_post}");
    assert!(
        asset_post.to_lowercase().contains("allow: get, head"),
        "{asset_post}"
    );

    let private = http_get(port, "/app/server/renderer.json");
    assert!(private.starts_with("HTTP/1.0 404"), "{private}");
}
