//! Compiler backends.

use crate::minify::*;
use crate::parse::*;
use crate::transform::*;
use crate::types::*;
use ferrite_core::FerriteError;
use ferrite_core::Result;
use std::sync::Arc;

/// Compiler abstraction (spec §5).
pub trait JsCompiler: Send + Sync {
    /// Parse a module and extract imports/exports.
    fn parse(&self, request: ParseRequest) -> Result<ParsedModule>;
    /// Transform a module (TS strip, JSX, lowering, defines).
    fn transform(&self, request: TransformRequest) -> Result<TransformResult>;
    /// Minify a module.
    fn minify(&self, request: MinifyRequest) -> Result<MinifyResult>;
    /// Compiler version string for cache keys.
    fn version(&self) -> &'static str {
        "custom-0"
    }
}

/// Compiler engine selection (§5).
#[derive(Clone)]
pub enum CompilerEngine {
    /// Oxc backend (default).
    Oxc,
    /// SWC backend (compatibility; see [`SwcCompiler`]).
    Swc,
    /// Custom compiler implementation.
    Custom(Arc<dyn JsCompiler>),
}

/// Build the compiler for an engine name (`oxc` / `swc`).
pub fn compiler_for_engine(engine: &str) -> Result<Arc<dyn JsCompiler>> {
    match engine.to_ascii_lowercase().as_str() {
        "oxc" => Ok(Arc::new(OxcCompiler::new(OxcOptions::default()))),
        "swc" => Ok(Arc::new(SwcCompiler)),
        other => Err(FerriteError::Other(format!(
            "unknown compiler engine `{other}`"
        ))),
    }
}

/// Oxc backend options (§90).
#[derive(Debug, Clone)]
pub struct OxcOptions {
    /// Target string recorded in cache keys (env lowering roadmap).
    pub target: String,
}

impl Default for OxcOptions {
    fn default() -> Self {
        Self {
            target: "es2022".to_string(),
        }
    }
}

/// Oxc-backed compiler: parser + transformer + minifier (§90).
#[derive(Debug, Clone)]
pub struct OxcCompiler {
    /// Backend options.
    pub options: OxcOptions,
}

impl OxcCompiler {
    /// Create an Oxc compiler.
    #[must_use]
    pub fn new(options: OxcOptions) -> Self {
        Self { options }
    }
}

impl JsCompiler for OxcCompiler {
    fn parse(&self, request: ParseRequest) -> Result<ParsedModule> {
        parse_module(&request.id, &request.code, &request.module_type)
    }

    fn transform(&self, request: TransformRequest) -> Result<TransformResult> {
        transform_module(request)
    }

    fn minify(&self, request: MinifyRequest) -> Result<MinifyResult> {
        minify_module(&request)
    }

    fn version(&self) -> &'static str {
        "oxc-0.151"
    }
}

/// SWC compatibility backend (§89).
///
/// Native SWC pipeline (TS strip, JSX, target lowering, minify) behind the
/// `swc` cargo feature; without it, transform/minify fail loudly and
/// parsing still works through the shared Oxc frontend.
#[derive(Debug, Clone, Copy, Default)]
pub struct SwcCompiler;

impl JsCompiler for SwcCompiler {
    fn parse(&self, request: ParseRequest) -> Result<ParsedModule> {
        // Parsing is engine-agnostic through the Oxc frontend so both
        // engines agree on the module graph.
        OxcCompiler::new(OxcOptions::default()).parse(request)
    }

    fn transform(&self, request: TransformRequest) -> Result<TransformResult> {
        #[cfg(feature = "swc")]
        {
            crate::swc_impl::transform_module_swc(request)
        }
        #[cfg(not(feature = "swc"))]
        {
            let _ = request;
            Err(FerriteError::Other(
                "the SWC transform backend is not compiled into this build; rebuild with `--features swc` or set `[compiler] engine = \"oxc\"` (default)".to_string(),
            ))
        }
    }

    fn minify(&self, request: MinifyRequest) -> Result<MinifyResult> {
        #[cfg(feature = "swc")]
        {
            crate::swc_impl::minify_module_swc(&request)
        }
        #[cfg(not(feature = "swc"))]
        {
            let _ = request;
            Err(FerriteError::Other(
                "the SWC minify backend is not compiled into this build; rebuild with `--features swc` or set `[compiler] engine = \"oxc\"` (default)".to_string(),
            ))
        }
    }

    fn version(&self) -> &'static str {
        // Tracks the workspace `swc_core` major version.
        "swc-81"
    }
}
