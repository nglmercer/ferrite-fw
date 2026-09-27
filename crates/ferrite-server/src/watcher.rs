//! Dev server file watcher.

//! Dev server module transform pipeline.

use crate::util::*;
use crate::DevServer;
use ferrite_core::FerriteError;
use ferrite_core::ModuleId;
use ferrite_core::Result;
use ferrite_hmr::plan_update;
use ferrite_hmr::HmrPlan;
use ferrite_plugin::HotUpdateEvent;
use ferrite_plugin::PluginContext;
use ferrite_plugin::WatchEvent;
use ferrite_plugin::WatchKind;
use notify::Event;
use notify::EventKind;
use notify::RecursiveMode;
use notify::Watcher as _;
use std::time::Duration;
use std::time::Instant;

impl DevServer {
    /// Start the file watcher (§60 HMR invalidation).
    pub(crate) fn start_watcher(&self) -> Result<()> {
        let inner = self.inner.clone();
        // The notify callback runs off-runtime: capture a handle here
        // (constructor runs on the runtime) instead of `tokio::spawn`.
        let runtime = tokio::runtime::Handle::current();
        let debounce_window = Duration::from_millis(80);
        let mut watcher = notify::recommended_watcher(
            move |result: std::result::Result<Event, notify::Error>| {
                let Ok(event) = result else { return };
                if !matches!(
                    event.kind,
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                ) {
                    return;
                }
                let watch_kind = match event.kind {
                    EventKind::Create(_) => WatchKind::Create,
                    EventKind::Remove(_) => WatchKind::Remove,
                    _ => WatchKind::Modify,
                };
                for path in event.paths {
                    // Debounce.
                    let now = Instant::now();
                    if let Ok(mut debounce) = inner.debounce.lock() {
                        if let Some(last) = debounce.get(&path) {
                            if now.duration_since(*last) < debounce_window {
                                continue;
                            }
                        }
                        debounce.insert(path.clone(), now);
                    }
                    // Skip output/cache dirs.
                    if path.components().any(|component| {
                        matches!(
                            component.as_os_str().to_str(),
                            Some("dist" | ".ferrite" | "node_modules" | "target")
                        )
                    }) {
                        continue;
                    }
                    let url = ferrite_core::file_to_url(&inner.config.root, &path);
                    let id = ModuleId::new(url.clone());
                    let tracked = inner.graph.contains(&id);
                    if tracked {
                        inner.graph.invalidate_tree(&id);
                    }
                    let timestamp = now_millis();
                    // Plugin `watchChange` + `hotUpdate` hooks run on the
                    // async runtime. `watchChange` fires for every accepted
                    // event (tracked or not); HMR planning only for tracked
                    // modules.
                    let inner_clone = inner.clone();
                    let file = url.clone();
                    let watch_path = path.clone();
                    let runtime = runtime.clone();
                    runtime.spawn(async move {
                        let environment = inner_clone.config.client_env();
                        let ctx = PluginContext {
                            graph: &inner_clone.graph,
                            resolver: &inner_clone.client_resolver,
                            environment: &environment,
                            emitted: &inner_clone.emitted,
                            watch_files: &inner_clone.watch_files,
                            warnings: &inner_clone.warnings,
                        };
                        if let Err(error) = inner_clone
                            .plugins
                            .hook_watch_change(
                                &ctx,
                                WatchEvent {
                                    path: watch_path,
                                    kind: watch_kind,
                                },
                            )
                            .await
                        {
                            tracing::warn!("watch_change hook failed: {error}");
                        }
                        if !tracked {
                            // Untracked file (e.g. new CSS referenced later):
                            // nothing to push until an importer pulls it.
                            return;
                        }
                        let modules = vec![id.clone()];
                        let event = HotUpdateEvent {
                            file,
                            modules,
                            timestamp,
                        };
                        if let Ok(Some(custom)) = inner_clone
                            .plugins
                            .hook_hot_update(&ctx, event)
                            .await
                        {
                            if custom.full_reload {
                                inner_clone.hmr.send_full_reload(Some(id.0.clone()));
                                return;
                            }
                            if !custom.modules.is_empty() {
                                let updates = custom
                                    .modules
                                    .iter()
                                    .map(|module| ferrite_hmr::HmrUpdate {
                                        kind: "js-update".to_string(),
                                        path: module.0.clone(),
                                        accepted_path: module.0.clone(),
                                        timestamp,
                                        css_only: false,
                                    })
                                    .collect();
                                inner_clone.hmr.send_update(updates);
                                return;
                            }
                        }
                        match plan_update(&inner_clone.graph, &id, timestamp) {
                            HmrPlan::Update(updates) => inner_clone.hmr.send_update(updates),
                            HmrPlan::FullReload => {
                                inner_clone.hmr.send_full_reload(Some(id.0.clone()));
                            }
                        }
                    });
                }
            },
        )
        .map_err(|error| FerriteError::Other(format!("watcher failed: {error}")))?;
        watcher
            .watch(&self.inner.config.root, RecursiveMode::Recursive)
            .map_err(|error| FerriteError::Other(format!("watch failed: {error}")))?;
        if let Ok(mut slot) = self.watcher.lock() {
            *slot = Some(watcher);
        }
        Ok(())
    }
}
