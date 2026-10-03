//! Native SWC transform + minify backend (§89, `swc` feature).
//!
//! Mirrors the Oxc pipeline stage for stage: TS strip, JSX (automatic or
//! classic), target lowering, word-safe defines, optional minify, source
//! maps chained through `input_map`. Parsing for imports/exports stays on
//! the shared Oxc frontend (`parse_module`), so both engines agree on the
//! module graph.

use ferrite_core::{FerriteError, ModuleType, Result, SourceMap, Target};
use swc_core::common::{BytePos, FileName, Globals, Mark, SourceMap as SwcSourceMap, GLOBALS};
use swc_core::ecma::ast::{Pass as _, Program};
use swc_core::ecma::codegen::text_writer::JsWriter;
use swc_core::ecma::codegen::{self, Emitter};
use swc_core::ecma::parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax};
use swc_core::ecma::transforms::base::resolver;
use swc_core::ecma::transforms::typescript;
use swc_core::ecma::visit::VisitMutWith as _;

use crate::{MinifyRequest, MinifyResult, TransformRequest, TransformResult};

/// Parse `code` into an swc [`Program`].
fn parse_program(
    cm: swc_core::common::sync::Lrc<SwcSourceMap>,
    id: &str,
    code: &str,
    module_type: &ModuleType,
) -> Result<(Program, swc_core::common::comments::SingleThreadedComments)> {
    let comments = swc_core::common::comments::SingleThreadedComments::default();
    let file = cm.new_source_file(
        swc_core::common::sync::Lrc::new(FileName::Custom(id.to_string())),
        code.to_string(),
    );
    let tsx = matches!(module_type, ModuleType::Tsx | ModuleType::Jsx);
    let syntax = if module_type.is_js_like() && !matches!(module_type, ModuleType::Js) {
        Syntax::Typescript(TsSyntax {
            tsx,
            decorators: true,
            ..Default::default()
        })
    } else {
        Syntax::Es(swc_core::ecma::parser::EsSyntax {
            jsx: tsx || matches!(module_type, ModuleType::Jsx),
            ..Default::default()
        })
    };
    let lexer = Lexer::new(
        syntax,
        Default::default(),
        StringInput::from(&*file),
        Some(&comments as &dyn swc_core::common::comments::Comments),
    );
    let mut parser = Parser::new_from(lexer);
    match parser.parse_program() {
        Ok(program) => Ok((program, comments)),
        Err(error) => {
            let message = format!("{error:?}");
            Err(FerriteError::Parse {
                id: id.to_string(),
                message: message.chars().take(300).collect(),
                frame: Some(ferrite_core::code_frame(code, 0, 2)),
            })
        }
    }
}

/// Print `program` with optional source-map mappings.
fn print(
    cm: swc_core::common::sync::Lrc<SwcSourceMap>,
    program: &Program,
    minify: bool,
    want_map: bool,
) -> Result<(
    String,
    Vec<(BytePos, swc_core::common::source_map::LineCol)>,
)> {
    let mut buf = Vec::new();
    let mut mappings = Vec::new();
    {
        let writer = JsWriter::new(
            cm.clone(),
            "\n",
            &mut buf,
            want_map.then_some(&mut mappings),
        );
        let mut config = codegen::Config::default();
        config.minify = minify;
        let mut emitter = Emitter {
            cfg: config,
            cm: cm.clone(),
            comments: None,
            wr: writer,
        };
        emitter
            .emit_program(program)
            .map_err(|error| FerriteError::Other(format!("swc codegen failed: {error:?}")))?;
    }
    let code = String::from_utf8(buf)
        .map_err(|error| FerriteError::Other(format!("swc emitted invalid UTF-8: {error}")))?;
    Ok((code, mappings))
}

/// Build the output map, chaining `input_map` (`code` → original).
fn build_map(
    cm: &SwcSourceMap,
    mappings: &[(BytePos, swc_core::common::source_map::LineCol)],
    input_map: Option<&SourceMap>,
) -> Result<SourceMap> {
    let orig = input_map
        .map(|map| {
            swc_sourcemap::SourceMap::from_slice(map.mappings.as_bytes()).map_err(|error| {
                FerriteError::Build(format!("cannot decode input source map: {error}"))
            })
        })
        .transpose()?;
    let map = cm.build_source_map(
        mappings,
        orig,
        swc_core::common::source_map::DefaultSourceMapGenConfig,
    );
    let mut buf = Vec::new();
    map.to_writer(&mut buf)
        .map_err(|error| FerriteError::Build(format!("cannot encode source map: {error}")))?;
    let json = String::from_utf8(buf)
        .map_err(|error| FerriteError::Build(format!("source map is not UTF-8: {error}")))?;
    Ok(SourceMap::external(json))
}

