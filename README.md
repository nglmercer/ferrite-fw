# Ferrite — Rust-Native SSR Web Toolchain

Vite-like developer experience with a Rust-native dev server, JS/TS
compilation, npm resolution, SSR runtime, and production packager —
**without requiring Node.js** at runtime or for normal development/builds.

Documentation: [guides, examples and E2E reference](docs/README.md).

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
ferrite e2e                          # boot server + run tests/e2e.rs (Chromium)
ferrite e2e --check                  # verify Chromium launches
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
| `ferrite-plugin` | Rust plugin API, container, hook filters, tier-2 JS host (§11–§14, §56–§57) |
| `ferrite-server` | Dev server: HTTP/WS, pipelines, watcher (§24–§26) |
| `ferrite-hmr` | HMR protocol + browser client (§32–§35) |
| `ferrite-html` | Entry discovery, core rewrites, tag injection, import maps (§31) |
| `ferrite-css` | Imports, modules, injection, minify (§29) |
| `ferrite-assets` | `?raw`/`?url`/`?inline`/`?worker`/`?wasm`, hashing (§30) |
| `ferrite-npm` | Registry, semver, tarballs, `ferrite.lock` (§16) |
| `ferrite-bundler` | Chunks, hashing, manifests, source maps (§37–§39) |
| `ferrite-runtime` | `JsRuntime` trait, value bridge, pool, opt-in napi-vm backend (§20, §84–§87) |
| `ferrite-ssr` | Adapters, externals, `.node` shims, streaming, islands, RPC (§21–§23, §46–§50) |
| `ferrite-cache` | Memory/disk caches, transform keys (§59–§61) |
| `ferrite-manifest` | Client + SSR manifest schemas (§40) |
| `ferrite-wasm` | `rust:` packages, WASM loader (§45, §74) |
| `ferrite-test` | Temp projects, fixtures, assertions (§67) |
| `ferrite-e2e` | Chromium-first e2e: CDP browser, pages, locators, runner |
| `ferrite-cli` | `ferrite` binary (§1, §80) |

Plus `packages/ferrite-client` (typed HMR client reference) and
`examples/{vanilla-ts,react,rust-wasm,ssr}`.

## Embedded runtime, `.node`, and import maps

