//! Live route configuration and ownership of in-flight handler calls.
use crate::{
    driver::{routing_matchers, sorted_matches, ConsoleSink, RouteMatcher},
    CancellationToken, E2eError, E2eResult, RouteAction, RouteHandlerEntry, RouteInfo, RouteRule,
};
use futures::FutureExt;
use std::{
    collections::{HashMap, HashSet},
    panic::AssertUnwindSafe,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

/// Policy for calls already running when routes are removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UnrouteBehavior {
    /// Return immediately and release active requests to the network. Existing
    /// callbacks continue; subsequent decisions are discarded and errors recorded.
    #[default]
    Default,
    /// Wait for existing calls and their native decisions to settle.
    Wait,
    /// Release active requests without waiting; suppress later callback errors.
    IgnoreErrors,
    /// Cancel existing handler futures and abort their requests (Rust extension).
    Cancel,
}

/// Route removal options. Zero timeout disables the local deadline.
#[derive(Debug, Clone, Default)]
pub struct UnrouteOptions {
    pub behavior: UnrouteBehavior,
    pub timeout: Option<Duration>,
    pub cancellation: Option<CancellationToken>,
}
impl UnrouteOptions {
    #[must_use]
    pub fn behavior(mut self, behavior: UnrouteBehavior) -> Self {
        self.behavior = behavior;
        self
    }
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
    #[must_use]
    pub fn cancellation(mut self, token: CancellationToken) -> Self {
        self.cancellation = Some(token);
        self
    }
}

pub(crate) type PumpSlot = tokio::sync::Mutex<Option<RoutePump>>;
pub(crate) struct RoutePump {
    handle: tokio::task::AbortHandle,
    pub(crate) stopped: CancellationToken,
}
impl RoutePump {
    pub(crate) fn new(handle: tokio::task::AbortHandle, stopped: CancellationToken) -> Self {
        Self { handle, stopped }
    }
    pub(crate) fn abort(&self) {
        self.handle.abort();
    }
    pub(crate) fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }
}
pub(crate) struct PumpCompletion(pub(crate) CancellationToken);
impl Drop for PumpCompletion {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

#[derive(Clone)]
pub(crate) struct RouteConfiguration {
    rules: Vec<RouteRule>,
    handlers: Vec<RouteHandlerEntry>,
    rule_matchers: Vec<RouteMatcher>,
    handler_matchers: Vec<RouteMatcher>,
}
impl RouteConfiguration {
    pub(crate) fn new(rules: Vec<RouteRule>, handlers: Vec<RouteHandlerEntry>) -> E2eResult<Self> {
        Ok(Self {
            rule_matchers: routing_matchers(
                rules
                    .iter()
                    .map(|r| (r.pattern.as_str(), r.matcher.as_ref())),
            )?,
            handler_matchers: routing_matchers(
                handlers
                    .iter()
                    .map(|r| (r.pattern.as_str(), r.matcher.as_ref())),
            )?,
            rules,
            handlers,
        })
    }
    pub(crate) fn rules(&self) -> &[RouteRule] {
        &self.rules
    }
    fn empty(&self) -> bool {
        self.rules.is_empty() && self.handlers.is_empty()
    }
}

struct ActiveCall {
    handler: usize,
    ignore_errors: Arc<AtomicBool>,
    cancel: CancellationToken,
    done: CancellationToken,
    forward: CancellationToken,
}
#[derive(Default)]
struct State {
    configuration: Option<Arc<RouteConfiguration>>,
    fetch_owner: Option<crate::route_options::RouteFetchOwner>,
    retired: HashSet<usize>,
    calls: HashMap<u64, ActiveCall>,
    next_call: u64,
    requests: usize,
}
#[derive(Default)]
pub(crate) struct RouteRuntime {
    state: Mutex<State>,
    pub(crate) changed: tokio::sync::Notify,
}

pub(crate) fn handler_id(entry: &RouteHandlerEntry) -> usize {
    Arc::as_ptr(&entry.hits) as usize
}
fn reserve(hits: &AtomicU32, times: Option<u32>) -> bool {
    hits.fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
        if times.is_some_and(|limit| count >= limit) {
            None
        } else {
            Some(count.saturating_add(1))
        }
    })
    .is_ok()
}

impl RouteRuntime {
    pub(crate) fn set_fetch_owner(&self, owner: crate::route_options::RouteFetchOwner) {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .fetch_owner = Some(owner);
    }