/// Shared transform core: parse → passes → print.
#[allow(clippy::too_many_arguments)]
fn run_passes(
    request_id: &str,
    code: &str,
    module_type: &ModuleType,
    jsx_runtime: &str,
    development: bool,
    target: &Target,
    minify_print: bool,
    want_map: bool,
) -> Result<(String, Option<SourceMap>)> {
    let cm: swc_core::common::sync::Lrc<SwcSourceMap> = Default::default();
    let (mut program, comments) = parse_program(cm.clone(), request_id, code, module_type)?;
    let is_ts = matches!(module_type, ModuleType::Ts | ModuleType::Tsx);
    let is_jsx = matches!(module_type, ModuleType::Jsx | ModuleType::Tsx);
    let jsx_options = swc_core::ecma::transforms::react::jsx::Options {
        runtime: if jsx_runtime == "classic" {
            Some(swc_core::ecma::transforms::react::jsx::Runtime::Classic)
        } else {
            Some(swc_core::ecma::transforms::react::jsx::Runtime::Automatic)
        },
        development: Some(development),
        ..Default::default()
    };
    GLOBALS.set(&Globals::new(), || {
        let unresolved_mark = Mark::new();
        let top_level_mark = Mark::new();
        resolver(unresolved_mark, top_level_mark, is_ts).process(&mut program);
        if is_ts {
            typescript::strip(unresolved_mark, top_level_mark).process(&mut program);
        }
        if is_jsx {
            swc_core::ecma::transforms::react::jsx::jsx(
                cm.clone(),
                Some(comments.clone()),
                jsx_options,
                top_level_mark,
                unresolved_mark,
            )
            .process(&mut program);
        }
        // Target lowering, newest pass first (mirrors preset-env staging).
        apply_compat(&mut program, target, unresolved_mark, comments.clone());
        program.visit_mut_with(&mut swc_core::ecma::transforms::base::fixer::fixer(None));
        program.visit_mut_with(&mut swc_core::ecma::transforms::base::hygiene::hygiene());
    });
    let (code, mappings) = print(cm.clone(), &program, minify_print, want_map)?;
    let map = if want_map {
        Some(build_map(&cm, &mappings, None)?)
    } else {
        None
    };
    Ok((code, map))
}

/// Lowering passes for `target` (each pass lowers one version level).
fn apply_compat(
    program: &mut Program,
    target: &Target,
    unresolved_mark: Mark,
    comments: swc_core::common::comments::SingleThreadedComments,
) {
    use swc_core::ecma::transforms::compat::*;
    // Explicit per-version passes with default configs; older targets run
    // more stages.
    let run_2022 = !matches!(target, Target::Es2022 | Target::EsNext | Target::Custom(_));
    let run_2021 = run_2022;
    let run_2020 = matches!(target, Target::Es2015);
    let run_2019 = run_2020;
    let run_2018 = run_2020;
    let run_2017 = run_2020;
    let run_2016 = run_2020;
    let run_2015 = run_2020;
    if run_2022 {
        es2022::es2022(Default::default(), unresolved_mark).process(program);
    }
    if run_2021 {
        es2021::es2021().process(program);
    }
    if run_2020 {
        es2020::es2020(Default::default(), unresolved_mark).process(program);
    }
    if run_2019 {
        es2019::es2019().process(program);
    }
    if run_2018 {
        es2018::es2018(Default::default()).process(program);
    }
    if run_2017 {
        es2017::es2017(Default::default(), unresolved_mark).process(program);
    }
    if run_2016 {
        es2016::es2016().process(program);
    }
    if run_2015 {
        es2015(unresolved_mark, Some(comments), Default::default()).process(program);
    }
}

