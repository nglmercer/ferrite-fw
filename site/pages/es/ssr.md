---
title: SSR
order: 10
---

# SSR

El render de servidor tiene tres capas (`ferrite-ssr`, spec
§21–§23, §46–§50): adaptadores (el boundary del framework), carga de
módulos con externals y el runtime de islands + RPC.

```bash
ferrite ssr                    # dev + adaptador shell estático
ferrite ssr --runtime napi-vm  # dev + render(url) del entry-server
```

## Adaptadores

`SsrAdapter` renderiza una URL a HTML. Elige uno:

| Adaptador | Uso |
|---|---|
| `StaticShellAdapter` | shell `index.html` + inyección de preloads (default). |
| `JsSsrAdapter` | `render(url)` en JS embebido vía cualquier `JsRuntime`. |
| `FnAdapter` | Closure Rust inline (`Fn(SsrContext) -> SsrResponse`). |

```rust
// SSR Rust puro (Leptos/Dioxus/custom) — sin motor JS.
let adapter = ferrite::ssr::FnAdapter::new(|ctx| async move {
    Ok(ferrite::ssr::SsrResponse::html(format!("<h1>{}</h1>", ctx.url)))
});
server.set_ssr_adapter(std::sync::Arc::new(adapter)).await;
```

Con `napi-vm`, se carga `src/entry-server.*` que exporte `render(url)`
y se llama por request; sin él se usa el shell estático y se avisa
ruidosamente (ver [Runtime](runtime)).

## Externals

```toml
[ssr]
entry = "src/server.rs"
external = ["sharp"]
no_external = ["my-esm-dep"]
bundle_all = false
```

Los builtins de Node siempre son externos; los binarios `.node` siempre
son externos (nunca se empaquetan) — sin entrada en el allowlist
resuelven a un stub que lanza un error accionable.

## Streaming

`SsrResponse::stream(chunks)` vuelca HTML progresivamente; el inyector
del shell pone los preloads antes de los primeros bytes. El streaming
funciona con todo adaptador — es una forma de respuesta, no un backend.

## Islands

Hidratación parcial sin atarte a un framework:

```rust
let tag = ferrite::ssr::island_tag(&ferrite::ssr::Island {
    name: "Counter".into(),
    props: serde_json::json!({"n": 0}),
    // …
});
```

Las islands renderizan en servidor e hidratan en cliente vía
`island_hydration_script()`. Cada island embarca solo su propio closure.

## Funciones de servidor (RPC)

`POST /_ferrite/rpc/<hash>` llama a un handler Rust registrado. El
encoding negocia vía `Content-Type` y la respuesta lo refleja:

| Content-Type | Encoding |
|---|---|
| `application/json` (default) | JSON |
| `application/msgpack` / `application/x-msgpack` | MessagePack |
| `application/cbor` | CBOR |

Los cuerpos malformados son un error RPC ruidoso, nunca `null` silencioso.

```rust
let mut rpc = ferrite::ssr::RpcRegistry::new();
rpc.register("add", |args: serde_json::Value| async move {
    Ok(serde_json::json!(args["a"].as_i64().unwrap_or(0) + 1))
});
```

## Rutas

`SsrRouter` matchea patrones (`/users/:id`) a handlers y extrae params
(`match_route`). Es agnóstico al framework: úsalo solo o tras tus rutas
Axum en [modo middleware](dev-server).
