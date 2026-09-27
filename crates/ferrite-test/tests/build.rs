//! Vite compat: SWC engine and scope-hoisting builds.

use ferrite_test::TempProject;

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
