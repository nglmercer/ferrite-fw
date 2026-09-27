# Ferrite — Rust-Native SSR Web Toolchain
## Design & Implementation Specification

> **Goal:** Build a Rust-first development server, SSR runtime, bundler, package manager integration layer, and production packager with a developer experience intentionally similar to Vite — but **without requiring Node.js at runtime or for normal development/builds**.
>
> Working name in this document: **Ferrite**. Rename as desired.

---

## 1. Product Definition

Ferrite should feel like:

```text
Vite DX
+ Rust-native dev server
+ Rust-native JS/TS compilation
+ Rust-native npm package resolution
+ Rust SSR runtime
+ WASM/browser client
+ standalone production packaging
```

Primary commands:

```bash
ferrite dev
ferrite build
ferrite preview
ferrite ssr
ferrite add react
ferrite add three
ferrite inspect
```

A minimum project:

```text
my-app/
├── Cargo.toml
├── ferrite.toml
├── package.json           # optional
├── src/
│   ├── server.rs
│   ├── app.rs
│   └── client.rs
├── web/
│   ├── index.html
│   ├── main.ts
│   └── style.css
└── public/
```

Normal use must not require:

```text
node
npm
pnpm
yarn
bun
vite
webpack
rollup
```

Those tools may be supported for compatibility/testing, but they must not be runtime requirements.

---

# 2. Non-Goals

Ferrite v1 should **not** try to:

- implement a JavaScript VM from scratch;
- emulate every Node.js built-in;
- transparently execute every arbitrary npm package on the server;
- reproduce every Vite implementation detail internally;
- support every React/Vue/Svelte plugin before the core architecture is stable;
- turn Rust into JavaScript or JavaScript into Rust automatically.

Instead, expose clean compatibility layers.

---

# 3. Core Architecture

```text
                         ferrite CLI
                             │
                   ┌─────────┴─────────┐
                   │                   │
              Dev Pipeline        Build Pipeline
                   │                   │
           File watcher          Entry discovery
                   │                   │
            Module graph          Module graph
                   │                   │
        Resolver + plugins    Resolver + plugins
                   │                   │
       JS/TS compiler core    JS/TS compiler core
                   │                   │
            HMR server             Bundler
                   │                   │
        HTTP / WS server      Chunk + asset emit
                   │                   │
             SSR runtime         SSR bundle
                   │                   │
             Browser            standalone app
```

Recommended workspace:

```text
crates/
├── ferrite-cli
├── ferrite-core
├── ferrite-config
├── ferrite-resolver
├── ferrite-graph
├── ferrite-transform
├── ferrite-bundler
├── ferrite-plugin
├── ferrite-server
├── ferrite-hmr
├── ferrite-html
├── ferrite-css
├── ferrite-assets
├── ferrite-npm
├── ferrite-runtime
├── ferrite-ssr
├── ferrite-wasm
├── ferrite-cache
├── ferrite-manifest
└── ferrite-test
```

---

# 4. Recommended Rust Stack

## HTTP / networking

```toml
axum
hyper
tower
tower-http
tokio
tokio-tungstenite
```

Use Axum for the default dev/SSR HTTP server while keeping the core server abstraction independent enough to support Actix, Salvo, Poem, etc.

## File system

```toml
notify
ignore
walkdir
globset
camino
```

## Serialization / configuration

```toml
serde
serde_json
toml
json5
```

## Async / concurrency

```toml
tokio
futures
rayon
dashmap
parking_lot
```

## Diagnostics

```toml
miette
thiserror
tracing
tracing-subscriber
```

## Hashing/cache

```toml
blake3
xxhash-rust
```

---

# 5. JavaScript / TypeScript Compiler Strategy

Ferrite should define its own compiler abstraction:

```rust
pub trait JsCompiler: Send + Sync {
    fn parse(&self, request: ParseRequest) -> Result<ParsedModule>;
    fn transform(&self, request: TransformRequest) -> Result<TransformResult>;
    fn minify(&self, request: MinifyRequest) -> Result<MinifyResult>;
}
```

Support at least two engines:

```text
OxcCompiler
SwcCompiler
```

Recommended default:

```text
Parser       → Oxc
Resolver     → Oxc resolver or custom resolver
Transformer  → Oxc
Minifier     → Oxc
Fallback     → SWC
```

SWC remains useful for ecosystem compatibility and transforms not yet supported by the default backend.

Example config:

```toml
[compiler]
engine = "oxc"

[compiler.fallback]
engine = "swc"
```

Or:

```rust
pub enum CompilerEngine {
    Oxc,
    Swc,
    Custom(Arc<dyn JsCompiler>),
}
```

Do **not** tightly couple the module graph to an Oxc or SWC AST.

---

# 6. Source Types

Native module types:

```rust
pub enum ModuleType {
    Js,
    Jsx,
    Ts,
    Tsx,
    Json,
    Css,
    Html,
    Wasm,
    Asset,
    Text,
    Data,
    Custom(String),
}
```

Extensions:

```text
.js
.mjs
.cjs
.jsx
.ts
.mts
.cts
.tsx
.json
.css
.module.css
.html
.wasm
.rs
```

Plugins can register additional module types.

---

# 7. Module Graph

The module graph is the center of the system.

```rust
pub struct ModuleGraph {
    modules: DashMap<ModuleId, ModuleNode>,
}

pub struct ModuleNode {
    pub id: ModuleId,
    pub url: String,
    pub file: Option<PathBuf>,
    pub module_type: ModuleType,

    pub imports: Vec<ImportEdge>,
    pub importers: Vec<ModuleId>,

    pub transform_hash: Hash,
    pub last_invalidated: Option<Instant>,

    pub ssr: ModuleEnvironmentData,
    pub client: ModuleEnvironmentData,

    pub hmr: HmrMetadata,
}
```

Import edges:

```rust
pub struct ImportEdge {
    pub specifier: String,
    pub resolved: ModuleId,
    pub kind: ImportKind,
}

pub enum ImportKind {
    Static,
    Dynamic,
    Css,
    Url,
    Worker,
    Wasm,
}
```

The graph should support independent environments:

```text
client
ssr
worker
test
custom
```

This is important because a module may need different transformation or resolution depending on environment.

---

# 8. Vite-Compatible Concepts

Ferrite should intentionally mirror the high-value parts of the Vite API.

## CLI mapping

| Vite | Ferrite |
|---|---|
| `vite` | `ferrite dev` |
| `vite build` | `ferrite build` |
| `vite preview` | `ferrite preview` |
| `vite --host` | `ferrite dev --host` |
| `vite --port` | `ferrite dev --port` |
| `vite --mode` | `ferrite --mode` |
| `vite --config` | `ferrite --config` |

---

# 9. Public Rust API

Equivalent to Vite's programmatic API:

```rust
use ferrite::{
    create_server,
    create_builder,
    build,
    preview,
    resolve_config,
};
```

