//! Static `ferrite.config.*` / `vite.config.*` loading (spec §10).
//!
//! The config file is *parsed* with Oxc and evaluated statically — it is
//! never executed, so no JS runtime is required. Only plain data is read:
//! string/number/boolean literals, arrays, and object literals, optionally
//! wrapped in `defineConfig(...)`. Anything dynamic (function calls other
//! than `defineConfig`, identifiers without a local `const` initializer,
//! spreads of unknown values, `plugins: [react()]`, ...) is skipped with a
//! warning, or rejected with an actionable error when the default export
//! itself is not a static object.

use crate::LibConfig;
use crate::SourceMapConfig;
use crate::UserConfig;
use ferrite_core::FerriteError;
use ferrite_core::Result;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

/// A statically loaded JS/TS config file.
#[derive(Debug)]
pub struct LoadedConfigFile {
    /// File the config was read from.
    pub path: PathBuf,
    /// Extracted user config.
    pub config: UserConfig,
    /// Keys/values skipped because they are not statically evaluable.
    pub warnings: Vec<String>,
}

/// Config file stems, in precedence order.
const CANDIDATE_STEMS: [&str; 2] = ["ferrite.config", "vite.config"];

/// Config file extensions, in precedence order.
const CANDIDATE_EXTS: [&str; 8] = ["mts", "cts", "ts", "tsx", "mjs", "cjs", "js", "jsx"];

/// Load `ferrite.config.*` (falling back to `vite.config.*`) from `dir`,
/// the `loadConfigFromFile` equivalent. Returns `None` when no candidate
/// file exists.
pub fn load_config_from_file(dir: &Path) -> Result<Option<LoadedConfigFile>> {
    for stem in CANDIDATE_STEMS {
        for ext in CANDIDATE_EXTS {
            let path = dir.join(format!("{stem}.{ext}"));
            if path.is_file() {
                return load_one(&path).map(Some);
            }
        }
    }
    Ok(None)
}

/// Parse one config file.
pub(crate) fn load_one(path: &Path) -> Result<LoadedConfigFile> {
    let text = std::fs::read_to_string(path)?;
    let source_type = SourceType::from_path(path)
        .map(|source_type| source_type.with_module(true))
        .unwrap_or_else(|_| SourceType::mjs());
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &text, source_type).parse();
    if parsed.fatal_error || !parsed.diagnostics.is_empty() {
        let message = parsed
            .diagnostics
            .iter()
            .take(3)
            .map(|diagnostic| diagnostic.message.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(FerriteError::Config(format!(
            "{}: cannot parse config: {message}",
            path.display()
        )));
    }
    let file = path.display().to_string();
    // Top-level `const NAME = <expr>` bindings (for `export default NAME`).
    let mut locals: HashMap<String, &Expression> = HashMap::new();
    let mut default_export: Option<&Expression> = None;
    let mut module_exports: Option<&Expression> = None;
    for statement in &parsed.program.body {
        match statement {
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    if let (BindingPattern::BindingIdentifier(id), Some(init)) =
                        (&declarator.id, &declarator.init)
                    {
                        locals.insert(id.name.to_string(), init);
                    }
                }
            }
            Statement::ExportDefaultDeclaration(declaration) => {
                if let Some(expression) = declaration.declaration.as_expression() {
                    default_export = Some(expression);
                } else {
                    return Err(dynamic_export_error(&file, "function or class"));
                }
            }
            Statement::ExpressionStatement(statement) => {
                if let Expression::AssignmentExpression(assignment) = &statement.expression {
                    if is_module_exports(&assignment.left) {
                        module_exports = Some(&assignment.right);
                    }
                }
            }
            _ => {}
        }
    }
    let exported = default_export.or(module_exports).ok_or_else(|| {
        FerriteError::Config(format!(
            "{file}: no default export found; expected `export default {{ ... }}` \
             or `module.exports = {{ ... }}`"
        ))
    })?;
    let evaluator = Evaluator {
        locals: &locals,
        file: &file,
    };
    let mut warnings = Vec::new();
    // Top-level keys evaluate independently: one dynamic value skips its
    // key with a warning instead of failing the whole file.
    let object = evaluator
        .eval_top_object(exported, &mut warnings)
        .map_err(|reason| {
            FerriteError::Config(format!(
                "{file}: default export is not statically evaluable ({reason}); \
                 Ferrite parses config files without executing them, so only plain \
                 object literals (optionally wrapped in `defineConfig(...)`) are supported"
            ))
        })?;
    let config = map_config(&object, &mut warnings, &file);
    Ok(LoadedConfigFile {
        path: path.to_path_buf(),
        config,
        warnings,
    })
}

