//! JSON callbacks. Native preloads are owned and removed with registrations.
use crate::{BrowserContext, E2eError, E2eResult, Frame, Page};
use futures::FutureExt;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

/// Caller identity for a JSON binding. Frame handles cover the main document
/// and same-origin child frames; cross-origin binding dispatch is excluded.
#[derive(Clone)]
pub struct BindingSource {
    pub context: BrowserContext,
    pub page: Page,
    pub frame: Frame,
}
type CallbackFuture = Pin<Box<dyn Future<Output = E2eResult<Value>> + Send>>;
type Callback = Arc<dyn Fn(Option<BindingSource>, Vec<Value>) -> CallbackFuture + Send + Sync>;
static NEXT_REGISTRATION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub(crate) struct Registration {
    pub(crate) tag: u64,
    callback: Callback,
    binding: bool,
    cancellation: Option<crate::CancellationToken>,
    pub(crate) preload: Option<String>,
}
impl Registration {
    fn new(callback: Callback, binding: bool) -> Self {
        Self {
            tag: NEXT_REGISTRATION.fetch_add(1, Ordering::Relaxed),
            callback,
            binding,
            cancellation: None,
            preload: None,
        }
    }
}
#[derive(Clone, Default)]
pub(crate) struct ExposedState {
    pub(crate) registrations: Arc<Mutex<HashMap<String, Registration>>>,
    pub(crate) gate: Arc<tokio::sync::Mutex<()>>,
    pump: Arc<Mutex<Option<tokio::task::AbortHandle>>>,
    tasks: Arc<Mutex<Vec<(u64, String, tokio::task::AbortHandle)>>>,
}
impl ExposedState {
    pub(crate) fn stop(&self) {
        if let Some(handle) = self.pump.lock().unwrap_or_else(|e| e.into_inner()).take() {
            handle.abort();
        }
        for (_, _, handle) in self
            .tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
        {
            handle.abort();
        }
        self.registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
    pub(crate) fn contains(&self, name: &str) -> bool {
        self.registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(name)
    }
}

struct PageInstallation {
    page: Page,
    name: String,
    tag: u64,
    preload: Option<String>,
    armed: bool,
}
impl Drop for PageInstallation {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let page = self.page.with_cancellation(crate::CancellationToken::new());
        let (name, tag, preload) = (self.name.clone(), self.tag, self.preload.take());
        tokio::spawn(async move {
            if let Some(id) = preload {
                let _ = page.driver.remove_owned_init_script(&id).await;
            }
            let _ = page.evaluate_value(&remove_source(&name, tag)).await;
        });
    }
}
struct ContextInstallation {
    context: BrowserContext,
    name: String,
    tag: u64,
    preload: Option<String>,
    armed: bool,
}
impl Drop for ContextInstallation {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.context
            .callbacks
            .registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.name);
        let (context, name, tag, preload) = (
            self.context.clone(),
            self.name.clone(),
            self.tag,
            self.preload.take(),
        );
        tokio::spawn(async move {
            let _gate = context.callbacks.gate.lock().await;
            for page in context.pages() {
                let _page_gate = page.exposed.gate.lock().await;
                let matches = page
                    .exposed
                    .registrations
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(&name)
                    .is_some_and(|entry| entry.tag == tag);
                if matches {
                    let _ = page.remove_callback(&name).await;
                }
            }
            if let Some(id) = preload {
                let _ = context.remove_callback_preload(&id).await;
            }
        });
    }
}