Example:

```rust
#[tokio::main]
async fn main() -> ferrite::Result<()> {
    let mut server = ferrite::create_server(Default::default()).await?;
    server.listen().await?;
    Ok(())
}
```

Builder:

```rust
let builder = ferrite::create_builder(config).await?;
builder.build_app().await?;
```

Single environment:

```rust
builder.build("client").await?;
builder.build("ssr").await?;
```

---

# 10. Configuration

Primary config:

```toml
# ferrite.toml

root = "."
base = "/"

[server]
host = "127.0.0.1"
port = 5173
strict_port = false
open = false

[build]
out_dir = "dist"
sourcemap = true
minify = true
target = "es2022"

[ssr]
entry = "src/server.rs"
external = []
no_external = []

[resolve]
conditions = ["browser", "module", "import"]
extensions = [".mjs", ".js", ".mts", ".ts", ".jsx", ".tsx", ".json"]

[npm]
registry = "https://registry.npmjs.org"
lockfile = "ferrite.lock"

[compiler]
engine = "oxc"
```

Optional Rust config:

```rust
use ferrite::prelude::*;

pub fn config() -> Config {
    Config::default()
        .plugin(my_plugin())
        .alias("@", "./src")
}
```

Possible config precedence:

```text
CLI flags
↓
ferrite.local.toml
↓
ferrite.toml
↓
Cargo metadata
↓
defaults
```

---

# 11. Plugin API

Ferrite must expose a Rust-native plugin API that mirrors Vite/Rollup semantics.

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &'static str;

    fn enforce(&self) -> Enforce {
        Enforce::Normal
    }

    fn apply(&self) -> Apply {
        Apply::All
    }

    async fn config(&self, _config: &mut UserConfig) -> Result<()> {
        Ok(())
    }

    async fn config_resolved(&self, _config: &ResolvedConfig) -> Result<()> {
        Ok(())
    }

    async fn configure_server(&self, _server: &mut DevServer) -> Result<()> {
        Ok(())
    }

    async fn build_start(&self, _ctx: &PluginContext) -> Result<()> {
        Ok(())
    }

    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        _request: ResolveRequest,
    ) -> Result<Option<ResolvedId>> {
        Ok(None)
    }

    async fn load(
        &self,
        _ctx: &PluginContext,
        _request: LoadRequest,
    ) -> Result<Option<LoadResult>> {
        Ok(None)
    }

    async fn transform(
        &self,
        _ctx: &PluginContext,
        _request: TransformRequest,
    ) -> Result<Option<TransformResult>> {
        Ok(None)
    }

    async fn transform_index_html(
        &self,
        _ctx: &PluginContext,
        _html: HtmlTransformContext,
    ) -> Result<Option<HtmlTransformResult>> {
        Ok(None)
    }

    async fn handle_hot_update(
        &self,
        _ctx: &PluginContext,
        _event: HotUpdateEvent,
    ) -> Result<Option<HotUpdateResult>> {
        Ok(None)
    }

    async fn generate_bundle(
        &self,
        _ctx: &PluginContext,
        _bundle: &mut OutputBundle,
    ) -> Result<()> {
        Ok(())
    }

    async fn write_bundle(
        &self,
        _ctx: &PluginContext,
        _bundle: &OutputBundle,
    ) -> Result<()> {
        Ok(())
    }

    async fn close_bundle(&self) -> Result<()> {
        Ok(())
    }
}
```

Ordering:

```rust
pub enum Enforce {
    Pre,
    Normal,
    Post,
}
```

Application mode:

```rust
pub enum Apply {
    Serve,
    Build,
    All,
}
```

---

# 12. Plugin Hook Compatibility Table

| Vite / Rollup | Ferrite Rust |
|---|---|
| `config` | `Plugin::config` |
| `configResolved` | `Plugin::config_resolved` |
| `configureServer` | `Plugin::configure_server` |
| `configurePreviewServer` | `Plugin::configure_preview_server` |
| `buildStart` | `Plugin::build_start` |
| `resolveId` | `Plugin::resolve_id` |
| `load` | `Plugin::load` |
| `transform` | `Plugin::transform` |
| `transformIndexHtml` | `Plugin::transform_index_html` |
| `handleHotUpdate` | `Plugin::handle_hot_update` |
| `moduleParsed` | `Plugin::module_parsed` |
| `buildEnd` | `Plugin::build_end` |
| `renderStart` | `Plugin::render_start` |
| `renderChunk` | `Plugin::render_chunk` |
| `augmentChunkHash` | `Plugin::augment_chunk_hash` |
| `generateBundle` | `Plugin::generate_bundle` |
| `writeBundle` | `Plugin::write_bundle` |
| `closeBundle` | `Plugin::close_bundle` |

Support hook filtering:

```rust
TransformHook {
    filter: HookFilter {
        id: Some(Regex::new(r"\.tsx?$")?),
        code: None,
        query: None,
    },
    handler: ...
}
```

This avoids calling every plugin for every module.

---

# 13. Plugin Context

```rust
pub struct PluginContext<'a> {
    graph: &'a ModuleGraph,
    resolver: &'a Resolver,
    emitter: &'a AssetEmitter,
    environment: &'a Environment,
}
```

Equivalent helper APIs:

```rust
ctx.resolve(...)
ctx.load(...)
ctx.parse(...)
ctx.emit_file(...)
ctx.get_file_name(...)
ctx.get_module_info(...)
ctx.get_module_ids(...)
ctx.add_watch_file(...)
ctx.warn(...)
ctx.error(...)
```

---

# 14. Virtual Modules

Use the Vite/Rollup convention:

```text
virtual:ferrite/routes
virtual:ferrite/env
virtual:ferrite/manifest
```

Resolved internally as:

```text
\0virtual:ferrite/routes
```

Example:

```rust
async fn resolve_id(
    &self,
    req: ResolveRequest
) -> Result<Option<ResolvedId>> {
    if req.specifier == "virtual:hello" {
        return Ok(Some(ResolvedId::new("\0virtual:hello")));
    }

    Ok(None)
}
```

---

# 15. Resolver

The resolver must support:

```text
relative imports
absolute imports
bare npm imports
package exports
package imports
browser condition
development condition
production condition
module/main fields
TypeScript extensions
directory indexes
aliases
symlinks
CSS imports
URL imports
virtual modules
```

Examples:

```ts
import React from "react";
import x from "./x";
import "@/components/button";
import "pkg/subpath";
import "#internal";
```

Suggested API:

```rust
pub struct ResolveRequest<'a> {
    pub specifier: &'a str,
    pub importer: Option<&'a ModuleId>,
    pub environment: EnvironmentKind,
    pub kind: ResolveKind,
}
```

Output:

```rust
pub struct ResolvedId {
    pub id: ModuleId,
    pub external: bool,
    pub side_effects: Option<bool>,
    pub module_type: Option<ModuleType>,
    pub meta: serde_json::Value,
}
```

---

# 16. npm Without Node.js

Ferrite needs its own npm consumer.

## Required pieces

```text
npm registry client
package metadata fetch
semver resolver
integrity validation
tarball download
tar extraction
package cache
dependency graph
lockfile
package.json parser
exports/imports resolver
peer dependency handling
optional dependency handling
OS/CPU filtering
```

Cache:

```text
~/.cache/ferrite/npm/
├── registry/
├── tarballs/
├── packages/
└── metadata/
```

Project lockfile:

```text
ferrite.lock
```

Example:

```toml
[[package]]
name = "react"
version = "19.1.0"
source = "npm"
integrity = "sha512-..."
dependencies = []
```

CLI:

```bash
ferrite add react
ferrite add three@latest
ferrite remove lodash
ferrite update
ferrite install
```

`package.json` compatibility should still be supported:

```json
{
  "dependencies": {
    "three": "^0.180.0"
  }
}
```

But Ferrite should not require npm itself.

---

# 17. npm Package Execution Model

Not all npm packages are equivalent.

Classify packages:

```rust
pub enum PackageRuntime {
    BrowserEsm,
    BrowserCjs,
    Universal,
    NodeCompatible,
    NodeRequired,
    BuildTimeOnly,
}
```

Ferrite should distinguish:

```text
"can resolve this package"
"can transform this package"
"can bundle this package"
"can execute this package during SSR"
```

These are separate questions.

---

# 18. CommonJS

Implement CJS compatibility:

```js
const x = require("x");
module.exports = foo;
exports.foo = foo;
```

Convert when safe:

```text
CJS
↓
AST analysis
↓
synthetic ESM wrapper
↓
normal module graph
```

Runtime helper:

```js
function __ferrite_commonjs(factory) {
    const module = { exports: {} };
    factory(module, module.exports);
    return module.exports;
}
```

Support:

```text
require()
module
exports
__filename
__dirname
```

For browser builds, Node-specific modules should fail or require a polyfill plugin.

---

# 19. Node Compatibility Layer

Do not make Node.js a requirement.

Instead expose optional shims:

```text
node:path
node:buffer
node:events
node:util
node:process
node:url
```

Config:

```toml
[node_compat]
enabled = true
mode = "browser-shims"
```

Unsupported server-only modules should emit a useful error:

```text
Package "foo" requires node:child_process.

