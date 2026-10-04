//! Development SSR using the shared compiler graph and explicitly selected runtime.
use std::sync::Arc;

/// Create a renderer which revalidates its graph and styles on every request.
/// Graph replacement and invocation share one lock and persistent runtime worker.
/// The supplied shell should already have development HTML transforms applied.
/// A weak server handle allows server shutdown without an ownership cycle.
pub async fn create_dev_ssr_adapter(
    server: &crate::DevServer,
    resolved: &crate::ResolvedConfig,
    shell: &str,
) -> crate::Result<Arc<dyn ferrite_ssr::SsrAdapter>> {
    let entry = crate::config::resolve_js_server_entry(resolved)?
        .ok_or_else(|| crate::FerriteError::Ssr("no JavaScript/TypeScript server entry found; configure [ssr].entry or create src/entry-server.js".into()))?;
    let path = resolved.root.join(&entry);
    let graph = server
        .ssr_runtime_graph(&format!("/{}", entry.replace('\\', "/")))
        .await
        .map_err(|error| {
            crate::FerriteError::Ssr(format!(
                "cannot compile SSR graph {}: {error}",
                path.display()
            ))
        })?;
    let adapter = ferrite_ssr::JsSsrAdapter::from_resolved_graph(resolved, graph)?
        .with_shell(shell.to_string());
    let adapter = Arc::new(tokio::sync::Mutex::new(adapter));
    let server = Arc::new(server.weak_handle());
    let entry_url = format!("/{}", entry.replace('\\', "/"));
    Ok(Arc::new(ferrite_ssr::FnAdapter::new(
        move |request: ferrite_ssr::SsrHttpRequest, mut context: ferrite_ssr::SsrContext| {
            let server = server.clone();
            let adapter = adapter.clone();
            let entry_url = entry_url.clone();
            async move {
                let server = server()
                    .ok_or_else(|| crate::FerriteError::Ssr("SSR dev server has closed".into()))?;
                // Keep graph replacement, evaluation, and invocation together: a
                // persistent runtime must not interleave different request graphs.
                let mut adapter = adapter.lock().await;
                let (graph, styles) = server.ssr_runtime_graph_with_styles(&entry_url).await?;
                context.preload.extend(styles);
                adapter.replace_graph(graph)?;
                ferrite_ssr::SsrAdapter::render(&*adapter, request, context).await
            }
        },
    )))
}