    pub(crate) fn configure(&self, configuration: RouteConfiguration) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.configuration = Some(Arc::new(configuration));
        state.retired.clear();
        drop(state);
        self.changed.notify_one();
    }
    fn rollback_handler(&self, entry: &RouteHandlerEntry) {
        let id = handler_id(entry);
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(configuration) = state.configuration.as_ref() {
            let mut configuration = (**configuration).clone();
            let pairs = configuration
                .handlers
                .into_iter()
                .zip(configuration.handler_matchers)
                .filter(|(handler, _)| handler_id(handler) != id);
            (configuration.handlers, configuration.handler_matchers) = pairs.unzip();
            state.configuration = Some(Arc::new(configuration));
        }
        for call in state.calls.values().filter(|call| call.handler == id) {
            call.cancel.cancel();
            call.forward.cancel();
        }
        drop(state);
        self.changed.notify_one();
    }
    pub(crate) fn clear(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.configuration = None;
        for call in state.calls.values() {
            call.cancel.cancel();
        }
        drop(state);
        self.changed.notify_one();
    }
    pub(crate) fn empty(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .configuration
            .as_ref()
            .is_none_or(|c| c.empty())
    }
    pub(crate) fn idle(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.requests == 0
            && state.calls.is_empty()
            && state.configuration.as_ref().is_none_or(|c| c.empty())
    }
    /// Removal is synchronous so no old snapshot can begin another removed call.
    pub(crate) fn retire(
        &self,
        entries: &[RouteHandlerEntry],
        behavior: UnrouteBehavior,
    ) -> Vec<CancellationToken> {
        let ids: HashSet<_> = entries.iter().map(handler_id).collect();
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.retired.extend(&ids);
        state
            .calls
            .values()
            .filter(|call| ids.contains(&call.handler))
            .map(|call| {
                if behavior == UnrouteBehavior::IgnoreErrors {
                    call.ignore_errors.store(true, Ordering::Release);
                }
                if matches!(
                    behavior,
                    UnrouteBehavior::Default | UnrouteBehavior::IgnoreErrors
                ) {
                    call.forward.cancel();
                }
                if behavior == UnrouteBehavior::Cancel {
                    call.cancel.cancel();
                }
                call.done.clone()
            })
            .collect()
    }
    pub(crate) fn request(self: &Arc<Self>) -> RequestGuard {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .requests += 1;
        RequestGuard {
            runtime: self.clone(),
            forward: CancellationToken::new(),
        }
    }
    fn begin(
        self: &Arc<Self>,
        entry: &RouteHandlerEntry,
        forward: &CancellationToken,
    ) -> Option<HandlerGuard> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let id = handler_id(entry);
        if state.retired.contains(&id)
            || !state
                .configuration
                .as_ref()
                .is_some_and(|c| c.handlers.iter().any(|e| handler_id(e) == id))
            || !reserve(&entry.hits, entry.times)
        {
            return None;
        }
        state.next_call = state.next_call.wrapping_add(1);
        let key = state.next_call;
        let ignore_errors = Arc::new(AtomicBool::new(false));
        let cancel = CancellationToken::new();
        let done = CancellationToken::new();
        state.calls.insert(
            key,
            ActiveCall {
                handler: id,
                ignore_errors: ignore_errors.clone(),
                cancel: cancel.clone(),
                done: done.clone(),
                forward: forward.clone(),
            },
        );
        Some(HandlerGuard {
            runtime: self.clone(),
            key,
            ignore_errors,
            cancel,
            done,
        })
    }
    pub(crate) async fn decide(
        self: &Arc<Self>,
        mut info: RouteInfo,
        sink: &ConsoleSink,
        forward: &CancellationToken,
    ) -> (Option<RouteAction>, Option<HandlerGuard>) {
        let Some(configuration) = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .configuration
            .clone()
        else {
            return (None, None);
        };
        info.set_fetch_owner(
            self.state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .fetch_owner
                .clone(),
        );
        for index in sorted_matches(&configuration.handler_matchers, &info.url) {
            let entry = &configuration.handlers[index];
            let Some(guard) = self.begin(entry, forward) else {
                continue;
            };
            // Contain panics both when the user creates and when it polls a future.
            let future =
                std::panic::catch_unwind(AssertUnwindSafe(|| (entry.handler)(info.clone())));
            let result = match future {
                Ok(future) => tokio::select! { biased;
                    _ = guard.cancel.cancelled() => Ok(RouteAction::Abort),
                    result = AssertUnwindSafe(future).catch_unwind() => result.unwrap_or_else(|_| Err(E2eError::Config("route handler future panicked".into()))),
                },
                Err(_) => Err(E2eError::Config("route handler creation panicked".into())),
            };
            match result {
                Ok(RouteAction::Fallback) if forward.is_cancelled() => return (None, Some(guard)),
                Ok(RouteAction::Fallback) => continue,
                Ok(action) => return (Some(action), Some(guard)),
                Err(error) => {
                    if !guard.ignore_errors.load(Ordering::Acquire) {
                        sink.record(
                            "route",
                            if forward.is_cancelled() {
                                format!("handler failed after removal: {error}")
                            } else {
                                format!("handler failed, aborting: {error}")
                            },
                        );
                    }
                    return (Some(RouteAction::Abort), Some(guard));
                }
            }
        }
        for index in sorted_matches(&configuration.rule_matchers, &info.url) {
            let rule = &configuration.rules[index];
            if matches!(rule.action, RouteAction::Fallback) {
                continue;
            }
            let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if state
                .configuration
                .as_ref()
                .is_some_and(|c| c.rules.iter().any(|r| Arc::ptr_eq(&r.hits, &rule.hits)))
                && reserve(&rule.hits, rule.times)
            {
                return (Some(rule.action.clone()), None);
            }
        }
        (None, None)
    }
}

