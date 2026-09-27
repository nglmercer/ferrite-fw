# Vision

## Problem

Utility-first CSS is productive, but current tooling is often tightly coupled to a specific JavaScript runtime or build ecosystem. A modern compiler should be reusable from native Rust, Node.js, Bun, Deno, Vite, and other hosts without making any one of those environments the architectural center.

## Vision

Build a high-performance utility CSS compiler platform in Rust.

The project provides:

- a class-name DSL,
- fast source scanning,
- utility and variant parsing,
- theme/config resolution,
- deterministic CSS generation,
- incremental compilation,
- native and JavaScript-facing APIs,
- optional AST-assisted extraction,
- first-class host adapters.

## Product principles

### 1. Compiler first

The primary product is a compiler.

CLI, Vite, Node, Bun, Deno, and SWC integrations are clients of the compiler.

### 2. Runtime agnostic

The semantic core must be usable without JavaScript.

### 3. Fast default path

Basic source discovery should not require parsing full JavaScript/TypeScript ASTs.

### 4. Structured internals

Class names form a DSL and must be represented structurally.

### 5. Deterministic output

Given identical source candidates, configuration, feature flags, and compiler version, output must be byte-for-byte stable.

### 6. Incremental by design

Watch mode is not an afterthought. Data structures and APIs must support invalidating only affected work.

### 7. Extensible, not arbitrary

Extension points should be typed and explicit. Avoid arbitrary runtime execution inside the compiler core.

### 8. Useful diagnostics

Errors should identify:

- what failed,
- where,
- why,
- how to fix it when possible.

## Goals

- competitive scan and rebuild performance,
- stable class-name grammar,
- zero browser JavaScript requirement,
- portable native core,
- excellent Vite integration,
- usable Node/Bun/Deno APIs,
- optional SWC-based smart extraction,
- documented plugin/extension model,
- reliable source maps or source attribution where applicable.

## Non-goals for v1

- fully cloning Tailwind's syntax,
- executing Tailwind plugins,
- supporting every historical PostCSS behavior,
- arbitrary JavaScript configuration in core,
- browser runtime styling,
- CSS-in-JS runtime injection,
- automatic understanding of all dynamically generated strings.

## Success criteria

A successful v1 can:

1. scan realistic multi-language projects,
2. detect complete static utility candidates,
3. compile a useful utility/variant subset,
4. rebuild incrementally,
5. expose native + Node/Bun/Deno-compatible interfaces,
6. integrate cleanly with Vite,
7. remain semantically identical across all adapters.
