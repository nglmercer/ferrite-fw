# Ferrite — Rust-Native SSR Web Toolchain

Vite-like developer experience with a Rust-native dev server, JS/TS
compilation, npm resolution, SSR runtime, and production packager —
**without requiring Node.js** at runtime or for normal development/builds.

Full design: [`ferrite-rust-ssr-framework-spec.md`](ferrite-rust-ssr-framework-spec.md).

## Quickstart

```bash
cargo build --release -p ferrite-cli
alias ferrite=./target/release/ferrite

ferrite create my-app
cd my-app
ferrite dev
```

No `node`, `npm`, `pnpm`, `bun`, or `node_modules` required.

```bash
ferrite add react react-dom three   # installs into .ferrite/npm/
ferrite build                        # → dist/ + manifest.json
ferrite preview                      # serve dist/ locally
ferrite ssr                          # dev + SSR shell adapter
ferrite transform src/main.ts        # one-shot transform
ferrite inspect                      # resolved config / plugins / lockfile
ferrite compat                       # live self-checks
ferrite clean                        # remove .ferrite/
```

## Programmatic API

```rust
#[tokio::main]
async fn main() -> ferrite::Result<()> {
    let server = ferrite::create_server(ferrite::Config::default()).await?;
    server.listen().await?;
    Ok(())
}
```

```rust
let builder = ferrite::create_builder(ferrite::Config::default()).await?;
builder.build_app().await?;          // client (+ ssr when an entry exists)
```

Middleware mode embeds the dev pipeline into an existing Axum app:

```rust
let server = ferrite::create_server(ferrite::Config::default()).await?;
let app = my_router.merge(server.router());
```

## Workspace

| Crate | Role (spec) |
|---|---|
| `ferrite` | Public facade: `create_server`, `create_builder`, `build`, `preview` (§9) |
| `ferrite-core` | Errors, `ModuleId`, `ModuleType`, environments, hashing (§64) |
| `ferrite-config` | `ferrite.toml` loading + resolution (§10) |
| `ferrite-resolver` | Aliases, npm, exports/imports, conditions (§15) |
| `ferrite-graph` | Environment-aware module graph, HMR boundaries (§7, §34) |
| `ferrite-transform` | `JsCompiler` trait + Oxc backend (§5, §90) |
| `ferrite-plugin` | Rust plugin API, container, hook filters (§11–§14) |
| `ferrite-server` | Dev server: HTTP/WS, pipelines, watcher (§24–§26) |
| `ferrite-hmr` | HMR protocol + browser client (§32–§35) |
| `ferrite-html` | Entry discovery, core rewrites, tag injection (§31) |
| `ferrite-css` | Imports, modules, injection, minify (§29) |
| `ferrite-assets` | `?raw`/`?url`/`?inline`/`?worker`/`?wasm`, hashing (§30) |
| `ferrite-npm` | Registry, semver, tarballs, `ferrite.lock` (§16) |
| `ferrite-bundler` | Chunks, hashing, manifests, source maps (§37–§39) |
| `ferrite-runtime` | `JsRuntime` trait, value bridge, pool (§20, §84–§87) |
| `ferrite-ssr` | Adapters, externals, streaming, islands, RPC (§21–§23, §46–§50) |
| `ferrite-cache` | Memory/disk caches, transform keys (§59–§61) |
| `ferrite-manifest` | Client + SSR manifest schemas (§40) |
| `ferrite-wasm` | `rust:` packages, WASM loader (§45, §74) |
| `ferrite-test` | Temp projects, fixtures, assertions (§67) |
| `ferrite-cli` | `ferrite` binary (§1, §80) |

Plus `packages/ferrite-client` (typed HMR client reference) and
`examples/{vanilla-ts,react,rust-wasm,ssr}`.

## Conventions

- Dev serves native ESM per module; bare imports rewrite to `/@npm/<pkg>@<ver>/…`.
- Virtual modules resolve to `\0…` internally and serve at `/@id/…`.
- Asset imports rewrite to `…?asset-shim` (JS URL export); plain URLs serve raw bytes.
- Stylesheet `<link>`s rewrite to `…?direct` (CSS, not the JS wrapper).
- npm installs live in `.ferrite/npm/packages/`; `ferrite.lock` pins them.
- Only `FERRITE_*` / `PUBLIC_*` env vars reach `import.meta.env` (configurable).

## v0.1 scope vs roadmap

v0.1 delivers spec §101: `dev`/`build`, JS/TS/JSX/TSX/CSS/JSON/assets, Oxc
transforms, npm ESM packages, module graph, core plugin hooks, HMR,
production bundle + manifest, SSR foundations (adapters, `ssrLoadModule`,
externals, streaming, islands, RPC), and the `ferrite` CLI.

Explicitly roadmap (documented at each site): SWC backend, scope-hoisted
concatenation / Rolldown backend, statement-level tree-shaking, CSS file
extraction, embedded QuickJS/V8 backend, JS plugin hosting, single-binary
asset embedding, cross-target builds, remote imports, Vue/Svelte plugins.

## Testing

```bash
cargo test --workspace   # unit + tests/vite-compat/ suite, no Node needed
ferrite compat            # live self-checks against the real pipeline
```

## License

MIT — see [LICENSE-MIT](LICENSE-MIT).
