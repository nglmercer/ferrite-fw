---
title: SSR
order: 10
---

# SSR

服务端渲染分三层（`ferrite-ssr`，spec §21–§23、§46–§50）：
适配器（框架边界）、带 externals 的模块加载、islands + RPC 运行时。

```bash
ferrite ssr                    # dev + 静态 shell 适配器
ferrite ssr --runtime napi-vm  # dev + entry-server render(url)
```

## 适配器

`SsrAdapter` 把 URL 渲染成 HTML。三选一：

| 适配器 | 用途 |
|---|---|
| `StaticShellAdapter` | `index.html` shell + preload 注入（默认）。 |
| `JsSsrAdapter` | 经任意 `JsRuntime` 的内嵌 JS `render(url)`。 |
| `FnAdapter` | 内联 Rust 闭包（`Fn(SsrContext) -> SsrResponse`）。 |

```rust
// 纯 Rust SSR（Leptos/Dioxus/自研）——无 JS 引擎。
let adapter = ferrite::ssr::FnAdapter::new(|ctx| async move {
    Ok(ferrite::ssr::SsrResponse::html(format!("<h1>{}</h1>", ctx.url)))
});
server.set_ssr_adapter(std::sync::Arc::new(adapter)).await;
```

`napi-vm` 下加载导出 `render(url)` 的 `src/entry-server.*`，
逐请求调用；没有则用静态 shell 并明确告知（见[运行时](runtime)）。

## Externals

```toml
[ssr]
entry = "src/server.rs"
external = ["sharp"]
no_external = ["my-esm-dep"]
bundle_all = false
```

Node 内置永远外部；`.node` 二进制永远外部（永不打包）——
无 allowlist 条目则解析为抛错 stub，错误可操作。

## 流式

`SsrResponse::stream(chunks)` 渐进 flush HTML；shell 注入器把
preload 放在首字节之前。流式适用于所有适配器——它是响应形态，
不是后端。

## Islands

不绑定框架的部分水合：

```rust
let tag = ferrite::ssr::island_tag(&ferrite::ssr::Island {
    name: "Counter".into(),
    props: serde_json::json!({"n": 0}),
    // …
});
```

Island 服务端渲染，经 `island_hydration_script()` 客户端水合。
每个 island 只带自己的闭包。

## 服务端函数（RPC）

`POST /_ferrite/rpc/<hash>` 调用注册的 Rust handler。
编码经 `Content-Type` 协商，响应镜像：

| Content-Type | 编码 |
|---|---|
| `application/json`（默认） | JSON |
| `application/msgpack` / `application/x-msgpack` | MessagePack |
| `application/cbor` | CBOR |

畸形 body 是明确的 RPC 错误，永不静默 `null` 参数。

```rust
let mut rpc = ferrite::ssr::RpcRegistry::new();
rpc.register("add", |args: serde_json::Value| async move {
    Ok(serde_json::json!(args["a"].as_i64().unwrap_or(0) + 1))
});
```

## 编程式工具

`DevServer` 向工具与测试暴露 Vite SSR 助手：

- `server.ssr_transform(code, url)`（`ssrTransform`）：核心 SSR
  变换 → 插件变换 → CJS 互操作 → 导入重写，不碰图与缓存。
  返回 `SsrTransformResult { code, map, deps, dynamic_deps }`。
- `server.ssr_fix_stacktrace(stack)`（`ssrFixStacktrace`）：经缓存
  的 SSR 变换 map 重写堆栈帧；无缓存 map 的帧只做归一化
  （去源、解码 `/@id/` URL），保留位置。
- `server.module_runner()`（`server.moduleRunner`）：带缓存的
  `ssrLoadModule`，含 `import(url)`、`invalidate(url)`
  （清缓存条目及其图子树）、`clear()`、`cached_urls()` 与
  `close()`。执行加载的图走配置的 `SsrAdapter`。

```rust
let runner = server.module_runner();
let module = runner.import("/src/entry-server.js").await?;
```

## 路由

`SsrRouter` 把模式（`/users/:id`）匹配到 handler 并抽取参数
（`match_route`）。框架无关：独立用，或在[中间件模式](dev-server)下
挂在 Axum 路由后。