/// Error for a non-static default export.
fn dynamic_export_error(file: &str, found: &str) -> FerriteError {
    FerriteError::Config(format!(
        "{file}: default export is {found}; Ferrite parses config files without \
         executing them, so only plain object literals (optionally wrapped in \
         `defineConfig(...)`) are supported"
    ))
}

/// True for a `module.exports = ...` assignment target.
fn is_module_exports(target: &AssignmentTarget) -> bool {
    let AssignmentTarget::StaticMemberExpression(member) = target else {
        return false;
    };
    let Expression::Identifier(object) = &member.object else {
        return false;
    };
    object.name.as_str() == "module" && member.property.name.as_str() == "exports"
}

/// Static expression evaluator: AST → JSON.
struct Evaluator<'a> {
    locals: &'a HashMap<String, &'a Expression<'a>>,
    file: &'a str,
}

impl<'a> Evaluator<'a> {
    /// Evaluate the default export as a top-level object, skipping dynamic
    /// keys with warnings instead of failing the whole file.
    fn eval_top_object(
        &self,
        exported: &'a Expression<'a>,
        warnings: &mut Vec<String>,
    ) -> std::result::Result<JsonObject, String> {
        let mut current = exported;
        let mut hops = 0;
        let object = loop {
            if hops > 32 {
                return Err("too many indirections".to_string());
            }
            hops += 1;
            match current {
                Expression::ObjectExpression(object) => break object,
                Expression::Identifier(ident) => {
                    let Some(init) = self.locals.get(ident.name.as_str()) else {
                        return Err(format!("identifier `{}`", ident.name.as_str()));
                    };
                    current = init;
                }
                Expression::CallExpression(call) => {
                    let Expression::Identifier(callee) = &call.callee else {
                        return Err("method call".to_string());
                    };
                    if callee.name.as_str() != "defineConfig" || call.arguments.len() != 1 {
                        return Err(format!("call to `{}`", callee.name.as_str()));
                    }
                    let Some(argument) = call.arguments[0].as_expression() else {
                        return Err("spread argument".to_string());
                    };
                    current = argument;
                }
                Expression::ParenthesizedExpression(paren) => current = &paren.expression,
                Expression::TSAsExpression(cast) => current = &cast.expression,
                Expression::TSSatisfiesExpression(satisfies) => current = &satisfies.expression,
                Expression::TSNonNullExpression(non_null) => current = &non_null.expression,
                Expression::TSTypeAssertion(assertion) => current = &assertion.expression,
                other => return Err(expression_kind(other).to_string()),
            }
        };
        let mut map = JsonObject::with_capacity(object.properties.len());
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                warnings.push(format!("{}: ignoring a spread property", self.file));
                continue;
            };
            if property.kind != PropertyKind::Init || property.method {
                warnings.push(format!("{}: ignoring a method or accessor", self.file));
                continue;
            }
            let key = match property_key_name(&property.key) {
                Ok(key) => key,
                Err(reason) => {
                    warnings.push(format!("{}: ignoring a key ({reason})", self.file));
                    continue;
                }
            };
            match self.eval(&property.value, &mut Vec::new()) {
                Ok(value) => {
                    map.insert(key, value);
                }
                Err(reason) => warnings.push(format!(
                    "{}: ignoring `{key}`: not statically evaluable ({reason})",
                    self.file
                )),
            }
        }
        Ok(map)
    }

    /// Evaluate `expression` to JSON, or describe why it is dynamic.
    fn eval(
        &self,
        expression: &'a Expression<'a>,
        stack: &mut Vec<String>,
    ) -> std::result::Result<Value, String> {
        match expression {
            Expression::StringLiteral(literal) => Ok(Value::String(literal.value.to_string())),
            Expression::NumericLiteral(literal) => number_from_f64(literal.value)
                .map(Value::Number)
                .ok_or_else(|| "non-finite number".to_string()),
            Expression::BooleanLiteral(literal) => Ok(Value::Bool(literal.value)),
            Expression::NullLiteral(_) => Ok(Value::Null),
            Expression::ArrayExpression(array) => {
                let mut values = Vec::with_capacity(array.elements.len());
                for element in &array.elements {
                    let Some(element) = element.as_expression() else {
                        return Err("spread element".to_string());
                    };
                    values.push(self.eval(element, stack)?);
                }
                Ok(Value::Array(values))
            }
            Expression::ObjectExpression(object) => {
                let mut map = serde_json::Map::with_capacity(object.properties.len());
                for property in &object.properties {
                    let ObjectPropertyKind::ObjectProperty(property) = property else {
                        return Err("spread property".to_string());
                    };
                    if property.kind != PropertyKind::Init || property.method {
                        return Err("method or accessor".to_string());
                    }
                    let key = property_key_name(&property.key)?;
                    map.insert(key, self.eval(&property.value, stack)?);
                }
                Ok(Value::Object(map))
            }
            Expression::TemplateLiteral(template) => {
                if !template.expressions.is_empty() {
                    return Err("template with interpolations".to_string());
                }
                let mut text = String::new();
                for quasi in &template.quasis {
                    let Some(cooked) = quasi.value.cooked else {
                        return Err("template with invalid escape".to_string());
                    };
                    text.push_str(cooked.as_str());
                }
                Ok(Value::String(text))
            }
            Expression::UnaryExpression(unary) => self.eval_unary(unary, stack),
            Expression::Identifier(ident) => {
                let name = ident.name.to_string();
                if name == "undefined" {
                    return Ok(Value::Null);
                }
                if stack.contains(&name) {
                    return Err(format!("circular const `{name}`"));
                }
                let Some(init) = self.locals.get(&name) else {
                    return Err(format!("identifier `{name}`"));
                };
                stack.push(name);
                let value = self.eval(init, stack);
                stack.pop();
                value
            }
            Expression::CallExpression(call) => {
                let Expression::Identifier(callee) = &call.callee else {
                    return Err("method call".to_string());
                };
                if callee.name.as_str() != "defineConfig" {
                    return Err(format!("call to `{}`", callee.name.as_str()));
                }
                if call.arguments.len() != 1 {
                    return Err("`defineConfig` with != 1 argument".to_string());
                }
                let Some(argument) = call.arguments[0].as_expression() else {
                    return Err("spread argument".to_string());
                };
                self.eval(argument, stack)
            }
            Expression::ParenthesizedExpression(paren) => self.eval(&paren.expression, stack),
            Expression::TSAsExpression(cast) => self.eval(&cast.expression, stack),
            Expression::TSSatisfiesExpression(satisfies) => self.eval(&satisfies.expression, stack),
            Expression::TSNonNullExpression(non_null) => self.eval(&non_null.expression, stack),
            Expression::TSTypeAssertion(assertion) => self.eval(&assertion.expression, stack),
            other => Err(format!("{} (see {})", expression_kind(other), self.file)),
        }
    }

    /// Evaluate `+x` / `-x` / `!x` over literals.
    fn eval_unary(
        &self,
        unary: &'a UnaryExpression<'a>,
        stack: &mut Vec<String>,
    ) -> std::result::Result<Value, String> {
        match unary.operator {
            oxc_syntax::operator::UnaryOperator::UnaryNegation
            | oxc_syntax::operator::UnaryOperator::UnaryPlus => {
                let value = self.eval(&unary.argument, stack)?;
                let Value::Number(number) = value else {
                    return Err("unary +/- on a non-number".to_string());
                };
                let magnitude = number.as_f64().unwrap_or(0.0);
                let signed = if unary.operator == oxc_syntax::operator::UnaryOperator::UnaryNegation
                {
                    -magnitude
                } else {
                    magnitude
                };
                number_from_f64(signed)
                    .map(Value::Number)
                    .ok_or_else(|| "non-finite number".to_string())
            }
            oxc_syntax::operator::UnaryOperator::LogicalNot => {
                let value = self.eval(&unary.argument, stack)?;
                let Value::Bool(flag) = value else {
                    return Err("`!` on a non-boolean".to_string());
                };
                Ok(Value::Bool(!flag))
            }
            _ => Err("unary operator".to_string()),
        }
    }
}

