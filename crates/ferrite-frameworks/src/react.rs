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
/// named-export locals plus default exports. Anonymous defaults are addressed
/// through the module namespace without rewriting their declarations.
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
            let local = export
                .local
                .as_deref()
                .unwrap_or("__ferrite_refresh_exports__.default");
            registrations.push((local.to_string(), format!("{id} %default%")));
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

/// React-owned hook modules require a direct React import and a locally
/// declared exported hook name. Re-exports alone do not establish ownership.
fn is_react_hook_module(id: &str, code: &str) -> Result<bool> {
    if id.starts_with("/@npm/") || id.starts_with('\0') {
        return Ok(false);
    }
    let parsed =
        ferrite_transform::compiler_for_engine("oxc")?.parse(ferrite_transform::ParseRequest {
            id: id.into(),
            code: code.into(),
            module_type: ModuleType::Js,
        })?;
    Ok(parsed
        .imports
        .iter()
        .any(|import| import.specifier == "react")
        && parsed.export_details.iter().any(|export| {
            export.from.is_none()
                && export.local.as_deref().is_some_and(|name| {
                    name.strip_prefix("use")
                        .and_then(|suffix| suffix.chars().next())
                        .is_some_and(|first| first.is_ascii_uppercase() || first.is_ascii_digit())
                })
        }))
}

