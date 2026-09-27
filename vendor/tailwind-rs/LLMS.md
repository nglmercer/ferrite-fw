# LLMS.md

Instructions for LLMs and autonomous coding agents working in this repository.

## Mission

Build a fast, deterministic, runtime-agnostic utility CSS compiler in Rust with thin ecosystem adapters.

The compiler is not "a Rust port of Tailwind." It is its own compiler platform with a class-name DSL, theme system, variant system, incremental build model, and adapter layer.

The compiler can explain its own language. Prefer `utilitycss-cli explain`, `validate`, and
`capabilities --output-dir <dir>` when an agent needs to discover names, values, diagnostics,
provenance, or generated CSS. Generated `capabilities.json`, `capabilities.schema.json`,
`REFERENCE.md`, `llms.txt`, and `llms-full.txt` are derived from the active semantic registry.

## Read order

Before modifying architecture, read:

1. `docs/VISION.md`
2. `docs/ARCHITECTURE.md`
3. `docs/SPECS.md`
4. `docs/CLASS_DSL_SPEC.md`
5. `docs/COMPILER_PIPELINE.md`
6. `docs/CRATE_LAYOUT.md`
7. the active roadmap phase under `docs/roadmap/`

Before changing public APIs, also read:

- `docs/ADAPTERS.md`
- `docs/DECISIONS.md`
- `docs/RELEASE.md`

Before changing tests or performance-sensitive code, read:

- `docs/TESTING.md`
- `docs/PERFORMANCE.md`

## Agent priorities

In order:

1. correctness,
2. determinism,
3. API clarity,
4. incremental build behavior,
5. performance,
6. compatibility,
7. convenience.

Do not trade correctness or deterministic output for micro-optimizations without measurements.

## Hard architectural constraints

Agents MUST preserve these constraints:

- `utilitycss-core` cannot depend on runtime adapters.
- Host adapters cannot define compiler semantics.
- The default scanner is text/token based.
- AST-assisted extraction is optional and separate.
- The parser produces a stable structured representation before CSS generation.
- Utility semantics are separate from variant semantics.
- Configuration resolution is separate from code generation.
- CSS output must be deterministic for the same inputs.
- Filesystem, clock, environment, and process access must sit behind explicit boundaries.
- No required browser runtime is allowed for generated CSS.
- Public Rust APIs should not expose JavaScript-runtime types.
- N-API/WASM bindings must map onto core types, not duplicate semantics.

## Bun integration contract

`@utilitycss/node` is the generic JavaScript lifecycle adapter. `@utilitycss/bun` is the dedicated
Bun bundler/fullstack integration and MUST use Bun plugin lifecycle hooks rather than wrapping the
CLI watcher or starting a second filesystem watcher.

The Bun plugin MUST create a fresh compiler for each bundler build cycle, collect supported source
modules through Bun's module graph, expose generated CSS through the virtual `utilitycss` specifier,
defer CSS generation until source loading completes, preserve structured diagnostics, and reflect
source deletion by rebuilding from the current graph. Development MUST NOT require writing
`public/utilitycss.css`.

The repository example at `examples/bun` is the integration reference. Its `bunfig.toml` covers
`bun --hot src/server.ts`; `src/production-build.ts` covers explicit `Bun.build()` production
usage; and `bun run verify` covers compiler loading, CSS output, and mock API behavior. The browser
entry is a typed Preact application in `src/app.tsx`; Preact is example-level UI code and MUST NOT
become a dependency of the compiler or Bun adapter packages. The example keeps visual design in
`src/style.css` and repeated static utility combinations in `src/styles.ts`; it MUST NOT claim
Tailwind `@apply` compatibility without a dedicated CSS-entry implementation and conformance suite.

## Working method

For each task:

1. identify the active roadmap phase,
2. locate the owning crate/module,
3. write or update tests first for externally observable behavior,
4. make the smallest coherent implementation,
5. run formatting, linting, unit tests, integration tests, and targeted benchmarks,
6. update docs when semantics or public APIs change.

## When a task is ambiguous

Prefer the interpretation that:

- keeps the compiler core runtime-independent,
- adds the least new public API,
- preserves deterministic output,
- avoids global state,
- supports incremental rebuilds,
- can be tested without JavaScript.

If ambiguity changes product semantics, write an ADR or RFC before implementing.

## Prohibited shortcuts

Do not:

- parse class names with one large regex,
- put Vite-specific behavior in Rust core crates,
- read the filesystem from parser functions,
- execute arbitrary JavaScript configuration in core,
- silently accept invalid syntax when an error should be reported,
- introduce nondeterministic hash iteration into output ordering,
- cache using only file modification times,
- add unsafe Rust without a documented reason and focused tests,
- duplicate utility resolution logic across adapters,
- make AST parsing mandatory for basic class discovery.

## Preferred implementation shape

Use pure functions where practical:

```rust
scan(source) -> candidates
parse(candidate) -> CandidateAst
resolve(candidate_ast, theme, registry) -> ResolvedUtility
lower(resolved) -> CssIr
emit(css_ir) -> String
```

Real implementation may batch work, but semantic boundaries should remain visible.

The facade also exposes registry-backed `explain`, `validate`, `completions`, `hover`, and
capability APIs. Adapter code MUST delegate to these APIs instead of maintaining lists of utility
names or duplicating compiler semantics.

## Documentation obligations

Update documentation whenever you change:

- DSL grammar,
- theme/config rules,
- public APIs,
- crate boundaries,
- adapter contracts,
- error codes,
- output ordering,
- cache behavior,
- compatibility policy.

## Definition of done

A change is complete only when:

- behavior is tested,
- public semantics are documented,
- formatting/lints pass,
- new errors are actionable,
- no cross-layer dependency rule is violated,
- benchmarks are updated if the hot path changed.

## Suggested commit scope

Prefer commits such as:

- `scanner: recognize escaped arbitrary values`
- `parser: add modifier AST node`
- `compiler: make rule ordering deterministic`
- `vite: add invalidation bridge`
- `docs: define variant ordering`

Avoid giant cross-cutting commits unless the task is a planned migration.

## Generated code

Generated files must include a header stating how to regenerate them. Agents should modify the generator, not the generated output, unless the repository explicitly documents otherwise.

## Local release verification

Run `npm run release:check` before release preparation. It MUST report unavailable external platform
tests as `SKIP`, never as passing checks. GitHub Actions jobs that cannot start because of an
external billing/account limitation are not release evidence; keep the workflow configuration
valid and record local verification separately.

## Final agent note

If code behavior conflicts with these docs, do not automatically assume the code is correct. Determine whether the code is outdated, the docs are outdated, or an ADR supersedes both.