/// Short kind name for error messages.
fn expression_kind(expression: &Expression) -> &'static str {
    match expression {
        Expression::ArrowFunctionExpression(_)
        | Expression::FunctionExpression(_)
        | Expression::ClassExpression(_) => "function or class",
        Expression::NewExpression(_) => "`new` expression",
        Expression::AwaitExpression(_) | Expression::YieldExpression(_) => "async expression",
        Expression::ImportExpression(_) => "`import()`",
        Expression::ConditionalExpression(_) | Expression::LogicalExpression(_) => "conditional",
        Expression::BinaryExpression(_) => "binary expression",
        Expression::SequenceExpression(_) => "sequence",
        Expression::TaggedTemplateExpression(_) => "tagged template",
        Expression::UpdateExpression(_) => "update expression",
        Expression::JSXElement(_) | Expression::JSXFragment(_) => "JSX",
        Expression::ImportMeta(_) | Expression::NewTarget(_) | Expression::ThisExpression(_) => {
            "meta expression"
        }
        Expression::Super(_) => "`super`",
        Expression::RegExpLiteral(_) | Expression::BigIntLiteral(_) => "literal",
        _ => "dynamic expression",
    }
}

/// JSON number from an `f64`, preserving integer typing for whole values
/// (so `as_u64`/`as_i64` work downstream).
fn number_from_f64(value: f64) -> Option<serde_json::Number> {
    if !value.is_finite() {
        return None;
    }
    if value.fract() == 0.0 {
        if value >= 0.0 && value <= u64::MAX as f64 {
            return Some(serde_json::Number::from(value as u64));
        }
        if value >= i64::MIN as f64 && value <= i64::MAX as f64 {
            return Some(serde_json::Number::from(value as i64));
        }
    }
    serde_json::Number::from_f64(value)
}