The embedded JS runtime is **disabled by default**: plain builds have no JS
engine and no Node dependency. Opt in with the `napi-vm` cargo feature
(pure-Rust [napi-vm](https://github.com/nglmercer/napi-vm) core, no Node):

```bash
cargo build -p ferrite-cli --features napi-vm
ferrite ssr --runtime napi-vm     # SSR via src/entry-server.* `render(url)`
```

```toml
# ferrite.toml
[runtime]
backend = "napi-vm"              # auto | none | napi-vm
fuel_budget = 10_000_000         # 0 = engine default
native_allow = ["native/addon.node"]

[runtime.native_integrity]
"native/addon.node" = "<64 hex chars>"
```

`.node` binaries are always SSR-external (never bundled); without an
allowlist entry they resolve to a stub that throws an actionable error.
With the backend enabled, guest `require("./addon.node")` loads real
Node-API binaries through napi-vm's in-process host (Node-API C ABI;
V8/NAN/libuv addons stay on napi-vm's Node sidecar and fail loudly).

```toml
# ferrite.toml — dev-only import maps (build keeps hashed rewrites)
[npm]
dev_strategy = "import-map"      # rewrite (default) | import-map
```

With `import-map`, dev leaves bare specifiers untouched and injects an
inline `<script type="importmap">` (collected from the entry closure)
before the first module script. SSR keeps server-side rewriting.

JS (Vite/Rollup-style) plugins can run tier-2 through
`ferrite_plugin::js_host::{JsPluginHost, ForeignPluginHost}` on any
`JsRuntime` (hooks `resolveId`/`load`/`transform`, JSON bridge, no
filesystem/network/process access by default).

## Remote imports, RPC encodings, source maps

```toml
# ferrite.toml — remote imports (§72, disabled by default)
[remote]
enabled = true
allow = ["esm.example", "*.cdn.example"]
```

Allowed `https://` imports (plain `http` only for loopback) resolve to
virtual modules, fetch once, and cache by URL hash under
`.ferrite/cache/remote/`. Relative imports inside remote modules rebase
onto the remote origin. Fresh checkouts refetch; `ferrite clean` drops
the cache.

Server functions (`/_ferrite/rpc/…`) negotiate the encoding via
`Content-Type`: `application/json` (default), `application/msgpack`
(`application/x-msgpack` accepted), or `application/cbor`. Responses
mirror the request encoding. Malformed bodies are a loud RPC error,
never silent `null` args.

Production minification preserves source maps: the minify step emits
its own map and chains it through the transform map
(`ferrite_transform::chain_source_maps`, §41), so minified builds still
point at original sources. Positions that cannot be resolved pass
through sourceless instead of failing the build.

## Conventions

- Dev serves native ESM per module; bare imports rewrite to `/@npm/<pkg>@<ver>/…` (or stay bare under `[npm] dev_strategy = "import-map"`).
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

Explicitly roadmap (documented at each site): Rolldown backend.
Shipped since v0.1: opt-in napi-vm embedded backend (no Node), tier-2 JS
plugin hosting (`ferrite_plugin::js_host`), dev import maps, `.node` SSR
shims, MessagePack/CBOR RPC encodings, source-map chaining, remote
imports — plus the items below.

## Production build features

- **CSS extraction.** Production builds emit one hashed `.css` file per
  stylesheet module (preserving `@import` order) and inject `<link>` tags
  into the built HTML. Bare `import "./a.css"` statements are stripped
  from the JS; CSS-only modules produce no JS chunk.
- **WASM `cargo build` orchestration.** `ferrite_wasm::ensure_glue` runs
  `cargo build --target wasm32-unknown-unknown` + `wasm-bindgen` when the
  glue is stale (mtime-based), so `.wasm` imports work without manual
  steps. Missing toolchains fail with a loud hint, never silently.
- **Statement-level tree-shaking.** `BundleRequest.treeshake` (on by
  default in production builds) drops unused exports per statement with a
  used-exports fixpoint across the graph, then re-minifies. Barrel
  re-exports survive when any downstream consumer needs them.
- **Single-binary packaging + cross-target builds.** `ferrite build
  --target <triple>` embeds `dist/` into a standalone server binary
  (`BuildReport.standalone_binary`) via `include_bytes!` + gzip, and
  cross-compiles with `cargo build --target` (validated triples only).
- **SWC backend.** Opt-in `swc` cargo feature (`--features swc`) swaps
  the transform/minify engine to `swc_core` (TS strip, JSX
  auto/classic, es2015–2022 lowering, top-level DCE). Default builds
  keep the zero-cost Oxc frontend.
- **Scope-hoisted concatenation.** `ferrite build --scope-hoist` (or
  `BuildConfig.scope_hoist`) concatenates each entry closure into one
  module with `$f{index}$`-prefixed locals, bailing out to chunked
  output for graphs it cannot prove safe (namespaces, `eval`, etc.).
- **React / Vue / Svelte.** `ferrite-frameworks` ships a React plugin
  with partial dev-only Refresh instrumentation, including pinned Oxc hook
  signatures checked against `react-refresh@0.17.0`. A focused React/React DOM
  19.2.0 fixture verifies state preservation, hook-change resets, syntax recovery
  and production preview in Chromium/Firefox without a Node compiler. A
  memo/custom-hook variant also checks recursive signature invalidation.
  A separately imported TypeScript hook checks compatible updates and hook-order
  resets through its importing component. Direct React imports and local exported
  hook names select this instrumentation; hook modules add no component boundary.
  An alias/star re-export chain also checks hook edits and barrel retargeting to
  a newly loaded implementation without stale output or document reload.
  Named default components and anonymous functions exported through `memo` also
  have browser Refresh/reset/production-preview coverage.
  The full component/hook matrix and React generation profiles remain incomplete.
  Experimental Vue 3.5.22 and
  Svelte 5.39.6 compilation uses their official project-matched compilers on
  the explicitly enabled Node compiler host. Client templates are exercised
  in Chromium/Firefox; server compilation alone does not establish SSR rendering.
  Complete React Refresh and framework SSR profiles remain unavailable.
- **Tier-3 Node adapter.** `ferrite_plugin::NodeAdapterHost` hosts
  foreign ESM plugins in a real Node.js over JSON-lines stdio
  (`null` = skip). `register_hook_plugin` validates an explicit subset:
  one factory/object with JSON options, `resolveId(id, importer, options)`,
  `load(id, options)` and `transform(code, id, options)`, including unordered
  `{handler}` hooks. Unsupported hooks, ordering/filter metadata and plugin
  context methods fail loudly. `register_plugin`/`call_export` remains the
  separate typed-export worker contract used by compilers. This bridge does
  not provide complete Vite/Rollup compatibility or a Node sandbox. Node is
  never spawned unless configured; guest throws, unknown plugins and missing
  binaries fail loudly.
  For library pipelines, `ForeignHookPlugin::register(Arc<NodeAdapterHost>,
  name, entry_path, json_options)` returns a native `Plugin` adapter. Resolve
  results accept a string or `{id, external, sideEffects, moduleType, meta}`;
  load/transform accept code strings or `{code, map, dependencies, moduleType}`
  (load also accepts `sideEffects`). `moduleType` uses Ferrite enum names such
  as `Js`/`Ts`; dependencies are filesystem paths. Unknown result fields and
  invalid maps fail. Transform side-effect overrides are currently unsupported.
  Changing the entry requires recreating its host/registration; transitive
  plugin imports are not yet tracked automatically.
  Entry paths and file URLs resolve to the canonical local file, including
  symlink targets. File-URL queries/fragments and non-file URLs are rejected;
  alternate entry spellings cannot bypass hook re-registration checks.

  CLI and library configuration select the same experimental hook profile:

  ```toml
  [[foreign_plugins]]
  name = "my-transform"
  entry = "plugins/transform.mjs"
  host = "node"
  timeout_ms = 10000
  # node = "/path/to/node"  # optional explicit executable

  [foreign_plugins.options]
  prefix = "example"
  ```

  Entry/executable paths resolve from the project root. This selects only the
  plugin host; framework compiler hosts and SSR runtimes remain separate.
  `foreign_plugins = []` explicitly disables inherited profiles. Static JS
  configs accept the same `foreign_plugins` data; dynamic factories belong in
  the entry module, and dynamic profile configuration fails. Doctor only locates
  entries/executables and reports execution as unverified; dev/build validate
  the required hook contract. Inspect omits factory options from its report.
  Running Node hosts expose `profile()` with their actual executable, Node/V8
  and ABI versions, platform, architecture, execution arguments, `NODE_OPTIONS`,
  canonical working directory and a hash of the startup environment.
  Foreign-hook and official
  Vue/Svelte compiler cache identities include this profile and the bridge/
  compiler-wrapper implementation. This is cache identity, not a compatibility
  claim for untested Node versions; doctor still does not start a host.
  Persistent environment/cwd changes during registration or guest calls fail
  with an explicit host-restart error. This consistency check does not sandbox
  Node or track arbitrary files and asynchronous guest state.
  The tested Node 26.10.0 host tracks module files loaded during registration
  using synchronous loader hooks. Foreign-hook caches include those files and
  successful load/transform results retain them as watched dependencies. Editing
  or deleting loaded modules requires a new explicit host. Hooks that first load
  file dependencies during execution fail persistently; import those dependencies
  during registration. Hosts lacking `node:module.registerHooks` fail startup.
- **Markdown docs + vendored utility CSS.** `ferrite-docs` turns `.md`
  files into JS modules exporting rendered HTML; `ferrite-tailwind`
  compiles `ferrite:tailwind.css` from project content using the
  vendored `tailwind-rs` tree (`vendor/`, see `vendor/README.md` for
  the pin). The self-docs site in `site/` is built with Ferrite
  itself and is the project's production-verification target:
  `ferrite build site --standalone`, then serve the binary.

## Testing

```bash
cargo test --workspace   # unit + tests/vite-compat/ suite, no Node needed
ferrite compat            # live self-checks against the real pipeline
```

## End-to-end testing

Playwright-style e2e in pure Rust, no Node required: Chromium over
CDP plus Firefox over WebDriver BiDi behind one `Page` API.
`ferrite e2e` boots the web server (in-process dev server by default),
forwards its resolved configuration through `FERRITE_E2E_CONFIG` (with legacy
environment overrides), and runs your Rust suite. Use `Browser::launch_default()`
and `Runner::from_env()` to consume those settings:

```bash
ferrite e2e --check                 # verify Chromium launches
ferrite e2e --check --engine firefox
ferrite e2e                         # boot + cargo test --test e2e
ferrite e2e --engine firefox        # same suite on Firefox
ferrite e2e --headed --retries 2    # visible browser, retries
ferrite e2e --video only-on-failure # record test videos (webm)
ferrite e2e --grep auth --shard 1/3 # filter by name/tag, run one shard
ferrite e2e --project shop --project blog # run named projects only
ferrite e2e --url http://127.0.0.1:3000/ -- cargo test --test shop
```

```toml
# ferrite.toml
[e2e]
browser = "chromium"                # chromium | firefox
retries = 1
workers = 4
reporter = "list,json"              # list | json | junit (comma-separated)
screenshot = "only-on-failure"      # on | off | only-on-failure
video = "off"                       # on | off | only-on-failure (webm)
video_fps = 10
update_snapshots = "missing"        # missing | changed | all | none
snapshot_path_template = "{snapshotDir}/{browserName}{-projectName}/{platform}/{arg}{ext}"

[e2e.web_server]
url = "http://127.0.0.1:5190/"
```

Suites use the `ferrite_e2e` library (`Browser`, `Page`, locators with
`css`/`text=`/`xpath=`/`role=` engines plus `get_by_*`, `nth`/`first`/`last`,
chaining, filters, and `and_`/`or_`, auto-retrying `expect_*` with `not()`,
immediate `is_*` checks, `bounding_box`/`highlight`/`evaluate`, dialog info
and `wait_for_dialog`, `wait_for_selector`/`wait_for_popup`, permissions and
geolocation (plus clears), context options (locale, timezone, offline,
headers, credentials, device, JS, CSP, downloads, service workers, storage
state) with per-context setters, basic/digest HTTP credentials, context init
scripts, storage-state capture/replay, IndexedDB clear, `set_test_id_attribute`,
request routing
(handlers with fallback, `times` limits, abort reasons, `route_from_har`,
`unroute_all`) and observation, response bodies with HAR embed, aria snapshots with
screenshot/text/aria snapshot assertions (`missing`/`changed`/`all`/`none` update modes),
`Download`
objects with save/delete, screenshots, video recording, live frame streams,
traces, `ApiClient` verbs with `fetch`, parallel `Runner` with tags, sharding,
projects, `repeat_each`, fixtures, `TestInfo` attachments/annotations,
expected failures, `forbid_only`, controlled steps with timeout/skip/annotations,
automatic action/assertion/lifecycle diagnostics, portable HTML report bundles,
URL/network matchers and predicates, generated file uploads and console source metadata,
dot/list/json/junit/html reporters,
and `before_all`/`after_all` hooks). Chromium recordings assemble via `ffmpeg`
(fail-fast hint when missing; `FERRITE_FFMPEG_PATH` override); Firefox
records natively. Tests and retries receive fresh contexts; use
`default_context().new_page()` when explicitly sharing storage between pages.
Locators are strict by default and wait for actionable elements. The API also
includes same-origin lazy `FrameLocator`s, independent clock controls,
Chromium JS/CSS coverage, persistent profiles, context-linked API cookies,
per-project browser/context options, and named test locks. See the
[Playwright comparison](docs/e2e/PLAYWRIGHT-PARITY.md) and
[complete member matrix](docs/e2e/PLAYWRIGHT-API-MATRIX.md) for remaining differences,
and the [completed implementation checklist](docs/e2e/E2E-PARITY-TODO.md)
for task acceptance criteria and history.
See
`examples/e2e/` for a runnable project and
`crates/ferrite-e2e/tests/browser.rs` for coverage (per-engine tests
skip when that browser is missing; point `FERRITE_CHROMIUM_PATH` at any
Chromium/Chrome/headless-shell binary, `FERRITE_FIREFOX_PATH` at Firefox).

Engine notes: stock Firefox exposes BiDi only (one session per
process); user agent, proxy, and certificate acceptance are
launch-wide there (`[e2e] user_agent` / `proxy_server` /
`ignore_https_errors`). Offline, extra headers, locale, timezone, media,
HTTP credentials (basic and digest), device emulation, JavaScript toggle,
CSP bypass, download allow/deny and per-context download dirs, service-worker
blocking, per-origin storage clear, and response modification are
Chromium-only and fail loudly on Firefox; response bodies and HAR-embedded
content are Chromium-only and empty on Firefox (BiDi exposes no body
channel). Client certificates have no automation hook in stock CDP/BiDi and
are unsupported. Ferrite has no WebKit backend; adding one and managed browser
distribution are deferred. Playwright itself supports WebKit on Linux.

## License

MIT — see [LICENSE-MIT](LICENSE-MIT).