Ferrite SSR runtime does not provide this capability.

Options:
1. mark "foo" as external and run it in a Node adapter;
2. replace it with a web-compatible package;
3. provide a custom runtime plugin.
```

---

# 20. JavaScript Execution During SSR

Ferrite needs a JS execution interface.

```rust
#[async_trait]
pub trait JsRuntime: Send + Sync {
    async fn evaluate_module(
        &self,
        module: CompiledModule,
        env: RuntimeEnvironment,
    ) -> Result<ModuleNamespace>;
}
```

Possible backends:

```text
QuickJS
QuickJS-ng
V8
Boa
native Rust SSR only
external adapter
```

Recommended first backend:

```text
QuickJS/QuickJS-ng
```

because it can be embedded and shipped without Node.

Do **not** hard-code the framework around one JS engine.

---

# 21. SSR Modes

Ferrite should support three SSR modes.

## A. Pure Rust SSR

```text
Rust route
↓
Rust component tree
↓
HTML
```

Example frameworks:

```text
Leptos
Dioxus
custom renderer
```

## B. Embedded JavaScript SSR

```text
TS/JS application
↓
Ferrite transform
↓
embedded JS runtime
↓
render()
↓
HTML
```

## C. Hybrid Rust + JS

```text
Rust HTTP server
↓
Rust route
↓
JS SSR island
↓
HTML
↓
WASM/JS hydration
```

---

# 22. SSR Module Loading

Vite equivalent:

```text
server.ssrLoadModule("/src/entry-server.ts")
```

Ferrite:

```rust
let module = server
    .ssr_load_module("/src/entry-server.ts")
    .await?;

let render = module.get_function("render")?;
```

Internally:

```text
resolve
↓
load
↓
transform(ssr=true)
↓
dependency traversal
↓
compile
↓
instantiate runtime module
↓
evaluate
```

Plugin transforms receive environment metadata:

```rust
TransformRequest {
    ssr: true,
    environment: EnvironmentKind::Ssr,
    ...
}
```

---

# 23. SSR Externalization

Config:

```toml
[ssr]
external = ["pg", "sharp"]
no_external = ["my-esm-package"]
```

Behavior:

```text
external
→ do not bundle package

no_external
→ transform + bundle into SSR output
```

For fully standalone packaging:

```toml
[ssr]
bundle_all = true
```

Ferrite should attempt to bundle dependencies where licensing/runtime semantics permit.

---

# 24. Dev Server

Core API:

```rust
pub struct DevServer {
    pub config: ResolvedConfig,
    pub module_graph: Arc<ModuleGraph>,
    pub plugin_container: PluginContainer,
    pub watcher: FileWatcher,
    pub hmr: HmrServer,
}
```

Methods:

```rust
server.listen()
server.close()

server.transform_request(url)
server.transform_index_html(url, html)
server.ssr_load_module(url)
server.restart()
server.reload_module(id)
server.invalidate_module(id)
```

---

# 25. HTTP Request Pipeline

```text
HTTP request
    │
    ├── public asset?
    │      └── serve
    │
    ├── transformed module?
    │      └── cache
    │
    ├── HTML?
    │      └── transformIndexHtml
    │
    ├── module?
    │      ├── resolveId
    │      ├── load
    │      ├── transform
    │      └── rewrite imports
    │
    └── SSR route?
           └── SSR handler
