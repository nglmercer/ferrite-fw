//! Tier-3 Node adapter (spec §56–§57): run foreign JS plugins in a real
//! Node.js subprocess over a JSON-lines stdio bridge.
//!
//! [`NodeAdapterHost`] implements the §57 [`ForeignPluginHost`] interface
//! with the §56 guest contract: each hook is an exported function
//! `(input) -> output | null | undefined` with plain-JSON values. `null`
//! means "hook not implemented" (caller skips, Rollup-style); guest
//! throws, unknown plugins, and protocol violations are loud errors.
//!
//! Node.js is never required unless a Node plugin is configured: the
//! adapter spawns nothing until [`NodeAdapterHost::spawn`] runs.

use std::collections::HashMap;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use ferrite_core::{FerriteError, Result};

use crate::{ForeignPluginHost, HookName, PluginHandle};

/// Embedded guest driver (stdin JSON-lines → hook calls).
const ADAPTER_SCRIPT: &str = include_str!("adapter.mjs");

/// Default per-hook timeout.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

/// Tier-3 host: foreign plugins in a Node.js child process.
pub struct NodeAdapterHost {
    inner: Arc<NodeAdapterInner>,
}

struct NodeAdapterInner {
    child: Mutex<Child>,
    stdin: Mutex<std::process::ChildStdin>,
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, mpsc::Sender<AdapterResponse>>>,
    registered: Mutex<HashMap<String, String>>,
    timeout: Duration,
    stopped: AtomicBool,
    _script: tempfile::TempPath,
    /// Reader thread handle (joined on drop after killing the child).
    reader: Mutex<Option<std::thread::JoinHandle<()>>>,
}

#[derive(Debug, serde::Deserialize)]
struct AdapterResponse {
    id: u64,
    ok: bool,
    #[serde(default)]
    result: serde_json::Value,
    #[serde(default)]
    error: Option<String>,
}

impl NodeAdapterHost {
    /// Spawn `node` (explicit path or `PATH` lookup) running the embedded
    /// adapter. Fails loudly when Node.js is unavailable.
    pub fn spawn(node: Option<PathBuf>) -> Result<Self> {
        Self::spawn_with_timeout(node, DEFAULT_TIMEOUT)
    }

