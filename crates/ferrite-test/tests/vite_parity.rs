//! Vite parity: the previously missing APIs, end to end.
//!
//! Covers the render pipeline (`options` / `outputOptions` / `renderStart` /
//! `renderChunk` / `augmentChunkHash` / `banner` / `footer`, `buildEnd` on
//! failure), static JS config files, dotenv expansion, and the plugin-driven
//! preview server.

use ferrite::plugin::Plugin;
use ferrite::plugin::PluginContext;
use ferrite::Result;
use ferrite_test::TempProject;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Default)]
struct RenderProbe {
    options_entries: Mutex<Vec<String>>,
    render_entries: Mutex<Vec<String>>,
    build_end_error: Mutex<Option<Option<String>>>,
    closed: Mutex<bool>,
}

struct RenderPlugin {
    probe: Arc<RenderProbe>,
}

#[async_trait::async_trait]
impl Plugin for RenderPlugin {
    fn name(&self) -> &'static str {
        "render-probe"
    }

    async fn options(
        &self,
        _ctx: &PluginContext,
        options: &mut ferrite::plugin::BundleOptions,
    ) -> Result<()> {
        *self.probe.options_entries.lock().unwrap() = options.entries.clone();
        Ok(())
    }

    async fn output_options(
        &self,
        _ctx: &PluginContext,
        options: &mut ferrite::plugin::OutputOptions,
    ) -> Result<()> {
        options.chunk_pattern = "probed/[name]-[hash].js".to_string();
        Ok(())
    }

    async fn render_start(
        &self,
        _ctx: &PluginContext,
        start: ferrite::plugin::RenderStart,
    ) -> Result<()> {
        *self.probe.render_entries.lock().unwrap() =
            start.entries.iter().map(|id| id.0.clone()).collect();
        Ok(())
    }

    async fn render_chunk(
        &self,
        _ctx: &PluginContext,
        chunk: ferrite::plugin::RenderChunk,
    ) -> Result<Option<ferrite::plugin::RenderChunkResult>> {
        Ok(Some(ferrite::plugin::RenderChunkResult {
            code: Some(format!("{}\n// render-chunk:{}\n", chunk.code, chunk.id)),
            map: None,
        }))
    }

    async fn augment_chunk_hash(
        &self,
        _ctx: &PluginContext,
        chunk_id: &str,
    ) -> Result<Option<String>> {
        Ok(Some(format!("probe:{chunk_id}")))
    }

    async fn banner(
        &self,
        _ctx: &PluginContext,
        _chunk: ferrite::plugin::RenderChunk,
    ) -> Result<Option<String>> {
        Ok(Some("/* probe-banner */".to_string()))
    }

    async fn footer(
        &self,
        _ctx: &PluginContext,
        _chunk: ferrite::plugin::RenderChunk,
    ) -> Result<Option<String>> {
        Ok(Some("/* probe-footer */".to_string()))
    }

    async fn build_end(&self, _ctx: &PluginContext, end: ferrite::plugin::BuildEnd) -> Result<()> {
        *self.probe.build_end_error.lock().unwrap() = Some(end.error);
        Ok(())
    }

    async fn close_bundle(&self) -> Result<()> {
        *self.probe.closed.lock().unwrap() = true;
        Ok(())
    }
}

fn production_config(project: &TempProject) -> ferrite::ResolvedConfig {
    ferrite::resolve_config(
        ferrite::UserConfig::default(),
        Some(project.root.clone()),
        ferrite::CliOverrides {
            mode: Some("production".to_string()),
            ..Default::default()
        },
    )
    .expect("resolve")
}

#[tokio::test]
async fn render_pipeline_end_to_end() {
    let files = ferrite_test::vanilla_files();
    let project = TempProject::new(&files);
    let probe = Arc::new(RenderProbe::default());
    let builder = ferrite::Builder::new(
        production_config(&project),
        vec![Arc::new(RenderPlugin {
            probe: probe.clone(),
        })],
    );
    let report = builder.build("client").await.unwrap();
    assert_eq!(report.env, "client");
    // `options` saw the HTML entries; `renderStart` saw module ids.
    assert_eq!(*probe.options_entries.lock().unwrap(), vec!["index.html"]);
    let render_entries = probe.render_entries.lock().unwrap().clone();
    assert_eq!(render_entries, vec!["/src/main.ts"]);
    // Custom chunk pattern honored.
    let mut blob = String::new();
    let mut chunks = 0;
    for entry in std::fs::read_dir(report.out_dir.join("probed")).expect("probed dir") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("js") {
            continue;
        }
        chunks += 1;
        blob.push_str(&std::fs::read_to_string(&path).expect("chunk"));
    }
    assert!(chunks > 0, "expected probed chunks");
    for needle in [
        "/* probe-banner */",
        "/* probe-footer */",
        "// render-chunk:/src/main.ts",
    ] {
        assert!(blob.contains(needle), "missing `{needle}` in:\n{blob}");
    }
    // Success lifecycle.
    assert_eq!(*probe.build_end_error.lock().unwrap(), Some(None));
    assert!(*probe.closed.lock().unwrap());
}