```

---

# 26. Native ESM Dev Mode

Do not bundle the full app in dev.

Like Vite:

```text
browser requests /src/main.ts
↓
Ferrite transforms only that module
↓
imports rewritten to URLs
↓
browser requests dependencies on demand
```

Example source:

```ts
import React from "react";
import App from "./App.tsx";
```

Transformed dev output:

```ts
import React from "/@npm/react@19.1.0/index.js";
import App from "/src/App.tsx";
```

This is critical for fast startup.

---

# 27. Dependency Pre-Bundling

Optional optimization:

```text
npm dependency graph
↓
detect large CJS/ESM packages
↓
prebundle
↓
cache immutable output
```

Cache key:

```text
compiler version
+ ferrite version
+ package lock hash
+ target
+ defines
+ environment
```

Output:

```text
.ferrite/deps/
```

---

# 28. Import Rewriting

Input:

```ts
import { x } from "pkg";
import "./style.css";
const m = await import("./lazy.ts");
```

Development transformation:

```ts
import { x } from "/@npm/pkg@1.2.3/index.js";
import "/src/style.css";
const m = await import("/src/lazy.ts");
```

Use AST ranges rather than regex.

---

# 29. CSS

Minimum v1:

```text
.css imports
CSS modules
@import
url()
HMR
asset hashing
minification
```

JS import:

```ts
import "./style.css";
```

Dev response can inject styles:

```js
import { updateStyle } from "/@ferrite/client";
updateStyle("module-id", "body{...}");
```

Production extracts:

```text
dist/assets/app-A1B2C3.css
```

---

# 30. Assets

Support:

```ts
import logo from "./logo.svg";
import raw from "./file.txt?raw";
import url from "./large.bin?url";
```

Queries:

```text
?raw
?url
?inline
?worker
?sharedworker
?wasm
```

Production hashing:

```text
logo.4ad83f.svg
```

---

# 31. HTML Transformation

Equivalent to:

```text
transformIndexHtml
```

Pipeline:

```text
HTML parse
↓
pre hooks
↓
core rewrites
↓
normal hooks
↓
post hooks
↓
serialize
```

Plugins may inject:

```text
<script>
<link>
<meta>
preload
modulepreload
```

---

# 32. HMR

WebSocket endpoint:

```text
/@ferrite/hmr
```

Injected client:

```ts
import "/@ferrite/client";
```

Protocol:

```json
{
  "type": "update",
  "updates": [
    {
      "type": "js-update",
      "path": "/src/App.tsx",
      "acceptedPath": "/src/App.tsx",
      "timestamp": 123456789
    }
  ]
}
```

Other messages:

```text
connected
update
full-reload
custom
error
prune
invalidate
```

Client API:

```ts
if (import.meta.hot) {
    import.meta.hot.accept((mod) => {
        // update
    });

    import.meta.hot.dispose((data) => {
        // cleanup
    });
}
```

---

# 33. `import.meta.hot`

Transform:

```ts
import.meta.hot
```

into runtime-bound HMR context.

API target:

```ts
interface HotContext {
    data: Record<string, any>;

    accept(): void;
    accept(cb: AcceptCallback): void;
    accept(dep: string, cb?: AcceptCallback): void;
    accept(deps: string[], cb?: AcceptCallback): void;

    dispose(cb: DisposeCallback): void;
    prune(cb: () => void): void;

    invalidate(message?: string): void;

    on(event: string, cb: Function): void;
    off(event: string, cb: Function): void;
    send(event: string, data?: unknown): void;
}
```

---

# 34. HMR Graph Algorithm

When file `X` changes:

```text
X changed
↓
invalidate X
↓
walk importers
↓
find nearest HMR acceptance boundary
↓
if found:
    send update chain
else:
    full reload
```

CSS updates should normally be accepted automatically.

---

# 35. Error Overlay

Dev errors:

```text
parse errors
transform errors
resolve errors
SSR errors
runtime HMR errors
```

Send over WebSocket:

```json
{
  "type": "error",
  "err": {
    "message": "...",
    "stack": "...",
    "id": "...",
    "frame": "..."
  }
}
```

Browser overlay should show:

```text
file
line
column
code frame
plugin name
cause chain
```

---

# 36. Environment API

Model builds as explicit environments.

```rust
pub enum EnvironmentKind {
    Client,
    Ssr,
    Worker,
    Test,
    Custom(String),
}
```

Each environment owns:

```rust
pub struct Environment {
    pub name: String,
    pub kind: EnvironmentKind,
    pub target: Target,
    pub conditions: Vec<String>,
    pub define: HashMap<String, String>,
    pub plugins: Vec<Arc<dyn Plugin>>,
}
```

Builder:

```rust
let builder = create_builder(config).await?;

builder.build("client").await?;
builder.build("ssr").await?;
```

---

# 37. Production Bundler

Do not treat parsing/transformation as bundling.

Bundler responsibilities:

```text
entry discovery
module graph traversal
tree shaking
side-effect analysis
chunk graph creation
dynamic import splitting
shared chunk extraction
asset emission
import rewriting
hashing
manifest generation
source maps
```

Architecture:

```rust
pub trait Bundler {
    async fn bundle(&self, graph: &ModuleGraph, config: &BuildConfig)
        -> Result<OutputBundle>;
}
```

Backends:

```text
FerriteBundler
RolldownBackend
CustomBackend
```

If reusing Rolldown internals is practical, keep a narrow adapter around them.

Avoid exposing a public API that permanently depends on Rolldown implementation types.

---

# 38. Chunk Model

```rust
pub struct Chunk {
    pub id: ChunkId,
    pub modules: Vec<ModuleId>,
    pub imports: Vec<ChunkId>,
    pub dynamic_imports: Vec<ChunkId>,
    pub exports: Vec<String>,
    pub entry: bool,
    pub name: String,
    pub code: String,
    pub map: Option<SourceMap>,
}
```

Default filenames:

```text
assets/[name]-[hash].js
assets/[name]-[hash].css
assets/[name]-[hash][ext]
```

---

# 39. Tree Shaking

Required analysis:

```text
ES module imports/exports
live bindings
side-effect-free statements
package.json sideEffects
dynamic imports
namespace usage
re-exports
top-level side effects
```

Do not implement production-grade tree shaking using text matching.

---

# 40. Production Manifest

Generate:

```json
{
  "src/main.ts": {
    "file": "assets/main-b13a9.js",
    "src": "src/main.ts",
    "isEntry": true,
    "css": ["assets/main-113aa.css"],
    "imports": ["assets/vendor-fa342.js"]
  }
}
```

SSR manifest:

```json
{
  "src/components/Button.tsx": [
    "assets/Button-a93fd.js",
    "assets/Button-b831.css"
  ]
}
```

---

# 41. Source Maps

Support:

```text
inline
external
hidden
disabled
```

Every transform result:

```rust
pub struct TransformResult {
    pub code: String,
    pub map: Option<SourceMap>,
    pub dependencies: Vec<String>,
}
```

Plugin transforms must preserve source-map chains.

---

# 42. Environment Variables

Expose:

```text
import.meta.env.MODE
import.meta.env.BASE_URL
import.meta.env.PROD
import.meta.env.DEV
import.meta.env.SSR
```

Load:

```text
.env
.env.local
.env.[mode]
.env.[mode].local
```

Only expose configured prefixes:

```toml
[env]
prefix = ["FERRITE_", "PUBLIC_"]
```

Do not leak arbitrary host environment variables to the browser.

---

# 43. `define`

Config:

```toml
[define]
"__VERSION__" = "\"1.2.3\""
"process.env.NODE_ENV" = "\"production\""
```

Perform AST-safe or token-safe replacement during transform.

---

# 44. Workers

Target:

```ts
new Worker(new URL("./worker.ts", import.meta.url), {
    type: "module"
});
```

Also support:

```ts
import Worker from "./worker.ts?worker";
```

Worker environment has its own graph and build target.

---

# 45. WASM

Support both:

```ts
import init from "./module.wasm";
```

and Rust-produced WASM.

Rust client example:

```text
cargo build --target wasm32-unknown-unknown
↓
wasm-bindgen processing
↓
generated JS glue
↓
Ferrite module graph
↓
browser
```

Ferrite should provide a built-in Rust-WASM plugin.

---

# 46. Rust Framework Integration

Define an adapter:

```rust
#[async_trait]
pub trait SsrAdapter {
    async fn render(
        &self,
        request: HttpRequest,
        context: SsrContext,
    ) -> Result<SsrResponse>;
}
```

Adapters:

```text
ferrite-leptos
ferrite-dioxus
ferrite-yew
ferrite-maud
ferrite-askama
```

---

# 47. Framework API

A higher-level application API:

```rust
let app = FerriteApp::new()
    .route("/", page(home))
    .route("/users/:id", page(user))
    .api("/api/users", users_api)
    .middleware(auth())
    .static_dir("public")
    .run()
    .await?;