    /// Spawn with a per-hook timeout.
    pub fn spawn_with_timeout(node: Option<PathBuf>, timeout: Duration) -> Result<Self> {
        let node = match node {
            Some(path) => path,
            None => find_on_path("node").ok_or_else(|| {
                FerriteError::Build(
                    "tier-3 node adapter: `node` is not on PATH (install Node.js 18+ or point `node_path` at it)"
                        .to_string(),
                )
            })?,
        };
        let script_file = write_adapter_script()?;
        let mut child = Command::new(&node)
            .arg("--max-old-space-size=256")
            .arg(&script_file)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| {
                FerriteError::Build(format!(
                    "tier-3 node adapter: cannot spawn {}: {error}",
                    node.display()
                ))
            })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            FerriteError::Build("tier-3 node adapter: cannot pipe stdin".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            FerriteError::Build("tier-3 node adapter: cannot pipe stdout".to_string())
        })?;
        let inner = Arc::new(NodeAdapterInner {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            next_id: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            registered: Mutex::new(HashMap::new()),
            timeout,
            stopped: AtomicBool::new(false),
            _script: script_file,
            reader: Mutex::new(None),
        });
        // Handshake waiter first: the adapter prints `{"ferrite":3}` on
        // boot, potentially before we would otherwise listen.
        let (tx, rx) = mpsc::channel();
        inner
            .pending
            .lock()
            .map_err(|_| poison("pending"))?
            .insert(0, tx);
        *inner.reader.lock().map_err(|_| poison("reader"))? = Some(std::thread::spawn(
            reader_loop(stdout, Arc::downgrade(&inner)),
        ));
        let boot = rx.recv_timeout(timeout).map_err(|_| {
            FerriteError::Build(
                "tier-3 node adapter: node did not boot in time (is it a working Node.js?)"
                    .to_string(),
            )
        })?;
        if !boot.ok {
            return Err(FerriteError::Build(
                boot.error.unwrap_or_else(|| "node boot failed".into()),
            ));
        }
        inner
            .pending
            .lock()
            .map_err(|_| poison("pending"))?
            .remove(&0);
        Ok(Self { inner })
    }

    /// Register `entry` (file path or `file://` URL) under `name`.
    pub fn register_plugin(&self, name: &str, entry: &str) -> Result<()> {
        self.register(name, entry, None)
    }

    /// Register one default factory/object or named-hook module in the validated
    /// resolveId/load/transform subset. Factories receive explicit JSON options once. Required
    /// unsupported hooks/metadata fail registration; no Vite compatibility claim.
    pub fn register_hook_plugin(
        &self,
        name: &str,
        entry: &str,
        options: serde_json::Value,
    ) -> Result<()> {
        self.register(name, entry, Some(options))
    }

    fn register(&self, name: &str, entry: &str, options: Option<serde_json::Value>) -> Result<()> {
        let profile = if options.is_some() {
            "hooks"
        } else {
            "exports"
        };
        let response = self
            .inner
            .request(serde_json::json!({"cmd": "register", "name": name, "entry": entry, "profile": profile, "options": options}))?;
        if !response.ok {
            return Err(FerriteError::Build(format!(
                "tier-3 node adapter: cannot register `{name}`: {}",
                response
                    .error
                    .unwrap_or_else(|| "unknown error".to_string())
            )));
        }
        self.inner
            .registered
            .lock()
            .map_err(|_| poison("registered"))?
            .insert(name.to_string(), entry.to_string());
        Ok(())
    }

    /// Call a typed JSON export; unlike optional hooks, missing exports fail.
    pub async fn call_export(
        &self,
        name: &str,
        export: &str,
        input: serde_json::Value,
    ) -> Result<serde_json::Value> {
        if !self.contains(name) {
            return Err(FerriteError::Build(format!(
                "unknown node compiler/plugin {name}"
            )));
        }
        let inner = self.inner.clone();
        let name = name.to_owned();
        let export = export.to_owned();
        let response = tokio::task::spawn_blocking(move || {
            inner.request(
                serde_json::json!({"cmd": "call", "name": name, "export": export, "input": input}),
            )
        })
        .await
        .map_err(|error| FerriteError::Build(format!("node worker task failed: {error}")))??;
        if !response.ok {
            return Err(FerriteError::Build(
                response
                    .error
                    .unwrap_or_else(|| "node compiler export failed".into()),
            ));
        }
        Ok(response.result)
    }

    /// Explicit cancellation/shutdown terminates this worker and all pending calls.
    /// A timed-out or cancelled worker is never silently restarted.
    pub fn shutdown(&self) {
        self.inner.stop("node worker cancelled or shut down");
    }

    /// Persistent child identity, useful for host diagnostics.
    pub fn process_id(&self) -> Result<u32> {
        Ok(self.inner.child.lock().map_err(|_| poison("child"))?.id())
    }

    /// True when `name` is registered.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.inner
            .registered
            .lock()
            .map(|registered| registered.contains_key(name))
            .unwrap_or(false)
    }
}

#[async_trait::async_trait]
impl ForeignPluginHost for NodeAdapterHost {
    fn name(&self) -> &'static str {
        "node-adapter"
    }

    async fn call_hook(
        &self,
        plugin: &PluginHandle,
        hook: HookName,
        input: serde_json::Value,
    ) -> Result<serde_json::Value> {
        if plugin.host != "node-adapter" {
            return Err(FerriteError::Build(format!(
                "tier-3 node adapter: cannot call hook on `{}`-hosted plugin `{}`",
                plugin.host, plugin.name
            )));
        }
        if !self.contains(&plugin.name) {
            return Err(FerriteError::Build(format!(
                "tier-3 node adapter: unknown plugin `{}`",
                plugin.name
            )));
        }
        let inner = self.inner.clone();
        let body = serde_json::json!({
            "cmd": "hook",
            "name": plugin.name,
            "hook": hook_name(hook),
            "input": input,
        });
        let response = tokio::task::spawn_blocking(move || inner.request(body))
            .await
            .map_err(|error| FerriteError::Build(format!("node hook task failed: {error}")))??;
        if !response.ok {
            return Err(FerriteError::Build(format!(
                "tier-3 node adapter: `{}` hook `{}` threw: {}",
                plugin.name,
                hook_name(hook),
                response
                    .error
                    .unwrap_or_else(|| "unknown error".to_string())
            )));
        }
        Ok(response.result)
    }
}

