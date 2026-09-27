//! Vite compat: tree-shaking builds.

use ferrite_test::TempProject;

// --- tree-shaking -------------------------------------------------------------------------

#[tokio::test]
async fn production_build_drops_unused_exports() {
    let project = TempProject::new(&[
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <script type=\"module\" src=\"/src/main.ts\"></script></body></html>",
        ),
        (
            "src/main.ts",
            "import { keepme } from \"./lib\";\nconsole.log(keepme);\n",
        ),
        (
            "src/lib.ts",
            "export const keepme = 1;\nexport const dropme = 2;\n",
        ),
    ]);
    let config = project.resolve_config_mode("production");
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    let mut found_keep = false;
    let mut found_drop = false;
    for entry in std::fs::read_dir(report.out_dir.join("assets")).expect("assets") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("js") {
            continue;
        }
        let code = std::fs::read_to_string(&path).expect("chunk");
        // Skip the entry chunk (entries keep everything); the lib chunk
        // must keep `keepme` and lose `dropme`.
        if code.contains("console") {
            continue;
        }
        found_keep |= code.contains("keepme");
        found_drop |= code.contains("dropme");
    }
    assert!(found_keep, "shaken lib chunk missing `keepme`");
    assert!(!found_drop, "shaken lib chunk still contains `dropme`");
}

#[tokio::test]
async fn production_build_keeps_barrel_reexports_in_use() {
    let project = TempProject::new(&[
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <script type=\"module\" src=\"/src/main.ts\"></script></body></html>",
        ),
        (
            "src/main.ts",
            "import { via } from \"./barrel\";\nconsole.log(via);\n",
        ),
        ("src/barrel.ts", "export { via } from \"./leaf\";\n"),
        (
            "src/leaf.ts",
            "export const via = 1;\nexport const gone = 2;\n",
        ),
    ]);
    let config = project.resolve_config_mode("production");
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
    assert!(blob.contains("via"), "{blob}");
    assert!(!blob.contains("gone"), "{blob}");
}