```

Potential file-system routing:

```text
src/routes/
├── index.rs
├── about.rs
├── users/
│   ├── index.rs
│   └── [id].rs
└── api/
    └── users.rs
```

Generated virtual module:

```text
virtual:ferrite/routes
```

---

# 48. SSR Streaming

API:

```rust
pub enum RenderBody {
    Full(String),
    Stream(Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>),
}
```

HTTP:

```text
request
↓
shell
↓
stream chunks
↓
deferred data
↓
hydration
```

---

# 49. Islands

Optional architecture:

```html
<div data-ferrite-island="Counter" data-props="...">
    <button>0</button>
</div>
```

Client:

```text
scan islands
↓
dynamic import component
↓
hydrate only that island
```

Allows Rust SSR with selective JavaScript/WASM hydration.

---

# 50. Server Functions / RPC

Expose:

```rust
#[server]
async fn get_user(id: UserId) -> Result<User> {
    ...
}
```

Compile into:

```text
server:
real Rust function

client:
generated HTTP/RPC stub
```

Transport:

```text
POST /_ferrite/rpc/<hash>
```

Possible encodings:

```text
JSON
MessagePack
CBOR
```

---

# 51. Standalone Packaging

This is a major product requirement.

Command:

```bash
ferrite build --standalone
```

Output:

```text
dist/
├── ferrite-app          # native executable
├── assets/
├── manifest.json
└── ferrite.lock.json
```

or:

```text
dist/app
```

as a single executable with embedded static assets.

Config:

```toml
[package]
standalone = true
embed_assets = true
compress_assets = true
```

---

# 52. Single Binary Asset Embedding

Production:

```text
Rust executable
├── SSR runtime
├── routes
├── API handlers
├── manifest
├── JS chunks
├── CSS
├── images
└── optional embedded JS engine
```

Access:

```rust
static ASSETS: EmbeddedAssets = include_ferrite_assets!();
```

HTTP responses can use immutable cache headers for fingerprinted files.

---

# 53. Cross Compilation

Target:

```bash
ferrite build --target x86_64-unknown-linux-musl
ferrite build --target aarch64-unknown-linux-gnu
ferrite build --target x86_64-pc-windows-msvc
```

Ideal deployment:

```bash
scp dist/app server:
./app
```

No:

```text
node_modules
npm install
npm run build
Node runtime
```

---

# 54. Docker

Generated Dockerfile:

```dockerfile
FROM scratch
COPY app /app
ENTRYPOINT ["/app"]
```

If TLS certificates or system dependencies are required, provide compatible base options.

---

# 55. Library Mode

Equivalent to Vite library mode:

```toml
[build.lib]
entry = "src/index.ts"
name = "MyLibrary"
formats = ["es", "cjs"]
```

Output:

```text
dist/index.js
dist/index.cjs
dist/index.d.ts
```

Declaration generation may initially shell out to an optional TS compatibility adapter or use isolated declarations where supported.

---

# 56. Plugin Compatibility with Existing Vite Plugins

There are three compatibility tiers.

## Tier 1 — Rust native

```text
ferrite-plugin-*
```

Best performance and no JS runtime required.

## Tier 2 — JS plugins inside embedded JS runtime

Load compatible plugin packages and translate hooks:

```text
JS Vite plugin
↓
QuickJS/V8 plugin host
↓
Ferrite hook bridge
↓
Rust module graph
```

Example bridge:

```text
plugin.resolveId()
plugin.load()
plugin.transform()
```

This can support a useful subset of Vite/Rollup plugins without Node.

## Tier 3 — Node adapter

Optional compatibility process:

```text
Ferrite
↔ RPC
Node plugin host
```

This is explicitly optional and not part of the Node-free core.

---

# 57. JavaScript Plugin Host

Plugin host interface:

```rust
#[async_trait]
pub trait ForeignPluginHost {
    async fn call_hook(
        &self,
        plugin: PluginHandle,
        hook: HookName,
        input: serde_json::Value,
    ) -> Result<serde_json::Value>;
}
```

Embedded runtime must expose a narrow host API.

Do not expose unrestricted Rust memory or filesystem by default.

---

# 58. Security Model for Build Plugins

Plugin permissions:

```toml
[[plugins]]
name = "example"
permissions = [
    "fs:read:src/**",
    "fs:write:.ferrite/**",
    "net:registry.npmjs.org"
]
```

Potential capabilities:

```text
filesystem read
filesystem write
network
environment variables
process spawning
native libraries
```

Future goal:

```text
sandboxed plugin execution
```

---

# 59. Caching

Layers:

```text
resolver cache
registry metadata cache
package cache
parse cache
transform cache
dependency optimization cache
bundle cache
SSR module cache
```

Transform key:

```text
blake3(
    source
    + module id
    + compiler version
    + plugin pipeline hash
    + environment
    + target
    + mode
)
```

---

# 60. Incremental Compilation

File change:

```text
changed file
↓
content hash
↓
invalidate graph node
↓
invalidate dependent transform results
↓
recompile only affected modules
↓
HMR
```

Avoid rebuilding the entire graph.

---

# 61. Persistent Cache

```text
.ferrite/
├── cache/
├── deps/
├── graph/
├── npm/
└── metadata.json
```

Allow:

```bash
ferrite clean
```

---

# 62. Parallelism

Parallelize:

```text
independent transforms
dependency package compilation
minification
hashing
asset processing
```

Do **not** parallelize plugin hooks whose semantics are sequential.

---

# 63. Dev Startup Target

Performance goals for a medium application:

```text
CLI startup              < 100 ms
server ready             < 300 ms
cold first transform     < 50 ms typical module
cached transform         < 5 ms
HMR server dispatch      < 20 ms excluding framework work
```

Treat these as design targets, not guarantees.

---

# 64. Error Model

```rust
#[derive(Debug, thiserror::Error)]
pub enum FerriteError {
    Resolve(ResolveError),
    Parse(ParseError),
    Transform(TransformError),
    Plugin(PluginError),
    Runtime(RuntimeError),
    Build(BuildError),
    Io(std::io::Error),
}
```

Diagnostics:

```text
error[FERRITE_RESOLVE_001]
Cannot resolve "foo/bar"

  src/main.ts:4:19
  4 │ import thing from "foo/bar";
    │                   ^^^^^^^^^