/// Property key text (`{ a: 1 }`, `{ "a": 1 }`, `{ 0: 1 }`).
fn property_key_name(key: &PropertyKey) -> std::result::Result<String, String> {
    match key {
        PropertyKey::StaticIdentifier(name) => Ok(name.name.to_string()),
        PropertyKey::StringLiteral(literal) => Ok(literal.value.to_string()),
        PropertyKey::NumericLiteral(literal) => Ok(literal.value.to_string()),
        _ => Err("computed key".to_string()),
    }
}

type JsonObject = serde_json::Map<String, Value>;

/// Map a static config object onto [`UserConfig`], warning on skipped keys.
fn map_config(object: &JsonObject, warnings: &mut Vec<String>, file: &str) -> UserConfig {
    let mut config = UserConfig::default();
    for (key, value) in object {
        match key.as_str() {
            "root" | "base" | "mode" => {
                if let Some(text) = value.as_str() {
                    match key.as_str() {
                        "root" => config.root = Some(text.to_string()),
                        "base" => config.base = Some(text.to_string()),
                        _ => config.mode = Some(text.to_string()),
                    }
                } else {
                    warnings.push(format!("{file}: ignoring `{key}`: expected a string"));
                }
            }
            "define" => {
                if let Value::Object(defines) = value {
                    for (name, replacement) in defines {
                        if let Some(text) = scalar_text(replacement) {
                            config.define.insert(name.clone(), text);
                        } else {
                            warnings.push(format!(
                                "{file}: ignoring `define.{name}`: expected a scalar"
                            ));
                        }
                    }
                } else {
                    warnings.push(format!("{file}: ignoring `define`: expected an object"));
                }
            }
            "server" => map_server(value, &mut config, warnings, file),
            "build" => map_build(value, &mut config, warnings, file),
            "resolve" => map_resolve(value, &mut config, warnings, file),
            "envPrefix" => {
                config.env.prefix = string_list(value, "envPrefix", warnings, file);
            }
            "env" => {
                if let Value::Object(env) = value {
                    if let Some(prefix) = env.get("prefix") {
                        config.env.prefix = string_list(prefix, "env.prefix", warnings, file);
                    }
                    for key in env.keys() {
                        if key != "prefix" {
                            warnings.push(format!("{file}: ignoring unknown key `env.{key}`"));
                        }
                    }
                } else {
                    warnings.push(format!("{file}: ignoring `env`: expected an object"));
                }
            }
            "ssr" => map_ssr(value, &mut config, warnings, file),
            "npm" => {
                if let Value::Object(npm) = value {
                    if let Some(text) = get_str(npm, &["registry"]) {
                        config.npm.registry = text;
                    }
                    if let Some(text) = get_str(npm, &["lockfile"]) {
                        config.npm.lockfile = text;
                    }
                    if let Some(text) = get_str(npm, &["devStrategy", "dev_strategy"]) {
                        config.npm.dev_strategy = text;
                    }
                } else {
                    warnings.push(format!("{file}: ignoring `npm`: expected an object"));
                }
            }
            "compiler" => {
                if let Value::Object(compiler) = value {
                    if let Some(text) = get_str(compiler, &["engine"]) {
                        config.compiler.engine = text;
                    }
                } else {
                    warnings.push(format!("{file}: ignoring `compiler`: expected an object"));
                }
            }
            "plugins" => warnings.push(format!(
                "{file}: ignoring `plugins`: JS plugins cannot be evaluated statically; \
                 add Rust plugins programmatically instead"
            )),
            _ => warnings.push(format!("{file}: ignoring unknown config key `{key}`")),
        }
    }
    config
}

