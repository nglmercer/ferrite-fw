# tests/vite-compat/ — compatibility fixtures (spec §67)

The runnable suite lives at `crates/ferrite-test/tests/vite_compat.rs`
(workspace roots carry no package, so integration tests hang off the
`ferrite-test` crate). This directory holds the fixtures and the
conformance checklist for external runners.

Categories covered:

- config (defaults, file merge, CLI precedence)
- resolver (relative, alias, bare + exports, virtual, node shims)
- plugin ordering (pre/normal/post) + hook chaining
- virtual modules (`virtual:ferrite/env` → `/@id/…`)
- HTML transforms (client injection, entry resolution)
- HMR (boundaries, protocol, client)
- import.meta.env (prefix filtering)
- assets (`?raw`, `?url`, `?inline`, `?worker`, `?wasm`, shims)
- CSS (imports, modules, `?direct`, injection)
- dynamic imports (rewrite + graph edges)
- SSR (`ssrLoadModule` traversal, no client injection)
- SSR externalization (`external` / `noExternal`)
- build manifest (`manifest.json`, hashed HTML)
- source maps (transform maps)

Run: `cargo test --workspace` (unit + integration, no Node required).