/// SWC transform: mirrors [`crate::transform_module`] stage for stage.
pub fn transform_module_swc(request: TransformRequest) -> Result<TransformResult> {
    if request.module_type == ModuleType::Json {
        let value: serde_json::Value =
            serde_json::from_str(&request.code).map_err(|error| FerriteError::Parse {
                id: request.id.clone(),
                message: format!("invalid JSON: {error}"),
                frame: None,
            })?;
        let code = format!(
            "export default {};\n",
            serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string())
        );
        return Ok(TransformResult {
            code,
            map: None,
            dependencies: Vec::new(),
            imports: Vec::new(),
            exports: vec!["default".to_string()],
        });
    }
    if !request.module_type.is_js_like() {
        return Ok(TransformResult {
            code: request.code.clone(),
            map: None,
            dependencies: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
        });
    }
    let needs_transform = matches!(
        request.module_type,
        ModuleType::Ts | ModuleType::Tsx | ModuleType::Jsx
    );
    let (mut code, mut map) = if needs_transform {
        let (code, map) = run_passes(
            &request.id,
            &request.code,
            &request.module_type,
            &request.jsx_runtime,
            request.development,
            &request.target,
            false,
            request.sourcemap,
        )?;
        (code, map)
    } else {
        crate::parse::parse_module(&request.id, &request.code, &request.module_type)?;
        (request.code.clone(), None)
    };
    if !request.define.is_empty() {
        let unchanged = code.clone();
        let (defined, define_map) =
            crate::apply_defines_mapped(&request.id, &code, &request.define, request.sourcemap)?;
        map = match (define_map, map) {
            (Some(outer), Some(inner)) => Some(SourceMap::external(crate::chain_source_maps(
                &outer,
                &inner.mappings,
            )?)),
            (Some(outer), None) => Some(SourceMap::external(outer)),
            (None, inner) if defined == unchanged => inner,
            (None, _) => None,
        };
        code = defined;
    }
    if request.minify {
        let minified = minify_module_swc(&MinifyRequest {
            id: request.id.clone(),
            code,
            sourcemap: request.sourcemap,
            input_map: map,
        })?;
        code = minified.code;
        map = minified.map;
    }
    let parsed = crate::parse::parse_module(&request.id, &code, &ModuleType::Js)?;
    Ok(TransformResult {
        code,
        map,
        dependencies: Vec::new(),
        imports: parsed.imports,
        exports: parsed.exports,
    })
}

/// SWC minify: parse → optimize → minified print, map chained.
pub fn minify_module_swc(request: &MinifyRequest) -> Result<MinifyResult> {
    let cm: swc_core::common::sync::Lrc<SwcSourceMap> = Default::default();
    // Minify inputs are transformed JS; accept JSX defensively.
    let (program, _) = parse_program(cm.clone(), &request.id, &request.code, &ModuleType::Jsx)?;
    let program = GLOBALS.set(&Globals::new(), || {
        let unresolved_mark = Mark::new();
        let top_level_mark = Mark::new();
        let mut program = program;
        resolver(unresolved_mark, top_level_mark, false).process(&mut program);
        // Module inputs: top-level DCE collects bindings the statement
        // shake unwrapped (mirrors the Oxc minifier contract).
        let compress = swc_core::ecma::minifier::option::CompressOptions {
            module: true,
            top_level: Some(swc_core::ecma::minifier::option::TopLevelOptions { functions: true }),
            ..Default::default()
        };
        swc_core::ecma::minifier::optimize(
            program,
            cm.clone(),
            None,
            None,
            &swc_core::ecma::minifier::option::MinifyOptions {
                compress: Some(compress),
                mangle: Some(Default::default()),
                ..Default::default()
            },
            &swc_core::ecma::minifier::option::ExtraOptions {
                unresolved_mark,
                top_level_mark,
                mangle_name_cache: None,
            },
        )
    });
    let (code, mappings) = print(cm.clone(), &program, true, request.sourcemap)?;
    let map = if request.sourcemap {
        Some(build_map(&cm, &mappings, request.input_map.as_ref())?)
    } else {
        None
    };
    Ok(MinifyResult { code, map })
}

#[cfg(test)]
mod tests {
    use ferrite_core::Target;

    use super::*;
    use crate::{JsCompiler as _, SwcCompiler};

    fn swc() -> SwcCompiler {
        SwcCompiler
    }

    #[test]
    fn strips_typescript() {
        let result = swc()
            .transform(TransformRequest::new(
                "/a.ts",
                "export function add(a: number, b: number): number { return a + b; }\n",
                ModuleType::Ts,
            ))
            .unwrap();
        assert!(result.code.contains("function add"), "{}", result.code);
        assert!(!result.code.contains(": number"), "{}", result.code);
        assert!(result.exports.contains(&"add".to_string()));
    }

