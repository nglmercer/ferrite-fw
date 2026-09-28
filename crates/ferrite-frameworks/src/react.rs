//! React Refresh plugin (§69).
//!
//! Dev-only: serves the `react-refresh` preamble as a virtual module,
//! injects it into `index.html` (only when the `react-refresh` package
//! resolves — non-React projects get no injection), and appends a
//! Refresh registration + HMR-boundary footer to JSX modules.
//! Production builds are untouched (JSX is already compiled by the
//! core pipeline).

use std::collections::HashMap;
use std::sync::Mutex;

use ferrite_config::ResolvedConfig;
use ferrite_core::{ModuleType, Result};
use ferrite_plugin::{
    HookFilter, HtmlInjectTo, HtmlTag, HtmlTransformContext, HtmlTransformResult, LoadRequest,
    LoadResult, Plugin, PluginContext, ResolveHookRequest, TransformRequest, TransformResult,
};

/// Preamble specifier (imported by the HTML shell and every footer).
pub const REFRESH_SPEC: &str = "/@react-refresh";
/// Preamble virtual id.
pub const REFRESH_VIRTUAL: &str = "\0react-refresh";
/// Refresh runtime the preamble and footers import (single source of
/// truth: preamble code, footer code, and the injection gate below).
pub const REFRESH_RUNTIME_SPEC: &str = "react-refresh/runtime";

/// React Refresh plugin (`ferrite:react-refresh`).
#[derive(Debug, Default)]
pub struct ReactPlugin {
    /// False in production or when `react.refresh = false`.
    enabled: Mutex<bool>,
}

impl ReactPlugin {
    /// Create with refresh enabled (corrected by `config_resolved`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            enabled: Mutex::new(true),
        }
    }

    /// Create with an explicit enabled flag (tests).
    #[must_use]
    pub fn with_enabled(enabled: bool) -> Self {
        Self {
            enabled: Mutex::new(enabled),
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled.lock().map(|flag| *flag).unwrap_or(false)
    }
}

/// Preamble: install the Refresh runtime globals. Requires the
/// `react-refresh` npm package (resolved loudly when missing).
#[must_use]
pub fn preamble_code() -> String {
    format!(
        "import RefreshRuntime from {runtime:?};\n\
         RefreshRuntime.injectIntoGlobalHook(window);\n\
         window.$RefreshReg$ = (type, id) => RefreshRuntime.register(type, {spec:?} + \" \" + id);\n\
         window.$RefreshSig$ = RefreshRuntime.createSignatureFunctionForTransform;\n\
         window.__ferrite_react_preamble_installed__ = true;\n",
        spec = REFRESH_SPEC,
        runtime = REFRESH_RUNTIME_SPEC,
    )
}

/// One refresh registration: (`local binding`, `debug id`).
pub type Registration = (String, String);

/// Detect refreshable components in transformed JSX output: capitalized
/// named-export locals plus named default exports. Anonymous default
/// components are skipped (documented heuristic gap).
#[must_use]
pub fn detect_components(id: &str, code: &str) -> Vec<Registration> {
    let parsed = ferrite_transform::compiler_for_engine("oxc").and_then(|compiler| {
        compiler.parse(ferrite_transform::ParseRequest {
            id: id.to_string(),
            code: code.to_string(),
            module_type: ModuleType::Js,
        })
    });
    let Ok(parsed) = parsed else {
        return Vec::new();
    };
    let mut registrations = Vec::new();
    for export in &parsed.export_details {
        if export.from.is_some() {
            continue;
        }
        if export.exported == "default" {
            if let Some(local) = export.local.as_deref() {
                registrations.push((local.to_string(), format!("{id} %default%")));
            }
            continue;
        }
        let capitalized = export
            .exported
            .chars()
            .next()
            .is_some_and(|c| c.is_uppercase());
        if capitalized {
            if let Some(local) = export.local.as_deref() {
                registrations.push((local.to_string(), format!("{id} {}", export.exported)));
            }
        }
    }
    registrations
}

/// Refresh footer: preamble import, registrations, HMR boundary.
/// Uses the `globalThis` hot factory because plugin-appended code runs
/// after `import.meta.hot` detection.
#[must_use]
pub fn refresh_footer(id: &str, registrations: &[Registration]) -> String {
    let mut footer = format!(
        "\nimport {spec:?};\nimport RefreshRuntime from {runtime:?};\n",
        spec = REFRESH_SPEC,
        runtime = REFRESH_RUNTIME_SPEC,
    );
    for (local, debug_id) in registrations {
        footer.push_str(&format!("$RefreshReg$({local}, {debug_id:?});\n"));
    }
    footer.push_str(&format!(
        "if (import.meta.hot || globalThis.__ferrite_create_hot__) {{\n\
         const __ferrite_hot__ = globalThis.__ferrite_create_hot__({id:?});\n\
         __ferrite_hot__.accept((next) => {{\n\
         if (next == null) return;\n\
         if (!RefreshRuntime.isLikelyComponentModule(next)) {{\n\
         __ferrite_hot__.invalidate();\n\
         return;\n\
         }}\n\
         RefreshRuntime.performReactRefresh();\n\
         }});\n\
         }}\n",
    ));
    footer
}

/// True for ids the plugin transforms (`.jsx`/`.tsx`, query allowed).
#[must_use]
pub fn is_jsx_id(id: &str) -> bool {
    let path = id.split('?').next().unwrap_or(id);
    path.ends_with(".jsx") || path.ends_with(".tsx")
}