/// Map the `server` section.
fn map_server(value: &Value, config: &mut UserConfig, warnings: &mut Vec<String>, file: &str) {
    let Value::Object(server) = value else {
        warnings.push(format!("{file}: ignoring `server`: expected an object"));
        return;
    };
    if let Some(text) = get_str(server, &["host"]) {
        config.server.host = text;
    }
    if let Some(port) = server.get("port").and_then(Value::as_u64) {
        config.server.port = port.min(u16::MAX as u64) as u16;
    } else if server.contains_key("port") {
        warnings.push(format!("{file}: ignoring `server.port`: expected a number"));
    }
    if let Some(flag) = get_bool(server, &["strictPort", "strict_port"]) {
        config.server.strict_port = flag;
    }
    if let Some(flag) = get_bool(server, &["open"]) {
        config.server.open = flag;
    }
    if let Some(flag) = get_bool(server, &["hmr"]) {
        config.server.hmr = flag;
    }
    if let Some(flag) = get_bool(server, &["middlewareMode", "middleware_mode"]) {
        config.server.middleware_mode = flag;
    }
    if let Some(proxy) = server.get("proxy") {
        if let Value::Object(rules) = proxy {
            for (prefix, target) in rules {
                match target {
                    Value::String(target) => {
                        config.server.proxy.insert(prefix.clone(), target.clone());
                    }
                    Value::Object(options) => {
                        if let Some(target) = options.get("target").and_then(Value::as_str) {
                            config
                                .server
                                .proxy
                                .insert(prefix.clone(), target.to_string());
                        } else {
                            warnings.push(format!(
                                "{file}: ignoring `server.proxy.{prefix}`: expected a target URL"
                            ));
                        }
                    }
                    _ => warnings.push(format!(
                        "{file}: ignoring `server.proxy.{prefix}`: expected a URL or options object"
                    )),
                }
            }
        } else {
            warnings.push(format!(
                "{file}: ignoring `server.proxy`: expected an object"
            ));
        }
    }
}

