//! Actual generated-app CLI acceptance. Explicitly execute; never skip missing tools.
use ferrite_e2e::{Browser, BrowserKind, LaunchOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn cli() -> PathBuf {
    std::env::var_os("FERRITE_CLI_PATH")
        .map(PathBuf::from)
        .expect("set FERRITE_CLI_PATH to the freshly built Ferrite CLI")
        .canonicalize()
        .unwrap()
}
async fn command(binary: &Path, root: &Path, args: &[&str], node_enabled: bool) {
    let output = tokio::process::Command::new(binary)
        .args(args)
        .current_dir(root)
        .env(
            "PATH",
            if node_enabled {
                std::env::var_os("PATH").unwrap_or_default()
            } else {
                std::ffi::OsString::new()
            },
        )
        .kill_on_drop(true)
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
async fn server(
    binary: &Path,
    root: &Path,
    mode: &str,
    node_enabled: bool,
) -> (tokio::process::Child, String) {
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port().to_string();
    drop(socket);
    let child = tokio::process::Command::new(binary)
        .args([mode, "--port", &port])
        .current_dir(root)
        .env(
            "PATH",
            if node_enabled {
                std::env::var_os("PATH").unwrap_or_default()
            } else {
                std::ffi::OsString::new()
            },
        )
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let url = format!("http://127.0.0.1:{port}");
    ferrite_e2e::wait_for_url(&url, Duration::from_secs(15))
        .await
        .unwrap();
    (child, url)
}
async fn acceptance(kind: BrowserKind, frameworks: &[&str]) {
    let binary = cli();
    let executable = match kind {
        BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
        BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
    }
    .expect("generated-app acceptance requires the selected real browser");
    let browser = Browser::launch(
        LaunchOptions::default()
            .browser(kind)
            .executable(executable),
    )
    .await
    .unwrap();
    for framework in frameworks {
        let node_enabled = *framework != "vanilla";
        for language in ["js", "ts"] {
            let project = ferrite_test::TempProject::new(&[]);
            let destination = project.root.join("app");
            command(
                &binary,
                &project.root,
                &[
                    "create",
                    destination.to_str().unwrap(),
                    "--framework",
                    framework,
                    "--language",
                    language,
                    "--rendering",
                    "client",
                    "--compiler-host",
                    if node_enabled { "node" } else { "native" },
                ],
                node_enabled,
            )
            .await;
            // Exercise a non-default lock across installer, compiler host,
            // resolver and production pipeline, with no fallback lock present.
            let lock_path = destination.join(if node_enabled && language == "ts" {
                let config = destination.join("ferrite.toml");
                let mut source = std::fs::read_to_string(&config).unwrap();
                source.push_str("\n[npm]\nlockfile = 'selected.lock'\n");
                std::fs::write(config, source).unwrap();
                std::fs::rename(
                    destination.join("ferrite.lock"),
                    destination.join("selected.lock"),
                )
                .unwrap();
                "selected.lock"
            } else {
                "ferrite.lock"
            });
            let lock = std::fs::read(&lock_path).unwrap();
            assert_eq!(
                std::fs::read_link(destination.join("node_modules")).unwrap(),
                PathBuf::from(".ferrite/npm/node_modules")
            );
            if node_enabled {
                let graph = ferrite::npm::Lockfile::read(&lock_path).unwrap();
                let identity = &graph.importers["."].dependencies[*framework];
                let actual = destination
                    .join("node_modules")
                    .join(framework)
                    .canonicalize()
                    .unwrap();
                assert_eq!(
                    actual,
                    destination
                        .join(".ferrite/npm/packages")
                        .join(identity)
                        .canonicalize()
                        .unwrap(),
                    "editor resolution must use the same concrete importer edge before dev starts"
                );
            }

            command(
                &binary,
                &destination,
                &["install", "--frozen-lockfile"],
                node_enabled,
            )
            .await;
            assert_eq!(std::fs::read(&lock_path).unwrap(), lock);
            let (mut dev, url) = server(&binary, &destination, "dev", node_enabled).await;
            let page = browser.new_page().await.unwrap();
            page.goto(&url).await.unwrap();
            page.wait_for_function(
                "document.querySelector('#counter')?.textContent === 'count: 0'",
                Duration::from_secs(15),
            )
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "{framework}/{language}: {error}; {:?}; {:?}",
                    page.page_errors(),
                    page.console_messages()
                )
            });
            page.locator("#counter").click().await.unwrap();
            page.wait_for_function(
                "document.querySelector('#counter')?.textContent === 'count: 1'",
                Duration::from_secs(10),
            )
            .await
            .unwrap();
            if node_enabled {
                let color: String = page
                    .evaluate("getComputedStyle(document.querySelector('#counter')).color")
                    .await
                    .unwrap();
                assert_eq!(color, "rgb(128, 0, 0)", "initial generated scoped CSS");
            }
            let main = destination.join(if node_enabled {
                format!("src/App.{framework}")
            } else {
                format!("src/main.{language}")
            });
            let source = std::fs::read_to_string(&main).unwrap();
            let invalid = match *framework {
                "vue" => "<script setup>const broken = ;</script><template><button>broken</button></template>",
                "svelte" => "<script>const broken = ;</script><button>broken</button>",
                _ => "const broken = ;",
            };
            std::fs::write(&main, invalid).unwrap();
            page.wait_for_function(
                "document.querySelector('#ferrite-error-overlay')?.textContent.length > 0",
                Duration::from_secs(15),
            )
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "{framework}/{language}: {error}; errors={:?}; console={:?}",
                    page.page_errors(),
                    page.console_messages()
                )
            });
            let overlay: String = page
                .evaluate("document.querySelector('#ferrite-error-overlay').textContent")
                .await
                .unwrap();
            assert!(
                overlay.contains(main.file_name().unwrap().to_str().unwrap()),
                "{overlay}"
            );
            assert_eq!(
                page.evaluate::<String>("document.querySelector('#counter').textContent")
                    .await
                    .unwrap(),
                "count: 1",
                "invalid source must keep the running application"
            );

            let changed = source.replace("count += 1", "count += 2");
            let changed = if node_enabled {
                changed
                    .replace(">count: ", ">edited count: ")
                    .replace("rgb(128, 0, 0)", "rgb(0, 0, 128)")
            } else {
                format!("{changed}\ndocument.body.dataset.revision = 'edited';\n")
            };
            std::fs::write(&main, changed).unwrap();
            let prefix = if node_enabled {
                "edited count"
            } else {
                "count"
            };
            page.wait_for_function(
                &format!("document.querySelector('#counter')?.textContent === '{prefix}: 0'"),
                Duration::from_secs(15),
            )
            .await
            .unwrap();
            page.locator("#counter").click().await.unwrap();
            page.wait_for_function(
                &format!("document.querySelector('#counter')?.textContent === '{prefix}: 2'"),
                Duration::from_secs(10),
            )
            .await
            .unwrap();
            assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
            assert!(
                page.console_messages()
                    .iter()
                    .all(|message| message.kind != "error"),
                "{:?}",
                page.console_messages()
            );
            if node_enabled {
                let color: String = page
                    .evaluate("getComputedStyle(document.querySelector('#counter')).color")
                    .await
                    .unwrap();
                assert_eq!(color, "rgb(0, 0, 128)", "updated generated scoped CSS");
            }
            dev.kill().await.unwrap();
            dev.wait().await.unwrap();
            command(
                &binary,
                &destination,
                &["build", "--scope-hoist"],
                node_enabled,
            )
            .await;
            assert!(destination.join("dist/index.html").exists());
            for asset in std::fs::read_dir(destination.join("dist/assets")).unwrap() {
                let asset = asset.unwrap().path();
                if asset.extension().is_some_and(|extension| extension == "js") {
                    let code = std::fs::read_to_string(asset).unwrap();
                    assert!(!code.contains("/@ferrite/client"));
                    assert!(!code.contains("__ferrite_create_hot__"));
                    assert!(!code.contains("react-refresh/runtime"));
                }
            }

            let (mut preview, url) = server(&binary, &destination, "preview", node_enabled).await;
            let page = browser.new_page().await.unwrap();
            page.goto(&url).await.unwrap();
            page.wait_for_function(
                &format!("document.querySelector('#counter')?.textContent === '{prefix}: 0'"),
                Duration::from_secs(10),
            )
            .await
            .unwrap();
            page.locator("#counter").click().await.unwrap();
            page.wait_for_function(
                &format!("document.querySelector('#counter')?.textContent === '{prefix}: 2'"),
                Duration::from_secs(10),
            )
            .await
            .unwrap();
            assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
            assert!(
                page.console_messages()
                    .iter()
                    .all(|message| message.kind != "error"),
                "{:?}",
                page.console_messages()
            );
            if node_enabled {
                let color: String = page
                    .evaluate("getComputedStyle(document.querySelector('#counter')).color")
                    .await
                    .unwrap();
                assert_eq!(color, "rgb(0, 0, 128)", "production extracted scoped CSS");
            }
            preview.kill().await.unwrap();
            preview.wait().await.unwrap();
            eprintln!("{kind:?}: {framework}/{language}/client complete (Node compiler enabled: {node_enabled})");
        }
    }
    browser.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires freshly built FERRITE_CLI_PATH and real Chromium"]
