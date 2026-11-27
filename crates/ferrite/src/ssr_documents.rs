//! Versioned, source-free route document metadata and shared host selection.
use crate::{FerriteError, Result, SsrRendererArtifact};
use ferrite_runtime::{CompiledModuleGraph, JsRuntime};
use ferrite_ssr::{FnAdapter, SsrAdapter, SsrHttpRequest, SsrResponse};
use std::{collections::HashSet, path::Path, sync::Arc};
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SsrDocumentArtifact {
    /// None is the fallback; otherwise a validated base-relative route pattern.
    pub route: Option<String>,
    pub graph: CompiledModuleGraph,
    pub stylesheets: Vec<String>,
    /// Transformed client HTML. None uses the host's default shell.
    pub shell: Option<String>,
}
pub(crate) fn validate_documents(artifact: &SsrRendererArtifact) -> Result<()> {
    if artifact.documents.is_empty() {
        return Ok(());
    }
    if artifact.version != 2 {
        return Err(FerriteError::Ssr(
            "route documents require artifact version 2".into(),
        ));
    }
    let mut seen = HashSet::new();
    for doc in &artifact.documents {
        if !seen.insert(&doc.route) {
            return Err(FerriteError::Ssr(
                "duplicate document route/fallback".into(),
            ));
        }
        let routes = ferrite_config::SsrConfig {
            routes: doc.route.as_ref().map(|route| vec![route.clone()]),
            ..Default::default()
        };
        routes.validate_routes()?;
        if doc
            .shell
            .as_ref()
            .is_some_and(|shell| shell.matches("<!--ssr-outlet-->").count() != 1)
        {
            return Err(FerriteError::Ssr(
                "document shell requires exactly one ssr-outlet".into(),
            ));
        }
        SsrRendererArtifact {
            version: 1,
            documents: vec![],
            graph: doc.graph.clone(),
            stylesheets: doc.stylesheets.clone(),
        }
        .validate()?;
    }
    Ok(())
}
pub(crate) fn document_adapter(
    documents: Vec<SsrDocumentArtifact>,
    runtime: Arc<dyn JsRuntime>,
    default_shell: String,
    base: &str,
    streaming: bool,
) -> Result<Arc<dyn SsrAdapter>> {
    let mut adapters = Vec::new();
    for doc in documents {
        let shell = doc.shell.unwrap_or_else(|| default_shell.clone());
        if shell.matches("<!--ssr-outlet-->").count() != 1 {
            return Err(FerriteError::Ssr(
                "document shell requires exactly one ssr-outlet".into(),
            ));
        }
        let single = SsrRendererArtifact {
            version: 1,
            documents: vec![],
            graph: doc.graph,
            stylesheets: doc.stylesheets,
        };
        adapters.push((
            doc.route,
            single.into_streaming_adapter(runtime.clone(), shell, base, streaming)?,
        ));
    }
    let base = crate::loader::with_trailing_slash(base);
    Ok(Arc::new(FnAdapter::new(
        move |request: SsrHttpRequest, context| {
            let path = request.uri.split('?').next().unwrap_or(&request.uri);
            let relative = path.strip_prefix(&base).map(|path| format!("/{path}"));
            let selected = relative
                .as_ref()
                .and_then(|path| {
                    adapters
                        .iter()
                        .filter(|(route, _)| {
                            route
                                .as_ref()
                                .is_some_and(|route| ferrite_config::ssr_route_matches(route, path))
                        })
                        .min_by_key(|(route, _)| {
                            ferrite_config::ssr_route_priority(route.as_deref().unwrap())
                        })
                        .or_else(|| adapters.iter().find(|(route, _)| route.is_none()))
                })
                .map(|(_, adapter)| adapter.clone());
            async move {
                if let Some(adapter) = selected {
                    adapter.render(request, context).await
                } else {
                    let mut response = SsrResponse::html("Not Found");
                    response.status = 404;
                    Ok(response)
                }
            }
        },
    )))
}
pub(crate) fn write_documents(
    artifact: &mut SsrRendererArtifact,
    server_dir: &Path,
    config: &ferrite_config::ResolvedConfig,
) -> Result<()> {
    if config.ssr.entries.is_empty() && config.ssr.shell.is_none() {
        return Ok(());
    }
    let manifest = ferrite_manifest::BuildManifest::read(&server_dir.join("manifest.json"))?;
    let mut fallback = config.clone();
    fallback.ssr.entries.clear();
    let default = ferrite_config::resolve_js_server_entry(&fallback)?;
    let entries: Vec<_> = config
        .ssr
        .entries
        .iter()
        .map(|entry| {
            (
                Some(entry.route.clone()),
                entry.entry.clone(),
                entry.shell.as_ref().or(config.ssr.shell.as_ref()),
            )
        })
        .chain(default.map(|entry| (None, entry, config.ssr.shell.as_ref())))
        .collect();
    for (route, source, shell) in entries {
        let entry = if config.ssr.entries.is_empty() {
            manifest
                .entries
                .values()
                .find(|entry| entry.is_entry == Some(true))
        } else {
            manifest.entries.get(&format!("/{source}"))
        }
        .ok_or_else(|| FerriteError::Ssr(format!("missing emitted document entry `{source}`")))?;
        let shell = shell
            .map(|path| std::fs::read_to_string(config.out_dir().join(path)))
            .transpose()?;
        if shell
            .as_ref()
            .map_or_else(
                || std::fs::read_to_string(config.out_dir().join("index.html")),
                |shell| Ok(shell.clone()),
            )?
            .matches("<!--ssr-outlet-->")
            .count()
            != 1
        {
            return Err(FerriteError::Ssr(
                "document shell requires exactly one ssr-outlet".into(),
            ));
        }

        let mut graph = crate::ssr_build::load_emitted_ssr_entry(server_dir, &entry.file)?;
        for module in &mut graph.modules {
            module.url = Some(module.id.clone());
        }
        artifact.documents.push(SsrDocumentArtifact {
            route,
            graph,
            stylesheets: entry.css.clone(),
            shell,
        });
    }
    Ok(())
}