Package "foo" does not export "./bar".
```

---

# 65. Logging

```bash
ferrite dev --log-level debug
ferrite dev --profile
```

Structured tracing:

```text
resolve: 1.3ms
load: 0.2ms
transform[oxc]: 3.4ms
plugin[react]: 1.1ms
```

---

# 66. Inspector

```bash
ferrite inspect
```

Web UI:

```text
module graph
plugins
hook timings
resolved config
npm dependency graph
HMR boundaries
chunks
SSR externals
cache hits
```

Routes:

```text
/@ferrite/inspect
```

---

# 67. Compatibility Test Suite

Do not claim Vite compatibility without tests.

Create:

```text
tests/vite-compat/
```

Test categories:

```text
config
resolver
plugin ordering
virtual modules
HTML transforms
HMR
import.meta.env
assets
CSS
dynamic imports
SSR
SSR externalization
build manifest
source maps
```

Where licensing permits, run representative Vite/Rollup behavior fixtures.

---

# 68. Reference Plugin Example

```rust
use ferrite::plugin::*;

pub struct RawTextPlugin;

#[async_trait]
impl Plugin for RawTextPlugin {
    fn name(&self) -> &'static str {
        "raw-text"
    }

    async fn load(
        &self,
        _ctx: &PluginContext,
        request: LoadRequest,
    ) -> Result<Option<LoadResult>> {
        if request.id.ends_with(".txt?raw") {
            let path = request.id.trim_end_matches("?raw");
            let value = tokio::fs::read_to_string(path).await?;

            return Ok(Some(LoadResult {
                code: format!("export default {:?};", value),
                module_type: ModuleType::Js,
                ..Default::default()
            }));
        }

        Ok(None)
    }
}
```

---

# 69. React Support

Plugin:

```text
ferrite-plugin-react
```

Responsibilities:

```text
TSX transform
JSX transform
React Refresh
automatic JSX runtime
development metadata
SSR JSX mode
```

Config:

```toml
[react]
refresh = true
runtime = "automatic"
```

---

# 70. Vue / Svelte Support

These should be plugins, not hardcoded into core.

```text
ferrite-plugin-vue
ferrite-plugin-svelte
```

Plugin transforms may generate virtual/submodules:

```text
Component.vue
Component.vue?type=script
Component.vue?type=style&index=0
Component.vue?type=template
```

The core graph must therefore support multiple derived module IDs for one physical file.

---

# 71. Import Maps

Support generated browser import maps.

Example:

```html
<script type="importmap">
{
  "imports": {
    "react": "/@npm/react@19.1.0/index.js"
  }
}
</script>
```

Possible modes:

```toml
[npm]
dev_strategy = "rewrite"
# or
dev_strategy = "import-map"
```

---

# 72. Remote Imports

Optional:

```ts
import x from "https://esm.example/pkg";
```

Disabled by default.

Config:

```toml
[remote]
enabled = true
allow = ["esm.example"]
```

Cache remote modules by integrity/hash.

---

# 73. Package Conditions

Resolution should support environment-specific conditions:

Client:

```text
browser
development
import
module
default
```

SSR:

```text
development
import
node-compatible
default
```

Ferrite-specific optional condition:

```text
ferrite
```

---

# 74. Rust Package Integration

Allow JS imports from generated Rust/WASM packages:

```ts
import { hash } from "rust:my_crypto";
```

Resolver:

```text
rust:my_crypto
↓
Cargo metadata
↓
compile wasm target
↓
wasm-bindgen
↓
virtual JS entry
```

This can become a major differentiator from Vite.

---

# 75. Unified Dependency Graph

Long-term:

```text
JS modules
TS modules
CSS
WASM
Rust crates
assets
SSR routes
```

represented in one graph abstraction.

Do not make Rust crate dependencies behave exactly like ESM imports internally, but expose cross-language edges.

---

# 76. Dev Middleware API

Equivalent to Vite middleware mode:

```rust
let server = ferrite::create_server(
    ServerConfig {
        middleware_mode: true,
        ..Default::default()
    }
).await?;

my_axum_app
    .layer(server.middleware());
```

This makes Ferrite embeddable into existing Rust servers.

---

# 77. HTML Entry Discovery

Default:

```text
index.html
```

But support:

```toml
[build]
entries = [
    "index.html",
    "admin.html"
]
```

And programmatic entries:

```rust
config.entry("admin", "src/admin.ts");
```

---

# 78. Build Lifecycle

```text
load config
↓
resolve config
↓
initialize plugins
↓
buildStart
↓
resolve entries
↓
construct graph
↓
load + transform
↓
tree shake
↓
chunk
↓
render chunks
↓
minify
↓
generateBundle
↓
writeBundle
↓
closeBundle
```

---

# 79. Dev Lifecycle

```text
load config
↓
resolve config
↓
initialize plugins
↓
configureServer
↓
start watcher
↓
start HTTP/WS
↓
lazy transform requests
↓
watch changes
↓
invalidate graph
↓
HMR
```

---

# 80. CLI UX

```bash
$ ferrite dev

  FERRITE v0.1.0

  Local:   http://localhost:5173/
  Network: use --host to expose
  SSR:     enabled
  HMR:     ready

  press h + enter to show help
```

Commands:

```text
r + enter  restart
u + enter  print URL
o + enter  open browser
c + enter  clear
q + enter  quit
```

---

# 81. Lockfile Philosophy

`ferrite.lock` must lock:

```text
npm package version
source registry
integrity
dependencies
optional deps
peer resolution
platform filters
```

Do not put Cargo dependencies into the same lock format; `Cargo.lock` remains authoritative for Rust.

---

# 82. Workspace Support

```text
repo/
├── Cargo.toml
├── ferrite.workspace.toml
├── apps/
│   ├── web/
│   └── admin/
└── packages/
    └── ui/