struct FailPlugin {
    probe: Arc<RenderProbe>,
}

#[async_trait::async_trait]
impl Plugin for FailPlugin {
    fn name(&self) -> &'static str {
        "fail-probe"
    }

    async fn generate_bundle(
        &self,
        _ctx: &PluginContext,
        _bundle: &mut ferrite::plugin::OutputBundle,
    ) -> Result<()> {
        Err(ferrite::FerriteError::Build("boom".to_string()))
    }

    async fn build_end(&self, _ctx: &PluginContext, end: ferrite::plugin::BuildEnd) -> Result<()> {
        *self.probe.build_end_error.lock().unwrap() = Some(end.error);
        Ok(())
    }

    async fn close_bundle(&self) -> Result<()> {
        *self.probe.closed.lock().unwrap() = true;
        Ok(())
    }
}

#[tokio::test]
async fn build_end_reports_failure() {
    let files = ferrite_test::vanilla_files();
    let project = TempProject::new(&files);
    let probe = Arc::new(RenderProbe::default());
    let builder = ferrite::Builder::new(
        production_config(&project),
        vec![Arc::new(FailPlugin {
            probe: probe.clone(),
        })],
    );
    let error = builder.build("client").await.unwrap_err();
    assert!(error.to_string().contains("boom"), "{error}");
    // Failure lifecycle: the original error is reported, close still runs.
    let reported = probe.build_end_error.lock().unwrap().clone().flatten();
    assert!(reported.is_some_and(|message| message.contains("boom")));
    assert!(*probe.closed.lock().unwrap());
}

#[tokio::test]
async fn js_config_drives_server() {
    let project = TempProject::new(&[
        (
            "vite.config.js",
            "export default {\n\
             \x20 server: { port: 5321 },\n\
             \x20 resolve: { alias: { \"@\": \"./src\" } },\n\
             };",
        ),
        ("src/main.js", "export const x = 42;\n"),
    ]);
    let server = ferrite::create_server(ferrite::Config {
        root: Some(project.root.clone()),
        ..Default::default()
    })
    .await
    .unwrap();
    assert_eq!(server.inner().config.server.port, 5321);
    let module = server
        .pipeline_module(&ferrite::ModuleId::new("@/main.js"), None, "client")
        .await
        .unwrap();
    assert!(module.code.contains("42"), "{}", module.code);
}

#[tokio::test]
async fn dotenv_expansion_flows_into_server() {
    let project = TempProject::new(&[
        (
            ".env",
            "FERRITE_BASE=/srv\nFERRITE_URL=${FERRITE_BASE}/api\nSECRET=no\n",
        ),
        (".env.production", "FERRITE_MODE_TAG=prod-${FERRITE_BASE}\n"),
    ]);
    let server = ferrite::create_server(ferrite::Config {
        root: Some(project.root.clone()),
        overrides: ferrite::CliOverrides {
            mode: Some("production".to_string()),
            ..Default::default()
        },
        ..Default::default()
    })
    .await
    .unwrap();
    let env = &server.inner().env_vars;
    assert_eq!(env.get("FERRITE_URL").unwrap(), "/srv/api");
    assert_eq!(env.get("FERRITE_MODE_TAG").unwrap(), "prod-/srv");
    assert!(!env.contains_key("SECRET"));
}

// --- preview end to end ------------------------------------------------------

struct PreviewPlugin {
    origin: String,
    mount_dir: std::path::PathBuf,
}

#[async_trait::async_trait]
impl Plugin for PreviewPlugin {
    fn name(&self) -> &'static str {
        "preview-probe"
    }

    async fn configure_preview(&self, preview: &mut ferrite::PreviewControl) -> Result<()> {
        preview.add_header("x-ferrite-test", "yes");
        preview.add_mount("/docs", self.mount_dir.clone());
        preview.add_proxy("/api", self.origin.clone());
        Ok(())
    }
}

/// Minimal raw-HTTP GET (no client dependency in this crate).
async fn http_get(port: u16, path: &str) -> (String, String) {
    http_request(port, path, "GET", "*/*").await
}

async fn http_request(port: u16, path: &str, method: &str, accept: &str) -> (String, String) {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connect");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nhost: x\r\naccept: {accept}\r\nconnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).await.expect("write");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.expect("read");
    let text = String::from_utf8_lossy(&raw).into_owned();
    let split = text.find("\r\n\r\n").expect("header/body split");
    let head = text[..split].to_string();
    let body = String::from_utf8_lossy(&raw[split + 4..]).into_owned();
    (head, body)
}

/// Tiny echo origin for the preview proxy (responds `origin:{path}`).
async fn spawn_echo_origin() -> (String, tokio::task::JoinHandle<()>) {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let task = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let Ok(read) = stream.read(&mut buf).await else {
                    return;
                };
                let request = String::from_utf8_lossy(&buf[..read]);
                let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
                let body = format!("origin:{path}");
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    (format!("http://{addr}"), task)
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("probe")
        .local_addr()
        .expect("addr")
        .port()
}

