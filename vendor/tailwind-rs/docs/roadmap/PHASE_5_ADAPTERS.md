# Phase 5 — Build Tool and Runtime Adapters

## Objective

Provide production-grade host integrations.

## Vite

Requirements:

- compiler instance persists across HMR,
- source updates are incremental,
- CSS virtual/entry module invalidates correctly,
- dev/build output semantics match,
- diagnostics integrate with Vite errors.

## Node

Provide direct API and watcher helpers only if they add value beyond the binding.

## Bun

Test the Node-compatible binding first.

Provide plugin wrapper for Bun lifecycle if useful.

## Deno

Document supported path:

- WASM,
- npm compatibility,
- or both.

Avoid hidden Node assumptions.

## Adapter tests

Create shared fixture harness.

## Exit criteria

- Vite example app works in dev/build,
- HMR updates only affected sources,
- adapter semantic fixtures are identical,
- error reporting is usable,
- docs contain installation examples.

## Won't yet

- framework-specific AST inference.
