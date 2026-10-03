//! JavaScript/TypeScript compiler abstraction (spec §5, §90).
//!
//! [`JsCompiler`] is the stable boundary: the module graph, server, and
//! bundler only see [`ParsedModule`] / [`TransformResult`], never Oxc or SWC
//! AST types (§92).

#[cfg(feature = "swc")]
mod swc_impl;

mod commonjs;
mod compiler;
mod defines;
pub use commonjs::{
    analyze_commonjs, commonjs_facade, commonjs_factory, commonjs_factory_id, commonjs_inline,
    commonjs_json_factory, validate_commonjs, CommonJsAnalysis, CJS_FACTORY_QUERY,
    CJS_REQUIRE_EXPORT,
};
pub mod concat;
mod minify;
mod parse;
mod rewrite;
mod transform;
mod types;

pub use compiler::{
    compiler_for_engine, CompilerEngine, JsCompiler, OxcCompiler, OxcOptions, SwcCompiler,
};
pub use defines::apply_defines_mapped;
pub use minify::chain_source_maps;
pub use rewrite::{
    apply_define, apply_text_edits, inject_hmr_mapped, rewrite_import_meta_hot, rewrite_specifiers,
    rewrite_specifiers_mapped, with_hmr_client,
};
pub use transform::drop_unused_exports;
pub use types::{
    ImportBinding, MinifyRequest, MinifyResult, ParseRequest, ParsedExport, ParsedImport,
    ParsedImportKind, ParsedModule, ShakeInfo, TransformRequest, TransformResult,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_module;
    use ferrite_core::ModuleType;
    use std::collections::HashMap;

    fn compiler() -> OxcCompiler {
        OxcCompiler::new(OxcOptions::default())
    }

    #[test]
    fn named_reexports_prune_unused_aliases_and_preserve_dependency_effects() {
        let source = "export { ref, TrackOpTypes as Tracking } from './reactivity.js';";
        let kept = std::collections::HashSet::from(["ref".to_string()]);
        let code = drop_unused_exports("/barrel.js", source, &kept)
            .unwrap()
            .unwrap();
        assert!(code.contains("ref"));
        assert!(!code.contains("Tracking"));
        assert!(!code.contains("TrackOpTypes"));
        let empty = drop_unused_exports("/barrel.js", source, &std::collections::HashSet::new())
            .unwrap()
            .unwrap();
        let parsed = parse_module("/barrel.js", &empty, &ModuleType::Js).unwrap();
        assert!(parsed.exports.is_empty());
        assert_eq!(parsed.imports.len(), 1);
        assert_eq!(parsed.imports[0].specifier, "./reactivity.js");
    }

    #[test]
    fn parses_imports_with_ranges() {
        let parsed = compiler()
            .parse(ParseRequest {
                id: "/src/main.ts".to_string(),
                code: "import { x } from \"./x\";\nconst m = await import(\"./lazy.ts\");\nconsole.log(x, m);\n".to_string(),
                module_type: ModuleType::Ts,
            })
            .unwrap();
        assert_eq!(parsed.imports.len(), 2);
        assert!(parsed.has_module_syntax);
        for import in &parsed.imports {
            let (start, end) = import.range;
            assert!(start < end);
        }
    }

    #[test]
    fn strips_typescript() {
        let result = compiler()
            .transform(TransformRequest::new(
                "/src/main.ts",
                "const x: number = 1;\nexport default x;\n",
                ModuleType::Ts,
            ))
            .unwrap();
        assert!(result.code.contains("const x = 1"), "{}", result.code);
        assert!(result.exports.contains(&"default".to_string()));
    }

    #[test]
    fn transforms_jsx_automatic() {
        let result = compiler()
            .transform(TransformRequest::new(
                "/src/app.tsx",
                "export const App = () => <div className=\"a\">hi</div>;\n",
                ModuleType::Tsx,
            ))
            .unwrap();
        assert!(result.code.contains("jsx"), "{}", result.code);
        assert!(!result.code.contains("<div"), "{}", result.code);
    }

    #[test]
    fn parses_side_effect_imports() {
        let parsed = compiler()
            .parse(ParseRequest {
                id: "/src/main.ts".to_string(),
                code: "import \"./style.css\";\nimport \"./polyfill\";\nconsole.log(1);\n"
                    .to_string(),
                module_type: ModuleType::Ts,
            })
            .unwrap();
        let specifiers: Vec<&str> = parsed
            .imports
            .iter()
            .map(|i| i.specifier.as_str())
            .collect();
        assert!(specifiers.contains(&"./style.css"), "{specifiers:?}");
        assert!(specifiers.contains(&"./polyfill"), "{specifiers:?}");
    }

    #[test]
    fn rewrites_specifiers_by_ast() {
        let code =
            "import { x } from \"pkg\";\nimport y from \"./local.ts\";\nconsole.log(x, y);\n";
        let mapping = HashMap::from([
            ("pkg".to_string(), "/@npm/pkg@1.0.0/index.js".to_string()),
            ("./local.ts".to_string(), "/src/local.ts".to_string()),
        ]);
        let (rewritten, _) = rewrite_specifiers(code, &ModuleType::Ts, &mapping).unwrap();
        assert!(
            rewritten.contains("/@npm/pkg@1.0.0/index.js"),
            "{rewritten}"
        );
        assert!(rewritten.contains("/src/local.ts"), "{rewritten}");
    }

    #[test]
    fn json_becomes_esm() {
        let result = compiler()
            .transform(TransformRequest::new(
                "/data.json",
                "{\"a\":1}",
                ModuleType::Json,
            ))
            .unwrap();
        assert!(result.code.contains("export default"), "{}", result.code);
    }

    #[test]
    fn define_replacement_is_word_safe() {
        let mut define = HashMap::new();
        define.insert("__VERSION__".to_string(), "\"1.2.3\"".to_string());
        let output = apply_define("const v = __VERSION__; const w = __VERSION__X;", &define);
        assert!(output.contains("\"1.2.3\";"));
        assert!(output.contains("__VERSION__X"));
    }

    #[test]
    fn define_values_are_not_reexpanded() {
        let mut define = HashMap::new();
        define.insert("A".to_string(), "B".to_string());
        define.insert("B".to_string(), "1".to_string());
        // `A` expands to `B`, but the inserted `B` must not expand again.
        assert_eq!(apply_define("const v = A;", &define), "const v = B;");
        assert_eq!(apply_define("const v = B;", &define), "const v = 1;");
    }

    #[test]
    fn define_rewrites_adjacent_occurrences() {
        let mut define = HashMap::new();
        define.insert("A".to_string(), "1".to_string());
        assert_eq!(apply_define("f(A,A);", &define), "f(1,1);");
        assert_eq!(apply_define("A+A", &define), "1+1");
    }

    #[cfg(feature = "swc")]
    #[test]
    fn compiler_versions_differ_by_backend() {
        let oxc = compiler_for_engine("oxc").unwrap();
        let swc = compiler_for_engine("swc").unwrap();
        assert_ne!(oxc.version(), swc.version());
        assert!(oxc.version().starts_with("oxc-"));
        assert!(swc.version().starts_with("swc-"));
    }

    #[test]
    fn jsx_settings_apply_to_automatic_and_classic_compilation() {
        #[cfg(not(feature = "swc"))]
        let engines = ["oxc"];
        #[cfg(feature = "swc")]
        let engines = ["oxc", "swc"];
        for engine in engines {
            let compiler = compiler_for_engine(engine).unwrap();
            for development in [true, false] {
                let mut request = TransformRequest::new(
                    "/settings.tsx",
                    "export const tree = <><button>hello</button></>;",
                    ModuleType::Tsx,
                );
                request.jsx_import_source = Some("./selected-runtime".into());
                request.development = development;
                let code = compiler.transform(request).unwrap().code;
                assert!(
                    code.contains(if development {
                        "./selected-runtime/jsx-dev-runtime"
                    } else {
                        "./selected-runtime/jsx-runtime"
                    }),
                    "{engine}: {code}"
                );
                assert!(!code.contains("react/jsx"), "{engine}: {code}");
            }
            let mut request = TransformRequest::new(
                "/settings.jsx",
                "export const tree = <><button>hello</button></>;",
                ModuleType::Jsx,
            );
            request.jsx_runtime = "classic".into();
            request.jsx_factory = Some("UI.h".into());
            request.jsx_fragment = Some("UI.Fragment".into());
            let code = compiler.transform(request.clone()).unwrap().code;
            assert!(
                code.contains("UI.h") && code.contains("UI.Fragment"),
                "{engine}: {code}"
            );
            assert!(!code.contains("React.createElement"), "{engine}: {code}");
            request.jsx_import_source = Some("forbidden".into());
            assert!(compiler.transform(request).is_err());
            let mut request = TransformRequest::new("/bad.jsx", "<div/>", ModuleType::Jsx);
            request.jsx_runtime = "unknown".into();
            assert!(compiler.transform(request).is_err());
        }
    }

    #[test]
    fn minifies() {
        let result = compiler()
            .minify(MinifyRequest {
                id: "/a.js".to_string(),
                code: "const  longName  =  1 + 2;\nconsole.log(longName);\n".to_string(),
                sourcemap: false,
                input_map: None,
            })
            .unwrap();
        assert!(result.code.len() < 60, "{}", result.code);
    }

    #[test]
    fn minify_chains_through_transform_map() {
        let compiler = compiler();
        let transformed = compiler
            .transform({
                let mut request = TransformRequest::new(
                    "/src/a.ts",
                    "const greeting: string = \"hi\";\nconsole.log(greeting);\n",
                    ModuleType::Ts,
                );
                request.sourcemap = true;
                request
            })
            .unwrap();
        let input_map = transformed.map.expect("transform map");
        let minified = compiler
            .minify(MinifyRequest {
                id: "/gen/a.js".to_string(),
                code: transformed.code,
                sourcemap: true,
                input_map: Some(input_map),
            })
            .unwrap();
        let chained = minified.map.expect("chained map");
        let decoded =
            oxc_sourcemap::SourceMap::from_json_string(&chained.mappings).expect("decode chain");
        // Chained sources are the ORIGINAL .ts file, not the intermediate.
        let sources: Vec<&str> = decoded.get_sources().collect();
        assert_eq!(sources, vec!["/src/a.ts"]);
        // Minified output maps back to original lines (no dangling refs).
        let mut mapped = 0;
        for token in decoded.get_tokens() {
            if let Some(source_id) = token.get_source_id() {
                assert!(decoded.get_source(source_id).is_some());
                mapped += 1;
            }
        }
        assert!(mapped > 0, "expected mapped tokens");
    }

    #[test]
    fn chain_unmapped_positions_pass_through_sourceless() {
        fn build(source: &str, tokens: &[(u32, u32, u32, u32, bool)]) -> String {
            let mut builder = oxc_sourcemap::SourceMapBuilder::default();
            builder.set_source_and_content(source, "content");
            for (dst_line, dst_col, src_line, src_col, mapped) in tokens {
                let (src_line, src_col, src_id) = if *mapped {
                    (*src_line, *src_col, Some(0))
                } else {
                    (0, 0, None)
                };
                builder.add_token(*dst_line, *dst_col, src_line, src_col, src_id, None);
            }
            builder.into_sourcemap().to_json_string()
        }
        // inner: intermediate (0,0) → original (7,3).
        let inner = build("orig.ts", &[(0, 0, 7, 3, true)]);
        // outer: hit at intermediate (0,0), miss at (5,0), sourceless.
        let outer = build(
            "mid.js",
            &[(0, 2, 0, 0, true), (0, 9, 5, 0, true), (0, 12, 0, 0, false)],
        );
        let chained = chain_source_maps(&outer, &inner).unwrap();
        let decoded = oxc_sourcemap::SourceMap::from_json_string(&chained).unwrap();
        assert_eq!(decoded.get_sources().collect::<Vec<_>>(), vec!["orig.ts"]);
        let tokens: Vec<_> = decoded.get_tokens().collect();
        assert_eq!(tokens.len(), 3);
        // Hit remaps to the original position.
        assert_eq!(
            (
                tokens[0].get_src_line(),
                tokens[0].get_src_col(),
                tokens[0].get_source_id()
            ),
            (7, 3, Some(0))
        );
        // Miss and sourceless stay sourceless.
        assert_eq!(tokens[1].get_source_id(), None);
        assert_eq!(tokens[2].get_source_id(), None);
    }

    #[test]
    fn chain_rejects_bad_json() {
        assert!(chain_source_maps("nope", "{\"version\":3}").is_err());
        let inner = "{\"version\":3,\"sources\":[],\"names\":[],\"mappings\":\"\"}";
        assert!(chain_source_maps("nope", inner).is_err());
        assert!(chain_source_maps(inner, "nope").is_err());
    }

    #[cfg(not(feature = "swc"))]
    #[test]
    fn swc_engine_without_feature_fails_loudly() {
        let error = compiler_for_engine("swc")
            .err()
            .expect("unavailable factory must fail before execution");
        assert!(error.to_string().contains("--features swc"), "{error}");
        // Direct parser access is retained without advertising a usable backend.
        let compiler = SwcCompiler;
        let error = compiler
            .transform(TransformRequest::new(
                "/a.ts",
                "const x = 1;\n",
                ModuleType::Ts,
            ))
            .unwrap_err();
        assert!(error.to_string().contains("--features swc"), "{error}");
        let error = compiler
            .minify(MinifyRequest {
                id: "/a.js".to_string(),
                code: "const x = 1;\n".to_string(),
                sourcemap: false,
                input_map: None,
            })
            .unwrap_err();
        assert!(error.to_string().contains("--features swc"), "{error}");
        // Parsing still works: the graph never depends on the engine.
        let parsed = compiler
            .parse(ParseRequest {
                id: "/a.ts".to_string(),
                code: "import x from \"./x\";\n".to_string(),
                module_type: ModuleType::Ts,
            })
            .unwrap();
        assert_eq!(parsed.imports.len(), 1);
    }

    fn used(names: &[&str]) -> std::collections::HashSet<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    fn minified(id: &str, code: &str) -> String {
        compiler()
            .minify(MinifyRequest {
                id: id.to_string(),
                code: code.to_string(),
                sourcemap: false,
                input_map: None,
            })
            .unwrap()
            .code
    }

    #[test]
    fn drops_unused_specifiers() {
        let code = "const alpha = 1;\nconst beta = 2;\nexport { alpha, beta };\n";
        let dropped = drop_unused_exports("/a.js", code, &used(&["alpha"]))
            .unwrap()
            .expect("changed");
        assert!(dropped.contains("alpha"), "{dropped}");
        let export_line = dropped
            .lines()
            .find(|line| line.contains("export"))
            .expect("export line");
        assert!(!export_line.contains("beta"), "{dropped}");
        // Minifier collects the orphaned binding.
        let min = minified("/a.js", &dropped);
        assert!(!min.contains("beta"), "{min}");
    }

    #[test]
    fn unwraps_unused_declarations() {
        let code = "export const alpha = 1;\nexport function beta() { return 2; }\n";
        let dropped = drop_unused_exports("/a.js", code, &used(&["alpha"]))
            .unwrap()
            .expect("changed");
        assert!(dropped.contains("export const alpha"), "{dropped}");
        assert!(dropped.contains("function beta"), "{dropped}");
        assert!(!dropped.contains("export function"), "{dropped}");
        let min = minified("/a.js", &dropped);
        assert!(!min.contains("beta"), "{min}");
    }

    #[test]
    fn keeps_used_and_returns_none_when_clean() {
        let code = "export const alpha = 1;\nside();\n";
        assert!(drop_unused_exports("/a.js", code, &used(&["alpha"]))
            .unwrap()
            .is_none());
    }

    #[test]
    fn default_function_rules() {
        let named = "export default function fargo() { return 1; }\n";
        let dropped = drop_unused_exports("/a.js", named, &used(&[]))
            .unwrap()
            .expect("changed");
        assert!(dropped.contains("function fargo"), "{dropped}");
        assert!(!dropped.contains("export"), "{dropped}");
        let anon = "export default function () { return 1; }\n";
        let dropped = drop_unused_exports("/a.js", anon, &used(&[]))
            .unwrap()
            .expect("changed");
        assert!(!dropped.contains("return 1"), "{dropped}");
        // Default expressions may run: kept.
        let expr = "export default init();\n";
        assert!(drop_unused_exports("/a.js", expr, &used(&[]))
            .unwrap()
            .is_none());
    }

    #[test]
    fn keeps_reexports_classes_and_complex_patterns() {
        let code = "export * from \"./x.js\";\n\
            export const { deep } = unpack();\n\
            export default class extends Base {}\n";
        assert!(drop_unused_exports("/a.js", code, &used(&[]))
            .unwrap()
            .is_none());
        // Partially-used multi-declarator export: kept whole (conservative).
        let multi = "export const mixed = 1, other = 2;\n";
        assert!(drop_unused_exports("/a.js", multi, &used(&["mixed"]))
            .unwrap()
            .is_none());
        // Fully-unused multi-declarator export: unwrapped, minifier collects.
        let dropped = drop_unused_exports("/a.js", multi, &used(&[]))
            .unwrap()
            .expect("changed");
        assert!(!dropped.contains("export"), "{dropped}");
    }

    #[test]
    fn drops_empty_specifier_list() {
        let code = "const alpha = 1;\nexport { alpha };\n";
        let dropped = drop_unused_exports("/a.js", code, &used(&[]))
            .unwrap()
            .expect("changed");
        assert!(!dropped.contains("export"), "{dropped}");
    }

    #[test]
    fn parse_captures_bindings_and_export_details() {
        let parsed = parse_module(
            "/a.js",
            "import def, { named as alias } from \"./d.js\";\n\
             import * as ns from \"./n.js\";\n\
             import \"./s.js\";\n\
             const local = 1;\n\
             export { local as renamed };\n\
             export { x } from \"./r.js\";\n\
             export * from \"./star.js\";\n",
            &ModuleType::Js,
        )
        .unwrap();
        let by_spec = |spec: &str| {
            parsed
                .imports
                .iter()
                .find(|import| import.specifier == spec)
                .unwrap_or_else(|| panic!("{spec}"))
                .bindings
                .clone()
        };
        assert!(by_spec("./d.js").contains(&ImportBinding::Default));
        assert!(by_spec("./d.js").contains(&ImportBinding::Named("named".to_string())));
        assert_eq!(by_spec("./n.js"), vec![ImportBinding::Namespace]);
        assert_eq!(by_spec("./s.js"), vec![ImportBinding::SideEffect]);
        assert!(by_spec("./r.js").contains(&ImportBinding::Reexport));
        assert!(by_spec("./star.js").contains(&ImportBinding::Reexport));
        type Detail<'a> = (&'a str, Option<&'a str>, Option<&'a str>, Option<&'a str>);
        let details: Vec<Detail<'_>> = parsed
            .export_details
            .iter()
            .map(|detail| {
                (
                    detail.exported.as_str(),
                    detail.local.as_deref(),
                    detail.from.as_deref(),
                    detail.imported.as_deref(),
                )
            })
            .collect();
        assert!(
            details.contains(&("renamed", Some("local"), None, None)),
            "{details:?}"
        );
        assert!(
            details.contains(&("x", None, Some("./r.js"), Some("x"))),
            "{details:?}"
        );
        assert!(
            details.contains(&("*", None, Some("./star.js"), None)),
            "{details:?}"
        );
    }

    #[test]
    fn sourcemap_requested() {
        let mut request =
            TransformRequest::new("/src/a.ts", "const x: number = 1;\n", ModuleType::Ts);
        request.sourcemap = true;
        let result = compiler().transform(request).unwrap();
        assert!(result.map.is_some());
    }

    #[test]
    fn hot_rewrite_replaces_real_usage() {
        let code = "if (import.meta.hot) { import.meta.hot.accept(); }\n";
        let out = rewrite_import_meta_hot(code, "/src/main.js");
        assert!(!out.contains("import.meta.hot"), "{out}");
        assert_eq!(out.matches("__ferrite_create_hot__").count(), 2, "{out}");
        assert!(
            out.contains("globalThis.__ferrite_create_hot__(\"/src/main.js\")"),
            "{out}"
        );
    }

    #[test]
    fn hot_rewrite_skips_strings_comments_and_template_text() {
        let code = "const a = \"import.meta.hot\";\n\
            const b = 'x import.meta.hot y';\n\
            // import.meta.hot\n\
            /* import.meta.hot */\n\
            const c = `doc: import.meta.hot`;\n\
            console.log(a, b, c);\n";
        assert_eq!(rewrite_import_meta_hot(code, "/x.js"), code);
    }

    #[test]
    fn hot_rewrite_handles_template_expressions_and_spacing() {
        let code = "const c = `${import.meta.hot ? 1 : 0}`;\nif (import . meta . hot) {}\n";
        let out = rewrite_import_meta_hot(code, "/x.js");
        assert_eq!(out.matches("__ferrite_create_hot__").count(), 2, "{out}");
    }

    #[test]
    fn hot_rewrite_leaves_unparseable_code_alone() {
        let code = "const = ; // import.meta.hot\n";
        assert_eq!(rewrite_import_meta_hot(code, "/x.js"), code);
    }

    #[test]
    fn parse_flags_real_hot_only() {
        let used = parse_module("/a.js", "if (import.meta.hot) {}\n", &ModuleType::Js).unwrap();
        assert!(used.uses_import_meta_hot);
        let prose = parse_module(
            "/b.js",
            "const page = {\"html\": \"<code>import.meta.hot</code>\"};\n",
            &ModuleType::Js,
        )
        .unwrap();
        assert!(!prose.uses_import_meta_hot);
    }
    #[test]
    fn source_map_chain_handles_dense_minified_lines_and_unmapped_barriers() {
        let mut inner = oxc_sourcemap::SourceMapBuilder::default();
        let source = inner.set_source_and_content("original.ts", "source");
        let mut outer = oxc_sourcemap::SourceMapBuilder::default();
        let intermediate = outer.set_source_and_content("intermediate.js", "code");
        for index in 0..10_000 {
            inner.add_token(
                0,
                index * 2,
                index,
                0,
                if index == 5_000 { None } else { Some(source) },
                None,
            );
            for column in [index * 2, index * 2 + 1] {
                outer.add_token(0, column, 0, column, Some(intermediate), None);
            }
        }
        let json = chain_source_maps(
            &outer.into_sourcemap().to_json_string(),
            &inner.into_sourcemap().to_json_string(),
        )
        .unwrap();
        let chained = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap();
        assert_eq!(chained.get_tokens().count(), 20_000);
        for token in chained.get_tokens() {
            if token.get_dst_col() / 2 == 5_000 {
                assert!(
                    token.get_source_id().is_none(),
                    "unmapped helper barriers must stop lookups"
                );
            } else {
                assert_eq!(token.get_source_id(), Some(0));
                assert_eq!(token.get_src_line(), token.get_dst_col() / 2);
            }
        }
    }
}