// Always enter each same-origin window separately. A fresh nonce guards against
// stale async results after navigation even when the new document reuses ID 1.
const WALK: &str = "const walk = (w, f) => { try { void w.document; f(w); for (let i=0;i<w.frames.length;i++) walk(w.frames[i],f); } catch (_) {} };";
fn install_source(name: &str, tag: u64) -> String {
    let name = json!(name);
    format!(
        r#"(() => {{ {WALK}
      walk(window, w => {{
        const bx = w.__ferriteExpose ||= {{ nonce: w.crypto?.randomUUID?.() || String(Math.random()), seq: 0, queue: [], pending: {{}}, tags: {{}} }};
        if (bx.tags[{name}] === {tag}) return;
        bx.tags[{name}] = {tag};
        w[{name}] = (...args) => new Promise((resolve, reject) => {{
          if (Object.keys(bx.pending).length >= 256) {{ reject(new w.Error('exposed callback queue is full')); return; }}
          const id = ++bx.seq; bx.pending[id] = {{resolve, reject, tag: {tag}}};
          bx.queue.push([{name}, {tag}, bx.nonce, id, args]);
        }});
      }}); return true;
    }})()"#
    )
}
fn remove_source(name: &str, tag: u64) -> String {
    let name = json!(name);
    format!(
        r#"(() => {{ {WALK} walk(window,w=>{{ const bx=w.__ferriteExpose;
      if (!bx || bx.tags[{name}] !== {tag}) return;
      delete bx.tags[{name}]; delete w[{name}];
      bx.queue=bx.queue.filter(c=>c[1]!=={tag});
      for (const [id,entry] of Object.entries(bx.pending)) if(entry.tag==={tag}) {{
        delete bx.pending[id]; entry.reject(new w.Error('exposed callback removed'));
      }}
    }}); return true; }})()"#
    )
}
fn settle_source(nonce: &str, id: u64, tag: u64, result: E2eResult<Value>) -> String {
    let (method, value) = match result {
        Ok(value) => ("resolve", value),
        Err(error) => ("reject", json!(error.to_string())),
    };
    let nonce = json!(nonce);
    let value = if method == "reject" {
        format!("new w.Error({value})")
    } else {
        value.to_string()
    };
    format!(
        r#"(() => {{ {WALK} walk(window,w=>{{const bx=w.__ferriteExpose;
      if(!bx || bx.nonce!=={nonce}) return; const entry=bx.pending[{id}];
      if(!entry || entry.tag!=={tag}) return; delete bx.pending[{id}]; entry.{method}({value});
    }}); return true; }})()"#
    )
}

