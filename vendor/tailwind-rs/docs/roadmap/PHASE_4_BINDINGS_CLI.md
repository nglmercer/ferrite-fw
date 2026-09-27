# Phase 4 — CLI and Bindings

## Objective

Expose the compiler outside Rust while preserving one semantic implementation.

## CLI

Capabilities:

- build,
- watch,
- input/source globs,
- output file/stdout,
- minified/dev output modes,
- diagnostics,
- config file selection,
- benchmark/debug stats mode if useful.

Filesystem behavior belongs here, not in core.

## N-API binding

Expose batched APIs.

Avoid one FFI call per candidate.

Provide TypeScript declarations.

## WASM binding

Add only the APIs practical for Deno/browser-like execution.

Be conscious of binary size and crossing overhead.

## Compatibility fixtures

CLI, N-API, and WASM outputs must match core fixtures.

## Exit criteria

- CLI can build/watch a representative app,
- Node package smoke-tested,
- Bun compatibility tested where available,
- Deno path documented,
- no semantic code duplicated in bindings.

## Won't yet

- polished Vite HMR,
- large plugin ecosystem.
