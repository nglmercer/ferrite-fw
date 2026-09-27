# Phase 1 — Scanner and Parser

## Objective

Discover static candidates quickly and parse them into a stable AST.

## Deliverables

- text scanner,
- candidate span tracking,
- parser,
- escape handling,
- arbitrary-value structural parsing,
- parser diagnostics,
- fuzz targets,
- scanner/parser benchmarks.

## Scanner principles

The scanner may over-collect.

It must remain:

- fast,
- language-agnostic,
- safe on malformed input.

## Parser scope

Support:

- basic utility names,
- values,
- negative prefix,
- modifier,
- variant chain,
- arbitrary values,
- important marker if approved.

## Tests

Must cover:

- HTML,
- JSX,
- TSX,
- template literals,
- Vue/Svelte-like markup,
- Unicode,
- escaping,
- malformed brackets,
- long input.

## Exit criteria

- scanner never panics on fuzz corpus,
- parser never panics on arbitrary bytes converted/handled safely,
- candidate AST documented,
- insertion/input order does not affect parsed semantics,
- benchmarks establish baseline throughput.

## Won't yet

- utility CSS generation,
- AST smart extraction,
- Vite.
