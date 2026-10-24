# Production CSS, chunk and source-map contracts

Client entry HTML links only the entry's static CSS closure. Styles reachable
only through dynamic imports remain lazy. Literal dynamic imports with styles
are wrapped by an emitted ESM helper that appends ordered stylesheet links and
waits for all of them before requesting/executing the module. Concurrent callers
share in-flight CSS promises. Failed helper-owned links are removed and their
promise cache is evicted, allowing a later import to retry. Existing loaded
stylesheets are reused. Direct dynamic CSS imports retain a JavaScript facade.
Nonliteral dynamic imports remain outside this statically discovered contract.

CSS roots follow depth-first source import order, preserving the cascade between
a dependency's styles and later importer overrides. Native CSS `@import`s remain
in their owning stylesheet and are not also linked separately by the HTML entry.
Each manifest chunk records CSS and emitted asset closures; lazy resources do not
enter the initial client closure. The helper is a real manifest dependency.
Relative helper/chunk/style URLs also work with nested output patterns and a
non-root deployment base.

JavaScript names include reachable CSS output names and the preload helper
version in their hash material. Changing a lazy stylesheet therefore invalidates
its importer cone even when JavaScript source did not change. This is dependency
hashing, not a hash of final circularly referenced emitted bytes.

Final source maps compose CSS-import removal, hashed-specifier rewriting, preload
insertion and banner/intro/outro/footer offsets. Inserted runtime code is unmapped;
original code retains its source positions. CSS import removal preserves existing
minified formatting. Code-changing `renderChunk` hooks currently cross a
string-only bundler boundary without a composable map; their stale original maps
are dropped. Full plugin render-map composition remains a separate open contract.

SSR keeps eager all-graph style preloading and never imports the browser helper.
Per-route SSR CSS isolation remains open. These fixtures do not establish full
Vite/Rollup graph parity, arbitrary plugin behavior, external-map section loading,
all worker/Wasm paths or complete unused-asset elimination.

## Reproduce

```bash
python3 scripts/acceptance.py --profile production
```

Requires Cargo and explicitly installed Chromium/Firefox. No Node compiler host
is needed for this production profile. The browser fixture executes all four
minify/scope-hoist combinations, including nested output patterns, missing-CSS
failure/retry, CSS-before-module evaluation, native ESM singleton sharing, direct
CSS import and real image fetching under `/app/`. Additional cases verify final
original TypeScript positions after wrappers and CSS-driven cache invalidation.