```

Support local JS packages:

```json
{
  "dependencies": {
    "@app/ui": "workspace:*"
  }
}
```

---

# 83. Production Server

`ferrite preview` is only for preview.

Production app should be the compiled SSR binary:

```bash
./dist/app
```

Environment:

```text
PORT
HOST
RUST_LOG
```

Static assets:

```text
Cache-Control: public, max-age=31536000, immutable
```

for fingerprinted content.

---

# 84. JS Runtime Isolation

For embedded JS SSR:

```text
runtime pool
↓
one or more JS contexts
↓
compiled module cache
↓
request-local globals
```

Avoid creating a full VM for every request.

Potential pool:

```rust
pub struct RuntimePool {
    workers: Vec<RuntimeWorker>,
}
```

---

# 85. SSR Runtime Globals

Provide controlled globals:

```text
console
URL
URLSearchParams
TextEncoder
TextDecoder
crypto subset
fetch
Headers
Request
Response
setTimeout
clearTimeout
```

Avoid pretending every Node API exists.

---

# 86. `fetch`

Use Rust HTTP internally:

```text
JS fetch()
↓
runtime bridge
↓
reqwest/hyper
↓
Promise result
```

Support AbortSignal eventually.

---

# 87. Rust ↔ JS Value Bridge

Core type:

```rust
pub enum JsValue {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsValue>),
    Object(IndexMap<String, JsValue>),
    Bytes(Vec<u8>),
    Handle(JsHandle),
}
```

Avoid serializing everything through JSON when native handles are possible.

---

# 88. Native Addons

Packages requiring `.node` N-API binaries cannot automatically run inside a non-Node embedded runtime.

Ferrite should detect them and report:

```text
Native Node addon detected: sharp-linux-x64.node
```

Options:

```text
Rust-native replacement adapter
WASM alternative
external Node compatibility host
unsupported
```

---

# 89. SWC Integration

Provide a compiler adapter, not a dependency leak.

```rust
pub struct SwcCompiler {
    source_map: Lrc<SourceMap>,
    globals: Globals,
}
```

Responsibilities:

```text
parse JS/TS/JSX/TSX
syntax lowering
TS stripping
JSX transform
decorators
source maps
selected plugins
minification if configured
```

Avoid using SWC's deprecated/retiring bundler path as the foundation.

---

# 90. Oxc Integration

Oxc is a good default for:

```text
parser
semantic analysis
resolver
transformer
minifier
```

Wrapper:

```rust
pub struct OxcCompiler {
    options: OxcOptions,
}
```

Keep a stable Ferrite `ParsedModule` and `TransformResult` API so Oxc can be upgraded independently.

---

# 91. Rolldown Strategy

There are two valid strategies.

## Strategy A — integrate selected Rolldown Rust crates

Pros:

```text
mature bundling work
Rollup-style behavior
ecosystem compatibility
performance
```

Cons:

```text
internal API stability
project coupling
licensing/version coordination
```

## Strategy B — implement Ferrite bundler

Pros:

```text
full control
native Rust API
optimized for SSR + WASM + Rust
```

Cons:

```text
tree shaking is hard
chunking is hard
CJS interop is hard
source-map correctness is hard
years of edge cases
```

Recommendation:

```text
v0.x:
build an adapter around existing Rust bundling technology

v1+:
replace internals only where Ferrite needs unique behavior
```

---

# 92. API Stability Boundary

Public:

```text
Config
Plugin
PluginContext
DevServer
Builder
Resolver interfaces
Runtime interfaces
manifest schema
HMR protocol
```

Private:

```text
Oxc AST
SWC AST
Rolldown internal graph
QuickJS pointers
cache layout
```

Never expose compiler-specific AST types as the mandatory plugin API.

Instead:

```rust
ctx.parse_js(...)
ctx.transform_js(...)
```

with optional advanced compiler extensions.

---

# 93. JavaScript API Compatibility Layer

Optionally ship an npm package:

```text
@ferrite/dev
```

for developers migrating Vite configs.

Example:

```ts
import { defineConfig } from "@ferrite/dev";

export default defineConfig({
    plugins: [],
    server: {
        port: 5173
    }
});
```

Ferrite can evaluate this config through the embedded JS runtime.

However, native `ferrite.toml` and Rust configuration remain preferred.

---

# 94. Vite Config Migration

Command:

```bash
ferrite migrate vite.config.ts
```

Convert obvious settings:

```text
server.port
server.host
resolve.alias
define
css.modules
build.outDir
build.sourcemap
ssr.external
ssr.noExternal
```

Emit warnings for unsupported plugin behavior.

---

# 95. Compatibility Goal Definition

Do not say "100% Vite compatible" without defining compatibility.

Suggested labels:

```text
Config-compatible
Plugin-hook-compatible
HMR-API-compatible
Asset-semantics-compatible
SSR-API-compatible
Rollup-hook-compatible
```

Version report:

```bash
ferrite compat
```

Output:

```text
Vite config semantics      82%
Vite plugin hooks          91%
Rollup plugin hooks        88%
HMR client API             95%
SSR APIs                   76%
```

Percentages should only be generated from a real conformance test matrix.

---

# 96. MVP Roadmap

## Phase 0 — compiler spike

Implement:

```text
Oxc parse
TS strip
JSX transform
ESM import extraction
source maps
```

Acceptance:

```bash
ferrite transform src/main.ts
```

---

## Phase 1 — Vite-like dev server

Implement:

```text
HTTP server
HTML serving
lazy module transform
ESM rewrite
module graph
file watcher
basic HMR
CSS injection
```

Acceptance:

```bash
ferrite dev
```

runs a TS/JS application without Node.

---

## Phase 2 — npm

Implement:

```text
registry metadata
semver
download
integrity
lockfile
package exports
bare import resolution
CJS conversion
dependency cache
```

Acceptance:

```ts
import { debounce } from "lodash-es";
```

works without `node_modules` and without Node.

---

## Phase 3 — production build

Implement:

```text
bundle graph
tree shaking
chunks
dynamic imports
hashes
CSS extraction
assets
manifest
source maps
minify
```

Acceptance:

```bash
ferrite build
```

creates optimized static output.

---

## Phase 4 — SSR

Implement:

```text
SSR environment
SSR transforms
embedded JS runtime
ssrLoadModule
externals
streaming API
Rust SSR adapter
```

Acceptance:

```bash
ferrite dev
```

runs client + SSR builds together.

---

## Phase 5 — standalone packaging

Implement:

```text
Rust server compile
asset embedding
SSR manifest
single executable
cross compilation
```

Acceptance:

```bash
ferrite build --standalone
./dist/app
```

with no Node dependency.

---

## Phase 6 — plugin compatibility

Implement:

```text
remaining Rollup hooks
Vite-specific hooks
embedded JS plugin host
React plugin
Vue/Svelte experiments
compatibility test suite
```

---

# 97. Initial Repository Layout

```text
ferrite/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── crates/
│   ├── ferrite-cli/
│   ├── ferrite-core/
│   ├── ferrite-config/
│   ├── ferrite-resolver/
│   ├── ferrite-graph/
│   ├── ferrite-transform/
│   ├── ferrite-plugin/
│   ├── ferrite-server/
│   ├── ferrite-hmr/
│   ├── ferrite-html/
│   ├── ferrite-css/
│   ├── ferrite-assets/
│   ├── ferrite-npm/
│   ├── ferrite-bundler/
│   ├── ferrite-runtime/
│   ├── ferrite-ssr/
│   └── ferrite-test/
├── packages/
│   └── ferrite-client/
├── examples/
│   ├── vanilla-ts/
│   ├── react/
│   ├── rust-wasm/
│   └── ssr/
└── tests/
    ├── fixtures/
    └── vite-compat/