/// Refresh footer: preamble import, registrations, HMR boundary.
/// Uses the `globalThis` hot factory because plugin-appended code runs
/// after `import.meta.hot` detection.
#[must_use]
pub fn refresh_footer(id: &str, registrations: &[Registration]) -> String {
    let mut footer = format!(
        "\nimport {spec:?};\nimport RefreshRuntime from {runtime:?};\nimport * as __ferrite_refresh_exports__ from {id:?};\n",
        spec = REFRESH_SPEC,
        runtime = REFRESH_RUNTIME_SPEC,
    );
    for (local, debug_id) in registrations {
        footer.push_str(&format!("$RefreshReg$({local}, {debug_id:?});\n"));
    }
    footer.push_str(&format!(
        "if (import.meta.hot || globalThis.__ferrite_create_hot__) {{\n\
         const __ferrite_hot__ = import.meta.hot || globalThis.__ferrite_create_hot__({id:?});\n\
         (import.meta.hot || __ferrite_hot__).accept((next) => {{\n\
         if (next == null) return;\n\
         let valid = false;\n\
         try {{\n\
         const previous = __ferrite_refresh_exports__;\n\
         const keys = Object.keys(previous);\n\
         const nextKeys = Object.keys(next);\n\
         let components = 0;\n\
         valid = keys.length === nextKeys.length && keys.every((key) => {{\n\
         if (!Object.prototype.hasOwnProperty.call(next, key)) return false;\n\
         if (RefreshRuntime.isLikelyComponentType(previous[key]) && RefreshRuntime.isLikelyComponentType(next[key])) {{ components++; return true; }}\n\
         return previous[key] === next[key];\n\
         }}) && components > 0;\n\
         }} catch {{ valid = false; }}\n\
         if (!valid) {{\n\
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

    fn cache_key(&self) -> String {
        format!(
            "{}:oxc-0.151.0-imported-hooks-v3:{}",
            self.name(),
            self.is_enabled()
        )
    }

    fn transform_filter(&self) -> Option<HookFilter> {
        Some(HookFilter {
            id: Some("\\.[cm]?[jt]sx?($|\\?)".to_string()),
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
        ctx: &PluginContext,
        request: ResolveHookRequest<'_>,
    ) -> Result<Option<ferrite_resolver::ResolvedId>> {
        if request.specifier == REFRESH_SPEC || request.specifier == REFRESH_VIRTUAL {
            if request.ssr
                || request.environment.is_ssr()
                || ctx.environment.kind.is_ssr()
                || !self.is_enabled()
            {
                return Err(ferrite_core::FerriteError::Build("React Refresh is available only in enabled client development; remove its import from server/production entries".into()));
            }
            return Ok(Some(ferrite_resolver::ResolvedId::new(REFRESH_VIRTUAL)));
        }
        Ok(None)
    }

    async fn load(&self, ctx: &PluginContext, request: LoadRequest) -> Result<Option<LoadResult>> {
        if request.id == REFRESH_VIRTUAL {
            if request.environment.is_ssr() || ctx.environment.kind.is_ssr() || !self.is_enabled() {
                return Err(ferrite_core::FerriteError::Build(
                    "React Refresh virtual module cannot load outside enabled client development"
                        .into(),
                ));
            }
            return Ok(Some(LoadResult {
                code: preamble_code(),
                module_type: ModuleType::Js,
                dependencies: Vec::new(),

                map: None,
                side_effects: None,
            }));
        }
        Ok(None)
    }

    async fn transform(
        &self,
        ctx: &PluginContext,
        request: TransformRequest,
    ) -> Result<Option<TransformResult>> {
        if !self.is_enabled()
            || request.ssr
            || request.environment.is_ssr()
            || ctx.environment.kind.is_ssr()
        {
            return Ok(None);
        }
        let jsx = is_jsx_id(&request.id)
            && (request.code.contains("react/jsx-")
                || request.code.contains("React.createElement"));
        let hook_module =
            request.module_type.is_js_like() && is_react_hook_module(&request.id, &request.code)?;
        if !jsx && !hook_module {
            return Ok(None);
        }
        let registrations = if jsx {
            detect_components(&request.id, &request.code)
        } else {
            Vec::new()
        };
        if registrations.is_empty() && !hook_module {
            // Mounting entries install the hook before react-dom evaluates,
            // without becoming component boundaries themselves.
            let (code, map) = ferrite_transform::apply_text_edits(
                &request.id,
                &request.code,
                &[(0, 0, format!("import {REFRESH_SPEC:?};\n"))],
                true,
            )?;
            return Ok(Some(TransformResult {
                code,
                map: map.map(ferrite_core::SourceMap::external),
                dependencies: Vec::new(),
                module_type: None,
            }));
        }
        // Keep compiler-generated registration/signature identifiers module-local,
        // so component names in different files cannot share a Refresh family.
        let mut reg = "__ferrite_refresh_reg__".to_string();
        while request.code.contains(&reg) {
            reg.push('_');
        }
        let mut sig = "__ferrite_refresh_sig__".to_string();
        while request.code.contains(&sig) {
            sig.push('_');
        }
        let (instrumented, instrument_map) =
            ferrite_transform::instrument_react_refresh(&request.id, &request.code, &reg, &sig)?;
        let mut runtime_binding = "__ferrite_refresh_runtime__".to_string();
        while request.code.contains(&runtime_binding) {
            runtime_binding.push('_');
        }
        let prefix = format!("import {spec:?};\nimport {runtime_binding} from {runtime:?};\nconst {reg} = (type, key) => {runtime_binding}.register(type, {id:?} + ' ' + key);\nconst {sig} = {runtime_binding}.createSignatureFunctionForTransform;\n", spec=REFRESH_SPEC, runtime=REFRESH_RUNTIME_SPEC, id=request.id);
        let footer = if registrations.is_empty() {
            String::new()
        } else {
            refresh_footer(&request.id, &registrations)
        };
        let (code, map) = ferrite_transform::apply_text_edits(
            &request.id,
            &instrumented,
            &[
                (0, 0, prefix),
                (instrumented.len(), instrumented.len(), footer),
            ],
            true,
        )?;
        let map = match (map, instrument_map) {
            (Some(outer), Some(inner)) => Some(ferrite_transform::chain_source_maps(
                &outer,
                &inner.mappings,
            )?),
            (map, _) => map,
        };
        Ok(Some(TransformResult {
            code,
            map: map.map(ferrite_core::SourceMap::external),
            dependencies: Vec::new(),
            module_type: None,
        }))
    }

    async fn transform_index_html(
        &self,
        ctx: &PluginContext,
        html: HtmlTransformContext,
    ) -> Result<Option<HtmlTransformResult>> {
        if !self.is_enabled() || html.ssr || ctx.environment.kind.is_ssr() {
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

    #[tokio::test]
    async fn refresh_is_excluded_from_server_and_disabled_profiles() {
        use ferrite_core::{Environment, EnvironmentKind};
        let root = tempfile::tempdir().unwrap();
        let resolver = ferrite_resolver::Resolver::new(root.path().into(), &Default::default());
        let graph = Default::default();
        let emitted = Mutex::new(HashMap::new());
        let watches = Mutex::new(Vec::new());
        let warnings = Mutex::new(Vec::new());
        let plugin = ReactPlugin::new();
        for (context_kind, request_kind, ssr) in [
            (EnvironmentKind::Client, EnvironmentKind::Client, true),
            (EnvironmentKind::Client, EnvironmentKind::Ssr, false),
            (EnvironmentKind::Ssr, EnvironmentKind::Client, false),
        ] {
            let environment = Environment::new("fixture", context_kind);
            let ctx = PluginContext {
                graph: &graph,
                resolver: &resolver,
                environment: &environment,
                emitted: &emitted,
                watch_files: &watches,
                warnings: &warnings,
            };
            assert!(plugin
                .transform(
                    &ctx,
                    TransformRequest {
                        id: "/App.jsx".into(),
                        code: "export function App() { return React.createElement('div'); }".into(),
                        module_type: ModuleType::Js,
                        environment: request_kind.clone(),
                        ssr
                    }
                )
                .await
                .unwrap()
                .is_none());
            let error = plugin
                .resolve_id(
                    &ctx,
                    ResolveHookRequest {
                        kind: ferrite_resolver::ResolveKind::Import,
                        specifier: REFRESH_SPEC,
                        importer: None,
                        environment: request_kind,
                        ssr,
                    },
                )
                .await
                .unwrap_err();
            assert!(error.to_string().contains("client development"));
        }
        let environment = Environment::new("client", EnvironmentKind::Client);
        let ctx = PluginContext {
            graph: &graph,
            resolver: &resolver,
            environment: &environment,
            emitted: &emitted,
            watch_files: &watches,
            warnings: &warnings,
        };
        let hook_source =
            "import {useState} from 'react'; export function useCounter() { return useState(0); }";
        let hook = plugin
            .transform(
                &ctx,
                TransformRequest {
                    id: "/hooks.ts".into(),
                    code: hook_source.into(),
                    module_type: ModuleType::Js,
                    environment: EnvironmentKind::Client,
                    ssr: false,
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert!(hook.code.contains("__ferrite_refresh_sig__()"));
        assert!(
            !hook.code.contains(".accept("),
            "hook modules cannot form component boundaries"
        );
        assert!(is_react_hook_module("/hooks.js", hook_source).unwrap());
        assert!(!is_react_hook_module("/@npm/other/hooks.js", hook_source).unwrap());
        assert!(
            !is_react_hook_module("/other.js", "export function useCounter() {return 0;}").unwrap()
        );
        assert!(!is_react_hook_module(
            "/barrel.js",
            "import 'react'; export {useCounter} from './hooks.js';"
        )
        .unwrap());
        let entry = plugin.transform(&ctx, TransformRequest {
            id:"/main.jsx".into(), code:"import {createRoot} from 'react-dom/client'; createRoot(document.body).render(React.createElement(App));".into(),
            module_type:ModuleType::Js, environment:EnvironmentKind::Client, ssr:false,
        }).await.unwrap().unwrap();
        assert!(entry.code.starts_with("import \"/@react-refresh\";"));
        assert!(
            !entry.code.contains(".accept("),
            "entries without exports must not become Refresh boundaries"
        );
        let source = "const __ferrite_refresh_runtime__ = 1; import {useState} from 'react'; export function App() { const [count] = useState(0); return React.createElement('div', null, count); }";
        let instrumented = plugin
            .transform(
                &ctx,
                TransformRequest {
                    id: "/App.jsx".into(),
                    code: source.into(),
                    module_type: ModuleType::Js,
                    environment: EnvironmentKind::Client,
                    ssr: false,
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            instrumented.code.contains("__ferrite_refresh_sig__()"),
            "{}",
            instrumented.code
        );
        assert!(instrumented
            .code
            .contains("import __ferrite_refresh_runtime___ from"));
        ferrite_transform::compiler_for_engine("oxc")
            .unwrap()
            .parse(ferrite_transform::ParseRequest {
                id: "/App.jsx".into(),
                code: instrumented.code,
                module_type: ModuleType::Js,
            })
            .unwrap();
        let emitted_map = instrumented.map.unwrap();
        let map = oxc_sourcemap::SourceMap::from_json_string(&emitted_map.mappings).unwrap();
        assert!(map
            .get_source_contents()
            .any(|content| content == Some(source)));
        assert!(plugin
            .load(
                &ctx,
                LoadRequest {
                    id: REFRESH_VIRTUAL.into(),
                    environment: EnvironmentKind::Ssr
                }
            )
            .await
            .is_err());
        assert!(plugin
            .transform_index_html(
                &ctx,
                HtmlTransformContext {
                    html: "<html/>".into(),
                    path: "/".into(),
                    ssr: true
                }
            )
            .await
            .unwrap()
            .is_none());
        assert!(plugin
            .load(
                &ctx,
                LoadRequest {
                    id: REFRESH_VIRTUAL.into(),
                    environment: EnvironmentKind::Client
                }
            )
            .await
            .unwrap()
            .is_some());
        let client = plugin
            .transform(
                &ctx,
                TransformRequest {
                    id: "/App.jsx".into(),
                    code: "export function App() { return React.createElement('div'); }".into(),
                    module_type: ModuleType::Js,
                    environment: EnvironmentKind::Client,
                    ssr: false,
                },
            )
            .await
            .unwrap()
            .expect("enabled client transformation must still run");
        assert!(client.code.contains("$RefreshReg$(App"));
        for production in [false, true] {
            let mut user = ferrite_config::UserConfig::default();
            user.react.set_refresh(production);
            let config = ferrite_config::resolve_config(
                user,
                Some(root.path().into()),
                ferrite_config::CliOverrides {
                    mode: Some(
                        if production {
                            "production"
                        } else {
                            "development"
                        }
                        .into(),
                    ),
                    ..Default::default()
                },
            )
            .unwrap();
            plugin.config_resolved(&config).await.unwrap();
            assert!(plugin
                .load(
                    &ctx,
                    LoadRequest {
                        id: REFRESH_VIRTUAL.into(),
                        environment: EnvironmentKind::Client
                    }
                )
                .await
                .is_err());
            assert!(plugin
                .transform(
                    &ctx,
                    TransformRequest {
                        id: "/App.jsx".into(),
                        code: "export function App() { return React.createElement('div'); }".into(),
                        module_type: ModuleType::Js,
                        environment: EnvironmentKind::Client,
                        ssr: false
                    }
                )
                .await
                .unwrap()
                .is_none());
        }
    }

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
    fn registers_anonymous_default_without_registering_reexports() {
        let code = "import { jsx as _jsx } from \"react/jsx-runtime\";\n\
            export default () => _jsx(\"div\", {});\n\
            export { x } from \"./other\";\n";
        assert_eq!(
            detect_components("/src/anon.tsx", code),
            vec![(
                "__ferrite_refresh_exports__.default".into(),
                "/src/anon.tsx %default%".into()
            )]
        );
        for expression in ["function() {}", "class {}", "memo(() => null)"] {
            let code = format!("export default {expression};");
            assert_eq!(
                detect_components("/anonymous.jsx", &code),
                vec![(
                    "__ferrite_refresh_exports__.default".into(),
                    "/anonymous.jsx %default%".into()
                )],
                "{expression}"
            );
        }
    }

    #[test]
    fn cache_identity_tracks_registration_version_and_enablement() {
        let enabled = ReactPlugin::with_enabled(true).cache_key();
        assert!(enabled.contains("oxc-0.151.0-imported-hooks-v3"));
        assert_ne!(enabled, ReactPlugin::with_enabled(false).cache_key());
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
        assert!(footer.contains("isLikelyComponentType"), "{footer}");
        assert!(!footer.contains("isLikelyComponentModule"), "{footer}");
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