impl Drop for NodeAdapterInner {
    fn drop(&mut self) {
        self.stop("node worker dropped");
        if let Ok(mut reader) = self.reader.lock() {
            if let Some(handle) = reader.take() {
                if handle.thread().id() != std::thread::current().id() {
                    handle.join().ok();
                }
            }
        }
    }
}

/// Hook enum → guest export name.
fn hook_name(hook: HookName) -> &'static str {
    match hook {
        HookName::ResolveId => "resolveId",
        HookName::Load => "load",
        HookName::Transform => "transform",
        HookName::TransformIndexHtml => "transformIndexHtml",
        HookName::HandleHotUpdate => "handleHotUpdate",
        HookName::GenerateBundle => "generateBundle",
    }
}

fn poison(what: &str) -> FerriteError {
    FerriteError::Build(format!("tier-3 node adapter: {what} lock poisoned"))
}

impl NodeAdapterInner {
    fn stop(&self, reason: &str) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Ok(mut child) = self.child.lock() {
            child.kill().ok();
            child.wait().ok();
        }
        if let Ok(mut pending) = self.pending.lock() {
            for (id, sender) in pending.drain() {
                sender
                    .send(AdapterResponse {
                        id,
                        ok: false,
                        result: serde_json::Value::Null,
                        error: Some(reason.into()),
                    })
                    .ok();
            }
        }
    }

    /// Send one request, await its response (or timeout).
    fn request(&self, mut body: serde_json::Value) -> Result<AdapterResponse> {
        if self.stopped.load(Ordering::SeqCst) {
            return Err(FerriteError::Build(
                "node worker is stopped; explicitly create a new enabled host".into(),
            ));
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        body["id"] = serde_json::json!(id);
        let mut line = serde_json::to_string(&body).map_err(|error| {
            FerriteError::Build(format!(
                "tier-3 node adapter: cannot encode request: {error}"
            ))
        })?;
        if line.len() > MAX_MESSAGE_BYTES {
            return Err(FerriteError::Build(
                "node request exceeds the 16 MiB message limit".into(),
            ));
        }
        let (tx, rx) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| poison("pending"))?
            .insert(id, tx);
        line.push('\n');
        let write_result = self
            .stdin
            .lock()
            .map_err(|_| poison("stdin"))?
            .write_all(line.as_bytes())
            .map_err(|error| {
                FerriteError::Build(format!("tier-3 node adapter: node stdin closed: {error}"))
            });
        if let Err(error) = write_result {
            self.stop("node input transport failed; worker terminated");
            return Err(error);
        }
        match rx.recv_timeout(self.timeout) {
            Ok(response) => Ok(response),
            Err(_) => {
                self.stop("node compiler/plugin request timed out; worker terminated");
                Err(FerriteError::Build(format!(
                    "node worker request timed out after {} ms; worker terminated",
                    self.timeout.as_millis()
                )))
            }
        }
    }
}

/// Reader loop: route response lines to waiters by id.
fn reader_loop(stdout: ChildStdout, inner: std::sync::Weak<NodeAdapterInner>) -> impl FnOnce() {
    move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut bytes = Vec::new();
            let size = match reader
                .by_ref()
                .take((MAX_MESSAGE_BYTES + 1) as u64)
                .read_until(b'\n', &mut bytes)
            {
                Ok(size) => size,
                Err(_) => break,
            };
            if size == 0 {
                break;
            }
            if size > MAX_MESSAGE_BYTES {
                if let Some(inner) = inner.upgrade() {
                    inner.stop("node reply exceeds the 16 MiB limit");
                }
                break;
            }
            let Ok(line) = std::str::from_utf8(&bytes) else {
                if let Some(inner) = inner.upgrade() {
                    inner.stop("node protocol returned non-UTF8 data");
                }
                break;
            };
            if serde_json::from_str::<serde_json::Value>(line).is_ok_and(|value| {
                value.as_object().is_some_and(|map| {
                    map.len() == 1 && map.get("ferrite") == Some(&serde_json::json!(3))
                })
            }) {
                if let Some(inner) = inner.upgrade() {
                    if let Ok(mut pending) = inner.pending.lock() {
                        if let Some(tx) = pending.remove(&0) {
                            tx.send(AdapterResponse {
                                id: 0,
                                ok: true,
                                result: serde_json::Value::Null,
                                error: None,
                            })
                            .ok();
                        }
                    }
                }
                continue;
            }
            let parsed: std::result::Result<AdapterResponse, _> = serde_json::from_str(line);
            let Ok(response) = parsed else {
                if let Some(inner) = inner.upgrade() {
                    inner.stop(
                        "invalid node protocol response; guest output must use the log channel",
                    );
                }
                break;
            };
            if let Some(inner) = inner.upgrade() {
                let tx = inner
                    .pending
                    .lock()
                    .ok()
                    .and_then(|mut pending| pending.remove(&response.id));
                if let Some(tx) = tx {
                    tx.send(response).ok();
                }
            } else {
                break;
            }
        }
        if let Some(inner) = inner.upgrade() {
            inner.stop("node worker exited or closed its protocol channel");
        }
    }
}

