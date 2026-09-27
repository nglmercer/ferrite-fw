---
title: Dev server
order: 5
---

# Dev server

`ferrite dev` serves native ESM per module: resolve → load → transform →
rewrite imports → serve. There is no bundle step in dev, so edits are
visible after reloading (or instantly via HMR) exactly one module.

```bash
ferrite dev [--host 127.0.0.1] [--port 5173] [--open] [--no-hmr]
```

## Pipeline

1. **Resolve** the specifier (`ferrite-resolver`: aliases, npm,
   `exports`/`imports`, conditions, extensions).
2. **Load** through plugin hooks (Markdown, SFCs, WASM glue, `?raw`).
3. **Transform** with Oxc (or opt-in SWC): TS strip, JSX, lowering.
4. **Rewrite** bare imports to `/@npm/<pkg>@<ver>/…` (or leave them bare
   under `[npm] dev_strategy = "import-map"`).
5. **Serve** with ETag + cache headers; errors become loud overlays.

Virtual modules resolve to `\0…` internally and serve at `/@id/…`.
Asset imports rewrite to `…?asset-shim` (a JS URL export); plain URLs
serve raw bytes. Stylesheet `<link>`s rewrite to `…?direct`.

## HMR

The server watches the project (ignoring `node_modules`, `dist`,
`target`, `.ferrite`) and broadcasts over WebSocket:

- `ferrite-hmr` computes the boundary from the module graph
  (`plan_update`): exact importers accept, the rest full-reload.
- The browser client (`ferrite_hmr::client_source`, typed in
  `packages/ferrite-client`) applies the update or reloads the page.
- `import.meta.hot` is rewritten per module; React Refresh adds its
  preamble/footer in dev only (see [Frameworks](frameworks)).
- Disable with `--no-hmr` or `[server] hmr = false`.

```js
if (import.meta.hot) {
  import.meta.hot.accept((next) => render(next));
}
```

## Import maps (dev-only)

```toml
[npm]
dev_strategy = "import-map" # rewrite (default) | import-map
```

With `import-map`, dev leaves bare specifiers untouched and injects an
inline `<script type="importmap">` (collected from the entry closure)
before the first module script. Builds keep hashed rewrites; SSR keeps
server-side rewriting. Use it when you want the browser to resolve bare
imports (CDN shims, externals debugging).

## Environment variables

`.env` files load by mode, later winning (Vite `loadEnv`): `.env`,
`.env.local`, `.env.{mode}`, `.env.{mode}.local`. Values support
`$VAR`, `${VAR}`, `${VAR:-default}`, and `$$` escapes, expanded in one
pass against the process environment first, then already-loaded file
values (unknown variables without a default become empty). Lines
accept `export KEY=value`, `#` comments, single/double quotes (double
quotes interpret `\"` `\\` `\n` `\r` `\t`), and trailing ` # comment`
after unquoted values.

Only `FERRITE_*` / `PUBLIC_*` (configurable via `[env] prefix`) reach
`import.meta.env`; the rest stay server-side, and only file variables
are returned — the process environment feeds expansion, never the
result. Compile-time `define` replacements apply on top. The
programmatic form is `ferrite::load_env(mode, root, prefixes)` plus
`ferrite::expand_vars(value, loaded)`.

## Proxy

```toml
[server.proxy]
"/api" = "http://localhost:3000"
```

Prefix rules forward matching dev and preview requests to the target
origin with path and query preserved (the Vite `server.proxy`
shorthand). Longest prefix wins; matching is segment-boundary, so
`/api` covers `/api/users` but not `/apix`. Status, headers, and
bytes pass through; hop-by-hop headers (`connection`, `upgrade`,
`transfer-encoding`, …) are stripped both ways. JS config uses
`server.proxy` with the same strings or `{ target }` objects; preview
plugins can add more rules via `configure_preview` (see
[Plugins](plugins)).

## Middleware mode

Embed the dev pipeline into an existing Axum app instead of listening:

```rust
let server = ferrite::create_server(ferrite::Config::default()).await?;
let app = my_router.merge(server.router());
```

Set `[server] middleware_mode = true` (or just never call `listen()`).
HMR, the watcher, and the transform cache keep working; your router owns
ports, TLS, and fallback routes.

## HTML handling

Entry scripts are discovered from `index.html` (module scripts first).
The pipeline runs pre hooks → core rewrites → normal hooks → post hooks:
bare-import rewrites, HMR client injection, preload/tag injection, and
the dev import map. See [Configuration](configuration) for entries.
