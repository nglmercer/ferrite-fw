# Architecture

## Overview

```text
                    ┌─────────────────────┐
                    │     Host tools      │
                    │ Vite / CLI / Node   │
                    │ Bun / Deno / SWC    │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │ Bindings / adapters │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │ Compiler facade API │
                    └──────────┬──────────┘
                               │
          ┌────────────────────┼─────────────────────┐
          │                    │                     │
┌─────────▼────────┐ ┌─────────▼────────┐ ┌──────────▼─────────┐
│ source/scanner   │ │ DSL parser       │ │ config/theme       │
└─────────┬────────┘ └─────────┬────────┘ └──────────┬─────────┘
          │                    │                     │
          └────────────────────┼─────────────────────┘
                               │
                    ┌──────────▼──────────┐
                    │ semantic resolver  │
                    │ utilities/variants │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │      CSS IR         │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │ ordering / emit     │
                    └─────────────────────┘
```

## Layer responsibilities

### Scanner

Owns:

- reading already-provided source bytes/text,
- identifying possible candidate tokens,
- candidate offsets/spans,
- batch scanning,
- incremental candidate diffing.

Does not own:

- theme interpretation,
- utility validation,
- filesystem traversal policy,
- JavaScript AST parsing.

### DSL parser

Transforms a candidate string into a structured AST.

Example:

```text
dark:md:hover:bg-red-500/50
```

becomes conceptually:

```text
CandidateAst
├── variants
│   ├── dark
│   ├── md
│   └── hover
└── utility
    ├── family: bg
    ├── value: red-500
    └── modifier: 50
```

### Configuration/theme

Owns:

- design tokens,
- breakpoints,
- registered utilities,
- registered variants,
- feature flags,
- resolved configuration.

Configuration should be transformed into an immutable compiled representation before hot-path compilation.
The semantic registry is the single source of truth for resolver behavior, completion, hover,
generated documentation, capability manifests, JSON Schema, and repair suggestions. Presets are
explicit data fragments; compatibility policy MUST NOT leak into core parser branches.

### Semantic resolver

Maps parsed candidates to semantic CSS behavior.

Examples:

- `flex` → declaration `display:flex`
- `p-4` → spacing utility
- `hover:` → selector transformation
- `md:` → media wrapper

### CSS IR

A minimal internal CSS representation used before final emission.

It should support:

- style rules,
- at-rules,
- declarations,
- selector transformations,
- source attribution,
- deterministic ordering keys,
- custom-property dependencies for variable-backed declarations.

The IR is intentionally smaller than a general CSS AST. Browser-sensitive lowering belongs in a
named lowering pass or compatibility profile and MUST remain deterministic.

Do not adopt a huge generic CSS AST unless real requirements justify it.

### Compiler facade

Provides ergonomic batch/incremental APIs.

Possible interface:

```rust
pub struct Compiler { /* immutable compiled config + caches */ }

impl Compiler {
    pub fn compile_candidates(&mut self, candidates: &[CandidateInput])
        -> Result<CompileOutput, CompileError>;

    pub fn update_file(&mut self, change: SourceChange)
        -> Result<IncrementalOutput, CompileError>;
}
```

### Bindings

Bindings translate between core Rust types and host representations.

Examples:

- N-API for Node/Bun,
- WASM for Deno/browser-like hosts,
- C ABI only if a concrete use case appears.

Bindings MUST NOT implement utility semantics.

### Adapters

Adapters integrate with host lifecycle APIs:

- Vite plugin hooks,
- Node file watchers,
- Bun plugin hooks,
- Deno task/build workflows,
- SWC visitor/extractor hooks.

## Dependency rule

Lower layers cannot depend on higher layers.

Good:

```text
vite-adapter -> napi-binding -> compiler -> parser
```

Bad:

```text
parser -> vite-adapter
compiler -> node
theme -> deno
```

## Data flow

```text
source change
  ↓
candidate scan
  ↓
candidate diff
  ↓
parse only new/changed candidates
  ↓
resolve semantic rules
  ↓
update rule cache/reference counts
  ↓
emit deterministic CSS
```

## Incremental model

The compiler should maintain indexes such as:

```text
file -> candidate set
candidate -> parsed AST
candidate -> compiled CSS rule(s)
rule -> reference count
```

A file update then becomes set-diff based.

## Concurrency

Parallelism may be used for:

- scanning files,
- parsing independent candidates,
- resolving independent candidates.

Final ordering must remain deterministic.

Do not expose internal thread scheduling in public semantics.

## Global state

Avoid global mutable state.

Registries should belong to a compiler/config instance.

## Error model

Errors should carry:

- stable error code,
- human-readable message,
- optional source span,
- optional filename/source id,
- optional help text.

Adapters may transform the presentation, but not the meaning.
