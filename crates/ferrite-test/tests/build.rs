//! Vite compat: SWC engine and scope-hoisting builds.

use ferrite_test::TempProject;

#[tokio::test]
async fn missing_relative_css_inputs_fail_build_and_recover_when_created() {
    for (css, missing, contents, diagnostic) in [
        (
            "body { background: url('./missing.svg'); }",
            "missing.svg",
            "<svg xmlns='http://www.w3.org/2000/svg'></svg>",
            "CSS asset",
        ),
        (
            "@import './missing.css'; body { color: blue; }",
            "missing.css",
            "h1 { color: red; }",
            "CSS @import",
        ),
    ] {
        let project = TempProject::new(&[
            (
                "index.html",
                "<script type='module' src='/main.js'></script>",
            ),
            ("main.js", "import './style.css'; console.log('ready');"),
            ("style.css", css),
        ]);
        let builder = ferrite::create_builder(ferrite::Config {
            root: Some(project.root.clone()),
            ..Default::default()
        })
        .await
        .unwrap();
        let error = builder.build("client").await.unwrap_err().to_string();
        assert!(error.contains(diagnostic), "{error}");
        assert!(error.contains(missing), "{error}");
        assert!(error.contains("style.css"), "{error}");
        assert!(!project.root.join("dist").exists());
        std::fs::write(project.root.join(missing), contents).unwrap();
        let report = builder.build("client").await.unwrap();
        let outputs: Vec<_> = std::fs::read_dir(report.out_dir.join("assets"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        let emitted = outputs
            .iter()
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("missing")
            })
            .expect("the recovered CSS input must be emitted");
        let output = std::fs::read_to_string(emitted).unwrap();
        if missing.ends_with(".svg") {
            assert_eq!(output, contents);
        } else {
            // CSS formatting/minification can change whitespace.
            assert!(output.contains("h1") && output.contains("red"), "{output}");
        }
        let stylesheet = outputs
            .iter()
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("style-")
            })
            .expect("the importing stylesheet must be emitted");
        let stylesheet = std::fs::read_to_string(stylesheet).unwrap();
        assert!(
            stylesheet.contains(&emitted.file_name().unwrap().to_string_lossy().to_string()),
            "{stylesheet}"
        );
        assert!(
            !stylesheet.contains(&format!("./{missing}")),
            "{stylesheet}"
        );
    }
}

#[tokio::test]
async fn named_build_modes_load_their_env_and_remove_development_hooks() {
    let project = TempProject::new(&[
        ("ferrite.toml", "mode = 'staging'\n[build]\nminify = false\n"),
        (".env.staging", "FERRITE_PROFILE=named-staging\n"),
        ("index.html", "<script type='module' src='/main.js'></script>"),
        ("main.js", "import './style.css'; globalThis.profile = {mode: import.meta.env.MODE, prod: import.meta.env.PROD, dev: import.meta.env.DEV, value: import.meta.env.FERRITE_PROFILE, nodeEnv: process.env.NODE_ENV}; if (import.meta.hot) import.meta.hot.accept();"),
        ("style.css", "body { color: blue; }"),
    ]);
    let builder = ferrite::create_builder(ferrite::Config {
        root: Some(project.root.clone()),
        ..Default::default()
    })
    .await
    .unwrap();
    assert_eq!(builder.config.mode, "staging");
    assert!(builder.config.is_production);
    let report = builder.build("client").await.unwrap();
    let mut code = String::new();
    for entry in std::fs::read_dir(report.out_dir.join("assets")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "js") {
            code.push_str(&std::fs::read_to_string(path).unwrap());
        }
    }
    assert!(code.contains("named-staging"), "{code}");
    assert!(code.contains("staging"), "{code}");
    assert!(code.contains("production"), "{code}");
    assert!(!code.contains("/@ferrite/client"), "{code}");
    assert!(!code.contains("import.meta.hot"), "{code}");
    assert!(!code.contains("import.meta.env"), "{code}");
}

/// Production build through the SWC engine: TS strip, JSX, minify,
/// and shake re-minification all run on SWC.
#[cfg(feature = "swc")]
#[tokio::test]
async fn production_build_with_swc_engine() {
    let project = TempProject::new(&[
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <script type=\"module\" src=\"/src/main.tsx\"></script></body></html>",
        ),
        (
            "src/main.tsx",
            "import { keepme } from \"./lib\";\n\
             export const el = <div>{keepme}</div>;\n\
             console.log(el);\n",
        ),
        (
            "src/lib.ts",
            "export const keepme: number = 1;\nexport const dropme: number = 2;\n",
        ),
    ]);
    let mut user = ferrite::UserConfig::default();
    user.compiler.engine = "swc".to_string();
    let config = ferrite::resolve_config(
        user,
        Some(project.root.clone()),
        ferrite::CliOverrides {
            mode: Some("production".to_string()),
            ..Default::default()
        },
    )
    .expect("resolve");
    assert_eq!(config.compiler.engine, "swc");
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    let mut blob = String::new();
    for entry in std::fs::read_dir(report.out_dir.join("assets")).expect("assets") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("js") {
            continue;
        }
        blob.push_str(&std::fs::read_to_string(&path).expect("chunk"));
    }
    // JSX compiled, TS stripped, unused export shaken — all via SWC.
    assert!(
        blob.contains("jsx-runtime") || blob.contains("jsxDEV"),
        "{blob}"
    );
    assert!(!blob.contains(": number"), "{blob}");
    assert!(!blob.contains("dropme"), "{blob}");
}

#[tokio::test]
async fn production_build_scope_hoists_entry_closure() {
    let project = TempProject::new(&[
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <script type=\"module\" src=\"/src/main.ts\"></script></body></html>",
        ),
        (
            "src/main.ts",
            "import { greet } from \"./greet\";\nconsole.log(greet(\"hoist\"));\n",
        ),
        (
            "src/greet.ts",
            "import { punct } from \"./punct\";\nexport function greet(n: string): string { return `hi ${n}${punct}`; }\n",
        ),
        ("src/punct.ts", "export const punct = \"!\";\n"),
    ]);
    let mut user = ferrite::UserConfig::default();
    user.build.scope_hoist = true;
    let config = ferrite::resolve_config(
        user,
        Some(project.root.clone()),
        ferrite::CliOverrides {
            mode: Some("production".to_string()),
            ..Default::default()
        },
    )
    .expect("resolve");
    assert!(config.build.scope_hoist);
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    let js: Vec<std::path::PathBuf> = std::fs::read_dir(report.out_dir.join("assets"))
        .expect("assets")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("js"))
        .collect();
    assert_eq!(js.len(), 1, "{js:?}");
    let code = std::fs::read_to_string(&js[0]).expect("chunk");
    // One scope: no surviving inside imports, prefixed bindings, kept log.
    assert!(!code.contains("from\"./"), "{code}");
    assert!(!code.contains("from \"./"), "{code}");
    assert!(code.contains("$f"), "{code}");
    assert!(code.contains("console.log"), "{code}");
}