/// Map the `build` section.
fn map_build(value: &Value, config: &mut UserConfig, warnings: &mut Vec<String>, file: &str) {
    let Value::Object(build) = value else {
        warnings.push(format!("{file}: ignoring `build`: expected an object"));
        return;
    };
    if let Some(text) = get_str(build, &["outDir", "out_dir"]) {
        config.build.out_dir = text;
    }
    match build.get("sourcemap") {
        Some(Value::Bool(enabled)) => config.build.sourcemap = SourceMapConfig::Bool(*enabled),
        Some(Value::String(mode)) => config.build.sourcemap = SourceMapConfig::Mode(mode.clone()),
        Some(_) => warnings.push(format!(
            "{file}: ignoring `build.sourcemap`: expected a boolean or mode string"
        )),
        None => {}
    }
    match build.get("minify") {
        Some(Value::Bool(minify)) => config.build.minify = *minify,
        Some(Value::String(mode)) => config.build.minify = mode != "false",
        Some(_) => warnings.push(format!(
            "{file}: ignoring `build.minify`: expected a boolean"
        )),
        None => {}
    }
    if let Some(text) = get_str(build, &["target"]) {
        config.build.target = text;
    }
    if let Some(lib) = build.get("lib") {
        if let Value::Object(lib) = lib {
            if let Some(entry) = get_str(lib, &["entry"]) {
                config.build.lib = Some(LibConfig {
                    entry,
                    name: get_str(lib, &["name"]),
                    formats: lib
                        .get("formats")
                        .map(|formats| string_list(formats, "build.lib.formats", warnings, file))
                        .unwrap_or_else(|| vec!["es".to_string()]),
                });
            } else {
                warnings.push(format!(
                    "{file}: ignoring `build.lib`: missing string `entry`"
                ));
            }
        } else {
            warnings.push(format!("{file}: ignoring `build.lib`: expected an object"));
        }
    }
}

/// Map the `resolve` section.
fn map_resolve(value: &Value, config: &mut UserConfig, warnings: &mut Vec<String>, file: &str) {
    let Value::Object(resolve) = value else {
        warnings.push(format!("{file}: ignoring `resolve`: expected an object"));
        return;
    };
    match resolve.get("alias") {
        Some(Value::Object(entries)) => {
            for (from, to) in entries {
                if let Some(to) = to.as_str() {
                    config.resolve.alias.insert(from.clone(), to.to_string());
                } else {
                    warnings.push(format!(
                        "{file}: ignoring `resolve.alias.{from}`: expected a string"
                    ));
                }
            }
        }
        Some(Value::Array(entries)) => {
            for entry in entries {
                let (Some(find), Some(replacement)) = (
                    entry.get("find").and_then(Value::as_str),
                    entry.get("replacement").and_then(Value::as_str),
                ) else {
                    warnings.push(format!(
                        "{file}: ignoring a `resolve.alias` entry: expected `{{find, replacement}}` strings"
                    ));
                    continue;
                };
                if find.starts_with('^') || find.contains('(') || find.contains('$') {
                    warnings.push(format!(
                        "{file}: alias `{find}` looks like a regex; Ferrite aliases are literal prefixes"
                    ));
                }
                config
                    .resolve
                    .alias
                    .insert(find.to_string(), replacement.to_string());
            }
        }
        Some(_) => warnings.push(format!(
            "{file}: ignoring `resolve.alias`: expected an object or array"
        )),
        None => {}
    }
    if let Some(conditions) = resolve.get("conditions") {
        config.resolve.conditions = string_list(conditions, "resolve.conditions", warnings, file);
    }
    if let Some(extensions) = resolve.get("extensions") {
        config.resolve.extensions = string_list(extensions, "resolve.extensions", warnings, file);
    }
    if let Some(flag) = get_bool(resolve, &["preserveSymlinks", "preserve_symlinks"]) {
        config.resolve.preserve_symlinks = flag;
    }
}

