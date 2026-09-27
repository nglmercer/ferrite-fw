---
title: Runtime
order: 11
---

# Runtime

The embedded JS runtime is **disabled by default**: plain builds have no
JS engine and no Node dependency. `JsRuntime` (`ferrite-runtime`, spec
§20, §84–§87) is the trait that keeps Ferrite independent of any single
engine — embed one, run pure-Rust SSR without one, or bridge out.

## Backends

```toml
[runtime]
backend = "auto" # auto | none | napi-vm
```

| Backend | Behavior |
|---|---|
| `auto` (default) | Pure-Rust SSR, no JS engine. |
| `none` | Forced off, even where a backend exists. |
| `napi-vm` | In-process JS via pure-Rust napi-vm core (no Node). |

```bash
cargo build -p ferrite-cli --features napi-vm
ferrite ssr --runtime napi-vm
```

`--runtime` overrides the config per invocation.

## Budgets

```toml
[runtime]
fuel_budget = 10_000_000 # 0 = engine default
loop_budget = 0          # 0 = engine default
```

Guests that exceed their budget fail loudly with the call site, never
by hanging the server. The runtime pool reuses warm isolates across
requests.

## Value bridge

`JsValue` crosses the Rust ↔ JS boundary: `undefined`, `null`, bool,
number, string, array, object, bytes. SSR adapters serialize
`SsrContext` in and deserialize HTML (or a stream) out; RPC uses the
same bridge for args and return values.

## `.node` binaries

`.node` files are always SSR-external (never bundled). Without an
allowlist entry they resolve to a stub that throws an actionable error:

```toml
[runtime]
native_allow = ["native/addon.node"]

[runtime.native_integrity]
"native/addon.node" = "<64 hex chars>"
```

With `napi-vm` enabled, guest `require("./addon.node")` loads real
Node-API binaries through napi-vm's in-process host (Node-API C ABI;
V8/NAN/libuv addons stay on napi-vm's Node sidecar and fail loudly).
Integrity hashes are verified before load; a mismatch refuses to start.

## Tier-2 JS plugins

Any `JsRuntime` can host Vite/Rollup-style JS plugins
(`resolveId`/`load`/`transform`) through
`ferrite_plugin::js_host::{JsPluginHost, ForeignPluginHost}`: JSON
bridge, no filesystem/network/process access by default. See
[Plugins](plugins). Tier-3 (real Node over JSON-lines stdio) exists for
plugins that truly need Node, which is never spawned unless configured.
