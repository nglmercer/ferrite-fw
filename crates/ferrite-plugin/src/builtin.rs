//! Built-in plugins.

use crate::plugin::*;
use crate::types::*;
use ferrite_core::ModuleType;
use ferrite_core::Result;
use std::sync::Arc;

pub(crate) fn wrap(
    plugin: &Arc<dyn Plugin>,
    hook: &str,
    error: ferrite_core::FerriteError,
) -> ferrite_core::FerriteError {
    ferrite_core::FerriteError::Plugin {
        plugin: plugin.name().to_string(),
        hook: hook.to_string(),
        message: error.to_string(),
    }
}

/// Reference plugin from spec §68: serves `*.txt?raw` as ESM strings.
pub struct RawTextPlugin;

#[async_trait::async_trait]
impl Plugin for RawTextPlugin {
    fn name(&self) -> &'static str {
        "raw-text"
    }

    async fn load(&self, _ctx: &PluginContext, request: LoadRequest) -> Result<Option<LoadResult>> {
        if let Some(path) = request.id.strip_suffix("?raw") {
            // Only handle text-like files; binary assets use `?url`.
            if path.ends_with(".txt") || path.ends_with(".md") {
                return Ok(Some(LoadResult {
                    code: format!("export default {path:?};\n"),
                    module_type: ModuleType::Js,
                    dependencies: vec![path.to_string()],
                }));
            }
        }
        Ok(None)
    }
}
