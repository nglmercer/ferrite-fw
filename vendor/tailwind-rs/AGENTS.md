# AGENTS.md

Repository instructions for AI agents and automated contributors.

## Scope

These instructions apply to the entire repository unless a nested `AGENTS.md` overrides them.

## Repository values

- small compiler core,
- explicit intermediate representations,
- deterministic behavior,
- zero unnecessary runtime coupling,
- measurable performance,
- boring and maintainable code.

## Commands

Expected baseline commands once the workspace exists:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p utilitycss-compiler
cargo bench -p utilitycss-bench
```

For JavaScript adapter packages, npm is the selected package manager:

```bash
npm run lint
npm test
npm run typecheck
```

Use the repository's actual package manager once chosen.

## Rust rules

- Rust stable is the default toolchain.
- `unsafe` requires a safety comment and justification.
- Prefer borrowed inputs where ownership is unnecessary.
- Do not prematurely optimize allocations before profiling.
- Public items require rustdoc unless trivially self-explanatory.
- Errors exposed across crate boundaries should be typed.
- Avoid panics on user-controlled input.
- Prefer deterministic collections or explicit sorting before emission.
- Do not leak adapter-specific types into compiler crates.

## TypeScript rules

- TypeScript strict mode is required.
- Bindings should be thin.
- Do not reimplement parsing or compiler semantics in TypeScript.
- Prefer generated types from a schema when practical.
- Treat runtime adapters as transport/integration layers.

## Testing rules

Every bug fix should include a regression test.

Snapshot tests are acceptable for:

- CSS output,
- diagnostics,
- serialized IR,
- adapter protocol fixtures.

Snapshots must be deterministic and readable.

## Performance rules

Any change to:

- scanning,
- candidate parsing,
- incremental invalidation,
- rule generation,
- output serialization

should be benchmarked if it is likely to affect a hot path.

## Docs rules

Use RFC 2119-style words consistently:

- MUST
- SHOULD
- MAY
- MUST NOT

Do not document unimplemented features as completed.

## Architecture boundaries

Dependency direction:

```text
adapters
   ↓
bindings
   ↓
compiler facade
   ↓
core semantic crates
```

Core semantic crates MUST NOT depend upward.

## Review checklist

Before finalizing a change, verify:

- Is the behavior deterministic?
- Is the correct layer responsible?
- Is there a test?
- Is the error useful?
- Is the API smaller than an alternative design?
- Does it work without Node?
- Could it break incremental builds?
- Do docs need updating?