#[tokio::test]
async fn preview_serves_hooks_mounts_and_proxy() {
    let project = TempProject::new(&[
        (
            "dist/index.html",
            "<!doctype html><html><body>app</body></html>",
        ),
        ("dist/app.js", "console.log(1);\n"),
        ("dist/company-logo.svg", "<svg>first</svg>"),
        ("dist/custom-deadbeef.js", "console.log('first');"),
        ("extra/hello.txt", "mounted\n"),
        ("extra/company-guide.txt", "mounted-first"),
    ]);
    let (origin, origin_task) = spawn_echo_origin().await;
    let port = free_port();
    let config = ferrite::resolve_config(
        ferrite::UserConfig::default(),
        Some(project.root.clone()),
        ferrite::CliOverrides {
            mode: Some("production".to_string()),
            port: Some(port),
            ..Default::default()
        },
    )
    .expect("resolve");
    let plugins: Vec<Arc<dyn Plugin>> = vec![Arc::new(PreviewPlugin {
        origin,
        mount_dir: project.root.join("extra"),
    })];
    let task = tokio::spawn(async move { ferrite::preview_with_plugins(&config, &plugins).await });

    // Wait for the listener (fail fast when the task errors).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if task.is_finished() {
            task.await.unwrap().unwrap();
            panic!("preview task exited early");
        }
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "preview never listened"
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let (head, body) = http_get(port, "/").await;
    assert!(head.contains("200"), "{head}");
    assert!(head.contains("x-ferrite-test: yes"), "{head}");
    assert!(body.contains("app"), "{body}");

    for (url, file, first, second) in [
        (
            "/company-logo.svg",
            "dist/company-logo.svg",
            "<svg>first</svg>",
            "<svg>second</svg>",
        ),
        (
            "/custom-deadbeef.js",
            "dist/custom-deadbeef.js",
            "console.log('first');",
            "console.log('second');",
        ),
        (
            "/docs/company-guide.txt",
            "extra/company-guide.txt",
            "mounted-first",
            "mounted-second",
        ),
    ] {
        let (head, body) = http_get(port, url).await;
        assert!(head.contains("cache-control: no-cache"), "{head}");
        assert!(!head.contains("immutable"), "{head}");
        assert_eq!(body, first);
        std::fs::write(project.root.join(file), second).unwrap();
        let (head, body) = http_get(port, url).await;
        assert!(head.contains("cache-control: no-cache"), "{head}");
        assert_eq!(body, second);
    }

    for missing in [
        "/assets/missing.js",
        "/assets/missing.css",
        "/assets/missing.js.map",
        "/docs/missing.txt",
    ] {
        let (head, body) = http_get(port, missing).await;
        assert!(head.contains("404"), "{missing}: {head}");
        assert!(!body.contains("<html>"), "{missing}: {body}");
    }
    let (head, body) = http_request(port, "/missing-route", "GET", "application/json").await;
    assert!(head.contains("404"), "{head}");
    assert!(!body.contains("<html>"), "{body}");
    let (head, body) = http_request(port, "/app.js", "HEAD", "*/*").await;
    assert!(head.contains("200"), "{head}");
    assert!(body.is_empty(), "{body}");
    let (head, _) = http_request(port, "/app.js", "POST", "*/*").await;
    assert!(
        head.contains("405") && head.contains("allow: GET, HEAD"),
        "{head}"
    );

    // SPA fallback + plugin headers on every static response.
    let (head, body) = http_get(port, "/missing-route").await;
    assert!(head.contains("200"), "{head}");
    assert!(head.contains("x-ferrite-test: yes"), "{head}");
    assert!(body.contains("app"), "{body}");

    // Plugin static mount.
    let (_, body) = http_get(port, "/docs/hello.txt").await;
    assert_eq!(body, "mounted\n");

    // Config/plugin proxy rule.
    let (_, body) = http_get(port, "/api/echo?x=1").await;
    assert_eq!(body, "origin:/api/echo?x=1");
    let (head, body) = http_request(port, "/api/echo", "POST", "*/*").await;
    assert!(head.contains("200"), "{head}");
    assert_eq!(body, "origin:/api/echo");

    task.abort();
    origin_task.abort();
}

#[tokio::test]
async fn facade_reexports_cover_parity_api() {
    assert_eq!(
        ferrite::normalize_path(std::path::Path::new("/a/./b/../c")),
        "/a/c"
    );
    let project = TempProject::new(&[("package.json", "{\"workspaces\": []}")]);
    assert_eq!(
        ferrite::search_for_workspace_root(&project.root),
        project.root
    );
    let mut over = ferrite::UserConfig::default();
    over.server.port = 3000;
    let merged = ferrite::merge_config(ferrite::UserConfig::default(), over);
    assert_eq!(merged.server.port, 3000);
    assert!(
        ferrite::define_config(ferrite::UserConfig::default())
            .server
            .hmr
    );
    let env = ferrite::load_env("development", &project.root, &["FERRITE_".to_string()]);
    assert!(env.is_empty());
}
