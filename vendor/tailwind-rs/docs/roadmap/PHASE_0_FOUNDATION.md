# Phase 0 — Foundation

## Objective

Create the smallest trustworthy foundation for the compiler.

## Deliverables

- Cargo workspace.
- Core crate skeletons.
- CI.
- rustfmt + Clippy policy.
- shared spans and diagnostics.
- minimal CSS IR.
- benchmark harness.
- documentation linked from root README.

## Required decisions

- minimum supported Rust version,
- error/diagnostic shape,
- source ID/span types,
- workspace dependency policy,
- benchmark runner.

## Tasks

1. Create workspace `Cargo.toml`.
2. Add `utilitycss-span`.
3. Add `utilitycss-diagnostics`.
4. Add minimal `utilitycss-css-ir`.
5. Add `utilitycss-compiler` placeholder facade.
6. Set formatting/lints.
7. Add test/benchmark CI.
8. Add architecture dependency checks if practical.

## Exit criteria

- `cargo fmt --check` passes.
- Clippy warnings fail CI.
- workspace tests pass.
- at least one benchmark runs.
- public placeholder APIs have docs.
- no runtime-specific dependency appears in core crates.

## Won't yet

- real scanner,
- DSL parser,
- utilities,
- Vite integration.
