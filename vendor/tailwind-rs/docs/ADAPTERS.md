# Runtime and Build Tool Adapters

## Principle

Adapters integrate host lifecycle APIs with the compiler.

Adapters MUST NOT own compiler semantics.

## Core API shape

All adapters should converge on a small common operation set:

```text
create compiler
compile source/candidates
update source
remove source
update config
get CSS
get diagnostics
dispose
```

## Node.js

Preferred native path: N-API.

Advantages:

- no separate Node ABI rebuild per version,
- strong performance,
- usable from Bun when compatible.

Package should expose TypeScript types.

Example conceptual API:

```ts
const compiler = createCompiler(options)
compiler.updateSource("src/app.tsx", code)
const result = compiler.build()
```

## Bun

`@utilitycss/node` is the generic JavaScript lifecycle/compiler API. `@utilitycss/bun` is the
first-class Bun bundler and fullstack integration built on that API. The Bun adapter MUST preserve
Rust compiler semantics and MUST NOT start a second filesystem watcher.

Use the plugin with Bun's bundler:

```ts
import { utilitycss } from "@utilitycss/bun";

const result = await Bun.build({
  entrypoints: ["src/server.ts"],
  outdir: "dist",
  target: "bun",
  plugins: [utilitycss()]
});
```

For Bun fullstack development, configure `bunfig.toml`:

```toml
[serve.static]
plugins = ["@utilitycss/bun"]
```

HTML references the virtual stylesheet without a generated development file:

```html
<link rel="stylesheet" href="utilitycss" />
```

The plugin MUST:

- rebuild compiler state from a persistent normalized-module source snapshot for each Bun build
  cycle, because Bun may omit unchanged modules during incremental rebuilds;
- collect `.html`, `.htm`, JavaScript, TypeScript, JSX/TSX, Vue, Svelte, and Astro modules through
  Bun `onLoad` hooks;
- defer the virtual CSS load until source modules have been loaded;
- normalize file URLs, `/@fs/` IDs, separators, queries, and real paths;
- fail builds for compiler errors and expose warnings with their structured source information;
- support opt-in `debug` lifecycle logs that identify failed HMR rebuilds and the retained
  last-successful bundle, with source locations where source text is available;
- regenerate CSS from the current module graph so deleted modules cannot leave stale utilities;
- transform imported `.css` files through the native stylesheet API and report `@apply` diagnostics;
- rely on `Bun.serve({ development: { hmr: true } })` for frontend graph/HMR behavior in development;
- avoid writing development CSS to disk and avoid an adapter-owned watcher.

The default virtual specifier is `utilitycss`; callers MAY override it with `specifier`. The package
exports the configurable `utilitycss()` factory and a zero-options plugin object as its default
export, which lets Bun load it directly from `bunfig.toml`.

The repository example in [`examples/bun`](../examples/bun/) uses Preact for its browser UI. Preact
is an application dependency, not part of the Bun adapter: `@utilitycss/node` remains the generic
compiler lifecycle API, while `@utilitycss/bun` supplies Bun bundler/fullstack/HMR hooks. The
example demonstrates authored `@apply` CSS alongside generated utility CSS.

## Deno

Preferred options:

1. WASM with a JS/TS wrapper,
2. npm compatibility using the Node package where supported,
3. native FFI only if there is a strong reason.

Avoid making Deno support dependent on Node globals.

The WASM compiler (`WasmCompiler`) mirrors the N-API adapter surface: the constructor accepts an
optional `pretty`/`configSource`/`browserTarget` triple, `updateSource` and `transformStylesheet`
take an optional host-language `path`, and `extractCandidates` runs the same SWC/framework dispatch
as native extraction. `build()` returns `{ css, diagnostics, stats }` with N-API-matching camel-case
counters. Introspection payloads (`explain`, `validate`, `capabilities`) are JSON strings in both
adapters; hosts `JSON.parse` one stable encoding. Hosts that already parse JavaScript-family or
framework source SHOULD call `updateSourceWithCandidates` with exact source spans. Method names are
camelCase (`updateSource`, `removeSource`) on both sides.

## Vite

The Vite adapter should:

- initialize one compiler instance,
- translate file changes into incremental updates,
- remove source state from watcher delete notifications,
- invalidate virtual CSS modules as needed,
- preserve source errors,
- keep dev and build semantics aligned.

The virtual module is:

```text
virtual:utilitycss.css
```

The plugin should avoid rescanning the full project on every HMR update.

## SWC and framework extraction

`utilitycss-swc` is the production JavaScript-family extraction path. The CLI and N-API binding use
it automatically for JavaScript, TypeScript, JSX, and TSX inputs when the host does not provide
candidate spans.

Use cases:

- class helper analysis,
- JSX attribute extraction,
- constrained static expression evaluation,
- framework-specific syntax.

SWC extraction produces candidates. It does not generate CSS semantics independently. Vue, Svelte,
and Astro markup use the conservative framework extractor for static `:class`, `class:`, and
`class:list` forms; dynamic bindings remain intentionally unsupported.

```text
SWC AST
  ↓
candidate extractor
  ↓
CandidateInput[]
  ↓
normal compiler pipeline
```

## PostCSS

A compatibility adapter MAY be added later.

It should be implemented as an adapter around the compiler, not as the canonical architecture.

## Adapter conformance tests

The canonical behavior fixtures are the Rust conformance suite under
`crates/utilitycss-compiler/tests/fixtures`: input candidates, config, expected diagnostics, and
expected CSS. That suite pins compiler semantics; the JavaScript adapters do not execute it
directly. Instead, adapter behavior is pinned per adapter: native Node smoke coverage runs after a
platform N-API build, while the regular JavaScript tests use injectable fakes so they remain
runnable without a native binary. A shared cross-adapter fixture runner MAY be added later; until
then, adapter changes MUST extend the adapter's own tests.

The Bun package additionally tests HTML links, JavaScript and TSX module extraction, source
replacement, deleted-source invalidation, diagnostics, deterministic output, and packed-artifact
installation. `examples/bun` runs an actual `Bun.build()` production smoke and provides the Bun
`--hot` fullstack server path.

This prevents semantic drift.

## Serialization boundary

Bindings should use compact plain data types.

Avoid exposing internal Rust enums directly if doing so would freeze implementation details.

Prefer versioned transport structs where needed.

## Lifecycle

Compiler instances should be reusable and explicitly disposable by hosts.

Adapters should avoid hidden process-global singleton compilers.

The Node adapter's `dispose()` releases the native handle and blocks further use; every later call
throws. N-API instances are otherwise garbage-collected, so the native binding exposes no explicit
dispose entry point. WASM instances are released with the wasm-bindgen-generated `free()`.