async fn chromium_generated_profiles() {
    acceptance(BrowserKind::Chromium, &["vanilla"]).await;
}
#[tokio::test]
#[ignore = "requires freshly built FERRITE_CLI_PATH and real Firefox"]
async fn firefox_generated_profiles() {
    acceptance(BrowserKind::Firefox, &["vanilla"]).await;
}

#[tokio::test]
#[ignore = "requires freshly built CLI, Node and real Chromium"]
async fn chromium_generated_components() {
    acceptance(BrowserKind::Chromium, &["svelte", "vue"]).await;
}
#[tokio::test]
#[ignore = "requires freshly built CLI, Node and real Firefox"]
async fn firefox_generated_components() {
    acceptance(BrowserKind::Firefox, &["svelte", "vue"]).await;
}

async fn dependency_acceptance(kind: BrowserKind) {
    let binary = cli();
    let project = ferrite_test::TempProject::new(&[
        ("index.html", "<html><head><link rel='icon' href='data:,'></head><body><button id='counter'>loading</button><span id='other'></span><script type='module' src='/main.js'></script></body></html>"),
        ("dep.js", "export const step = 1; if (import.meta.hot) import.meta.hot.dispose(() => { globalThis.disposals = (globalThis.disposals || 0) + 1; });"),
        ("main.js", "import {step} from './dep.js'; import './other.js'; globalThis.parentRuns = (globalThis.parentRuns || 0) + 1; globalThis.session = Math.random(); let increment = step; let count = 0; const button = document.querySelector('#counter'); const render = () => button.textContent = `count: ${count}, step: ${increment}`; render(); button.onclick = () => { count += increment; render(); }; if (import.meta.hot) { import.meta.hot.accept(['./dep.js'], ([next]) => { increment = next.step; render(); }); import.meta.hot.dispose(() => { throw new Error('accepting parent was disposed'); }); }"),
        ("other.js", "import {step} from './dep.js'; globalThis.otherRuns = (globalThis.otherRuns || 0) + 1; document.querySelector('#other').textContent = `other: ${step}`; if (import.meta.hot) import.meta.hot.accept('./dep.js', next => { document.querySelector('#other').textContent = `other: ${next.step}`; globalThis.otherCallbacks = (globalThis.otherCallbacks || 0) + 1; });"),
    ]);
    let executable = match kind {
        BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
        BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
    }
    .expect("dependency acceptance requires a real browser");
    let browser = Browser::launch(
        LaunchOptions::default()
            .browser(kind)
            .executable(executable),
    )
    .await
    .unwrap();
    let (mut dev, url) = server(&binary, &project.root, "dev", false).await;
    let page = browser.new_page().await.unwrap();
    page.goto(&url).await.unwrap();
    page.wait_for_function("document.querySelector('#counter')?.textContent === 'count: 0, step: 1' && document.querySelector('#other')?.textContent === 'other: 1'", Duration::from_secs(10)).await.unwrap_or_else(|error| panic!("{error}: errors={:?}; console={:?}", page.page_errors(), page.console_messages()));
    let session: f64 = page.evaluate("globalThis.session").await.unwrap();
    page.locator("#counter").click().await.unwrap();
    let mut count = 1;
    std::fs::write(project.root.join("dep.js"), "export const step = ;").unwrap();
    page.wait_for_function(
        "document.querySelector('#ferrite-error-overlay')?.textContent.includes('/dep.js')",
        Duration::from_secs(10),
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "{error}; errors={:?}; console={:?}",
            page.page_errors(),
            page.console_messages()
        )
    });
    assert_eq!(
        page.evaluate::<f64>("globalThis.session").await.unwrap(),
        session
    );
    assert_eq!(
        page.evaluate::<u32>("globalThis.disposals || 0")
            .await
            .unwrap(),
        0,
        "invalid compilation must not dispose live modules"
    );
    assert_eq!(
        page.evaluate::<String>("document.querySelector('#counter').textContent")
            .await
            .unwrap(),
        "count: 1, step: 1"
    );

    for step in [2, 3] {
        std::fs::write(project.root.join("dep.js"), format!("export const step = {step}; if (import.meta.hot) import.meta.hot.dispose(() => {{ globalThis.disposals = (globalThis.disposals || 0) + 1; }});")).unwrap();
        page.wait_for_function(&format!("document.querySelector('#counter')?.textContent === 'count: {count}, step: {step}' && document.querySelector('#other')?.textContent === 'other: {step}'"), Duration::from_secs(10)).await.unwrap_or_else(|error| panic!("{error}: errors={:?}; console={:?}", page.page_errors(), page.console_messages()));
        assert!(page
            .evaluate::<bool>("!document.querySelector('#ferrite-error-overlay')")
            .await
            .unwrap());
        assert_eq!(
            page.evaluate::<f64>("globalThis.session").await.unwrap(),
            session,
            "document must not reload"
        );
        assert_eq!(
            page.evaluate::<u32>("globalThis.parentRuns").await.unwrap(),
            1
        );
        assert_eq!(
            page.evaluate::<u32>("globalThis.otherRuns").await.unwrap(),
            1
        );
        assert_eq!(
            page.evaluate::<u32>("globalThis.disposals").await.unwrap(),
            step - 1,
            "shared dependency is disposed once"
        );
        assert_eq!(
            page.evaluate::<u32>("globalThis.otherCallbacks")
                .await
                .unwrap(),
            step - 1
        );
        page.locator("#counter").click().await.unwrap();
        count += step;
        page.wait_for_function(&format!("document.querySelector('#counter')?.textContent === 'count: {count}, step: {step}'"), Duration::from_secs(5)).await.unwrap();
    }
    assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
    assert!(
        page.console_messages()
            .iter()
            .all(|message| message.kind != "error"),
        "{:?}",
        page.console_messages()
    );
    dev.kill().await.unwrap();
    dev.wait().await.unwrap();
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires freshly built CLI and real Chromium"]
async fn chromium_dependency_acceptance() {
    dependency_acceptance(BrowserKind::Chromium).await;
}
#[tokio::test]
#[ignore = "requires freshly built CLI and real Firefox"]
async fn firefox_dependency_acceptance() {
    dependency_acceptance(BrowserKind::Firefox).await;
}

