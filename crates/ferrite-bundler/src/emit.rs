//! Chunk rendering and emission.

use crate::bundle::*;
use crate::chunks::*;
use crate::loader::*;
use ferrite_core::FerriteError;
use ferrite_core::Hash;
use ferrite_core::ModuleId;
use ferrite_core::ModuleType;
use ferrite_core::Result;
use ferrite_transform::rewrite_specifiers;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;

/// Rewrite import specifiers in rendered code (specifier → relative URL).
///
/// Operates on exact quoted-specifier matches produced by the loader's own
/// import list, so it cannot misfire on unrelated strings.
#[must_use]
/// CSS module ids in an entry closure (BFS in import order).
pub(crate) fn entry_css_files(
    entry: &ModuleId,
    modules: &HashMap<ModuleId, LoadedModule>,
    live: &HashSet<ModuleId>,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([entry.clone()]);
    let mut out = Vec::new();
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let Some(module) = modules.get(&id) else {
            continue;
        };
        if !live.contains(&id) {
            continue;
        }
        if module.css.is_some() {
            out.push(id.0.clone());
        }
        for (_, dep, _) in &module.imports {
            queue.push_back(dep.clone());
        }
    }
    out
}

/// Emit one CSS module's file, dependencies first (post-order).
///
/// Relative `@import`s rewrite to `./<dep-basename>` (all CSS files share
/// the flat `assets/` dir); remote `@import`s stay untouched. Circular
/// imports are a loud build error.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_css_module(
    id: &ModuleId,
    modules: &HashMap<ModuleId, LoadedModule>,
    live: &HashSet<ModuleId>,
    config: &BuildBundleConfig,
    file_names: &mut HashMap<String, String>,
    texts: &mut HashMap<String, String>,
    done: &mut HashSet<String>,
    stack: &mut Vec<String>,
) -> Result<()> {
    if done.contains(&id.0) {
        return Ok(());
    }
    if stack.contains(&id.0) {
        stack.push(id.0.clone());
        return Err(FerriteError::Build(format!(
            "circular CSS @import: {}",
            stack.join(" -> ")
        )));
    }
    let module = modules
        .get(id)
        .ok_or_else(|| FerriteError::Build(format!("missing CSS module `{id}` after traversal")))?;
    let Some(css) = module.css.as_ref() else {
        return Ok(());
    };
    stack.push(id.0.clone());
    let mut deps: Vec<(String, ModuleId)> = module
        .imports
        .iter()
        .filter(|(_, dep, _)| {
            live.contains(dep) && modules.get(dep).and_then(|dep| dep.css.as_ref()).is_some()
        })
        .map(|(spec, dep, _)| (spec.clone(), dep.clone()))
        .collect();
    deps.sort_by(|a, b| a.1 .0.cmp(&b.1 .0));
    for (_, dep) in &deps {
        emit_css_module(dep, modules, live, config, file_names, texts, done, stack)?;
    }
    let mut mapping: HashMap<String, String> = HashMap::new();
    for (spec, dep) in &deps {
        if let Some(dep_file) = file_names.get(&dep.0) {
            let base = dep_file.rsplit('/').next().unwrap_or(dep_file);
            mapping.insert(spec.clone(), format!("./{base}"));
        }
    }
    let final_text = ferrite_css::rewrite_css_imports(&css.text, |spec| mapping.get(spec).cloned());
    let hash = Hash::of_str(&final_text).short(8);
    let name = config
        .css_pattern
        .replace("[name]", &chunk_name(id))
        .replace("[hash]", &hash);
    file_names.insert(id.0.clone(), name);
    texts.insert(id.0.clone(), final_text);
    stack.pop();
    done.insert(id.0.clone());
    Ok(())
}

/// Drop bare imports (`import "<spec>";`) for extracted CSS.
///
/// Matches minified and pretty code (`import"…"`, optional whitespace /
/// semicolon). Binding imports (`import x from …`), dynamic imports, and
/// lookalike string contents (whose quotes are escaped) are never touched.
pub(crate) fn strip_bare_imports(code: &str, specs: &[&str]) -> String {
    if specs.is_empty() {
        return code.to_string();
    }
    let mut out = String::with_capacity(code.len());
    let mut cursor = 0;
    while cursor < code.len() {
        if let Some(end) = code
            .get(cursor..)
            .and_then(|rest| match_bare_import(rest, specs))
        {
            cursor += end;
        } else if let Some(next) = code[cursor..].chars().next() {
            out.push(next);
            cursor += next.len_utf8();
        } else {
            break;
        }
    }
    out
}

/// Length of the bare-import statement at `text` start, if any.
pub(crate) fn match_bare_import(text: &str, specs: &[&str]) -> Option<usize> {
    let after_import = text.strip_prefix("import")?;
    // `import(`, `import x`, `import*`… are not bare imports.
    if after_import.chars().next().is_some_and(|char| {
        char.is_alphanumeric() || char == '_' || char == '$' || char == '*' || char == '('
    }) {
        return None;
    }
    let trimmed = after_import.trim_start();
    let quote = trimmed.chars().next()?;
    if !matches!(quote, '"' | '\'' | '`') {
        return None;
    }
    for spec in specs {
        let mut candidate = String::with_capacity(spec.len() + 2);
        candidate.push(quote);
        candidate.push_str(spec);
        candidate.push(quote);
        if let Some(after_spec) = trimmed.strip_prefix(&candidate) {
            let tail = after_spec.trim_start();
            let semicolon = usize::from(tail.starts_with(';'));
            return Some(text.len() - tail[semicolon..].len());
        }
    }
    None
}

/// Rewrite import specifiers in `code` using parsed import ranges.
///
/// Only real import specifiers are replaced: identical text inside plain
/// strings or comments is left alone. Falls back to quoted-text replacement
/// when `code` does not parse.
pub fn rewrite_imports_text(code: &str, mapping: &HashMap<String, String>) -> String {
    if mapping.is_empty() {
        return code.to_string();
    }
    match rewrite_specifiers(code, &ModuleType::Js, mapping) {
        Ok((rewritten, _)) => rewritten,
        Err(_) => blind_rewrite_imports_text(code, mapping),
    }
}

/// Quoted-text fallback for unparseable inputs.
pub(crate) fn blind_rewrite_imports_text(code: &str, mapping: &HashMap<String, String>) -> String {
    let mut output = code.to_string();
    let mut pairs: Vec<(&String, &String)> = mapping.iter().collect();
    pairs.sort_by_key(|(spec, _)| std::cmp::Reverse(spec.len()));
    for (specifier, replacement) in pairs {
        for quote in ['"', '\'', '`'] {
            let from = format!("{quote}{specifier}{quote}");
            let to = format!("{quote}{replacement}{quote}");
            output = output.replace(&from, &to);
        }
    }
    output
}