pub(crate) struct HandlerGuard {
    runtime: Arc<RouteRuntime>,
    key: u64,
    ignore_errors: Arc<AtomicBool>,
    cancel: CancellationToken,
    done: CancellationToken,
}
impl Drop for HandlerGuard {
    fn drop(&mut self) {
        self.runtime
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .calls
            .remove(&self.key);
        self.done.cancel();
        self.runtime.changed.notify_one();
    }
}
pub(crate) struct RequestGuard {
    runtime: Arc<RouteRuntime>,
    pub(crate) forward: CancellationToken,
}
impl Drop for RequestGuard {
    fn drop(&mut self) {
        self.runtime
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .requests -= 1;
        self.runtime.changed.notify_one();
    }
}
pub(crate) async fn wait_calls(calls: Vec<CancellationToken>) -> E2eResult<()> {
    for done in calls {
        done.cancelled().await;
    }
    Ok(())
}

/// Roll back exactly this registration on error or future drop. The guard holds
/// no Page owner and reconciles already-registered runtime snapshots synchronously.
struct InstallationLease {
    handler: Mutex<Option<crate::page::RouteHandler>>,
    cancelled: CancellationToken,
}

pub(crate) struct HandlerInstallation {
    lease: Arc<InstallationLease>,
    storage: Arc<Mutex<Vec<RouteHandlerEntry>>>,
    entry: RouteHandlerEntry,
    runtime: std::sync::Weak<RouteRuntime>,
    pages: std::sync::Weak<Mutex<Vec<crate::Page>>>,
    committed: bool,
}
impl HandlerInstallation {
    pub(crate) fn new(
        storage: Arc<Mutex<Vec<RouteHandlerEntry>>>,
        mut entry: RouteHandlerEntry,
        runtime: std::sync::Weak<RouteRuntime>,
        pages: std::sync::Weak<Mutex<Vec<crate::Page>>>,
    ) -> Self {
        let lease = Arc::new(InstallationLease {
            handler: Mutex::new(Some(entry.handler.clone())),
            cancelled: CancellationToken::new(),
        });
        let callback_lease = lease.clone();
        entry.handler = Arc::new(move |info| {
            let lease = callback_lease.clone();
            Box::pin(async move {
                if lease.cancelled.is_cancelled() {
                    return Ok(RouteAction::Fallback);
                }
                lease
                    .cancelled
                    .run(async {
                        let handler = lease
                            .handler
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .clone();
                        match handler {
                            Some(handler) => handler(info).await,
                            None => Ok(RouteAction::Fallback),
                        }
                    })
                    .await
            })
        });
        storage
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(entry.clone());
        Self {
            lease,
            storage,
            entry,
            runtime,
            pages,
            committed: false,
        }
    }
    pub(crate) fn commit(mut self) {
        self.committed = true;
    }
}
impl Drop for HandlerInstallation {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        self.lease
            .cancelled
            .cancel_with_reason("route installation abandoned");
        self.lease
            .handler
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let id = handler_id(&self.entry);
        self.storage
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|entry| handler_id(entry) != id);
        if let Some(runtime) = self.runtime.upgrade() {
            runtime.rollback_handler(&self.entry);
        }
        let pages = self
            .pages
            .upgrade()
            .map(|pages| pages.lock().unwrap_or_else(|e| e.into_inner()).clone())
            .unwrap_or_default();
        for page in pages {
            page.route_runtime.rollback_handler(&self.entry);
        }
    }
}

