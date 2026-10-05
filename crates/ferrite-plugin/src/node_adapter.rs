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
const MAX_PENDING_REQUESTS: usize = 64;

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
    profile: Mutex<Option<NodeHostProfile>>,
    timeout: Duration,
    stopped: AtomicBool,
    _script: tempfile::TempPath,
    /// Reader thread handle (joined on drop after killing the child).
    reader: Mutex<Option<std::thread::JoinHandle<()>>>,
}

/// Actual persistent worker identity, captured before guest evaluation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NodeHostProfile {
    pub node_version: String,
    pub executable: PathBuf,
    pub platform: String,
    pub arch: String,
    pub versions: std::collections::BTreeMap<String, String>,
    pub exec_args: Vec<String>,
    pub node_options: Option<String>,
    pub cwd: PathBuf,
    pub environment_hash: String,
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
            profile: Mutex::new(None),
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

    /// Query/cache the actual worker profile; unknown/malformed profiles fail.
    /// This method starts no worker and never substitutes another executable.
    pub fn profile(&self) -> Result<NodeHostProfile> {
        let mut cached = self.inner.profile.lock().map_err(|_| poison("profile"))?;
        if let Some(profile) = cached.as_ref() {
            return Ok(profile.clone());
        }
        let response = self.inner.request(serde_json::json!({"cmd":"profile"}))?;
        if !response.ok {
            return Err(FerriteError::Build(
                response
                    .error
                    .unwrap_or_else(|| "Node host profile unavailable".into()),
            ));
        }
        let profile: NodeHostProfile =
            serde_json::from_value(response.result).map_err(|error| {
                FerriteError::Build(format!("Node host profile protocol mismatch: {error}"))
            })?;
        if profile.node_version.is_empty()
            || !profile.cwd.is_absolute()
            || profile.environment_hash.len() != 64
            || !profile
                .environment_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || !profile.executable.is_absolute()
            || profile.platform.is_empty()
            || profile.arch.is_empty()
            || profile
                .versions
                .get("node")
                .is_none_or(|version| format!("v{version}") != profile.node_version)
        {
            return Err(FerriteError::Build(
                "Node host profile is incomplete or inconsistent".into(),
            ));
        }
        *cached = Some(profile.clone());
        Ok(profile)
    }

    /// Stable identity of the running host ABI and protocol implementation.
    pub fn cache_identity(&self) -> Result<String> {
        Ok(profile_cache_identity(&self.profile()?))
    }

    /// Files actually loaded by this worker. No static import guessing.
    pub fn loaded_dependencies(&self) -> Result<std::collections::BTreeMap<PathBuf, String>> {
        let response = self
            .inner
            .request(serde_json::json!({"cmd":"dependencies"}))?;
        if !response.ok {
            return Err(FerriteError::Build(
                response
                    .error
                    .unwrap_or_else(|| "Node dependency snapshot unavailable".into()),
            ));
        }
        serde_json::from_value(response.result).map_err(|error| {
            FerriteError::Build(format!(
                "Node dependency snapshot protocol mismatch: {error}"
            ))
        })
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

fn profile_cache_identity(profile: &NodeHostProfile) -> String {
    ferrite_core::Hash::of_str(
        &serde_json::json!({"profile":profile, "bridge":ADAPTER_SCRIPT}).to_string(),
    )
    .0
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
        let started = std::time::Instant::now();
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
        {
            let mut pending = self.pending.lock().map_err(|_| poison("pending"))?;
            // Recheck under the queue lock: shutdown may have drained it since
            // the initial stopped check.
            if self.stopped.load(Ordering::SeqCst) {
                return Err(FerriteError::Build(
                    "node worker is stopped; explicitly create a new enabled host".into(),
                ));
            }
            if pending.len() >= MAX_PENDING_REQUESTS {
                return Err(FerriteError::Build(format!("node worker pending request limit ({MAX_PENDING_REQUESTS}) reached; reduce concurrent compiler/plugin calls or retry after requests finish")));
            }
            pending.insert(id, tx);
        }
        line.push('\n');
        let timed_out = AtomicBool::new(false);
        let timeout_error = || {
            FerriteError::Build(format!(
                "node worker request timed out after {} ms; worker terminated",
                self.timeout.as_millis()
            ))
        };
        // The deadline must also cover waiting for the writer lock and a full
        // pipe. Killing the child releases blocked writes on every platform.
        std::thread::scope(|scope| {
            let (finished_tx, finished_rx) = mpsc::channel::<()>();
            let deadline_expired = &timed_out;
            scope.spawn(move || {
                if matches!(
                    finished_rx.recv_timeout(self.timeout.saturating_sub(started.elapsed())),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ) {
                    deadline_expired.store(true, Ordering::SeqCst);
                    self.stop("node compiler/plugin request timed out; worker terminated");
                }
            });
            let result = (|| {
                let write_result = self
                    .stdin
                    .lock()
                    .map_err(|_| poison("stdin"))?
                    .write_all(line.as_bytes());
                if let Err(error) = write_result {
                    self.stop("node input transport failed; worker terminated");
                    return Err(FerriteError::Build(format!(
                        "tier-3 node adapter: node stdin closed: {error}"
                    )));
                }
                match rx.recv_timeout(self.timeout.saturating_sub(started.elapsed())) {
                    Ok(response) => Ok(response),
                    Err(_) => {
                        timed_out.store(true, Ordering::SeqCst);
                        self.stop("node compiler/plugin request timed out; worker terminated");
                        Err(timeout_error())
                    }
                }
            })();
            let _ = finished_tx.send(());
            if timed_out.load(Ordering::SeqCst) {
                Err(timeout_error())
            } else {
                result
            }
        })
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

    #[test]
    fn host_cache_identity_separates_runtime_and_abi_profiles() {
        let profile = NodeHostProfile {
            node_version: "v26.10.0".into(),
            executable: "/node".into(),
            platform: "linux".into(),
            arch: "x64".into(),
            versions: [
                ("node".into(), "26.10.0".into()),
                ("v8".into(), "tested".into()),
            ]
            .into(),
            exec_args: vec!["--max-old-space-size=256".into()],
            node_options: None,
            cwd: "/project".into(),
            environment_hash: "0".repeat(64),
        };
        let expected = profile_cache_identity(&profile);
        assert_eq!(expected, profile_cache_identity(&profile.clone()));
        for field in [
            "version",
            "executable",
            "platform",
            "arch",
            "abi",
            "args",
            "node-options",
            "cwd",
            "environment",
        ] {
            let mut changed = profile.clone();
            match field {
                "version" => {
                    changed.node_version = "v27.0.0".into();
                    changed.versions.insert("node".into(), "27.0.0".into());
                }
                "executable" => changed.executable = "/different-node".into(),
                "platform" => changed.platform = "darwin".into(),
                "arch" => changed.arch = "arm64".into(),
                "args" => changed.exec_args.push("--conditions=custom".into()),
                "node-options" => changed.node_options = Some("--conditions=custom".into()),
                "cwd" => changed.cwd = "/different-project".into(),
                "environment" => changed.environment_hash = "1".repeat(64),
                _ => {
                    changed.versions.insert("v8".into(), "different".into());
                }
            }
            assert_ne!(expected, profile_cache_identity(&changed), "{field}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn malformed_worker_profiles_fail_without_fallback() {
        for profile in [
            serde_json::Value::Null,
            serde_json::json!({"nodeVersion":"v26.10.0"}),
            serde_json::json!({"nodeVersion":"v26.10.0", "executable":"/node", "platform":"linux", "arch":"x64", "versions":{"node":"27.0.0"}, "execArgs":[], "cwd":"/project", "environmentHash":"0".repeat(64)}),
        ] {
            let dir = tempfile::tempdir().unwrap();
            fake_node_script(
                dir.path(),
                &format!("{EXTRACT_ID} echo '{{\"id\":'$id',\"ok\":true,\"result\":{profile}}}';"),
            );
            let host = spawn_fake(dir.path());
            let error = host.cache_identity().unwrap_err();
            assert!(error.to_string().contains("profile"), "{error}");
            host.shutdown();
        }
    }

    #[tokio::test]
    #[ignore = "requires real Node; executed explicitly"]
    async fn real_node_transitive_plugin_files_invalidate_cache_and_reject_stale_modules() {
        use crate::{ForeignHookPlugin, ForeignPluginHost, Plugin};
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("plugin.mjs");
        let helper = dir.path().join("helper.cjs");
        std::fs::write(&helper, "exports.value = 'first';").unwrap();
        std::fs::write(&entry, "import helper from './helper.cjs'; export default {transform() { return 'export default ' + JSON.stringify(helper.value); }};").unwrap();
        let host = std::sync::Arc::new(NodeAdapterHost::spawn(None).unwrap());
        let plugin =
            ForeignHookPlugin::register(host.clone(), "transitive", &entry, serde_json::json!({}))
                .unwrap();
        let handle = PluginHandle {
            name: "transitive".into(),
            host: "node-adapter".into(),
        };
        let request = || serde_json::json!({"code":"export default 0","id":"/app.js"});
        assert_eq!(
            host.call_hook(&handle, HookName::Transform, request())
                .await
                .unwrap(),
            "export default \"first\""
        );
        let key = plugin.cache_key();
        assert!(host
            .loaded_dependencies()
            .unwrap()
            .contains_key(&helper.canonicalize().unwrap()));
        std::fs::write(&helper, "exports.value = 'second';").unwrap();
        assert_ne!(key, plugin.cache_key());
        let error = host
            .call_hook(&handle, HookName::Transform, request())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("dependency changed"), "{error}");
        host.shutdown();
        let fresh = std::sync::Arc::new(NodeAdapterHost::spawn(None).unwrap());
        let plugin =
            ForeignHookPlugin::register(fresh.clone(), "transitive", &entry, serde_json::json!({}))
                .unwrap();
        assert_ne!(key, plugin.cache_key());
        assert_eq!(
            fresh
                .call_hook(&handle, HookName::Transform, request())
                .await
                .unwrap(),
            "export default \"second\""
        );
        std::fs::remove_file(&helper).unwrap();
        let error = fresh
            .call_hook(&handle, HookName::Transform, request())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("dependency changed"), "{error}");
        fresh.shutdown();
    }

    #[tokio::test]
    #[ignore = "requires real Node; executed explicitly"]
    async fn real_node_rejects_late_hook_dependencies_persistently() {
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("plugin.mjs");
        std::fs::write(dir.path().join("late.mjs"), "export const value = 'late';").unwrap();
        std::fs::write(&entry, "export default {async transform() {const {value} = await import('./late.mjs'); return 'export default ' + JSON.stringify(value); }};").unwrap();
        let host = NodeAdapterHost::spawn(None).unwrap();
        host.register_hook_plugin("late", &entry.to_string_lossy(), serde_json::json!({}))
            .unwrap();
        let handle = PluginHandle {
            name: "late".into(),
            host: "node-adapter".into(),
        };
        for _ in 0..2 {
            let error = host
                .call_hook(
                    &handle,
                    HookName::Transform,
                    serde_json::json!({"code":"", "id":"/app.js"}),
                )
                .await
                .unwrap_err();
            assert!(
                error.to_string().contains("recreate the explicit host"),
                "{error}"
            );
            assert!(error.to_string().contains("during"), "{error}");
        }
        host.shutdown();
    }

    #[tokio::test]
    #[ignore = "requires real Node; executed explicitly"]
    async fn real_node_reports_stable_running_host_identity() {
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("probe.mjs");
        std::fs::write(&entry, "import {createHash} from 'node:crypto'; export function probe() { return {nodeVersion:process.version, executable:process.execPath, platform:process.platform, arch:process.arch, versions:process.versions, execArgs:process.execArgv, nodeOptions:process.env.NODE_OPTIONS ?? null, cwd:process.cwd(), environmentHash:createHash('sha256').update(JSON.stringify(Object.entries(process.env).sort(([a],[b]) => a < b ? -1 : a > b ? 1 : 0))).digest('hex')}; }").unwrap();
        let host = NodeAdapterHost::spawn(None).unwrap();
        let profile = host.profile().unwrap();
        let identity = host.cache_identity().unwrap();
        host.register_plugin("probe", &entry.to_string_lossy())
            .unwrap();
        let observed: NodeHostProfile = serde_json::from_value(
            host.call_export("probe", "probe", serde_json::Value::Null)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(profile.node_version, observed.node_version);
        assert_eq!(profile.versions, observed.versions);
        assert_eq!(profile.exec_args, observed.exec_args);
        assert_eq!(profile.node_options, observed.node_options);
        assert_eq!(profile.cwd, observed.cwd.canonicalize().unwrap());
        assert_eq!(profile.environment_hash, observed.environment_hash);
        assert_eq!(
            profile.executable,
            observed.executable.canonicalize().unwrap()
        );
        assert_eq!(identity, host.cache_identity().unwrap());
        let other = NodeAdapterHost::spawn(None).unwrap();
        assert_ne!(host.process_id().unwrap(), other.process_id().unwrap());
        assert_eq!(identity, other.cache_identity().unwrap());
        host.shutdown();
        other.shutdown();
    }

    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "requires real Node; executed explicitly"]
    async fn real_node_environment_and_cwd_isolate_transform_cache_keys() {
        use crate::Plugin;
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("working-a");
        let second = dir.path().join("working-b");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        let entry = dir.path().join("plugin.mjs");
        std::fs::write(&entry, "export default () => { const selected = process.env.FERRITE_HOST_FIXTURE + '|' + process.cwd(); return {transform(code) { return {code:code.replace(\"'original'\", JSON.stringify(selected))}; }}; };").unwrap();
        let node = find_on_path("node")
            .expect("real Node required")
            .canonicalize()
            .unwrap();
        let quote =
            |path: &std::path::Path| format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"));
        let mut keys = Vec::new();
        let mut profiles = Vec::new();
        for (index, value, cwd) in [
            (0, "private-environment-value-alpha", &first),
            (1, "private-environment-value-beta", &first),
            (2, "private-environment-value-alpha", &second),
        ] {
            let wrapper = dir.path().join(format!("node-env-{index}"));
            std::fs::write(
                &wrapper,
                format!(
                    "#!/bin/sh\ncd {} || exit 1\nFERRITE_HOST_FIXTURE='{value}' exec {} \"$@\"\n",
                    quote(cwd),
                    quote(&node)
                ),
            )
            .unwrap();
            std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
            let host = Arc::new(NodeAdapterHost::spawn(Some(wrapper)).unwrap());
            let profile = host.profile().unwrap();
            assert_eq!(profile.cwd, cwd.canonicalize().unwrap());
            assert!(
                !serde_json::to_string(&profile).unwrap().contains(value),
                "raw environment value must not appear in host metadata"
            );
            let plugin = crate::ForeignHookPlugin::register(
                host.clone(),
                "fixture",
                &entry,
                serde_json::json!({}),
            )
            .unwrap();
            keys.push(plugin.cache_key());
            let result = crate::ForeignPluginHost::call_hook(
                host.as_ref(),
                &PluginHandle {
                    name: "fixture".into(),
                    host: "node-adapter".into(),
                },
                HookName::Transform,
                serde_json::json!({"code":"export const source = 'original';", "id":"/source.js"}),
            )
            .await
            .unwrap();
            let selected = format!("{value}|{}", cwd.canonicalize().unwrap().display());
            assert_eq!(
                result["code"],
                format!(
                    "export const source = {};",
                    serde_json::to_string(&selected).unwrap()
                )
            );
            profiles.push(profile);
            host.shutdown();
        }
        assert_eq!(profiles[0].cwd, profiles[1].cwd);
        assert_ne!(profiles[0].environment_hash, profiles[1].environment_hash);
        assert_ne!(
            keys[0], keys[1],
            "startup environment must isolate transformed output"
        );
        assert_ne!(
            keys[0], keys[2],
            "startup cwd must isolate transformed output"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "requires real Node; executed explicitly"]
    async fn real_node_rejects_environment_drift_during_guest_calls() {
        let dir = tempfile::tempdir().unwrap();
        for (index, source, stage) in [
            (0, "export default () => {process.env.FERRITE_HOST_MUTATION = 'changed'; return {transform(code) {return code}};};", "registration"),
            (1, "export default () => ({transform(code) {process.env.FERRITE_HOST_MUTATION = 'changed'; return code;}}); export function noop() {return true;}", "hook"),
            (2, "export function mutate(input) {process.chdir(input.cwd); return 'discarded';} export function noop() {return true;}", "export"),
        ] {
            let entry = dir.path().join(format!("mutation-{index}.mjs"));
            std::fs::write(&entry, source).unwrap();
            let host = NodeAdapterHost::spawn(None).unwrap();
            let error = match stage {
                "registration" => host.register_hook_plugin("mutation", &entry.to_string_lossy(), serde_json::json!({})).unwrap_err(),
                "hook" => {
                    host.register_hook_plugin("mutation", &entry.to_string_lossy(), serde_json::json!({})).unwrap();
                    crate::ForeignPluginHost::call_hook(&host, &PluginHandle {name:"mutation".into(), host:"node-adapter".into()}, HookName::Transform, serde_json::json!({"code":"export const value = 1;", "id":"/entry.js"})).await.unwrap_err()
                },
                _ => {
                    host.register_plugin("mutation", &entry.to_string_lossy()).unwrap();
                    host.call_export("mutation", "mutate", serde_json::json!({"cwd":dir.path()})).await.unwrap_err()
                }
            };
            assert!(error.to_string().contains("environment or cwd changed"), "{error}");
            assert!(error.to_string().contains("recreate the explicit host"), "{error}");
            if stage != "registration" {
                let error = host.call_export("mutation", "noop", serde_json::Value::Null).await.unwrap_err();
                assert!(error.to_string().contains("before call"), "{error}");
            }
            host.shutdown();
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "requires real Node; executed explicitly"]
    async fn real_node_conditions_isolate_foreign_transform_cache_keys() {
        use crate::Plugin;
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let package = dir.path().join("node_modules/conditional-fixture");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join("package.json"), r#"{"name":"conditional-fixture","type":"module","exports":{"ferrite-identity-test":"./custom.js","default":"./default.js"}}"#).unwrap();
        std::fs::write(package.join("default.js"), "export default 'default';").unwrap();
        std::fs::write(package.join("custom.js"), "export default 'custom';").unwrap();
        let entry = dir.path().join("plugin.mjs");
        std::fs::write(&entry, "import selected from 'conditional-fixture'; export function probe() { return selected; } export default () => ({transform(code) { return {code: code.replace(\"'original'\", JSON.stringify(selected))}; }});").unwrap();
        let node = find_on_path("node")
            .expect("real Node required")
            .canonicalize()
            .unwrap();
        let quoted_node = format!("'{}'", node.to_string_lossy().replace('\'', "'\\''"));
        let mut keys = Vec::new();
        let mut profiles = Vec::new();
        for (index, flags, selected) in [
            (0, "", "default"),
            (1, "--conditions=ferrite-identity-test", "custom"),
        ] {
            let wrapper = dir.path().join(format!("node-{index}"));
            std::fs::write(
                &wrapper,
                format!("#!/bin/sh\nNODE_OPTIONS='{flags}' exec {quoted_node} \"$@\"\n"),
            )
            .unwrap();
            std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
            let host = Arc::new(NodeAdapterHost::spawn(Some(wrapper)).unwrap());
            let profile = host.profile().unwrap();
            assert_eq!(profile.node_options.as_deref(), Some(flags));
            profiles.push(profile);
            let plugin = crate::ForeignHookPlugin::register(
                host.clone(),
                "fixture",
                &entry,
                serde_json::json!({}),
            )
            .unwrap();
            keys.push(plugin.cache_key());
            assert_eq!(
                host.call_export("fixture", "probe", serde_json::Value::Null)
                    .await
                    .unwrap(),
                selected
            );
            let result = crate::ForeignPluginHost::call_hook(
                host.as_ref(),
                &PluginHandle {
                    name: "fixture".into(),
                    host: "node-adapter".into(),
                },
                HookName::Transform,
                serde_json::json!({"code":"export const source = 'original';", "id":"/source.js"}),
            )
            .await
            .unwrap();
            assert_eq!(
                result["code"],
                format!("export const source = \"{selected}\";")
            );
            host.shutdown();
        }
        assert_eq!(profiles[0].executable, profiles[1].executable);
        assert_eq!(profiles[0].versions, profiles[1].versions);
        assert_eq!(profiles[0].exec_args, profiles[1].exec_args);
        assert_ne!(
            keys[0], keys[1],
            "conditional package code must not share a transform key"
        );
    }

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
    async fn real_node_hook_entries_share_canonical_file_identity() {
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("plugin with spaces.mjs");
        std::fs::write(
            &entry,
            r#"
let factories = 0;
export function probe() { return factories; }
export function entryURL() { return import.meta.url; }
export default () => { factories++; return {transform(code) { return code; }}; };
"#,
        )
        .unwrap();
        let host = NodeAdapterHost::spawn(None).unwrap();
        host.register_hook_plugin("initial", &entry.to_string_lossy(), serde_json::Value::Null)
            .unwrap();
        let file_url = host
            .call_export("initial", "entryURL", serde_json::Value::Null)
            .await
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        let aliases = [
            file_url.clone(),
            dir.path()
                .join("./plugin with spaces.mjs")
                .to_string_lossy()
                .into_owned(),
        ];
        for alias in aliases {
            let error = host
                .register_hook_plugin("alias", &alias, serde_json::Value::Null)
                .unwrap_err();
            assert!(error.to_string().contains("entry was loaded"), "{error}");
        }
        #[cfg(unix)]
        {
            let alias = dir.path().join("symlink.mjs");
            std::os::unix::fs::symlink(&entry, &alias).unwrap();
            let error = host
                .register_hook_plugin("symlink", &alias.to_string_lossy(), serde_json::Value::Null)
                .unwrap_err();
            assert!(error.to_string().contains("entry was loaded"), "{error}");
        }
        for invalid in [
            format!("{file_url}?version=2"),
            format!("{file_url}#changed"),
            "data:text/javascript,export default {}".into(),
        ] {
            let error = host
                .register_hook_plugin("invalid", &invalid, serde_json::Value::Null)
                .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("local file URL without query or fragment"),
                "{error}"
            );
        }
        assert_eq!(
            host.call_export("initial", "probe", serde_json::Value::Null)
                .await
                .unwrap(),
            1
        );
        host.shutdown();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires real Node; executed explicitly"]
    async fn real_node_stalled_input_obeys_request_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("paused.mjs");
        std::fs::write(&entry, "export function pause() { process.stdin.pause(); setInterval(() => {}, 1000); return true; } export function consume(value) { return value.length; }").unwrap();
        let host = Arc::new(
            NodeAdapterHost::spawn_with_timeout(None, Duration::from_millis(500)).unwrap(),
        );
        host.register_plugin("compiler", &entry.to_string_lossy())
            .unwrap();
        host.call_export("compiler", "pause", serde_json::Value::Null)
            .await
            .unwrap();
        let worker = host.clone();
        let mut task = tokio::spawn(async move {
            worker
                .call_export(
                    "compiler",
                    "consume",
                    serde_json::json!("x".repeat(4 * 1024 * 1024)),
                )
                .await
        });
        let completed = tokio::time::timeout(Duration::from_secs(2), &mut task).await;
        if completed.is_err() {
            // Cleanup keeps the regression safe even before the fix.
            host.shutdown();
            let _ = task.await;
            panic!("request blocked in stdin beyond its deadline");
        }
        let error = completed.unwrap().unwrap().unwrap_err();
        assert!(error.to_string().contains("timed out"), "{error}");
        assert!(host.inner.pending.lock().unwrap().is_empty());
        assert!(host
            .inner
            .child
            .lock()
            .unwrap()
            .try_wait()
            .unwrap()
            .is_some());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "requires real Node; executed explicitly"]
    async fn real_node_pending_limit_and_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("pending.mjs");
        std::fs::write(
            &entry,
            "export async function hang() { await new Promise(() => {}); }",
        )
        .unwrap();
        let host = Arc::new(NodeAdapterHost::spawn(None).unwrap());
        host.register_plugin("compiler", &entry.to_string_lossy())
            .unwrap();
        let mut calls = tokio::task::JoinSet::new();
        for _ in 0..MAX_PENDING_REQUESTS {
            let worker = host.clone();
            calls.spawn(async move {
                worker
                    .call_export("compiler", "hang", serde_json::Value::Null)
                    .await
            });
        }
        tokio::time::timeout(Duration::from_secs(5), async {
            while host.inner.pending.lock().unwrap().len() != MAX_PENDING_REQUESTS {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let error = host
            .call_export("compiler", "hang", serde_json::Value::Null)
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("pending request limit (64)"),
            "{error}"
        );
        assert!(!host.inner.stopped.load(Ordering::SeqCst));
        assert_eq!(
            host.inner.pending.lock().unwrap().len(),
            MAX_PENDING_REQUESTS
        );
        host.shutdown();
        tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(result) = calls.join_next().await {
                let error = result.unwrap().unwrap_err();
                assert!(error.to_string().contains("shut down"), "{error}");
            }
        })
        .await
        .unwrap();
        assert!(host.inner.pending.lock().unwrap().is_empty());
        assert!(host
            .inner
            .child
            .lock()
            .unwrap()
            .try_wait()
            .unwrap()
            .is_some());
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
            export function spin() { while (true) {} }
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
        let spinning =
            NodeAdapterHost::spawn_with_timeout(None, Duration::from_millis(500)).unwrap();
        spinning
            .register_plugin("compiler", &entry.to_string_lossy())
            .unwrap();
        let started = std::time::Instant::now();
        let error = spinning
            .call_export("compiler", "spin", serde_json::Value::Null)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("timed out"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(
            spinning
                .inner
                .child
                .lock()
                .unwrap()
                .try_wait()
                .unwrap()
                .is_some(),
            "CPU-bound timed-out worker must be reaped"
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
