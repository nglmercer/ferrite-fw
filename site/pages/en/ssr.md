---
title: SSR
order: 10
---

# SSR

Server-side rendering has three layers (`ferrite-ssr`, spec
§21–§23, §46–§50): adapters (the framework boundary), module loading
with externals, and the islands + RPC runtime.

```bash
ferrite ssr                    # dev + static shell adapter
ferrite ssr --runtime napi-vm  # dev + entry-server render(url)
```

## Adapters

`SsrAdapter` renders a URL to HTML. Pick one:

| Adapter | Use |
|---|---|
| `StaticShellAdapter` | `index.html` shell + preload injection (default). |
| `JsSsrAdapter` | Embedded-JS `render(url)` via any `JsRuntime`. |
| `FnAdapter` | Inline Rust closure (`Fn(SsrContext) -> SsrResponse`). |

```rust
// Pure-Rust SSR (Leptos/Dioxus/custom) — no JS engine.
let adapter = ferrite::ssr::FnAdapter::new(|ctx| async move {
    Ok(ferrite::ssr::SsrResponse::html(format!("<h1>{}</h1>", ctx.url)))
});
server.set_ssr_adapter(std::sync::Arc::new(adapter)).await;
```

With `napi-vm`, `src/entry-server.*` exporting `render(url)` is loaded
and called per request; without it the static shell is used and noted
loudly (see [Runtime](runtime)).

## Externals

```toml
[ssr]
entry = "src/server.rs"
external = ["sharp"]
no_external = ["my-esm-dep"]
bundle_all = false
```

Node builtins are always external; `.node` binaries are always external
(never bundled) — without an allowlist entry they resolve to a stub
that throws an actionable error.

## Streaming

`SsrResponse::stream(chunks)` flushes HTML progressively; the shell
injector places preloads before the first bytes. Streaming works with
every adapter — it is a response shape, not a backend.

## Islands

Partial hydration without a framework buy-in:

```rust
let tag = ferrite::ssr::island_tag(&ferrite::ssr::Island {
    name: "Counter".into(),
    props: serde_json::json!({"n": 0}),
    // …
});
```

Islands render server-side and hydrate client-side via
`island_hydration_script()`. Each island ships only its own closure.

## Server functions (RPC)

`POST /_ferrite/rpc/<hash>` calls a registered Rust handler. The encoding
negotiates via `Content-Type` and the response mirrors it:

| Content-Type | Encoding |
|---|---|
| `application/json` (default) | JSON |
| `application/msgpack` / `application/x-msgpack` | MessagePack |
| `application/cbor` | CBOR |

Malformed bodies are a loud RPC error, never silent `null` args.

```rust
let mut rpc = ferrite::ssr::RpcRegistry::new();
rpc.register("add", |args: serde_json::Value| async move {
    Ok(serde_json::json!(args["a"].as_i64().unwrap_or(0) + 1))
});
```

## Routing

`SsrRouter` matches patterns (`/users/:id`) to handlers and extracts
params (`match_route`). It is framework-agnostic: use it standalone or
behind your Axum routes in [middleware mode](dev-server).