#[async_trait::async_trait]
impl Plugin for ReactPlugin {
    fn name(&self) -> &'static str {
        "ferrite:react-refresh"
    }

    fn transform_filter(&self) -> Option<HookFilter> {
        Some(HookFilter {
            id: Some("\\.[jt]sx($|\\?)".to_string()),
            code: None,
            query: None,
        })
    }

    async fn config_resolved(&self, config: &ResolvedConfig) -> Result<()> {
        if let Ok(mut enabled) = self.enabled.lock() {
            *enabled = config.react.refresh && !config.is_production;
        }
        Ok(())
    }

    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        request: ResolveHookRequest<'_>,
    ) -> Result<Option<ferrite_resolver::ResolvedId>> {
        if request.specifier == REFRESH_SPEC {
            return Ok(Some(ferrite_resolver::ResolvedId::new(REFRESH_VIRTUAL)));
        }
        Ok(None)
    }

    async fn load(&self, _ctx: &PluginContext, request: LoadRequest) -> Result<Option<LoadResult>> {
        if request.id == REFRESH_VIRTUAL {
            return Ok(Some(LoadResult {
                code: preamble_code(),
                module_type: ModuleType::Js,
                dependencies: Vec::new(),
            }));
        }
        Ok(None)
    }

    async fn transform(
        &self,
        _ctx: &PluginContext,
        request: TransformRequest,
    ) -> Result<Option<TransformResult>> {
        if !self.is_enabled() || !is_jsx_id(&request.id) {
            return Ok(None);
        }
        // Only files that actually used JSX (post-core output references
        // the runtime); plain `.ts`-in-`.tsx` files pass through.
        if !request.code.contains("react/jsx-") && !request.code.contains("React.createElement") {
            return Ok(None);
        }
        let registrations = detect_components(&request.id, &request.code);
        if registrations.is_empty() {
            return Ok(None);
        }
        Ok(Some(TransformResult {
            code: format!(
                "{}{}",
                request.code,
                refresh_footer(&request.id, &registrations)
            ),
            map: None,
            dependencies: Vec::new(),
        }))
    }

    async fn transform_index_html(
        &self,
        ctx: &PluginContext,
        _html: HtmlTransformContext,
    ) -> Result<Option<HtmlTransformResult>> {
        if !self.is_enabled() {
            return Ok(None);
        }
        // No refresh runtime installed → nothing to inject. Skipping keeps
        // non-React projects clean; when refresh output IS produced (JSX
        // footers) but the runtime is missing, the pipeline's bare-import
        // resolution fails loudly with a `ferrite add` hint instead.
        if ctx.resolve(REFRESH_RUNTIME_SPEC, None).is_err() {
            return Ok(None);
        }
        Ok(Some(HtmlTransformResult {
            html: None,
            tags: vec![HtmlTag {
                tag: "script".to_string(),
                attrs: HashMap::from([("type".to_string(), "module".to_string())]),
                children: Some(format!("import {REFRESH_SPEC:?};\n")),
                inject_to: HtmlInjectTo::HeadPrepend,
            }],
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_capitalized_and_default_components() {
        let code = "import { jsx as _jsx } from \"react/jsx-runtime\";\n\
            export function App() { return _jsx(\"div\", {}); }\n\
            export const helper = 1;\n\
            export default function Page() { return _jsx(\"p\", {}); }\n";
        let registrations = detect_components("/src/App.tsx", code);
        let locals: Vec<&str> = registrations
            .iter()
            .map(|(local, _)| local.as_str())
            .collect();
        assert!(locals.contains(&"App"), "{locals:?}");
        assert!(locals.contains(&"Page"), "{locals:?}");
        assert!(!locals.contains(&"helper"), "{locals:?}");
    }

    #[test]
    fn skips_anonymous_default_and_reexports() {
        let code = "import { jsx as _jsx } from \"react/jsx-runtime\";\n\
            export default () => _jsx(\"div\", {});\n\
            export { x } from \"./other\";\n";
        assert!(detect_components("/src/anon.tsx", code).is_empty());
    }

    #[test]
    fn footer_registers_and_accepts() {
        let footer = refresh_footer(
            "/src/App.tsx",
            &[("App".to_string(), "/src/App.tsx App".to_string())],
        );
        assert!(footer.contains("\"/@react-refresh\""), "{footer}");
        assert!(
            footer.contains("$RefreshReg$(App, \"/src/App.tsx App\")"),
            "{footer}"
        );
        assert!(
            footer.contains("__ferrite_create_hot__(\"/src/App.tsx\")"),
            "{footer}"
        );
        assert!(footer.contains("performReactRefresh"), "{footer}");
        assert!(footer.contains("isLikelyComponentModule"), "{footer}");
    }

    #[test]
    fn preamble_installs_runtime() {
        let preamble = preamble_code();
        assert!(preamble.contains("react-refresh/runtime"), "{preamble}");
        assert!(preamble.contains("injectIntoGlobalHook"), "{preamble}");
        assert!(preamble.contains("$RefreshReg$"), "{preamble}");
    }

    #[test]
    fn jsx_ids_match() {
        assert!(is_jsx_id("/src/App.tsx"));
        assert!(is_jsx_id("/src/App.tsx?t=1"));
        assert!(is_jsx_id("/src/a.jsx"));
        assert!(!is_jsx_id("/src/a.ts"));
        assert!(!is_jsx_id("/src/a.js"));
    }
}
