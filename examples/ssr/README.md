# ssr example

```bash
cd examples/ssr
ferrite ssr        # dev server + SSR shell adapter
ferrite build      # builds client/ + server/ (+ ssr-manifest.json)
```

`src/entry-server.ts` exports `render(url)`; load it in Rust with
`server.ssr_load_module("/src/entry-server.ts")` and evaluate the returned
module through a [`ferrite::runtime::JsRuntime`] backend (or render it with a
pure-Rust [`ferrite::ssr::SsrAdapter`]).