async fn transitive_self_acceptance(kind: BrowserKind) {
    let binary = cli();
    let project = ferrite_test::TempProject::new(&[
        ("ferrite.toml", "[npm]\ndev_strategy = 'import-map'\n[resolve.alias]\nleaf = './leaf.js'\n"),
        ("index.html", "<html><head><link rel='icon' href='data:,'></head><body><button id='counter'>loading</button><span id='factor'></span><script type='module' src='/main.js'></script></body></html>"),
        ("leaf.js", "import './mid.js'; export const step = 1; globalThis.leafRuns = (globalThis.leafRuns || 0) + 1;"),
        ("mid.js", "export {step} from 'leaf'; globalThis.midRuns = (globalThis.midRuns || 0) + 1;"),
        ("boundary.js", "import {step} from './mid.js'; export const factor = step; globalThis.factor = factor; document.querySelector('#factor').textContent = `step: ${factor}`; globalThis.boundaryRuns = (globalThis.boundaryRuns || 0) + 1; if (import.meta.hot) { import.meta.hot.accept(next => { globalThis.factor = next.factor; globalThis.callbacks = (globalThis.callbacks || 0) + 1; }); import.meta.hot.dispose(() => { globalThis.disposals = (globalThis.disposals || 0) + 1; }); }"),
        ("main.js", "import './boundary.js'; globalThis.mainRuns = (globalThis.mainRuns || 0) + 1; globalThis.session = Math.random(); let count = 0; const button = document.querySelector('#counter'); button.textContent = `count: ${count}`; button.onclick = () => { count += globalThis.factor; button.textContent = `count: ${count}`; };"),
    ]);
    let executable = match kind {
        BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
        BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
    }
    .expect("transitive acceptance requires a real browser");
    let browser = Browser::launch(
        LaunchOptions::default()
            .browser(kind)
            .executable(executable),
    )
    .await
    .unwrap();
    let (mut dev, url) = server(&binary, &project.root, "dev", false).await;
    let page = browser.new_page().await.unwrap();
    page.goto(&url).await.unwrap();
    page.wait_for_function(
        "document.querySelector('#counter')?.textContent === 'count: 0' && globalThis.factor === 1",
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    let session: f64 = page.evaluate("globalThis.session").await.unwrap();
    page.locator("#counter").click().await.unwrap();
    let mut count = 1;
    std::fs::write(project.root.join("leaf.js"), "export const step = ;").unwrap();
    page.wait_for_function(
        "document.querySelector('#ferrite-error-overlay')?.textContent.includes('/leaf.js')",
        Duration::from_secs(10),
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "{error}; errors={:?}; console={:?}",
            page.page_errors(),
            page.console_messages()
        )
    });
    assert_eq!(
        page.evaluate::<f64>("globalThis.session").await.unwrap(),
        session
    );
    assert_eq!(
        page.evaluate::<u32>("globalThis.disposals || 0")
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        page.evaluate::<String>("document.querySelector('#counter').textContent")
            .await
            .unwrap(),
        "count: 1"
    );

    for step in [2, 3] {
        std::fs::write(
            project.root.join("leaf.js"),
            format!(
                "export const step = {step}; globalThis.leafRuns = (globalThis.leafRuns || 0) + 1;"
            ),
        )
        .unwrap();
        page.wait_for_function(&format!("globalThis.factor === {step} && globalThis.callbacks === {} && document.querySelector('#factor')?.textContent === 'step: {step}'", step - 1), Duration::from_secs(10)).await.unwrap_or_else(|error| panic!("{error}; errors={:?}; console={:?}", page.page_errors(), page.console_messages()));
        assert!(page
            .evaluate::<bool>("!document.querySelector('#ferrite-error-overlay')")
            .await
            .unwrap());
        assert_eq!(
            page.evaluate::<f64>("globalThis.session").await.unwrap(),
            session
        );
        assert_eq!(
            page.evaluate::<u32>("globalThis.mainRuns").await.unwrap(),
            1
        );
        for runs in ["leafRuns", "midRuns", "boundaryRuns"] {
            assert_eq!(
                page.evaluate::<u32>(&format!("globalThis.{runs}"))
                    .await
                    .unwrap(),
                step
            );
        }
        assert_eq!(
            page.evaluate::<u32>("globalThis.disposals").await.unwrap(),
            step - 1
        );
        assert_eq!(
            page.evaluate::<String>("document.querySelector('#counter').textContent")
                .await
                .unwrap(),
            format!("count: {count}")
        );
        page.locator("#counter").click().await.unwrap();
        count += step;
        page.wait_for_function(
            &format!("document.querySelector('#counter')?.textContent === 'count: {count}'"),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    }
    assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
    assert!(
        page.console_messages()
            .iter()
            .all(|message| message.kind != "error"),
        "{:?}",
        page.console_messages()
    );
    dev.kill().await.unwrap();
    dev.wait().await.unwrap();
    browser.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires freshly built CLI and real Chromium"]
async fn chromium_transitive_self_acceptance() {
    transitive_self_acceptance(BrowserKind::Chromium).await;
}
#[tokio::test]
#[ignore = "requires freshly built CLI and real Firefox"]
async fn firefox_transitive_self_acceptance() {
    transitive_self_acceptance(BrowserKind::Firefox).await;
}

async fn initial_load_recovery(kind: BrowserKind) {
    let binary = cli();
    let executable = match kind {
        BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
        BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
    }
    .expect("initial recovery requires a real browser");
    let browser = Browser::launch(
        LaunchOptions::default()
            .browser(kind)
            .executable(executable),
    )
    .await
    .unwrap();
    for scenario in [
        "invalid-entry",
        "missing-entry",
        "missing-import",
        "alias-index",
    ] {
        let project = ferrite_test::TempProject::new(&[("index.html", "<html><head><link rel='icon' href='data:,'></head><body><button id='counter'>loading</button><script type='module' src='/main.js'></script></body></html>")]);
        let main = project.root.join("main.js");
        let repaired_entry = "export const ready = true; let count = 0; const button = document.querySelector('#counter'); button.textContent = 'count: 0'; button.onclick = () => {count++; button.textContent = `count: ${count}`;};";
        let repair = match scenario {
            "invalid-entry" => {
                std::fs::write(&main, "export const count = ;").unwrap();
                main.clone()
            }
            "missing-entry" => main.clone(),
            "missing-import" => {
                std::fs::write(&main, format!("import './child'; {repaired_entry}")).unwrap();
                project.root.join("child.js")
            }
            "alias-index" => {
                std::fs::create_dir(project.root.join("folder")).unwrap();
                std::fs::write(
                    project.root.join("ferrite.toml"),
                    "[resolve.alias]\n'@child' = './folder'\n",
                )
                .unwrap();
                std::fs::write(&main, format!("import '@child'; {repaired_entry}")).unwrap();
                project.root.join("folder/index.js")
            }
            _ => unreachable!(),
        };
        let (mut dev, url) = server(&binary, &project.root, "dev", false).await;
        // Fail a module request before any HMR socket exists, then connect
        // through the actual document to require diagnostic replay.
        let failed_request = browser.new_page().await.unwrap();
        failed_request
            .goto(&format!("{url}/main.js"))
            .await
            .unwrap();
        failed_request
            .wait_for_function(
                "document.body.textContent.includes('error')",
                std::time::Duration::from_secs(10),
            )
            .await
            .unwrap();
        let page = browser.new_page().await.unwrap();
        page.goto(&url).await.unwrap();
        page.wait_for_function("document.readyState === 'complete' && document.querySelector('#ferrite-error-overlay')?.textContent.includes('/main.js')", Duration::from_secs(10)).await.unwrap_or_else(|error| panic!("{error}; errors={:?}; console={:?}", page.page_errors(), page.console_messages()));
        assert_eq!(
            page.evaluate::<String>("document.querySelector('#counter').textContent")
                .await
                .unwrap(),
            "loading"
        );
        let original_entry = std::fs::read(&main).ok();
        std::fs::write(
            &repair,
            if repair == main {
                repaired_entry
            } else {
                "export const repairedDependency = true;"
            },
        )
        .unwrap();
        if repair != main {
            assert_eq!(
                std::fs::read(&main).ok(),
                original_entry,
                "dependency creation alone must recover the importer"
            );
        }
        page.wait_for_function("!document.querySelector('#ferrite-error-overlay') && document.querySelector('#counter')?.textContent === 'count: 0'", Duration::from_secs(10)).await.unwrap();
        page.locator("#counter").click().await.unwrap();
        page.wait_for_function(
            "document.querySelector('#counter')?.textContent === 'count: 1'",
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        // Initial failing requests intentionally have browser network errors.
        // A fresh document must receive neither stale diagnostics nor errors.
        let clean = browser.new_page().await.unwrap();
        clean.goto(&url).await.unwrap();
        clean
            .wait_for_function(
                "document.querySelector('#counter')?.textContent === 'count: 0'",
                Duration::from_secs(10),
            )
            .await
            .unwrap();
        clean.locator("#counter").click().await.unwrap();
        clean
            .wait_for_function(
                "document.querySelector('#counter')?.textContent === 'count: 1'",
                Duration::from_secs(5),
            )
            .await
            .unwrap();
        assert!(clean
            .evaluate::<bool>("!document.querySelector('#ferrite-error-overlay')")
            .await
            .unwrap());
        assert!(clean.page_errors().is_empty(), "{:?}", clean.page_errors());
        assert!(
            clean
                .console_messages()
                .iter()
                .all(|message| message.kind != "error"),
            "{:?}",
            clean.console_messages()
        );
        dev.kill().await.unwrap();
        dev.wait().await.unwrap();
    }
    browser.close().await.unwrap();
}
#[tokio::test]
#[ignore = "requires freshly built CLI and real Chromium"]
async fn chromium_initial_load_recovery() {
    initial_load_recovery(BrowserKind::Chromium).await;
}
#[tokio::test]
#[ignore = "requires freshly built CLI and real Firefox"]
async fn firefox_initial_load_recovery() {
    initial_load_recovery(BrowserKind::Firefox).await;
}

async fn configured_foreign_hook_acceptance(kind: BrowserKind) {
    let binary = cli();
    let executable = match kind {
        BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
        BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
    }
    .expect("foreign-hook acceptance requires the selected real browser");
    let browser = Browser::launch(
        LaunchOptions::default()
            .browser(kind)
            .executable(executable),
    )
    .await
    .unwrap();
    let project = ferrite_test::TempProject::new(&[]);
    std::fs::write(project.root.join("ferrite.toml"), "[[foreign_plugins]]\nname = 'configured'\nentry = 'plugin.mjs'\nhost = 'node'\n[foreign_plugins.options]\nvalue = 'compiled-label'\n").unwrap();
    std::fs::write(project.root.join("plugin.mjs"), "export default options => ({transform(code, id, context) { if (!id.endsWith('/entry.js')) return null; if(context.ssr !== false) throw new Error('incorrect target'); return {code: code.replace('original-label', options.value)}; }});").unwrap();
    std::fs::write(project.root.join("index.html"), "<html><head><link rel='icon' href='data:,'></head><body><button id='counter'>loading</button><script type='module' src='/entry.js'></script></body></html>").unwrap();
    let source = "let count = 0; const button = document.querySelector('#counter'); const label = 'original-label'; const render = () => button.textContent = label + ': ' + count; button.onclick = () => { count += 1; render(); }; render();";
    std::fs::write(project.root.join("entry.js"), source).unwrap();
    let (mut dev, url) = server(&binary, &project.root, "dev", true).await;
    let page = browser.new_page().await.unwrap();
    page.goto(&url).await.unwrap();
    page.wait_for_function(
        "document.querySelector('#counter')?.textContent === 'compiled-label: 0'",
        Duration::from_secs(15),
    )
    .await
    .unwrap();
    page.locator("#counter").click().await.unwrap();
    page.wait_for_function(
        "document.querySelector('#counter')?.textContent === 'compiled-label: 1'",
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    std::fs::write(
        project.root.join("entry.js"),
        source.replace("count += 1", "count += 2"),
    )
    .unwrap();
    page.wait_for_function(
        "document.querySelector('#counter')?.textContent === 'compiled-label: 0'",
        Duration::from_secs(15),
    )
    .await
    .unwrap();
    page.locator("#counter").click().await.unwrap();
    page.wait_for_function(
        "document.querySelector('#counter')?.textContent === 'compiled-label: 2'",
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
    assert!(
        page.console_messages()
            .iter()
            .all(|message| message.kind != "error"),
        "{:?}",
        page.console_messages()
    );
    dev.kill().await.unwrap();
    dev.wait().await.unwrap();
    command(&binary, &project.root, &["build", "--scope-hoist"], true).await;
    let (mut preview, url) = server(&binary, &project.root, "preview", true).await;
    let page = browser.new_page().await.unwrap();
    page.goto(&url).await.unwrap();
    page.wait_for_function(
        "document.querySelector('#counter')?.textContent === 'compiled-label: 0'",
        Duration::from_secs(15),
    )
    .await
    .unwrap();
    page.locator("#counter").click().await.unwrap();
    page.wait_for_function(
        "document.querySelector('#counter')?.textContent === 'compiled-label: 2'",
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
    assert!(
        page.console_messages()
            .iter()
            .all(|message| message.kind != "error"),
        "{:?}",
        page.console_messages()
    );
    preview.kill().await.unwrap();
    preview.wait().await.unwrap();
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires real CLI, explicit Node hook profile and Chromium"]
async fn chromium_configured_foreign_hooks() {
    configured_foreign_hook_acceptance(BrowserKind::Chromium).await;
}

#[tokio::test]
#[ignore = "requires real CLI, explicit Node hook profile and Firefox"]
async fn firefox_configured_foreign_hooks() {
    configured_foreign_hook_acceptance(BrowserKind::Firefox).await;
}

async fn react_refresh_dom_acceptance(kind: BrowserKind, wrapped: bool, imported_hook: bool) {
    let binary = cli();
    let project = ferrite_test::TempProject::new(&[
        ("package.json", r#"{"private":true,"dependencies":{"react":"19.2.0","react-dom":"19.2.0","react-refresh":"0.17.0"}}"#),
        ("index.html", "<html><head><link rel='icon' href='data:,'></head><body><div id='root'></div><script type='module' src='/main.jsx'></script></body></html>"),
        ("main.jsx", "import {createRoot} from 'react-dom/client'; import {App} from './App.jsx'; globalThis.session = Math.random(); createRoot(document.querySelector('#root')).render(<App/>);"),
    ]);
    std::fs::write(project.root.join("hooks.ts"), "import {useState} from 'react'; export function useCounter(): [number, (value: number) => void] { return useState(0); }").unwrap();
    let source = if imported_hook {
        "import {memo} from 'react'; import {useCounter} from './hooks.ts'; export const App = memo(function Counter() { const [count, setCount] = useCounter(); return <button id='counter' onClick={() => setCount(count + 1)}>first: {count}</button>; });"
    } else if wrapped {
        "import {useState, memo} from 'react'; function useCounter() { return useState(0); } export const App = memo(function Counter() { const [count, setCount] = useCounter(); return <button id='counter' onClick={() => setCount(count + 1)}>first: {count}</button>; });"
    } else {
        "import {useState} from 'react'; export function App() { const [count, setCount] = useState(0); return <button id='counter' onClick={() => setCount(count + 1)}>first: {count}</button>; }"
    };
    std::fs::write(project.root.join("App.jsx"), source).unwrap();
    command(&binary, &project.root, &["install"], false).await;
    let executable = match kind {
        BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
        BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
    }
    .expect("React DOM acceptance requires a real browser");
    let browser = Browser::launch(
        LaunchOptions::default()
            .browser(kind)
            .executable(executable),
    )
    .await
    .unwrap();
    let (mut dev, url) = server(&binary, &project.root, "dev", false).await;
    let page = browser.new_page().await.unwrap();
    page.goto(&url).await.unwrap();
    page.wait_for_function(
        "document.querySelector('#counter')?.textContent === 'first: 0'",
        Duration::from_secs(15),
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "{error}; errors={:?}; console={:?}",
            page.page_errors(),
            page.console_messages()
        )
    });
    page.locator("#counter").click().await.unwrap();
    page.wait_for_function(
        "document.querySelector('#counter')?.textContent === 'first: 1'",
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    let session: f64 = page.evaluate("globalThis.session").await.unwrap();
    std::fs::write(
        project.root.join("App.jsx"),
        source.replace("first:", "edited:"),
    )
    .unwrap();
    let refresh_result = page
        .wait_for_function(
            "document.querySelector('#counter')?.textContent === 'edited: 1'",
            Duration::from_secs(15),
        )
        .await;
    if let Err(error) = refresh_result {
        let state: serde_json::Value = page.evaluate("({text:document.querySelector('#counter')?.textContent, session:globalThis.session, overlay:document.querySelector('#ferrite-error-overlay')?.textContent, renderers:globalThis.__REACT_DEVTOOLS_GLOBAL_HOOK__?.renderers?.size, preamble:globalThis.__ferrite_react_preamble_installed__})").await.unwrap();
        panic!("{error}; state={state}; errors={:?}", page.page_errors());
    }
    assert_eq!(
        page.evaluate::<f64>("globalThis.session").await.unwrap(),
        session,
        "Refresh must not reload the document"
    );
    page.locator("#counter").click().await.unwrap();
    page.wait_for_function(
        "document.querySelector('#counter')?.textContent === 'edited: 2'",
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    std::fs::write(project.root.join("App.jsx"), "export function App( {").unwrap();
    page.wait_for_function(
        "document.querySelector('#ferrite-error-overlay')?.textContent.includes('/App.jsx')",
        Duration::from_secs(15),
    )
    .await
    .unwrap();
    assert_eq!(
        page.evaluate::<String>("document.querySelector('#counter').textContent")
            .await
            .unwrap(),
        "edited: 2"
    );
    assert_eq!(
        page.evaluate::<f64>("globalThis.session").await.unwrap(),
        session
    );
    std::fs::write(
        project.root.join("App.jsx"),
        source.replace("first:", "recovered:"),
    )
    .unwrap();
    page.wait_for_function("document.querySelector('#counter')?.textContent === 'recovered: 2' && !document.querySelector('#ferrite-error-overlay')", Duration::from_secs(15)).await.unwrap();
    page.locator("#counter").click().await.unwrap();
    page.wait_for_function(
        "document.querySelector('#counter')?.textContent === 'recovered: 3'",
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    if imported_hook {
        std::fs::write(project.root.join("hooks.ts"), "import {useState} from 'react'; export function useCounter(): [number, (value: number) => void] { return useState(0).map((value, index) => index === 0 ? (value as number) + 10 : value) as [number, (value: number) => void]; }").unwrap();
        page.wait_for_function(
            "document.querySelector('#counter')?.textContent === 'recovered: 13'",
            Duration::from_secs(15),
        )
        .await
        .unwrap_or_else(|error| {
            panic!(
                "compatible imported hook update failed: {error}; errors={:?}",
                page.page_errors()
            )
        });
        assert_eq!(
            page.evaluate::<f64>("globalThis.session").await.unwrap(),
            session
        );
    }
    if imported_hook {
        std::fs::write(project.root.join("hooks.ts"), "import {useState} from 'react'; export function useCounter(): [number, (value: number) => void] { useState('extra'); return useState(0); }").unwrap();
        page.wait_for_function(
            "document.querySelector('#counter')?.textContent === 'recovered: 0'",
            Duration::from_secs(15),
        )
        .await
        .unwrap_or_else(|error| {
            panic!(
                "imported hook reset failed: {error}; errors={:?}; console={:?}",
                page.page_errors(),
                page.console_messages()
            )
        });
        assert_eq!(
            page.evaluate::<f64>("globalThis.session").await.unwrap(),
            session
        );
    }
    let changed_signature = if imported_hook {
        source.to_string()
    } else if wrapped {
        source.replace(
            "return useState(0);",
            "useState('extra'); return useState(0);",
        )
    } else {
        source.replace(
            "const [count, setCount]",
            "const [extra] = useState(0); const [count, setCount]",
        )
    }
    .replace("first:", "reset:");
    std::fs::write(project.root.join("App.jsx"), changed_signature).unwrap();
    page.wait_for_function(
        "document.querySelector('#counter')?.textContent === 'reset: 0'",
        Duration::from_secs(15),
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "signature reset failed: {error}; errors={:?}",
            page.page_errors()
        )
    });
    assert_eq!(
        page.evaluate::<f64>("globalThis.session").await.unwrap(),
        session,
        "hook changes reset the component without reloading its document"
    );
    page.locator("#counter").click().await.unwrap();
    page.wait_for_function(
        "document.querySelector('#counter')?.textContent === 'reset: 1'",
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
    assert!(
        page.console_messages()
            .iter()
            .all(|message| message.kind != "error"),
        "{:?}",
        page.console_messages()
    );
    dev.kill().await.unwrap();
    dev.wait().await.unwrap();
    command(&binary, &project.root, &["build", "--scope-hoist"], false).await;
    let mut directories = vec![project.root.join("dist")];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                directories.push(path);
            } else if path.extension().is_some_and(|extension| extension == "js") {
                let output = std::fs::read_to_string(&path).unwrap();
                assert!(
                    !output.contains("__ferrite_refresh_"),
                    "Refresh instrumentation in production: {}",
                    path.display()
                );
                assert!(
                    !output.contains("createSignatureFunctionForTransform"),
                    "Refresh runtime in production: {}",
                    path.display()
                );
            }
        }
    }
    let (mut preview, url) = server(&binary, &project.root, "preview", false).await;
    let page = browser.new_page().await.unwrap();
    page.goto(&url).await.unwrap();
    page.wait_for_function(
        "document.querySelector('#counter')?.textContent === 'reset: 0'",
        Duration::from_secs(15),
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "{error}; errors={:?}; console={:?}",
            page.page_errors(),
            page.console_messages()
        )
    });
    page.locator("#counter").click().await.unwrap();
    page.wait_for_function(
        "document.querySelector('#counter')?.textContent === 'reset: 1'",
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
    assert!(
        page.console_messages()
            .iter()
            .all(|message| message.kind != "error"),
        "{:?}",
        page.console_messages()
    );
    preview.kill().await.unwrap();
    preview.wait().await.unwrap();
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires freshly built CLI, registry packages and Chromium"]
async fn chromium_react_refresh_dom() {
    react_refresh_dom_acceptance(BrowserKind::Chromium, false, false).await;
}
#[tokio::test]
#[ignore = "requires freshly built CLI, registry packages and Firefox"]
async fn firefox_react_refresh_dom() {
    react_refresh_dom_acceptance(BrowserKind::Firefox, false, false).await;
}

#[tokio::test]
#[ignore = "requires freshly built CLI, registry packages and Chromium"]
async fn chromium_react_refresh_wrapped() {
    react_refresh_dom_acceptance(BrowserKind::Chromium, true, false).await;
}
#[tokio::test]
#[ignore = "requires freshly built CLI, registry packages and Firefox"]
async fn firefox_react_refresh_wrapped() {
    react_refresh_dom_acceptance(BrowserKind::Firefox, true, false).await;
}

#[tokio::test]
#[ignore = "requires freshly built CLI, registry packages and Chromium"]
async fn chromium_react_refresh_imported_hook() {
    react_refresh_dom_acceptance(BrowserKind::Chromium, true, true).await;
}
#[tokio::test]
#[ignore = "requires freshly built CLI, registry packages and Firefox"]
async fn firefox_react_refresh_imported_hook() {
    react_refresh_dom_acceptance(BrowserKind::Firefox, true, true).await;
}