#[cfg(test)]
mod installation_tests {
    use super::*;
    fn entry(pattern: &str) -> RouteHandlerEntry {
        RouteHandlerEntry {
            pattern: pattern.into(),
            matcher: None,
            handler: Arc::new(|_| Box::pin(async { Ok(RouteAction::Fallback) })),
            times: None,
            hits: Arc::new(AtomicU32::new(0)),
        }
    }
    #[test]
    fn abandoned_registration_preserves_concurrent_entries_and_runtime_matchers() {
        let storage = Arc::new(Mutex::new(Vec::new()));
        let runtime = Arc::new(RouteRuntime::default());
        let failed = entry("**/failed");
        let successful = entry("**/kept");
        let failed_id = handler_id(&failed);
        let successful_id = handler_id(&successful);
        let guard = HandlerInstallation::new(
            storage.clone(),
            failed.clone(),
            Arc::downgrade(&runtime),
            Default::default(),
        );
        let kept = HandlerInstallation::new(
            storage.clone(),
            successful.clone(),
            Arc::downgrade(&runtime),
            Default::default(),
        );
        kept.commit();
        runtime.configure(
            RouteConfiguration::new(Vec::new(), storage.lock().unwrap().clone()).unwrap(),
        );
        drop(guard);
        assert_eq!(
            storage
                .lock()
                .unwrap()
                .iter()
                .map(handler_id)
                .collect::<Vec<_>>(),
            vec![successful_id]
        );
        let state = runtime.state.lock().unwrap();
        let configuration = state.configuration.as_ref().unwrap();
        assert_eq!(configuration.handlers.len(), 1);
        assert_eq!(configuration.handler_matchers.len(), 1);
        assert_eq!(handler_id(&configuration.handlers[0]), successful_id);
        assert_ne!(handler_id(&configuration.handlers[0]), failed_id);
        drop(state);
        assert!(runtime.begin(&failed, &CancellationToken::new()).is_none());
        assert!(runtime
            .begin(&successful, &CancellationToken::new())
            .is_some());
    }
    #[tokio::test]
    async fn cancellation_drops_installation_metadata_and_callback_ownership() {
        let storage = Arc::new(Mutex::new(Vec::new()));
        let runtime = Arc::new(RouteRuntime::default());
        let owned = Arc::new(());
        let weak = Arc::downgrade(&owned);
        let mut handler = entry("**");
        handler.handler = Arc::new(move |_| {
            let owned = owned.clone();
            Box::pin(async move {
                drop(owned);
                Ok(RouteAction::Abort)
            })
        });
        let cancel = CancellationToken::new();
        let started = CancellationToken::new();
        let stale = Arc::new(Mutex::new(None::<RouteHandlerEntry>));
        let work = async {
            let installation = HandlerInstallation::new(
                storage.clone(),
                handler,
                Arc::downgrade(&runtime),
                Default::default(),
            );
            runtime.configure(
                RouteConfiguration::new(Vec::new(), storage.lock().unwrap().clone()).unwrap(),
            );
            *stale.lock().unwrap() = Some(storage.lock().unwrap()[0].clone());
            started.cancel();
            std::future::pending::<()>().await;
            installation.commit();
            Ok(())
        };
        let (result, ()) = tokio::join!(cancel.run(work), async {
            started.cancelled().await;
            assert_eq!(storage.lock().unwrap().len(), 1);
            cancel.cancel();
        });
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        assert!(storage.lock().unwrap().is_empty());
        assert!(runtime.idle());
        assert!(
            weak.upgrade().is_none(),
            "stale profile must not retain the abandoned user callback"
        );
        let stale = stale.lock().unwrap().take().unwrap();
        let result =
            (stale.handler)(RouteInfo::new("http://fixture/", "GET", Vec::new(), None)).await;
        assert!(matches!(result, Ok(RouteAction::Fallback)));
    }

    #[test]
    fn dropping_last_uncommitted_handler_leaves_runtime_idle() {
        let storage = Arc::new(Mutex::new(Vec::new()));
        let runtime = Arc::new(RouteRuntime::default());
        let guard = HandlerInstallation::new(
            storage.clone(),
            entry("**"),
            Arc::downgrade(&runtime),
            Default::default(),
        );
        runtime.configure(
            RouteConfiguration::new(Vec::new(), storage.lock().unwrap().clone()).unwrap(),
        );
        drop(guard);
        assert!(storage.lock().unwrap().is_empty());
        assert!(runtime.idle());
    }
}
