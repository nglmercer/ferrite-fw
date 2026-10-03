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
    let mut child = std::process::Command::new(&binary)
        .env("PORT", port.to_string())
        .env("HOST", "127.0.0.1")
        .current_dir(serve_dir.path())
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("spawn");
    wait_until_ready(port);
    let body = http_get(port, "/index.html");
    assert!(body.contains("200"), "{body}");
    assert!(body.contains("<script"), "{body}");
    let missing = http_get(port, "/does-not-exist");
    assert!(missing.contains("404"), "{missing}");
    child.kill().ok();
    child.wait().ok();
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
    use std::io::{Read as _, Write as _};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
    write!(stream, "GET {path} HTTP/1.0\r\nHost: x\r\n\r\n").expect("write");
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
            .contains("SSR standalone packaging is unavailable"),
        "{error}"
    );
    assert!(
        !output.exists(),
        "unsupported packaging must not write output"
    );
    let error = builder.build_app().await.unwrap_err();
    assert!(error.to_string().contains("static files"), "{error}");
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