impl Page {
    /// Expose a synchronous JSON callback in current/future same-origin documents.
    /// Duplicate names (including context registrations) return a Config error.
    pub async fn expose_function<F>(&self, name: &str, callback: F) -> E2eResult<()>
    where
        F: Fn(Vec<Value>) -> Value + Send + Sync + 'static,
    {
        let callback = Arc::new(callback);
        self.expose_function_async(name, move |args| {
            let result = callback(args);
            async move { Ok(result) }
        })
        .await
    }
    /// Expose an async JSON callback. Calls run independently; Rust errors and
    /// panics reject JS promises. Navigation aborts old-document work; closure
    /// and named removal abort outstanding calls and release handlers.
    pub async fn expose_function_async<F, Fut>(&self, name: &str, callback: F) -> E2eResult<()>
    where
        F: Fn(Vec<Value>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<Value>> + Send + 'static,
    {
        self.register_callback(
            name,
            Registration::new(Arc::new(move |_, args| Box::pin(callback(args))), false),
        )
        .await
    }
    /// Expose an async JSON binding with the owning context/page/frame.
    pub async fn expose_binding<F, Fut>(&self, name: &str, callback: F) -> E2eResult<()>
    where
        F: Fn(BindingSource, Vec<Value>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<Value>> + Send + 'static,
    {
        self.register_callback(
            name,
            Registration::new(
                Arc::new(move |source, args| {
                    Box::pin(callback(source.expect("binding source resolved"), args))
                }),
                true,
            ),
        )
        .await
    }
    async fn register_callback(&self, name: &str, mut registration: Registration) -> E2eResult<()> {
        self.run_operation(async {
            let context = self.context();
            let _context_gate = match &context {
                Some(context) => Some(context.callbacks.gate.lock().await),
                None => None,
            };
            let _gate = self.exposed.gate.lock().await;
            validate_name(name)?;
            if self
                .context()
                .is_some_and(|ctx| ctx.callbacks.contains(name))
            {
                return Err(duplicate(name));
            }
            registration.cancellation = Some(self.driver.caller_cancellation());
            self.install_callback(name, registration, true, true).await
        })
        .await
    }
    pub(crate) async fn install_callback(
        &self,
        name: &str,
        mut registration: Registration,
        preload: bool,
        initialize_current: bool,
    ) -> E2eResult<()> {
        if self.exposed.contains(name) {
            return Err(duplicate(name));
        }
        let source = install_source(name, registration.tag);
        if preload {
            registration.preload = Some(self.driver.add_owned_init_script(&source).await?);
        }
        let mut installation = PageInstallation {
            page: self.clone(),
            name: name.into(),
            tag: registration.tag,
            preload: registration.preload.clone(),
            armed: true,
        };
        if initialize_current {
            self.evaluate_value(&source).await
        } else {
            Ok(Value::Null)
        }?;
        self.exposed
            .registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(name.into(), registration);
        installation.armed = false;
        self.start_callback_pump();
        Ok(())
    }
    /// Remove a page registration and its preload. Context registrations must
    /// be removed from the context. Missing names are an idempotent success.
    pub async fn remove_exposed_function(&self, name: &str) -> E2eResult<()> {
        self.run_operation(async {
            let context = self.context();
            let _context_gate = match &context {
                Some(context) => Some(context.callbacks.gate.lock().await),
                None => None,
            };
            let _gate = self.exposed.gate.lock().await;
            if self
                .context()
                .is_some_and(|ctx| ctx.callbacks.contains(name))
            {
                return Err(E2eError::Config(
                    "remove context callbacks from their owning context".into(),
                ));
            }
            self.remove_callback(name).await
        })
        .await
    }
    pub(crate) async fn remove_callback(&self, name: &str) -> E2eResult<()> {
        let registration = self
            .exposed
            .registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(name)
            .cloned();
        let Some(registration) = registration else {
            return Ok(());
        };
        // Remove the native preload first: a failed native removal leaves a
        // usable registration rather than a function with no Rust handler.
        if let Some(id) = &registration.preload {
            self.driver.remove_owned_init_script(id).await?;
        }
        self.exposed
            .registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(name);
        for (tag, _, handle) in self
            .exposed
            .tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
        {
            if *tag == registration.tag {
                handle.abort();
            }
        }
        self.evaluate_value(&remove_source(name, registration.tag))
            .await?;
        if self
            .exposed
            .registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
        {
            self.exposed.stop();
        }
        Ok(())
    }
    /// Best-effort removal of all page callbacks (legacy return type). Use
    /// named removal when native removal errors need to be reported.
    pub async fn clear_exposed_functions(&self) {
        let names: Vec<_> = self
            .exposed
            .registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .cloned()
            .collect();
        for name in names {
            if !self
                .context()
                .is_some_and(|ctx| ctx.callbacks.contains(&name))
            {
                let _ = self.remove_exposed_function(&name).await;
            }
        }
    }
    fn start_callback_pump(&self) {
        let mut pump = self.exposed.pump.lock().unwrap_or_else(|e| e.into_inner());
        if pump.is_some() {
            return;
        }
        let page = self
            .owning_page()
            .with_cancellation(crate::CancellationToken::new());
        *pump = Some(tokio::spawn(callback_pump(page)).abort_handle());
    }
    async fn binding_source(&self, nonce: &str) -> E2eResult<BindingSource> {
        self.driver
            .observe_main_world(async {
                let context = self.context().ok_or_else(|| {
                    E2eError::Config("binding context no longer available".into())
                })?;
                for frame in self.document_frames().await? {
                    let actual: E2eResult<Option<String>> = self
                        .driver
                        .frame_main_world_evaluate(
                            frame.id(),
                            "window.__ferriteExpose?.nonce || null",
                        )
                        .await
                        .and_then(|v| serde_json::from_value(v).map_err(E2eError::Json));
                    if actual.ok().flatten().as_deref() == Some(nonce) {
                        return Ok(BindingSource {
                            context,
                            page: self.owning_page(),
                            frame,
                        });
                    }
                }
                Err(E2eError::Config(
                    "binding frame detached or outside same-origin scope".into(),
                ))
            })
            .await
    }
}
type NativeCall = (String, u64, String, u64, Vec<Value>);
type NativeBatch = (Vec<String>, Vec<NativeCall>);
async fn callback_pump(page: Page) {
    let mut tasks = tokio::task::JoinSet::new();
    loop {
        tokio::time::sleep(Duration::from_millis(25)).await;
        while tasks.try_join_next().is_some() {}
        let sample: E2eResult<NativeBatch>=page.evaluate(&format!(r#"(() => {{ {WALK} const nonces=[],calls=[];
                  walk(window,w=>{{const bx=w.__ferriteExpose; if(bx) {{nonces.push(bx.nonce); calls.push(...bx.queue); bx.queue=[];}}}}); return [nonces,calls]; }})()"#)).await;
        let (nonces, calls) = match sample {
            Ok(sample) => sample,
            Err(E2eError::Cancelled(_) | E2eError::Disconnected(_)) => {
                page.exposed.stop();
                break;
            }
            Err(_) if !page.is_closed() => continue,
            Err(_) => break,
        };
        {
            let mut active = page.exposed.tasks.lock().unwrap_or_else(|e| e.into_inner());
            active.retain(|(_, nonce, handle)| {
                if !nonces.contains(nonce) {
                    handle.abort();
                }
                !handle.is_finished() && nonces.contains(nonce)
            });
        }
        for (name, tag, nonce, id, args) in calls {
            let registration = page
                .exposed
                .registrations
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&name)
                .filter(|r| r.tag == tag)
                .cloned();
            let Some(registration) = registration else {
                let _ = page
                    .evaluate_value(&settle_source(
                        &nonce,
                        id,
                        tag,
                        Err(E2eError::Config("exposed callback removed".into())),
                    ))
                    .await;
                continue;
            };
            if tasks.len() >= 256 {
                let _ = page
                    .evaluate_value(&settle_source(
                        &nonce,
                        id,
                        tag,
                        Err(E2eError::Config("exposed callback queue is full".into())),
                    ))
                    .await;
                continue;
            }
            let call_page = page.clone();
            let call_nonce = nonce.clone();
            let handle = tasks.spawn(async move {
                let result = async {
                    if let Some(token) = &registration.cancellation {
                        token.check()?;
                    }
                    let source = if registration.binding {
                        Some(call_page.binding_source(&call_nonce).await?)
                    } else {
                        None
                    };
                    let future = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        (registration.callback)(source, args)
                    }))
                    .map_err(|_| E2eError::Config("exposed function panicked".into()))?;
                    let callback = async {
                        std::panic::AssertUnwindSafe(future)
                            .catch_unwind()
                            .await
                            .map_err(|_| E2eError::Config("exposed function panicked".into()))?
                    };
                    call_page
                        .run_operation(async {
                            match registration.cancellation {
                                Some(token) => token.run(callback).await,
                                None => callback.await,
                            }
                        })
                        .await
                }
                .await;
                let _ = call_page
                    .evaluate_value(&settle_source(&call_nonce, id, tag, result))
                    .await;
            });
            page.exposed
                .tasks
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push((tag, nonce, handle));
        }
    }
}
fn validate_name(name: &str) -> E2eResult<()> {
    if !crate::page::valid_expose_name(name) || name == "__ferriteExpose" {
        Err(E2eError::Config(format!(
            "exposed callback needs a JS identifier, got {name:?}"
        )))
    } else {
        Ok(())
    }
}
fn duplicate(name: &str) -> E2eError {
    E2eError::Config(format!("exposed callback {name:?} is already registered"))
}