/// Map the `ssr` section.
fn map_ssr(value: &Value, config: &mut UserConfig, warnings: &mut Vec<String>, file: &str) {
    let Value::Object(ssr) = value else {
        warnings.push(format!("{file}: ignoring `ssr`: expected an object"));
        return;
    };
    if let Some(external) = ssr.get("external") {
        config.ssr.external = string_list(external, "ssr.external", warnings, file);
    }
    match ssr.get("noExternal").or_else(|| ssr.get("no_external")) {
        Some(Value::Bool(true)) => config.ssr.bundle_all = true,
        Some(Value::Bool(false)) | None => {}
        Some(list) => {
            config.ssr.no_external = string_list(list, "ssr.noExternal", warnings, file);
        }
    }
}

/// First present string among `names`.
fn get_str(object: &JsonObject, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        object
            .get(*name)
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

/// First present boolean among `names`.
fn get_bool(object: &JsonObject, names: &[&str]) -> Option<bool> {
    names
        .iter()
        .find_map(|name| object.get(*name).and_then(Value::as_bool))
}

/// A string or string array as a list (non-strings warned + skipped).
fn string_list(value: &Value, what: &str, warnings: &mut Vec<String>, file: &str) -> Vec<String> {
    match value {
        Value::String(text) => vec![text.clone()],
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                if let Some(text) = item.as_str() {
                    out.push(text.to_string());
                } else {
                    warnings.push(format!("{file}: ignoring a non-string in `{what}`"));
                }
            }
            out
        }
        _ => {
            warnings.push(format!(
                "{file}: ignoring `{what}`: expected a string or array"
            ));
            Vec::new()
        }
    }
}

