---
title: Build pipeline
order: 9
---

# Build pipeline

Dev and build share one pipeline (`ferrite-transform` + `ferrite-graph`
+ `ferrite-bundler`): the bundler consumes modules through the same
`ModuleLoader` the server uses, so what you see in dev is what ships.

## Transform

`JsCompiler` is the stable boundary: graph, server, and bundler only
see `ParsedModule` / `TransformResult`, never engine AST types.

- **Oxc (default)**: TS strip, JSX automatic/classic, lowering,
  `import.meta.hot` rewrites, define replacement, HMR client injection.
- **SWC (opt-in)**: build with `--features swc` (or
  `[compiler] engine = "swc"`), swapping the transform/minify engine to
  `swc_core` (TS strip, JSX, es2015–2022 lowering, top-level DCE).
  Default builds keep the zero-cost Oxc frontend.

Debug one file without a full build:

```bash
ferrite transform src/app.tsx --out /tmp/app.js --sourcemap
```

## Bundler

`ferrite build` walks each entry closure, chunks, hashes, and emits
(`ferrite-bundler`, spec §37–§39):

1. Resolve + load through plugin hooks.
2. Transform (Oxc or SWC).
3. Statement-level tree-shake, then re-minify.
4. Chunk (hashed, one ESM file per module) or `--scope-hoist`.
5. Extract CSS (one hashed `.css` per stylesheet, `@import` order).
6. Emit the manifest + source maps.

## Tree-shaking

On by default in production (`BundleRequest.treeshake`). Unused exports
drop per statement with a used-exports fixpoint across the graph; barrel
re-exports survive when any downstream consumer needs them.

## Scope hoisting

```bash
ferrite build --scope-hoist
```

```toml
[build]
scope_hoist = true
```

Concatenates each entry closure into one module with `$f{index}$`-prefixed
locals. Graphs that cannot be proven safe (namespaces, `eval`, …) bail
out to chunked output rather than miscompiling.

## Source maps

`[build] sourcemap`: `true` (external, default), `false`, `"inline"`,
or `"hidden"` (emitted, no comment). Minification emits its own map and
chains it through the transform map
(`ferrite_transform::chain_source_maps`), so minified builds still point
at original sources. Unresolvable positions pass through sourceless.

## Manifest

`ferrite-manifest` (spec §40) writes a Vite-compatible client manifest
(`file`, `src`, `isEntry`, `css`, `imports`, `dynamicImports`) plus an
SSR module→chunk map. The standalone server and SSR loaders read it to
resolve hashed URLs at runtime.