impl BrowserContext {
    pub async fn expose_function<F>(&self, name: &str, callback: F) -> E2eResult<()>
    where
        F: Fn(Vec<Value>) -> Value + Send + Sync + 'static,
    {
        let callback = Arc::new(callback);
        self.expose_function_async(name, move |args| {
            let result = callback(args);
            async move { Ok(result) }
        })
        .await
    }
    /// Register for current/future pages and popup startup scripts. Uses native
    /// user-context preloads on Firefox and paused popup adoption on Chromium.
    pub async fn expose_function_async<F, Fut>(&self, name: &str, callback: F) -> E2eResult<()>
    where
        F: Fn(Vec<Value>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<Value>> + Send + 'static,
    {
        self.register_context_callback(
            name,
            Registration::new(Arc::new(move |_, args| Box::pin(callback(args))), false),
        )
        .await
    }
    pub async fn expose_binding<F, Fut>(&self, name: &str, callback: F) -> E2eResult<()>
    where
        F: Fn(BindingSource, Vec<Value>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<Value>> + Send + 'static,
    {
        self.register_context_callback(
            name,
            Registration::new(
                Arc::new(move |source, args| {
                    Box::pin(callback(source.expect("binding source resolved"), args))
                }),
                true,
            ),
        )
        .await
    }
    async fn register_context_callback(
        &self,
        name: &str,
        mut registration: Registration,
    ) -> E2eResult<()> {
        self.cancellation_token()
            .run(async {
                let _gate = self.callbacks.gate.lock().await;
                validate_name(name)?;
                if self.callbacks.contains(name)
                    || self.pages().iter().any(|p| p.exposed.contains(name))
                {
                    return Err(duplicate(name));
                }
                registration.preload = self
                    .add_callback_preload(&install_source(name, registration.tag))
                    .await?;
                let mut installation = ContextInstallation {
                    context: self.clone(),
                    name: name.into(),
                    tag: registration.tag,
                    preload: registration.preload.clone(),
                    armed: true,
                };
                self.callbacks
                    .registrations
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(name.into(), registration.clone());
                for page in self.pages() {
                    let _page_gate = page.exposed.gate.lock().await;
                    let mut entry = registration.clone();
                    entry.preload = None;
                    page.install_callback(name, entry, registration.preload.is_none(), true)
                        .await?;
                }
                installation.armed = false;
                Ok(())
            })
            .await
    }
    pub(crate) async fn apply_callbacks(
        &self,
        page: &Page,
        initialize_current: bool,
    ) -> E2eResult<()> {
        let _page_gate = page.exposed.gate.lock().await;
        let entries = self
            .callbacks
            .registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        for (name, mut entry) in entries {
            let preload = entry.preload.is_none();
            entry.preload = None;
            page.install_callback(&name, entry, preload, initialize_current)
                .await?;
        }
        Ok(())
    }
    /// Remove a context function/binding from all current/future documents.
    pub async fn remove_exposed_function(&self, name: &str) -> E2eResult<()> {
        self.cancellation_token()
            .run(async {
                let _gate = self.callbacks.gate.lock().await;
                let registration = self
                    .callbacks
                    .registrations
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(name)
                    .cloned();
                let Some(registration) = registration else {
                    return Ok(());
                };
                if let Some(id) = &registration.preload {
                    self.remove_callback_preload(id).await?;
                }
                for page in self.pages() {
                    let _page_gate = page.exposed.gate.lock().await;
                    page.remove_callback(name).await?;
                }
                self.callbacks
                    .registrations
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(name);
                Ok(())
            })
            .await
    }
}
