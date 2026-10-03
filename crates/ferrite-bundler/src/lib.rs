//! Production bundler (spec §37–§39).
//!
//! [`Bundler`] owns entry discovery, graph traversal, chunking, hashing,
//! manifest generation, and source maps. Transformation stays in
//! `ferrite-transform`; the bundler consumes modules through [`ModuleLoader`]
//! so dev and build share one pipeline.
//!
//! v0.1 emits one content-hashed ESM file per module with relative import
//! rewriting (correct under native ESM, HTTP/2-friendly). Scope-hoisted
//! concatenation is the documented Rolldown-backend roadmap (§91).

mod bundle;
mod bundler;
mod chunks;
mod emit;
mod hooks;
mod loader;
mod shake;

pub use bundle::{BuildBundleConfig, BundleOutput, BundleRequest, BundleStats, Bundler, Chunk};
pub use bundler::FerriteBundler;
pub use chunks::{chunk_name, relative_url};
pub use emit::rewrite_imports_text;
pub use hooks::{BundleHooks, NoHooks};
pub use loader::{CssExtract, LoadedModule, ModuleLoader};
pub use shake::{shake_statements, tree_shake, used_exports, UsedExports};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit::strip_bare_imports;
    use ferrite_core::{FerriteError, ModuleId, ModuleType, Result};
    use ferrite_graph::{ImportKind, ModuleGraph};
    use ferrite_transform::{ImportBinding, ShakeInfo};
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    struct MapLoader {
        modules: HashMap<ModuleId, LoadedModule>,
    }

    #[async_trait::async_trait]
    impl ModuleLoader for MapLoader {
        async fn load(&self, id: &ModuleId, _env: &str) -> Result<LoadedModule> {
            self.modules
                .get(id)
                .cloned()
                .ok_or_else(|| FerriteError::Build(format!("test loader missing `{id}`")))
        }
    }

    fn js_module(
        id: &str,
        code: &str,
        imports: Vec<(String, ModuleId, ImportKind)>,
    ) -> LoadedModule {
        LoadedModule {
            id: ModuleId::new(id),
            code: code.to_string(),
            imports,
            side_effects: None,
            module_type: ModuleType::Js,
            map: None,
            css: None,
            shake: None,
        }
    }

    fn css_module(id: &str, text: &str, is_modules: bool, deps: Vec<ModuleId>) -> LoadedModule {
        LoadedModule {
            id: ModuleId::new(id),
            code: "export default undefined;\n".to_string(),
            imports: deps
                .into_iter()
                .map(|dep| {
                    (
                        format!("./{}", dep.0.rsplit('/').next().unwrap_or(&dep.0)),
                        dep,
                        ImportKind::Static,
                    )
                })
                .collect(),
            side_effects: None,
            module_type: ModuleType::Js,
            map: None,
            css: Some(CssExtract {
                text: text.to_string(),
                is_modules,
                keep_js: false,
            }),
            shake: None,
        }
    }

    fn test_config() -> BuildBundleConfig {
        BuildBundleConfig {
            out_dir: std::env::temp_dir(),
            asset_pattern: "assets/[name]-[hash][ext]".to_string(),
            chunk_pattern: "assets/[name]-[hash].js".to_string(),
            css_pattern: "assets/[name]-[hash].css".to_string(),
        }
    }

    #[test]
    fn strips_only_bare_css_imports() {
        let code = "import \"/a.css\";\nimport x from \"/b.css\";\nimport(\"/c.css\");\nconst s = \"import \\\"/a.css\\\"\";\n";
        let out = strip_bare_imports(code, &["/a.css", "/b.css", "/c.css"]);
        assert!(!out.contains("import \"/a.css\""), "{out}");
        assert!(out.contains("import x from \"/b.css\""), "{out}");
        assert!(out.contains("import(\"/c.css\")"), "{out}");
        assert!(out.contains("const s = "), "{out}");
        assert!(out.ends_with('\n'));
        // Minified single line, no spaces.
        let min = "import{greet}from\"./g.js\";import\"/a.css\";console.log(1);";
        let out = strip_bare_imports(min, &["/a.css"]);
        assert_eq!(out, "import{greet}from\"./g.js\";console.log(1);");
        // Semicolon-less (ASI) and star imports survive.
        let edge = "import \"/a.css\"\nimport * as ns from \"/a.css\";\n";
        let out = strip_bare_imports(edge, &["/a.css"]);
        assert!(out.contains("import * as ns"), "{out}");
        assert!(!out.contains("import \"/a.css\"\n"), "{out}");
    }

    #[tokio::test]
    async fn extracts_css_per_module_with_import_rewrite() {
        let modules = HashMap::from([
            (
                ModuleId::new("/main.js"),
                js_module(
                    "/main.js",
                    "import \"/a.css\";\nimport styles from \"/m.module.css\";\nconsole.log(styles);\n",
                    vec![
                        (
                            "/a.css".to_string(),
                            ModuleId::new("/a.css"),
                            ImportKind::Static,
                        ),
                        (
                            "/m.module.css".to_string(),
                            ModuleId::new("/m.module.css"),
                            ImportKind::Static,
                        ),
                    ],
                ),
            ),
            (
                ModuleId::new("/a.css"),
                css_module(
                    "/a.css",
                    "@import \"./b.css\";\n@import \"https://fonts.example/f.css\";\n.a{color:red}\n",
                    false,
                    vec![ModuleId::new("/b.css")],
                ),
            ),
            (
                ModuleId::new("/b.css"),
                css_module("/b.css", ".b{color:blue}\n", false, Vec::new()),
            ),
            (
                ModuleId::new("/m.module.css"),
                css_module("/m.module.css", ".btn{color:green}\n", true, Vec::new()),
            ),
        ]);
        let bundler = FerriteBundler::new(Arc::new(MapLoader { modules }));
        let output = bundler
            .bundle(
                &ModuleGraph::new(),
                &test_config(),
                BundleRequest {
                    entries: vec![ModuleId::new("/main.js")],
                    env: "client".to_string(),
                    minify: false,
                    sourcemap: false,
                    map_comment: false,
                    treeshake: false,
                    engine: "oxc".to_string(),
                    scope_hoist: false,
                },
                &NoHooks,
            )
            .await
            .unwrap();
        let names: Vec<&String> = output.bundle.files.keys().collect();
        // One css file per css module; no JS chunk for plain css.
        let css_files: Vec<&&String> = names.iter().filter(|name| name.ends_with(".css")).collect();
        assert_eq!(css_files.len(), 3, "{names:?}");
        assert!(
            !names
                .iter()
                .any(|name| name.contains("a-") && name.ends_with(".js")),
            "{names:?}"
        );
        // Modules stub chunk survives (importers read the map).
        assert!(
            names
                .iter()
                .any(|name| name.contains("m-") && name.ends_with(".js")),
            "{names:?}"
        );
        // @import rewritten to the relative css file; remote untouched.
        let a_file = css_files
            .iter()
            .find(|name| name.contains("/a-"))
            .expect("a css")
            .to_string();
        let a_text = String::from_utf8(output.bundle.files[&a_file].contents.clone()).unwrap();
        assert!(a_text.contains("@import \"./b-"), "{a_text}");
        assert!(
            a_text.contains("@import \"https://fonts.example/f.css\""),
            "{a_text}"
        );
        // Bare import stripped from the JS chunk.
        let main_entry = output.manifest.entries.get("/main.js").expect("entry");
        let main_code =
            String::from_utf8(output.bundle.files[&main_entry.file].contents.clone()).unwrap();
        assert!(!main_code.contains("/a.css"), "{main_code}");
        assert!(!main_code.contains("/m.module.css"), "{main_code}");
        assert!(main_code.contains("./m-"), "{main_code}");
        // Manifest links the entry stylesheets.
        assert_eq!(main_entry.css.len(), 3, "{main_entry:?}");
        assert!(main_entry
            .css
            .iter()
            .all(|file| output.bundle.files.contains_key(file)));
    }

    #[tokio::test]
    async fn circular_css_import_fails_loudly() {
        let modules = HashMap::from([
            (
                ModuleId::new("/main.js"),
                js_module(
                    "/main.js",
                    "import \"/a.css\";\n",
                    vec![(
                        "/a.css".to_string(),
                        ModuleId::new("/a.css"),
                        ImportKind::Static,
                    )],
                ),
            ),
            (
                ModuleId::new("/a.css"),
                css_module(
                    "/a.css",
                    "@import \"./b.css\";\n",
                    false,
                    vec![ModuleId::new("/b.css")],
                ),
            ),
            (
                ModuleId::new("/b.css"),
                css_module(
                    "/b.css",
                    "@import \"./a.css\";\n",
                    false,
                    vec![ModuleId::new("/a.css")],
                ),
            ),
        ]);
        let bundler = FerriteBundler::new(Arc::new(MapLoader { modules }));
        let error = bundler
            .bundle(
                &ModuleGraph::new(),
                &test_config(),
                BundleRequest {
                    entries: vec![ModuleId::new("/main.js")],
                    env: "client".to_string(),
                    minify: false,
                    sourcemap: false,
                    map_comment: false,
                    treeshake: false,
                    engine: "oxc".to_string(),
                    scope_hoist: false,
                },
                &NoHooks,
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("circular CSS"), "{error}");
    }

    #[test]
    fn chunk_naming() {
        assert_eq!(chunk_name(&ModuleId::new("/src/main.ts")), "main");
        assert_eq!(
            chunk_name(&ModuleId::new("/@npm/react@19.0.0/index.js")),
            "index"
        );
    }

    #[test]
    fn relative_urls() {
        assert_eq!(relative_url("assets/a-1.js", "assets/b-2.js"), "./b-2.js");
        assert_eq!(
            relative_url("assets/nested/a-1.js", "assets/b-2.js"),
            "../b-2.js"
        );
    }

    #[test]
    fn rewrites_exact_specifiers() {
        let code = "import x from \"./a\";\nconst s = \"./a\";\n";
        let mapping = HashMap::from([("./a".to_string(), "./a-1.js".to_string())]);
        let output = rewrite_imports_text(code, &mapping);
        assert!(output.contains("from \"./a-1.js\""), "{output}");
    }

    #[test]
    fn rewrite_leaves_strings_and_comments_alone() {
        let code = "import x from \"./a\";\nconst s = \"./a\";\n// see \"./a\" docs\n";
        let mapping = HashMap::from([("./a".to_string(), "./a-1.js".to_string())]);
        let output = rewrite_imports_text(code, &mapping);
        assert!(output.contains("from \"./a-1.js\""), "{output}");
        assert!(output.contains("const s = \"./a\";"), "{output}");
        assert!(output.contains("// see \"./a\" docs"), "{output}");
    }

    #[tokio::test]
    async fn chunk_names_follow_transitive_content() {
        async fn bundle_with(dep_code: &str) -> BundleOutput {
            let modules = HashMap::from([
                (
                    ModuleId::new("/main.js"),
                    js_module(
                        "/main.js",
                        "import { x } from \"/dep.js\";\nconsole.log(x);\n",
                        vec![(
                            "/dep.js".to_string(),
                            ModuleId::new("/dep.js"),
                            ImportKind::Static,
                        )],
                    ),
                ),
                (
                    ModuleId::new("/dep.js"),
                    js_module("/dep.js", dep_code, vec![]),
                ),
            ]);
            let bundler = FerriteBundler::new(Arc::new(MapLoader { modules }));
            bundler
                .bundle(
                    &ModuleGraph::new(),
                    &test_config(),
                    BundleRequest {
                        entries: vec![ModuleId::new("/main.js")],
                        env: "client".to_string(),
                        minify: false,
                        sourcemap: false,
                        map_comment: false,
                        treeshake: false,
                        engine: "oxc".to_string(),
                        scope_hoist: false,
                    },
                    &NoHooks,
                )
                .await
                .unwrap()
        }

        let before = bundle_with("export const x = 1;\n").await;
        let after = bundle_with("export const x = 2;\n").await;
        let main_before = &before.manifest.entries["/main.js"].file;
        let main_after = &after.manifest.entries["/main.js"].file;
        let dep_before = &before.manifest.entries["/dep.js"].file;
        let dep_after = &after.manifest.entries["/dep.js"].file;
        // Both the changed chunk and its importer are renamed.
        assert_ne!(dep_before, dep_after);
        assert_ne!(main_before, main_after);
        // The importer's emitted code points at the new dependency file.
        let main_code = String::from_utf8(after.bundle.files[main_after].contents.clone()).unwrap();
        let dep_file = dep_after.rsplit('/').next().unwrap();
        assert!(main_code.contains(dep_file), "{main_code}");
    }

    fn named(name: &str) -> Vec<ImportBinding> {
        vec![ImportBinding::Named(name.to_string())]
    }

    fn shaken_module(
        id: &str,
        code: &str,
        imports: Vec<(String, ModuleId, ImportKind)>,
        bindings: Vec<Vec<ImportBinding>>,
        exports: Vec<ferrite_transform::ParsedExport>,
    ) -> LoadedModule {
        LoadedModule {
            id: ModuleId::new(id),
            code: code.to_string(),
            imports,
            side_effects: None,
            module_type: ModuleType::Js,
            map: None,
            css: None,
            shake: Some(ShakeInfo {
                import_bindings: bindings,
                exports,
            }),
        }
    }

    fn local_export(name: &str) -> ferrite_transform::ParsedExport {
        ferrite_transform::ParsedExport {
            exported: name.to_string(),
            local: Some(name.to_string()),
            from: None,
            imported: None,
            target: None,
        }
    }

    fn reexport(exported: &str, imported: &str, target: &str) -> ferrite_transform::ParsedExport {
        ferrite_transform::ParsedExport {
            exported: exported.to_string(),
            local: None,
            from: Some(target.to_string()),
            imported: Some(imported.to_string()),
            target: Some(ModuleId::new(target)),
        }
    }

    fn shake_request(entries: Vec<ModuleId>) -> BundleRequest {
        BundleRequest {
            entries,
            env: "client".to_string(),
            minify: false,
            sourcemap: false,
            map_comment: false,
            treeshake: true,
            engine: "oxc".to_string(),
            scope_hoist: false,
        }
    }

    #[test]
    fn usage_propagates_through_barrel() {
        let entry = ModuleId::new("/e.js");
        let barrel = ModuleId::new("/barrel.js");
        let leaf = ModuleId::new("/leaf.js");
        let modules = HashMap::from([
            (
                entry.clone(),
                shaken_module(
                    "/e.js",
                    "import { x } from \"./barrel.js\";\nconsole.log(x);\n",
                    vec![(
                        "./barrel.js".to_string(),
                        barrel.clone(),
                        ImportKind::Static,
                    )],
                    vec![named("x")],
                    vec![],
                ),
            ),
            (
                barrel.clone(),
                shaken_module(
                    "/barrel.js",
                    "export { x } from \"./leaf.js\";\n",
                    vec![("./leaf.js".to_string(), leaf.clone(), ImportKind::Static)],
                    vec![vec![ImportBinding::Reexport]],
                    vec![reexport("x", "x", "/leaf.js")],
                ),
            ),
            (
                leaf.clone(),
                shaken_module(
                    "/leaf.js",
                    "export const x = 1;\nexport const y = 2;\n",
                    vec![],
                    vec![],
                    vec![local_export("x"), local_export("y")],
                ),
            ),
        ]);
        let live: HashSet<ModuleId> = modules.keys().cloned().collect();
        let usage = used_exports(std::slice::from_ref(&entry), &modules, &live);
        assert!(usage.full.contains(&entry));
        assert_eq!(
            usage.used.get(&barrel).cloned().unwrap_or_default(),
            HashSet::from(["x".to_string()])
        );
        assert_eq!(
            usage.used.get(&leaf).cloned().unwrap_or_default(),
            HashSet::from(["x".to_string()])
        );
    }

    #[test]
    fn usage_conservative_cases() {
        let entry = ModuleId::new("/e.js");
        let ns = ModuleId::new("/ns.js");
        let dyn_target = ModuleId::new("/dyn.js");
        let opaque = ModuleId::new("/opaque.js");
        let opaque_dep = ModuleId::new("/opaque-dep.js");
        let modules = HashMap::from([
            (
                entry.clone(),
                shaken_module(
                    "/e.js",
                    "import * as ns from \"./ns.js\";\nimport(\"./dyn.js\");\n",
                    vec![
                        ("./ns.js".to_string(), ns.clone(), ImportKind::Static),
                        (
                            "./dyn.js".to_string(),
                            dyn_target.clone(),
                            ImportKind::Dynamic,
                        ),
                    ],
                    vec![vec![ImportBinding::Namespace], vec![]],
                    vec![],
                ),
            ),
            (
                ns.clone(),
                shaken_module(
                    "/ns.js",
                    "export const a = 1;\n",
                    vec![],
                    vec![],
                    vec![local_export("a")],
                ),
            ),
            (
                dyn_target.clone(),
                shaken_module(
                    "/dyn.js",
                    "export const b = 1;\n",
                    vec![],
                    vec![],
                    vec![local_export("b")],
                ),
            ),
            // Opaque: keeps itself and its dependency whole.
            (
                opaque.clone(),
                LoadedModule {
                    id: opaque.clone(),
                    code: "import \"./opaque-dep.js\";\n".to_string(),
                    imports: vec![(
                        "./opaque-dep.js".to_string(),
                        opaque_dep.clone(),
                        ImportKind::Static,
                    )],
                    side_effects: None,
                    module_type: ModuleType::Js,
                    map: None,
                    css: None,
                    shake: None,
                },
            ),
            (
                opaque_dep.clone(),
                shaken_module(
                    "/opaque-dep.js",
                    "export const c = 1;\n",
                    vec![],
                    vec![],
                    vec![local_export("c")],
                ),
            ),
        ]);
        // Entry must reach the opaque module for it to be live.
        let live: HashSet<ModuleId> = modules.keys().cloned().collect();
        let usage = used_exports(std::slice::from_ref(&entry), &modules, &live);
        assert!(usage.full.contains(&ns));
        assert!(usage.full.contains(&dyn_target));
        assert!(usage.full.contains(&opaque));
        assert!(usage.full.contains(&opaque_dep));
    }

    #[test]
    fn usage_star_leaks_only_unknown_names() {
        let entry = ModuleId::new("/e.js");
        let barrel = ModuleId::new("/barrel.js");
        let leaf = ModuleId::new("/leaf.js");
        let modules = HashMap::from([
            (
                entry.clone(),
                shaken_module(
                    "/e.js",
                    "import { x, own } from \"./barrel.js\";\nconsole.log(x, own);\n",
                    vec![(
                        "./barrel.js".to_string(),
                        barrel.clone(),
                        ImportKind::Static,
                    )],
                    vec![vec![
                        ImportBinding::Named("x".to_string()),
                        ImportBinding::Named("own".to_string()),
                    ]],
                    vec![],
                ),
            ),
            (
                barrel.clone(),
                shaken_module(
                    "/barrel.js",
                    "export const own = 0;\nexport * from \"./leaf.js\";\n",
                    vec![("./leaf.js".to_string(), leaf.clone(), ImportKind::Static)],
                    vec![vec![ImportBinding::Reexport]],
                    vec![
                        local_export("own"),
                        ferrite_transform::ParsedExport {
                            exported: "*".to_string(),
                            local: None,
                            from: Some("/leaf.js".to_string()),
                            imported: None,
                            target: Some(leaf.clone()),
                        },
                    ],
                ),
            ),
            (
                leaf.clone(),
                shaken_module(
                    "/leaf.js",
                    "export const x = 1;\nexport const y = 2;\n",
                    vec![],
                    vec![],
                    vec![local_export("x"), local_export("y")],
                ),
            ),
        ]);
        let live: HashSet<ModuleId> = modules.keys().cloned().collect();
        let usage = used_exports(std::slice::from_ref(&entry), &modules, &live);
        // `own` is local to the barrel; only `x` leaks to the leaf.
        assert_eq!(
            usage.used.get(&leaf).cloned().unwrap_or_default(),
            HashSet::from(["x".to_string()])
        );
    }

    #[test]
    fn named_barrel_shaking_keeps_emitted_links_and_facts_consistent() {
        let entry = ModuleId::new("/entry.js");
        let barrel = ModuleId::new("/barrel.js");
        let leaf = ModuleId::new("/leaf.js");
        let mut modules = HashMap::from([
            (
                entry.clone(),
                shaken_module(
                    "/entry.js",
                    "import {ref} from '/barrel.js'; console.log(ref);",
                    vec![("/barrel.js".into(), barrel.clone(), ImportKind::Static)],
                    vec![named("ref")],
                    vec![],
                ),
            ),
            (
                barrel.clone(),
                shaken_module(
                    "/barrel.js",
                    "export {ref, TrackOpTypes} from '/leaf.js';",
                    vec![("/leaf.js".into(), leaf.clone(), ImportKind::Static)],
                    vec![vec![ImportBinding::Reexport]],
                    vec![
                        reexport("ref", "ref", "/leaf.js"),
                        reexport("TrackOpTypes", "TrackOpTypes", "/leaf.js"),
                    ],
                ),
            ),
            (
                leaf.clone(),
                shaken_module(
                    "/leaf.js",
                    "export const ref = 1; export const TrackOpTypes = {GET:'get'};",
                    vec![],
                    vec![],
                    vec![local_export("ref"), local_export("TrackOpTypes")],
                ),
            ),
        ]);
        let live = modules.keys().cloned().collect();
        shake_statements(&mut modules, &live, &shake_request(vec![entry])).unwrap();
        for id in [&barrel, &leaf] {
            let module = &modules[id];
            assert!(!module.code.contains("TrackOpTypes"), "{}", module.code);
            let facts = module.shake.as_ref().unwrap();
            assert_eq!(facts.exports.len(), 1);
            assert_eq!(facts.exports[0].exported, "ref");
        }
        assert_eq!(
            modules[&barrel].shake.as_ref().unwrap().exports[0].target,
            Some(leaf)
        );
    }

    #[test]
    fn statements_drop_unused_and_reminify() {
        let entry = ModuleId::new("/e.js");
        let leaf = ModuleId::new("/leaf.js");
        let mut modules = HashMap::from([
            (
                entry.clone(),
                shaken_module(
                    "/e.js",
                    "import { x } from \"./leaf.js\";\nconsole.log(x);\n",
                    vec![("./leaf.js".to_string(), leaf.clone(), ImportKind::Static)],
                    vec![named("x")],
                    vec![],
                ),
            ),
            (
                leaf.clone(),
                shaken_module(
                    "/leaf.js",
                    "export const xray = 1;\nexport const yankee = 2;\n",
                    vec![],
                    vec![],
                    vec![local_export("xray"), local_export("yankee")],
                ),
            ),
        ]);
        // Entry imports `xray` (rename the binding to match the leaf).
        modules
            .get_mut(&entry)
            .unwrap()
            .shake
            .as_mut()
            .unwrap()
            .import_bindings = vec![named("xray")];
        let live: HashSet<ModuleId> = modules.keys().cloned().collect();
        let request = shake_request(vec![entry.clone()]);
        shake_statements(&mut modules, &live, &request).unwrap();
        let code = &modules.get(&leaf).unwrap().code;
        assert!(code.contains("xray"), "{code}");
        assert!(!code.contains("yankee"), "{code}");
        // Entry keeps everything.
        assert!(modules.get(&entry).unwrap().code.contains("console"));
    }

    fn hoist_request(entries: Vec<ModuleId>) -> BundleRequest {
        BundleRequest {
            entries,
            env: "client".to_string(),
            minify: false,
            sourcemap: false,
            map_comment: false,
            treeshake: false,
            engine: "oxc".to_string(),
            scope_hoist: true,
        }
    }

    fn js_files(output: &BundleOutput) -> Vec<String> {
        let mut files: Vec<String> = output
            .bundle
            .files
            .values()
            .filter(|file| file.name.ends_with(".js"))
            .map(|file| String::from_utf8_lossy(&file.contents).into_owned())
            .collect();
        files.sort();
        files
    }

    #[tokio::test]
    async fn scope_hoist_merges_entry_closure() {
        let modules = HashMap::from([
            (
                ModuleId::new("/e.js"),
                shaken_module(
                    "/e.js",
                    "import { value } from \"/lib.js\";\nconsole.log(value);\n",
                    vec![(
                        "/lib.js".to_string(),
                        ModuleId::new("/lib.js"),
                        ImportKind::Static,
                    )],
                    vec![named("value")],
                    vec![],
                ),
            ),
            (
                ModuleId::new("/lib.js"),
                shaken_module(
                    "/lib.js",
                    "export const value = 41 + 1;\n",
                    vec![],
                    vec![],
                    vec![local_export("value")],
                ),
            ),
        ]);
        let bundler = FerriteBundler::new(Arc::new(MapLoader { modules }));
        let output = bundler
            .bundle(
                &ModuleGraph::new(),
                &test_config(),
                hoist_request(vec![ModuleId::new("/e.js")]),
                &NoHooks,
            )
            .await
            .unwrap();
        let files = js_files(&output);
        assert_eq!(files.len(), 1, "{files:?}");
        assert!(files[0].contains("$f0$value"), "{}", files[0]);
        assert!(!files[0].contains("from \"/lib.js\""), "{}", files[0]);
    }

    #[tokio::test]
    async fn scope_hoist_splits_shared_and_dynamic() {
        let modules = HashMap::from([
            (
                ModuleId::new("/a.js"),
                shaken_module(
                    "/a.js",
                    "import { s } from \"/shared.js\";\nimport(\"/lazy.js\");\nconsole.log(s);\n",
                    vec![
                        (
                            "/shared.js".to_string(),
                            ModuleId::new("/shared.js"),
                            ImportKind::Static,
                        ),
                        (
                            "/lazy.js".to_string(),
                            ModuleId::new("/lazy.js"),
                            ImportKind::Dynamic,
                        ),
                    ],
                    vec![named("s"), vec![]],
                    vec![],
                ),
            ),
            (
                ModuleId::new("/b.js"),
                shaken_module(
                    "/b.js",
                    "import { s } from \"/shared.js\";\nconsole.log(s);\n",
                    vec![(
                        "/shared.js".to_string(),
                        ModuleId::new("/shared.js"),
                        ImportKind::Static,
                    )],
                    vec![named("s")],
                    vec![],
                ),
            ),
            (
                ModuleId::new("/shared.js"),
                shaken_module(
                    "/shared.js",
                    "export const s = 1;\n",
                    vec![],
                    vec![],
                    vec![local_export("s")],
                ),
            ),
            (
                ModuleId::new("/lazy.js"),
                shaken_module(
                    "/lazy.js",
                    "export const l = 2;\n",
                    vec![],
                    vec![],
                    vec![local_export("l")],
                ),
            ),
        ]);
        let bundler = FerriteBundler::new(Arc::new(MapLoader { modules }));
        let output = bundler
            .bundle(
                &ModuleGraph::new(),
                &test_config(),
                hoist_request(vec![ModuleId::new("/a.js"), ModuleId::new("/b.js")]),
                &NoHooks,
            )
            .await
            .unwrap();
        // /a.js, /b.js hoist alone (shared/dynamic split out) + 2 singles.
        assert_eq!(output.chunks.len(), 4, "{:?}", output.chunks.keys());
        let entry_a = &output.chunks["/a.js"];
        assert_eq!(entry_a.modules, vec![ModuleId::new("/a.js")]);
        // Cross-chunk import rewritten to the shared chunk file.
        let files = js_files(&output);
        let a_file = files
            .iter()
            .find(|file| file.contains("console.log"))
            .unwrap();
        assert!(a_file.contains("./shared-"), "{a_file}");
    }

    #[tokio::test]
    async fn scope_hoist_bails_to_singles() {
        let modules = HashMap::from([
            (
                ModuleId::new("/e.js"),
                shaken_module(
                    "/e.js",
                    "import * as ns from \"/lib.js\";\nconsole.log(ns.value);\n",
                    vec![(
                        "/lib.js".to_string(),
                        ModuleId::new("/lib.js"),
                        ImportKind::Static,
                    )],
                    vec![vec![ImportBinding::Namespace]],
                    vec![],
                ),
            ),
            (
                ModuleId::new("/lib.js"),
                shaken_module(
                    "/lib.js",
                    "export const value = 1;\n",
                    vec![],
                    vec![],
                    vec![local_export("value")],
                ),
            ),
        ]);
        let bundler = FerriteBundler::new(Arc::new(MapLoader { modules }));
        let output = bundler
            .bundle(
                &ModuleGraph::new(),
                &test_config(),
                hoist_request(vec![ModuleId::new("/e.js")]),
                &NoHooks,
            )
            .await
            .unwrap();
        assert_eq!(output.chunks.len(), 2);
    }

    #[test]
    fn shake_keeps_reachable() {
        let entries = vec![ModuleId::new("/a.js")];
        let modules = HashMap::from([
            (
                ModuleId::new("/a.js"),
                LoadedModule {
                    id: ModuleId::new("/a.js"),
                    code: String::new(),
                    imports: vec![(
                        "./b".to_string(),
                        ModuleId::new("/b.js"),
                        ImportKind::Static,
                    )],
                    side_effects: None,
                    module_type: ModuleType::Js,
                    map: None,
                    css: None,
                    shake: None,
                },
            ),
            (
                ModuleId::new("/b.js"),
                LoadedModule {
                    id: ModuleId::new("/b.js"),
                    code: String::new(),
                    imports: Vec::new(),
                    side_effects: None,
                    module_type: ModuleType::Js,
                    map: None,
                    css: None,
                    shake: None,
                },
            ),
            (
                ModuleId::new("/z.js"),
                LoadedModule {
                    id: ModuleId::new("/z.js"),
                    code: String::new(),
                    imports: Vec::new(),
                    side_effects: None,
                    module_type: ModuleType::Js,
                    map: None,
                    css: None,
                    shake: None,
                },
            ),
        ]);
        let live = tree_shake(&entries, &modules);
        assert!(live.contains(&ModuleId::new("/a.js")));
        assert!(live.contains(&ModuleId::new("/b.js")));
        assert!(!live.contains(&ModuleId::new("/z.js")));
    }

    struct RecordingHooks {
        started: std::sync::Mutex<Vec<String>>,
        extra: String,
        banner: Option<String>,
    }

    #[async_trait::async_trait]
    impl BundleHooks for RecordingHooks {
        async fn render_start(&self, entries: &[ModuleId]) -> Result<()> {
            *self.started.lock().unwrap() = entries.iter().map(|id| id.0.clone()).collect();
            Ok(())
        }

        async fn render_chunk(
            &self,
            _id: &str,
            code: String,
            _is_entry: bool,
        ) -> Result<Option<String>> {
            Ok(Some(format!("{code}\n// rendered\n")))
        }

        async fn chunk_hash_extra(&self, _chunk_id: &str) -> Result<Vec<String>> {
            Ok(vec![self.extra.clone()])
        }

        async fn chunk_wrapper(
            &self,
            _id: &str,
            _code: &str,
            _is_entry: bool,
        ) -> Result<ferrite_plugin::ChunkWrapper> {
            Ok(ferrite_plugin::ChunkWrapper {
                banner: self.banner.clone(),
                ..Default::default()
            })
        }
    }

    fn hook_modules() -> HashMap<ModuleId, LoadedModule> {
        HashMap::from([(
            ModuleId::new("/main.js"),
            js_module("/main.js", "console.log(1);\n", vec![]),
        )])
    }

    fn hook_request() -> BundleRequest {
        BundleRequest {
            entries: vec![ModuleId::new("/main.js")],
            env: "client".to_string(),
            minify: false,
            sourcemap: false,
            map_comment: false,
            treeshake: false,
            engine: "oxc".to_string(),
            scope_hoist: false,
        }
    }

    #[tokio::test]
    async fn render_hooks_shape_output_and_hash() {
        let hooks = RecordingHooks {
            started: std::sync::Mutex::new(Vec::new()),
            extra: "v1".to_string(),
            banner: Some("/* banner */".to_string()),
        };
        let bundler = FerriteBundler::new(std::sync::Arc::new(MapLoader {
            modules: hook_modules(),
        }));
        let output = bundler
            .bundle(&ModuleGraph::new(), &test_config(), hook_request(), &hooks)
            .await
            .unwrap();
        assert_eq!(*hooks.started.lock().unwrap(), vec!["/main.js"]);
        let entry = output.manifest.entries.get("/main.js").expect("entry");
        let code = String::from_utf8(output.bundle.files[&entry.file].contents.clone()).unwrap();
        assert!(code.starts_with("/* banner */\n"), "{code}");
        assert!(code.contains("// rendered"), "{code}");

        // Different hash extras rename the chunk.
        let other = RecordingHooks {
            started: std::sync::Mutex::new(Vec::new()),
            extra: "v2".to_string(),
            banner: None,
        };
        let bundler = FerriteBundler::new(std::sync::Arc::new(MapLoader {
            modules: hook_modules(),
        }));
        let output2 = bundler
            .bundle(&ModuleGraph::new(), &test_config(), hook_request(), &other)
            .await
            .unwrap();
        let entry2 = output2.manifest.entries.get("/main.js").expect("entry");
        assert_ne!(entry.file, entry2.file);
    }
}
