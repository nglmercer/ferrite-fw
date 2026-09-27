---
title: Runtime
order: 11
---

# Runtime

El runtime JS embebido está **desactivado por default**: los builds
planos no tienen motor JS ni dependencia de Node. `JsRuntime`
(`ferrite-runtime`, spec §20, §84–§87) es el trait que mantiene a
Ferrite independiente de un motor — embebe uno, corre SSR Rust puro sin
ninguno, o puentea fuera.

## Backends

```toml
[runtime]
backend = "auto" # auto | none | napi-vm
```

| Backend | Comportamiento |
|---|---|
| `auto` (default) | SSR Rust puro, sin motor JS. |
| `none` | Forzado off, aunque haya backend. |
| `napi-vm` | JS en proceso vía core napi-vm en Rust puro (sin Node). |

```bash
cargo build -p ferrite-cli --features napi-vm
ferrite ssr --runtime napi-vm
```

`--runtime` sobreescribe el config por invocación.

## Presupuestos

```toml
[runtime]
fuel_budget = 10_000_000 # 0 = default del motor
loop_budget = 0          # 0 = default del motor
```

Los guests que exceden su presupuesto fallan ruidosamente con el call
site, nunca colgando el servidor. El pool reusa isolates tibios entre
requests.

## Puente de valores

`JsValue` cruza el boundary Rust ↔ JS: `undefined`, `null`, bool,
number, string, array, object, bytes. Los adaptadores SSR serializan
`SsrContext` dentro y deserializan HTML (o un stream) fuera; RPC usa el
mismo puente para args y retornos.

## Binarios `.node`

Los `.node` siempre son externos de SSR (nunca se empaquetan). Sin
entrada en el allowlist resuelven a un stub que lanza un error accionable:

```toml
[runtime]
native_allow = ["native/addon.node"]

[runtime.native_integrity]
"native/addon.node" = "<64 hex>"
```

Con `napi-vm`, el `require("./addon.node")` del guest carga binarios
Node-API reales vía el host en proceso (ABI C de Node-API; los addons
V8/NAN/libuv quedan en el sidecar Node de napi-vm y fallan ruidosamente).
Los hashes de integridad se verifican antes de cargar; un mismatch se
niega a arrancar.

## Plugins JS tier-2

Cualquier `JsRuntime` puede hospedar plugins JS estilo Vite/Rollup
(`resolveId`/`load`/`transform`) vía
`ferrite_plugin::js_host::{JsPluginHost, ForeignPluginHost}`: puente
JSON, sin acceso a filesystem/red/procesos por default. Ver
[Plugins](plugins). El tier-3 (Node real por stdio JSON-lines) existe
para plugins que de verdad necesitan Node, que nunca se lanza salvo
configurado.