/// Scalar JSON as config text (`define` values).
fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Null => None,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ferrite-jsconfig-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn export_default_object_maps() {
        let dir = temp_dir("object");
        write(
            &dir,
            "ferrite.config.ts",
            r#"
            export default {
              base: "/app/",
              define: { __APP_VERSION__: "1.2.3", __DEBUG__: false, __PORT__: 3000 },
              server: {
                host: "0.0.0.0",
                port: 3000,
                strictPort: true,
                proxy: { "/api": "http://localhost:4000", "/v2": { target: "http://x" } },
              },
              build: { outDir: "out", sourcemap: "hidden", minify: false, target: "es2021" },
              resolve: {
                alias: [{ find: "@", replacement: "./src" }],
                conditions: ["node", "import"],
              },
              envPrefix: ["APP_", "PUBLIC_"],
              ssr: { noExternal: ["linked-dep"] },
            };
            "#,
        );
        let loaded = load_config_from_file(&dir).unwrap().expect("config");
        assert!(loaded.path.ends_with("ferrite.config.ts"));
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        let config = loaded.config;
        assert_eq!(config.base.as_deref(), Some("/app/"));
        assert_eq!(config.define.get("__APP_VERSION__").unwrap(), "1.2.3");
        assert_eq!(config.define.get("__DEBUG__").unwrap(), "false");
        assert_eq!(config.define.get("__PORT__").unwrap(), "3000");
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 3000);
        assert!(config.server.strict_port);
        assert_eq!(
            config.server.proxy.get("/api").unwrap(),
            "http://localhost:4000"
        );
        assert_eq!(config.server.proxy.get("/v2").unwrap(), "http://x");
        assert_eq!(config.build.out_dir, "out");
        assert!(config.build.sourcemap.hidden());
        assert!(!config.build.minify);
        assert_eq!(config.build.target, "es2021");
        assert_eq!(config.resolve.alias.get("@").unwrap(), "./src");
        assert_eq!(config.resolve.conditions, vec!["node", "import"]);
        assert_eq!(config.env.prefix, vec!["APP_", "PUBLIC_"]);
        assert_eq!(config.ssr.no_external, vec!["linked-dep"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn define_config_wrapper_and_locals() {
        let dir = temp_dir("wrapper");
        write(
            &dir,
            "vite.config.mjs",
            r#"
            import { defineConfig } from "vite";
            const port = 4000;
            const out = `dist-prod`;
            export default defineConfig({
              server: { port },
              build: { outDir: out, sourcemap: true },
            });
            "#,
        );
        let loaded = load_config_from_file(&dir).unwrap().expect("config");
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        assert_eq!(loaded.config.server.port, 4000);
        assert_eq!(loaded.config.build.out_dir, "dist-prod");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn template_with_interpolation_warns_not_errors() {
        let dir = temp_dir("interp");
        // The default export is an object, but one nested value is dynamic:
        // the whole file still loads (the key is skipped with a warning).
        write(
            &dir,
            "ferrite.config.js",
            "const suffix = process.env.X;\nexport default { base: `/app/${suffix}` };",
        );
        let loaded = load_config_from_file(&dir).unwrap().expect("config");
        // `base` failed to evaluate → skipped with a warning.
        assert!(loaded.config.base.is_none());
        assert!(
            loaded
                .warnings
                .iter()
                .any(|warning| warning.contains("base")),
            "{:?}",
            loaded.warnings
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn module_exports_and_cjs() {
        let dir = temp_dir("cjs");
        write(
            &dir,
            "ferrite.config.cjs",
            "module.exports = { server: { port: 5000 }, ssr: { noExternal: true } };",
        );
        let loaded = load_config_from_file(&dir).unwrap().expect("config");
        assert_eq!(loaded.config.server.port, 5000);
        assert!(loaded.config.ssr.bundle_all);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dynamic_default_export_errors_loudly() {
        let dir = temp_dir("dynamic");
        write(
            &dir,
            "vite.config.js",
            "export default defineConfig(({ mode }) => ({ base: mode }));",
        );
        let error = load_config_from_file(&dir).unwrap_err();
        assert!(
            error.to_string().contains("not statically evaluable"),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plugins_key_warns_and_rest_loads() {
        let dir = temp_dir("plugins");
        write(
            &dir,
            "vite.config.js",
            "import react from '@vitejs/plugin-react';\n\
             export default { plugins: [react()], server: { port: 3001 } };",
        );
        let loaded = load_config_from_file(&dir).unwrap().expect("config");
        assert_eq!(loaded.config.server.port, 3001);
        assert!(
            loaded
                .warnings
                .iter()
                .any(|warning| warning.contains("plugins")),
            "{:?}",
            loaded.warnings
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stem_precedence_and_missing() {
        let dir = temp_dir("precedence");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(load_config_from_file(&dir).unwrap().is_none());
        write(
            &dir,
            "vite.config.js",
            "export default { server: { port: 1 } };",
        );
        write(
            &dir,
            "ferrite.config.js",
            "export default { server: { port: 2 } };",
        );
        let loaded = load_config_from_file(&dir).unwrap().expect("config");
        assert_eq!(loaded.config.server.port, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_keys_warn() {
        let dir = temp_dir("unknown");
        write(
            &dir,
            "ferrite.config.js",
            "export default { test: { include: [] }, build: { sourcemap: 42 } };",
        );
        let loaded = load_config_from_file(&dir).unwrap().expect("config");
        assert_eq!(loaded.warnings.len(), 2, "{:?}", loaded.warnings);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
