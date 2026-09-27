---
title: 运行时
order: 11
---

# 运行时

内嵌 JS 运行时**默认关闭**：普通构建没有 JS 引擎，也不依赖
Node。`JsRuntime`（`ferrite-runtime`，spec §20、§84–§87）是让
Ferrite 不绑定任一引擎的 trait——嵌入一个、不带引擎跑纯 Rust
SSR，或桥出去。

## 后端

```toml
[runtime]
backend = "auto" # auto | none | napi-vm
```

| 后端 | 行为 |
|---|---|
| `auto`（默认） | 纯 Rust SSR，无 JS 引擎。 |
| `none` | 强制关闭，即使有后端。 |
| `napi-vm` | 纯 Rust napi-vm 核心的进程内 JS（无 Node）。 |

```bash
cargo build -p ferrite-cli --features napi-vm
ferrite ssr --runtime napi-vm
```

`--runtime` 按次覆盖配置。

## 预算

```toml
[runtime]
fuel_budget = 10_000_000 # 0 = 引擎默认
loop_budget = 0          # 0 = 引擎默认
```

超预算的 guest 带调用点明确失败，永不挂住服务器。
运行时池在请求间复用温 isolate。

## 值桥

`JsValue` 跨 Rust ↔ JS 边界：`undefined`、`null`、bool、
number、string、array、object、bytes。SSR 适配器把
`SsrContext` 序列化进、把 HTML（或流）反序列化出；RPC 的参数与
返回值走同一座桥。

## `.node` 二进制

`.node` 永远是 SSR 外部（永不打包）。无 allowlist 条目则解析为
抛错 stub，错误可操作：

```toml
[runtime]
native_allow = ["native/addon.node"]

[runtime.native_integrity]
"native/addon.node" = "<64 hex>"
```

启用 `napi-vm` 后，guest 的 `require("./addon.node")` 经进程内
host 加载真正的 Node-API 二进制（Node-API C ABI；
V8/NAN/libuv 插件留在 napi-vm 的 Node sidecar 并明确失败）。
加载前校验完整性哈希；不匹配拒绝启动。

## Tier-2 JS 插件

任意 `JsRuntime` 可经
`ferrite_plugin::js_host::{JsPluginHost, ForeignPluginHost}`
托管 Vite/Rollup 风格 JS 插件（`resolveId`/`load`/
`transform`）：JSON 桥，默认无文件系统/网络/进程权限。见
[插件](plugins)。Tier-3（经 JSON-lines stdio 的真 Node）留给真正
需要 Node 的插件，不配置永不启动。