/// Stage the adapter script (`.mjs`, run directly by Node).
fn write_adapter_script() -> Result<tempfile::TempPath> {
    let mut file = tempfile::Builder::new()
        .prefix("ferrite-node-adapter-")
        .suffix(".mjs")
        .tempfile()?;
    file.write_all(ADAPTER_SCRIPT.as_bytes())?;
    file.as_file().sync_all()?;
    Ok(file.into_temp_path())
}

/// Portable `PATH` lookup (no permission-bit checks: Windows-safe).
fn find_on_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    let names: Vec<String> = if cfg!(windows) {
        vec![
            format!("{name}.exe"),
            format!("{name}.cmd"),
            name.to_string(),
        ]
    } else {
        vec![name.to_string()]
    };
    std::env::split_paths(&paths).find_map(|dir| {
        names
            .iter()
            .map(|name| dir.join(name))
            .find(|path| path.is_file())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fake `node`: replays canned responses for `register`/`hook`.
    fn fake_node_script(dir: &std::path::Path, behavior: &str) {
        let script = format!(
            "#!/bin/sh\n\
             echo '{{\"ferrite\":3}}'\n\
             while IFS= read -r line; do\n\
             {behavior}\n\
             done\n"
        );
        let path = dir.join("node");
        std::fs::write(&path, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        // Settle the file before exec (avoids ETXTBSY on some filesystems).
        std::fs::File::open(&path)
            .and_then(|file| file.sync_all())
            .ok();
    }

    /// Extract the top-level numeric request id (nested string ids ignored).
    const EXTRACT_ID: &str =
        "id=$(echo \"$line\" | grep -o '\"id\":[0-9][0-9]*' | head -1 | sed 's/[^0-9]*//g');";

    /// Spawn with one ETXTBSY retry (fresh script + immediate exec).
    fn spawn_fake(dir: &std::path::Path) -> NodeAdapterHost {
        match NodeAdapterHost::spawn(Some(dir.join("node"))) {
            Ok(host) => host,
            Err(_) => {
                std::thread::sleep(std::time::Duration::from_millis(50));
                NodeAdapterHost::spawn(Some(dir.join("node"))).expect("fake node spawns")
            }
        }
    }

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ferrite-node-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn hook_roundtrips_through_fake_node() {
        let dir = test_dir("roundtrip");
        // Echo the input back as the result for `hook`; ack `register`.
        fake_node_script(
            &dir,
            &format!(
                "{EXTRACT_ID} echo '{{\"id\":'$id',\"ok\":true,\"result\":{{\"echo\":true}}}}';"
            ),
        );
        let host = spawn_fake(&dir);
        host.register_plugin("echo", "/fake/echo.mjs").unwrap();
        assert!(host.contains("echo"));
        let handle = PluginHandle {
            name: "echo".to_string(),
            host: "node-adapter".to_string(),
        };
        let result = host
            .call_hook(
                &handle,
                HookName::Transform,
                serde_json::json!({"id": "/a.js"}),
            )
            .await
            .unwrap();
        assert_eq!(result, serde_json::json!({"echo": true}));
    }

    #[test]
    fn guest_throw_is_loud() {
        let dir = test_dir("throw");
        fake_node_script(
            &dir,
            &format!("{EXTRACT_ID} echo '{{\"id\":'$id',\"ok\":false,\"error\":\"boom\"}}';"),
        );
        let host = spawn_fake(&dir);
        let error = host.register_plugin("bad", "/fake/bad.mjs").unwrap_err();
        assert!(error.to_string().contains("boom"), "{error}");
    }

    #[tokio::test]
    async fn unknown_plugin_is_loud() {
        let dir = test_dir("unknown");
        fake_node_script(&dir, "true");
        let host = spawn_fake(&dir);
        let handle = PluginHandle {
            name: "ghost".to_string(),
            host: "node-adapter".to_string(),
        };
        let error = host
            .call_hook(&handle, HookName::Load, serde_json::Value::Null)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("unknown plugin"), "{error}");
    }

    #[tokio::test]
    async fn wrong_host_is_loud() {
        let dir = test_dir("host");
        fake_node_script(&dir, "true");
        let host = spawn_fake(&dir);
        let handle = PluginHandle {
            name: "x".to_string(),
            host: "embedded-js".to_string(),
        };
        let error = host
            .call_hook(&handle, HookName::Load, serde_json::Value::Null)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("embedded-js"), "{error}");
    }

    #[test]
    fn missing_node_is_loud() {
        let result = NodeAdapterHost::spawn(Some(PathBuf::from("/nonexistent-node-xyz")));
        let error = result.err().expect("spawn fails");
        assert!(error.to_string().contains("cannot spawn"), "{error}");
    }

    /// Real end-to-end through the adapter script on a real Node.js.
    #[tokio::test]
    #[ignore = "needs real node on PATH"]
    async fn real_node_roundtrips_through_adapter() {
        assert!(
            super::find_on_path("node").is_some(),
            "real Node test requires node on PATH"
        );
        let dir = test_dir("realnode");
        std::fs::write(
            dir.join("plug.mjs"),
            "export function transform(code, id) {\n\
             \x20 if (String(id).endsWith('.txt')) return { code: String(code).toUpperCase() };\n\
             \x20 return null;\n\
             }\n",
        )
        .unwrap();
        let host = NodeAdapterHost::spawn(None).expect("real node spawns");
        let entry = dir.join("plug.mjs");
        host.register_plugin("upper", &entry.to_string_lossy())
            .unwrap();
        let handle = PluginHandle {
            name: "upper".to_string(),
            host: "node-adapter".to_string(),
        };
        let hit = host
            .call_hook(
                &handle,
                HookName::Transform,
                serde_json::json!({"code": "hello", "id": "/a.txt"}),
            )
            .await
            .unwrap();
        assert_eq!(hit, serde_json::json!({"code": "HELLO"}));
        let miss = host
            .call_hook(
                &handle,
                HookName::Transform,
                serde_json::json!({"code": "x", "id": "/a.js"}),
            )
            .await
            .unwrap();
        assert_eq!(miss, serde_json::Value::Null);
    }
    #[tokio::test]
    #[ignore = "requires real Node; executed explicitly"]
    async fn real_node_validates_factory_hooks_and_context() {
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("factory.mjs");
        std::fs::write(&entry, r#"
export default async function(options) {
  let calls = 0;
  return {
    name: 'factory',
    resolveId: {handler(id, importer, context) { return {id, importer, context, prefix: options.prefix, calls: ++calls}; }},
    load(id, context) { return {id, context}; },
    transform(code, id, context) { if (code === 'watch') return this.addWatchFile('dependency'); return {code: options.prefix + code, id, context, calls: ++calls}; }
  };
}
"#).unwrap();
        let host = NodeAdapterHost::spawn(None).unwrap();
        host.register_hook_plugin(
            "factory",
            &entry.to_string_lossy(),
            serde_json::json!({"prefix": "prefix:"}),
        )
        .unwrap();
        let handle = PluginHandle {
            name: "factory".into(),
            host: "node-adapter".into(),
        };
        let result = host
            .call_hook(
                &handle,
                HookName::ResolveId,
                serde_json::json!({"id":"child", "importer":"/parent.js", "options":{"ssr":true}}),
            )
            .await
            .unwrap();
        assert_eq!(
            result,
            serde_json::json!({"id":"child", "importer":"/parent.js", "context":{"ssr":true}, "prefix":"prefix:", "calls":1})
        );
        let result = host
            .call_hook(
                &handle,
                HookName::Transform,
                serde_json::json!({"code":"source", "id":"/parent.js", "options":{"ssr":false}}),
            )
            .await
            .unwrap();
        assert_eq!(
            result,
            serde_json::json!({"code":"prefix:source", "id":"/parent.js", "context":{"ssr":false}, "calls":2})
        );
        let result = host
            .call_hook(
                &handle,
                HookName::Load,
                serde_json::json!({"id":"/parent.js", "options":{"ssr":true}}),
            )
            .await
            .unwrap();
        assert_eq!(
            result,
            serde_json::json!({"id":"/parent.js", "context":{"ssr":true}})
        );
        let error = host
            .call_hook(
                &handle,
                HookName::Transform,
                serde_json::json!({"code":"watch", "id":"/parent.js"}),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("this.addWatchFile"), "{error}");
        assert_eq!(
            host.call_hook(&handle, HookName::GenerateBundle, serde_json::Value::Null)
                .await
                .unwrap(),
            serde_json::Value::Null
        );
        for (index, source, expected) in [
            (
                0,
                "export default () => ({transform: {order: 'pre', handler() {}}});",
                "order",
            ),
            (1, "export default {buildStart() {}};", "buildStart"),
            (2, "export default {transform: 42};", "invalid hook"),
            (3, "export default () => [];", "arrays"),
            (
                4,
                "export default {transform: {filter: {}, handler() {}}};",
                "filter",
            ),
            (
                5,
                "export default {configureServer() {}};",
                "configureServer",
            ),
        ] {
            let bad = dir.path().join(format!("bad-{index}.mjs"));
            std::fs::write(&bad, source).unwrap();
            let error = host
                .register_hook_plugin("bad", &bad.to_string_lossy(), serde_json::Value::Null)
                .unwrap_err();
            assert!(error.to_string().contains(expected), "{error}");
        }
        host.register_plugin("unconfigured", &entry.to_string_lossy())
            .unwrap();
        let error = host
            .call_hook(
                &PluginHandle {
                    name: "unconfigured".into(),
                    host: "node-adapter".into(),
                },
                HookName::Transform,
                serde_json::Value::Null,
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("register_hook_plugin"),
            "{error}"
        );
        host.shutdown();
    }

    #[tokio::test]
    #[ignore = "requires real Node; executed explicitly"]
    async fn real_node_typed_exports_logs_timeout_and_cancellation() {
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("worker.mjs");
        std::fs::write(
            &entry,
            r#"
            export async function compile(input) {
                console.log('ordinary log'); process.stdout.write('guest stdout log\n');
                await Promise.resolve(); return { value: input.value + 1 };
            }
            export async function hang() { await new Promise(() => {}); }
        "#,
        )
        .unwrap();
        let host = NodeAdapterHost::spawn_with_timeout(None, Duration::from_millis(500)).unwrap();
        host.register_plugin("compiler", &entry.to_string_lossy())
            .unwrap();
        let pid = host.process_id().unwrap();
        for _ in 0..2 {
            assert_eq!(
                host.call_export("compiler", "compile", serde_json::json!({"value": 41}))
                    .await
                    .unwrap()["value"],
                42
            );
            assert_eq!(host.process_id().unwrap(), pid);
        }
        assert!(host
            .call_export("compiler", "missing", serde_json::Value::Null)
            .await
            .unwrap_err()
            .to_string()
            .contains("missing"));
        assert!(host
            .call_export("compiler", "hang", serde_json::Value::Null)
            .await
            .unwrap_err()
            .to_string()
            .contains("timed out"));
        assert!(host
            .call_export("compiler", "compile", serde_json::Value::Null)
            .await
            .unwrap_err()
            .to_string()
            .contains("stopped"));
        assert!(
            host.inner
                .child
                .lock()
                .unwrap()
                .try_wait()
                .unwrap()
                .is_some(),
            "timed-out child must be reaped"
        );
        let host = Arc::new(NodeAdapterHost::spawn(None).unwrap());
        host.register_plugin("compiler", &entry.to_string_lossy())
            .unwrap();
        let running = host.clone();
        let task = tokio::spawn(async move {
            running
                .call_export("compiler", "hang", serde_json::Value::Null)
                .await
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        host.shutdown();
        let error = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert!(
            error.to_string().contains("cancelled") || error.to_string().contains("stopped"),
            "{error}"
        );
    }
}
