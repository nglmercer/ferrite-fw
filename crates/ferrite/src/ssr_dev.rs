//! Development SSR using the shared compiler graph and explicitly selected runtime.
use std::sync::Arc;

/// Select one document per request and revalidate its renderer, styles and shell.
/// All routes share one request-isolated runtime and its admission limits.
/// A weak server handle permits shutdown without an ownership cycle.
pub async fn create_dev_ssr_adapter(
    server: &crate::DevServer,
    resolved: &crate::ResolvedConfig,
    shell: &str,
) -> crate::Result<Arc<dyn ferrite_ssr::SsrAdapter>> {
    crate::config::resolve_js_server_entry(resolved)?.ok_or_else(|| {
        crate::FerriteError::Ssr("no JavaScript/TypeScript server entry found".into())
    })?;
    let mut fallback_config = resolved.clone();
    fallback_config.ssr.entries.clear();
    let fallback = crate::config::resolve_js_server_entry(&fallback_config)?;
    let runtime = ferrite_ssr::JsSsrAdapter::runtime_for_resolved(resolved);
    let server = Arc::new(server.weak_handle());
    let resolved = Arc::new(resolved.clone());
    let default_shell = shell.to_string();
    let base = crate::loader::with_trailing_slash(&resolved.base);
    Ok(Arc::new(ferrite_ssr::FnAdapter::new(
        move |request: ferrite_ssr::SsrHttpRequest, mut context: ferrite_ssr::SsrContext| {
            let server = server.clone();
            let resolved = resolved.clone();
            let runtime = runtime.clone();
            let default_shell = default_shell.clone();
            let base = base.clone();
            let fallback = fallback.clone();
            async move {
                let server = server()
                    .ok_or_else(|| crate::FerriteError::Ssr("SSR dev server has closed".into()))?;
                let path = request.uri.split('?').next().unwrap_or(&request.uri);
                let relative = path.strip_prefix(&base).map(|path| format!("/{path}"));
                let selected = relative
                    .as_ref()
                    .and_then(|path| resolved.ssr.selected_entry(path));
                let entry = selected
                    .map(|entry| entry.entry.as_str())
                    .or(fallback.as_deref());
                let Some(entry) = entry.filter(|_| relative.is_some()) else {
                    let mut response = ferrite_ssr::SsrResponse::html("Not Found");
                    response.status = 404;
                    return Ok(response);
                };
                let (graph, styles) = server
                    .ssr_runtime_graph_with_styles(&format!("/{entry}"))
                    .await?;
                context.preload.extend(styles);
                let shell = if let Some(path) = selected
                    .and_then(|entry| entry.shell.as_ref())
                    .or(resolved.ssr.shell.as_ref())
                {
                    let shell = server.transform_index_html(&format!("/{path}")).await?;
                    if shell.matches("<!--ssr-outlet-->").count() != 1 {
                        return Err(crate::FerriteError::Ssr(
                            "document shell requires exactly one ssr-outlet".into(),
                        ));
                    }
                    shell
                } else {
                    default_shell
                };
                if shell.matches("<!--ssr-outlet-->").count() != 1 {
                    return Err(crate::FerriteError::Ssr(
                        "document shell requires exactly one ssr-outlet".into(),
                    ));
                }
                let shell = crate::ssr_islands::inject_dev_bootstrap(&shell, &resolved);
                let adapter = ferrite_ssr::JsSsrAdapter::new_graph(runtime, graph)?
                    .with_shell(shell)
                    .with_streaming(resolved.ssr.streaming);
                ferrite_ssr::SsrAdapter::render(&adapter, request, context).await
            }
        },
    )))
}

/// Read-only diagnostics using this build's compiled graph runtime capability.
/// No compiler host, runtime worker or guest renderer is started.
pub fn inspect_capabilities(
    config: &crate::ResolvedConfig,
) -> crate::Result<crate::frameworks::doctor::DoctorReport> {
    crate::frameworks::doctor::inspect_with_runtime(
        config,
        Some(
            cfg!(feature = "napi-vm")
                && ferrite_runtime::compiled_graph_backend_available(&config.runtime.backend),
        ),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn diagnostics_use_this_build_and_do_not_substitute_a_runtime() {
        let root = tempfile::tempdir().unwrap();
        let mut config = crate::resolve_config(
            Default::default(),
            Some(root.path().into()),
            Default::default(),
        )
        .unwrap();
        config.ssr.entry = Some("src/entry-server.ts".into());
        for backend in ["napi-vm", "napi-vm-process", "node", "auto", "unknown"] {
            config.runtime.backend = backend.into();
            let report = super::inspect_capabilities(&config).unwrap();
            let available = ferrite_runtime::compiled_graph_backend_available(backend);
            assert_eq!(report.ssr_runtime, backend);
            assert_eq!(
                report.ssr_runtime_probe,
                if available {
                    "compiled-not-executed"
                } else {
                    "unavailable-in-build"
                }
            );
            let issue = report
                .issues
                .iter()
                .find(|issue| issue.message.contains("selected SSR runtime"));
            if available {
                assert!(issue.is_none());
            } else {
                let issue = issue.unwrap();
                assert_eq!(issue.severity, "error");
                assert!(issue.message.contains(backend));
                assert!(issue.action.contains("--features napi-vm"));
            }
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
        }
        let unverified = crate::frameworks::doctor::inspect(&config).unwrap();
        assert_eq!(unverified.ssr_runtime_probe, "unverified");
        assert!(unverified
            .issues
            .iter()
            .any(|issue| issue.severity == "warning"
                && issue.action.contains("inspect_capabilities")));
    }
}