    #[test]
    fn transforms_jsx_automatic_and_classic() {
        let code = "export const el = <div className=\"a\">hi</div>;\n";
        // Development defaults on: dev runtime.
        let auto = swc()
            .transform(TransformRequest::new("/a.tsx", code, ModuleType::Tsx))
            .unwrap();
        assert!(auto.code.contains("react/jsx-dev-runtime"), "{}", auto.code);
        let mut prod = TransformRequest::new("/a.tsx", code, ModuleType::Tsx);
        prod.development = false;
        let prod = swc().transform(prod).unwrap();
        assert!(prod.code.contains("react/jsx-runtime"), "{}", prod.code);
        let mut classic = TransformRequest::new("/a.jsx", code, ModuleType::Jsx);
        classic.jsx_runtime = "classic".to_string();
        let classic = swc().transform(classic).unwrap();
        assert!(
            classic.code.contains("React.createElement"),
            "{}",
            classic.code
        );
    }

    #[test]
    fn lowers_for_es2015_and_keeps_es2022() {
        let code = "export const get = (a) => a?.b ?? 'd';\n";
        let mut old = TransformRequest::new("/a.js", code, ModuleType::Js);
        // Plain JS skips the pass pipeline; force TS parsing for lowering.
        old.module_type = ModuleType::Ts;
        old.target = Target::Es2015;
        let lowered = swc().transform(old).unwrap();
        assert!(!lowered.code.contains("?."), "{}", lowered.code);
        assert!(!lowered.code.contains("??"), "{}", lowered.code);
        let mut modern = TransformRequest::new("/a.ts", code, ModuleType::Ts);
        modern.target = Target::Es2022;
        let kept = swc().transform(modern).unwrap();
        assert!(kept.code.contains("?."), "{}", kept.code);
    }

    #[test]
    fn minifies_and_keeps_exports() {
        let result = swc()
            .minify(crate::MinifyRequest {
                id: "/a.js".to_string(),
                code: "export const alpha = 1;\nexport function beta(x) { return x + 1; }\n"
                    .to_string(),
                sourcemap: false,
                input_map: None,
            })
            .unwrap();
        assert!(result.code.contains("alpha"), "{}", result.code);
        assert!(result.code.contains("export"), "{}", result.code);
        assert!(result.code.len() < 90, "{}", result.code);
    }

    #[test]
    fn collects_unwrapped_top_level_bindings() {
        // The statement shake unwraps unused exports; the minifier must
        // collect the orphaned top-level bindings (Oxc contract parity).
        let result = swc()
            .minify(crate::MinifyRequest {
                id: "/a.js".to_string(),
                code: "export const keepme = 1;\nconst dropme = 2;\n".to_string(),
                sourcemap: false,
                input_map: None,
            })
            .unwrap();
        assert!(result.code.contains("keepme"), "{}", result.code);
        assert!(!result.code.contains("dropme"), "{}", result.code);
    }

    #[test]
    fn chains_maps_through_transform_and_minify() {
        let mut request =
            TransformRequest::new("/a.ts", "export const alpha: number = 1;\n", ModuleType::Ts);
        request.sourcemap = true;
        let transformed = swc().transform(request).unwrap();
        let map = transformed.map.expect("transform map");
        assert!(map.mappings.contains("\"mappings\""), "{}", map.mappings);
        let minified = swc()
            .minify(crate::MinifyRequest {
                id: "/a.ts".to_string(),
                code: transformed.code,
                sourcemap: true,
                input_map: Some(map),
            })
            .unwrap();
        let chained = minified.map.expect("chained map");
        // Chained map still references the original TS source.
        assert!(chained.mappings.contains("/a.ts"), "{}", chained.mappings);
        assert!(
            chained.mappings.contains("\"mappings\""),
            "{}",
            chained.mappings
        );
    }

    #[test]
    fn applies_defines_and_json() {
        let mut request = TransformRequest::new("/a.js", "console.log(VERSION);\n", ModuleType::Js);
        request
            .define
            .insert("VERSION".to_string(), "\"1.2.3\"".to_string());
        let result = swc().transform(request).unwrap();
        assert!(result.code.contains("1.2.3"), "{}", result.code);
        let json = swc()
            .transform(TransformRequest::new(
                "/a.json",
                "{\"a\":1}",
                ModuleType::Json,
            ))
            .unwrap();
        assert!(json.code.contains("export default"), "{}", json.code);
    }

    #[test]
    fn bad_syntax_fails_loudly() {
        let error = swc()
            .transform(TransformRequest::new(
                "/a.ts",
                "const = ;\n",
                ModuleType::Ts,
            ))
            .unwrap_err();
        assert!(error.to_string().contains("/a.ts"), "{error}");
    }
}