```

---

# 98. Workspace Cargo Skeleton

```toml
[workspace]
resolver = "2"
members = [
    "crates/ferrite-cli",
    "crates/ferrite-core",
    "crates/ferrite-config",
    "crates/ferrite-resolver",
    "crates/ferrite-graph",
    "crates/ferrite-transform",
    "crates/ferrite-plugin",
    "crates/ferrite-server",
    "crates/ferrite-hmr",
    "crates/ferrite-html",
    "crates/ferrite-css",
    "crates/ferrite-assets",
    "crates/ferrite-npm",
    "crates/ferrite-bundler",
    "crates/ferrite-runtime",
    "crates/ferrite-ssr",
]

[workspace.dependencies]
anyhow = "1"
async-trait = "0.1"
axum = "0.8"
blake3 = "1"
bytes = "1"
dashmap = "6"
futures = "0.3"
miette = "7"
notify = "8"
parking_lot = "0.12"
regex = "1"
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
semver = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
toml = "0.9"
tracing = "0.1"
tracing-subscriber = "0.3"
url = "2"
```

Use current compatible crate versions when implementation begins; this is an architectural skeleton, not a pinned production lockfile.

---

# 99. First Core Types

```rust
pub struct Ferrite {
    config: ResolvedConfig,
    plugins: PluginContainer,
    graph: Arc<ModuleGraph>,
    resolver: Arc<Resolver>,
    compiler: Arc<dyn JsCompiler>,
}

impl Ferrite {
    pub async fn transform_request(
        &self,
        request: ModuleRequest,
    ) -> Result<TransformResult> {
        let resolved = self.resolver.resolve(request.resolve_request()).await?;

        let loaded = self.plugins.load(&resolved).await?
            .unwrap_or_else(|| self.load_from_fs(&resolved))?;

        let transformed = self.plugins
            .transform(TransformRequest {
                id: resolved.id,
                code: loaded.code,
                environment: request.environment,
                ssr: request.environment.is_ssr(),
            })
            .await?;

        Ok(transformed)
    }
}
```

---

# 100. Architectural Rule

The most important rule:

```text
Ferrite is not "SWC with an HTTP server."

Ferrite is:

dev server
+ environment-aware module graph
+ resolver
+ plugin container
+ compiler abstraction
+ npm resolver
+ HMR runtime
+ bundler
+ SSR runtime
+ production packager
```

SWC/Oxc are compiler components inside that architecture.

Likewise, Rolldown-style behavior is a bundling component, not the product architecture itself.

---

# 101. Recommended v0.1 Scope

Build only:

```text
ferrite dev
ferrite build

JS
TS
JSX
TSX
CSS
JSON
assets

Oxc transforms
npm ESM packages
module graph
basic plugin hooks
HMR
production bundle
manifest
```

Do not start with:

```text
full Vite plugin emulation
Vue
Svelte
Node native addons
complete Node core shims
single executable
remote imports
advanced SSR
```

Then add SSR once the client pipeline is correct.

---

# 102. Definition of Success

The first convincing demo should be:

```bash
cargo install ferrite
ferrite create my-app
cd my-app
ferrite add react react-dom three
ferrite dev
```

with:

```text
No Node installed.
No npm installed.
No pnpm installed.
No node_modules required.
```

Then:

```bash
ferrite build --standalone
./dist/my-app
```

serves:

```text
SSR HTML
hydrated browser application
npm-derived JavaScript dependencies
compiled CSS/assets
```

from a Rust-native deployment.

That is the product.

---

# 103. Upstream Projects to Study

The implementation should study these projects for semantics and test cases rather than blindly copying code:

- Vite — dev server, HMR, SSR, plugin semantics, environments.
- Rollup — plugin contract and chunk semantics.
- Rolldown — Rust bundling architecture and Rollup/Vite compatibility direction.
- Oxc — parser, transformer, resolver, minifier.
- SWC — parser/transforms/ecosystem compatibility.
- Cargo — package resolution, lockfiles, cache UX.
- Deno — Node/npm compatibility without requiring a traditional Node workflow.
- Bun — package manager/runtime ergonomics.
- esbuild — fast build-tool architecture.
- rspack / farm — Rust bundler architecture.
- Leptos / Dioxus — Rust SSR and hydration architecture.

---

# 104. Current Technical Notes

At the time this design was drafted:

- Vite exposes programmatic development APIs such as `createServer`, module transformation, HTML transformation, and SSR module loading.
- Vite's SSR plugin path provides environment/SSR information to resolution/loading/transformation hooks.
- Modern Vite supports environment-aware build concepts rather than assuming only one client build.
- Rolldown is a Rust bundler designed around Rollup/Vite ecosystem compatibility and exposes Rollup-like bundling/plugin APIs.
- Rolldown documents Node-compatible resolution, ESM/CJS interoperability, syntax lowering, `define`, `inject`, and ongoing HMR work.
- SWC's own bundling documentation recommends dedicated bundlers rather than using SWC bundling as the long-term foundation.
- Oxc exposes reusable Rust components for parsing, transformation, resolution, and minification.

These observations strongly support keeping Ferrite's compiler, bundler, resolver, server, and runtime as separate replaceable subsystems.

---

# 105. References

- Vite JavaScript API: https://vite.dev/guide/api-javascript
- Vite SSR Guide: https://vite.dev/guide/ssr
- Vite Environment API: https://vite.dev/guide/api-environment-frameworks
- Rolldown introduction: https://rolldown.rs/guide/introduction
- Rolldown plugin API: https://rolldown.rs/apis/plugin-api
- Rolldown bundler API: https://rolldown.rs/apis/bundler-api
- Oxc overview: https://oxc.rs/docs/guide/what-is-oxc
- Oxc parser: https://oxc.rs/docs/guide/usage/parser
- Oxc transformer: https://oxc.rs/docs/guide/usage/transformer
- Oxc minifier: https://oxc.rs/docs/guide/usage/minifier
- SWC core API: https://swc.rs/docs/usage/core
- SWC bundling notes: https://swc.rs/docs/configuration/bundling

---

# 106. Suggested First Engineering Milestone

Create this API and make it work end-to-end:

```rust
let server = ferrite::create_server(Config::default()).await?;
server.listen().await?;
```

For a request:

```text
GET /src/main.ts
```

implement:

```text
1. resolve module ID
2. read file
3. parse with Oxc
4. strip TypeScript
5. rewrite imports
6. inject HMR context
7. update module graph
8. return JS
9. watch source file
10. invalidate + send HMR update on change
```

Once that path is robust, add npm resolution.

Once npm resolution is robust, add production bundling.

Once production bundling is robust, add SSR.

This ordering keeps the hardest compatibility work isolated and testable.
