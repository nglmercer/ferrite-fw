//! Real emitted-renderer static generation, including rollback and public output.
#[cfg(feature = "napi-vm")]
#[tokio::test]
async fn real_ssr_prerender_is_deterministic_private_and_transactional() {
    let project = ferrite_test::TempProject::new(&[
        ("index.html", "<html><head></head><body><!--ssr-outlet--><script type='module' src='/src/main.ts'></script></body></html>"),
        ("src/main.ts", "import './main.css'; globalThis.prerenderClient = true;"),
        ("src/main.css", "body { color: red; }"),
        ("src/entry-server.ts", "export function render(url: string) { return `<h1>generated ${url}</h1>`; }"),
    ]);
    let mut config = project.resolve_config_mode("production");
    config.runtime.backend = "napi-vm".into();
    config.base = "/app/".into();
    let builder = ferrite::Builder::new(config, Vec::new());
    let routes = vec!["/products/item".into(), "/".into(), "/about.html".into()];
    let first = builder.prerender(&routes).await.unwrap();
    let output = project.root.join("dist");
    let root = std::fs::read_to_string(output.join("index.html")).unwrap();
    assert!(root.contains("<h1>generated /app/</h1>"), "{root}");
    assert!(root.contains("/app/assets/"), "{root}");
    assert!(!root.contains("/@ferrite/"), "{root}");
    assert!(
        std::fs::read_to_string(output.join("products/item/index.html"))
            .unwrap()
            .contains("generated /app/products/item")
    );
    assert!(std::fs::read_to_string(output.join("about.html"))
        .unwrap()
        .contains("generated /app/about.html"));
    assert!(
        !output.join("server").exists(),
        "SSG output must not expose its executable renderer"
    );
    let manifest = std::fs::read(output.join("prerender-manifest.json")).unwrap();
    let mut preview_config = builder.config.clone();
    preview_config.runtime.backend = "none".into();
    preview_config.ssr.entry = Some("src/entry-server.ts".into());
    let port = {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap().port()
    };
    preview_config.server.port = port;
    let serving =
        tokio::spawn(async move { ferrite::preview_with_plugins(&preview_config, &[]).await });
    struct Serving(tokio::task::JoinHandle<ferrite::Result<()>>);
    impl Drop for Serving {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let serving = Serving(serving);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_ok()
            {
                break;
            }
            assert!(
                !serving.0.is_finished(),
                "static preview must not require an SSR runtime"
            );
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    async fn get(port: u16, path: &str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut connection = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        connection
            .write_all(format!("GET {path} HTTP/1.0\r\nHost: localhost\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut response = String::new();
        connection.read_to_string(&mut response).await.unwrap();
        response
    }
    let page = get(port, "/app/products/item").await;
    assert!(
        page.starts_with("HTTP/1.0 200") && page.contains("generated /app/products/item"),
        "{page}"
    );
    assert!(get(port, "/app/about.html")
        .await
        .starts_with("HTTP/1.0 200"));
    assert!(get(port, "/app/missing").await.starts_with("HTTP/1.0 404"));
    assert!(get(port, "/app/server/renderer.json")
        .await
        .starts_with("HTTP/1.0 404"));

    assert_eq!(first, builder.prerender(&routes).await.unwrap());
    assert_eq!(
        manifest,
        std::fs::read(output.join("prerender-manifest.json")).unwrap()
    );
    std::fs::write(
        project.root.join("src/entry-server.ts"),
        "export function render() { throw new Error('render failed'); }",
    )
    .unwrap();
    assert!(builder
        .prerender(&routes)
        .await
        .unwrap_err()
        .to_string()
        .contains("render failed"));
    assert_eq!(
        root,
        std::fs::read_to_string(output.join("index.html")).unwrap()
    );
    assert_eq!(
        manifest,
        std::fs::read(output.join("prerender-manifest.json")).unwrap()
    );
}
