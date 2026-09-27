# Production contract

This document defines the minimum contract for a production release. A release MUST satisfy the
Rust, adapter, conformance, security, and packaging gates listed below; roadmap items MUST NOT be
described as supported until their gate is present.

## Supported extraction

The `utilitycss-swc` crate MUST parse JavaScript, TypeScript, JSX, and TSX with SWC and MUST
preserve byte-accurate candidate spans. It MUST extract static `class` and `className` attributes
and literals used by the configured class helpers (`clsx`, `classnames`, `cn`, `cva`, `tv`,
`twJoin`, and `twMerge`). It MUST NOT evaluate arbitrary JavaScript or claim that dynamic template
expressions are complete.

The dependency-free extractor remains available for environments that cannot ship SWC. Adapters
SHOULD select the SWC extractor for JavaScript-family files and MAY use the conservative extractor
for markup or unknown file types.

## Compiler behavior

Given the same configuration and candidate stream, the compiler MUST emit deterministic CSS and
deterministic diagnostics. Invalid user-controlled input MUST produce typed diagnostics rather than
panics. Source updates MUST invalidate only affected source state and MUST remain correct when the
same candidate occurs in multiple source units.

## Integration boundaries

Bindings and adapters MUST remain transport layers. Core semantic crates MUST NOT depend on Node,
SWC adapter policy, editor protocols, or framework runtimes. Plugin and compatibility behavior MUST
be represented by explicit, versioned configuration or registry data.

## Release gates

Every release MUST pass:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-features`
- compiler conformance fixtures and adapter protocol fixtures
- JavaScript lint, typecheck, and tests
- release builds for every declared N-API target and a WASM smoke test
- dependency, license, and package-content audits
- scanner/extractor benchmarks and fuzzing or property-based malformed-input tests
