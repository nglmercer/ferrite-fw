# Code Style

## Rust

Follow standard Rust style plus these project-specific rules.

### Formatting

Use `rustfmt`.

No manual alignment that fights the formatter.

### Lints

Use Clippy with warnings denied in CI, with narrowly documented exceptions.

### Naming

- crates: `utilitycss-*`
- modules/functions/variables: `snake_case`
- types/traits: `UpperCamelCase`
- constants: `SCREAMING_SNAKE_CASE`

Use semantic names:

Good:

```rust
CandidateAst
ResolvedVariant
RuleOrderKey
SourceChange
```

Avoid vague names:

```rust
Data
Thing
Manager
Helper
Processor
```

unless the abstraction is genuinely generic.

### Error handling

Use `Result` for recoverable failures.

Do not panic on malformed user input.

Use `debug_assert!` for internal invariants only when release behavior remains safe.

### Public APIs

Prefer small structs with private fields plus constructors only when invariants require it.

Avoid public enums with dozens of implementation-specific variants if a stable abstraction can be smaller.

### Ownership

Prefer borrowing in scanner/parser hot paths when it keeps APIs understandable.

Do not turn the codebase into lifetime puzzles to save trivial allocations.

Measure first.

### Collections

Output-relevant iteration MUST be deterministic.

If using `HashMap`/`HashSet` internally, sort before stable emission or use an explicitly deterministic strategy.

### Concurrency

Do not rely on execution order of parallel workers.

Concurrency should be an implementation detail.

### `unsafe`

`unsafe` is allowed only when:

- there is a measurable need or FFI boundary,
- the safety invariant is documented,
- tests target the invariant,
- a safe alternative was considered.

### Comments

Comments should explain why, not restate syntax.

Good:

```rust
// Keep source-order here because variant composition is not commutative.
```

Bad:

```rust
// Loop through variants.
```

### Modules

Prefer one clear responsibility per module.

Avoid `utils.rs` dumping grounds.

## TypeScript / JavaScript

### TypeScript

Use strict mode.

Public adapter APIs require explicit return types.

Avoid `any`; use `unknown` at external boundaries and validate.

### Bindings

TypeScript should:

- validate host-level options,
- translate paths/errors,
- call native/WASM compiler,
- manage plugin lifecycle.

TypeScript should NOT reimplement:

- class parsing,
- utility semantics,
- variant semantics,
- CSS ordering.

### Imports

Prefer explicit imports and package exports.

Avoid deep imports into private adapter internals.

## Markdown

Use:

- one H1 per file,
- sentence-case headings,
- fenced code blocks with language tags,
- relative links inside the repository,
- MUST/SHOULD/MAY consistently.

## API examples

Examples in docs should be executable or clearly marked conceptual.

Do not leave stale APIs in documentation after refactors.
