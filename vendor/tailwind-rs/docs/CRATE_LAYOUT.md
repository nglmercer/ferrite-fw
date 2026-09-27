# Proposed Cargo Workspace

## Workspace

```text
crates/
  utilitycss-span/
  utilitycss-diagnostics/
  utilitycss-scanner/
  utilitycss-extractor/
  utilitycss-syntax/
  utilitycss-theme/
  utilitycss-css-ir/
  utilitycss-stylesheet/
  utilitycss-utilities/
  utilitycss-variants/
  utilitycss-compiler/
  utilitycss-introspect/
  utilitycss-compat/
  utilitycss-config/
  utilitycss-cli/
  utilitycss-napi/
  utilitycss-wasm/
  utilitycss-protocol/
  utilitycss-swc/
  utilitycss-lsp/
  utilitycss-bench/
```

Not every crate should be created immediately. Split only when boundaries are useful.

## Suggested responsibilities

### `utilitycss-span`

Tiny common types:

- `SourceId`
- `Span`
- line/column helpers if needed.

Should have minimal dependencies.

### `utilitycss-diagnostics`

Typed diagnostic structures and stable error codes.

### `utilitycss-scanner`

Fast token/candidate extraction.

No theme or CSS dependencies.

### `utilitycss-syntax`

Class DSL parser and AST.

No filesystem or JS runtime dependencies.

### `utilitycss-extractor`

Conservative static extraction for quoted class attributes and known class helper calls. It may
depend on the scanner and span crates, but MUST NOT evaluate user code or own CSS semantics.

### `utilitycss-theme`

Compiled design-token representation.

### `utilitycss-css-ir`

Minimal CSS internal representation and serializer.

### `utilitycss-stylesheet`

Token-aware authored CSS parser and transformer for `@apply`-compatible composition. It delegates
utility and variant semantics to `utilitycss-compiler` and does not become a dependency of the
compiler facade.

### `utilitycss-utilities`

Built-in utility registry and lowering.

### `utilitycss-variants`

Built-in variant registry and transformations.

### `utilitycss-compiler`

High-level compiler facade, caches, incremental indexes.

### `utilitycss-introspect`

Thin discoverable package boundary for explain, validate, completion, hover, and generated
capability artifacts. It owns no parser or resolver semantics.

### `utilitycss-compat`

Explicit compatibility profile and conformance-reporting boundary. Compatibility claims remain
scoped and data-backed.

Depends on lower semantic crates.

### `utilitycss-config`

Configuration parsing/merging/validation.

Keep execution of arbitrary JavaScript out of this crate.

### `utilitycss-cli`

Filesystem traversal, watch mode, terminal diagnostics.

### `utilitycss-napi`

Node/Bun native bindings.

No compiler semantics.

### `utilitycss-wasm`

WASM bindings for Deno/browser-compatible hosts when justified.

### `utilitycss-protocol`

Versioned plain-data request/response messages for external hosts. It may depend on the compiler
facade, but MUST NOT expose internal AST or CSS IR implementation details.

### `utilitycss-swc`

AST-assisted candidate extractor for JavaScript-family sources. It may depend on SWC crates and
feeds candidates into the same compiler pipeline.

### `utilitycss-lsp`

Language Server Protocol adapter. It owns editor lifecycle state and delegates extraction and
semantic work to the compiler crates.

### `utilitycss-bench`

Benchmark fixtures and harness.

## Dependency constraints

Use workspace tooling or CI checks to prevent cycles and upward dependencies.

Ideal dependency shape:

```text
span
diagnostics
  ↑
scanner      syntax      theme      css-ir
   \           |          |          /
    \          |          |         /
      utilities + variants
              ↓
           compiler
          /   |    \
       config cli bindings
```

Exact arrows may vary, but host/runtime dependencies must stay at the edge.

## Split criteria

Create a separate crate when at least one is true:

- it has a distinct dependency profile,
- it should compile in a reduced target,
- it is reusable independently,
- it creates a strong architecture boundary,
- it materially improves compile-time feature isolation.

Do not create crates only for aesthetic symmetry.
