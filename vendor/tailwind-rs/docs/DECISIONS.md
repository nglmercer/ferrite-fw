# Architectural Decisions

This file summarizes active decisions. Detailed decisions should be recorded as ADRs using `templates/ADR_TEMPLATE.md`.

## ADR-0001 — Rust core is runtime-agnostic

Status: accepted.

Decision:

The semantic compiler core does not depend on Node.js, Bun, Deno, Vite, or SWC.

Reason:

- portability,
- testability,
- reuse,
- stable architecture.

## ADR-0002 — Default scanning is text based

Status: accepted.

Decision:

Static candidate scanning does not require host-language AST parsing.

Reason:

- performance,
- language independence,
- simpler dependency graph.

AST extraction is optional.

## ADR-0003 — Class names are parsed as a DSL

Status: accepted.

Decision:

Candidates are parsed into structured AST before semantic resolution.

Reason:

- correctness,
- maintainability,
- diagnostics,
- extensibility.

## ADR-0004 — Utility and variant semantics are separate

Status: accepted.

Decision:

Utilities lower base CSS behavior; variants transform selectors/wrappers/rules.

Reason:

Avoid combinatorial duplication.

## ADR-0005 — Output is deterministic

Status: accepted.

Decision:

CSS output cannot depend on hash iteration, thread order, or filesystem enumeration.

## ADR-0006 — No arbitrary JS execution in core config

Status: accepted.

Decision:

Core configuration is declarative/serializable.

Node adapters may later support resolving user JS config externally into core-compatible data.

## ADR-0007 — Incremental state is first-class

Status: accepted.

Decision:

Compiler APIs and indexes are designed for updates/removals, not only one-shot builds.

## ADR-0008 — Stylesheet parsing is a separate token-aware layer

Status: accepted.

Decision:

`utilitycss-stylesheet` owns CSS stylesheet parsing, source spans, `@apply` discovery, and
authored-CSS serialization. It uses the standards-oriented `cssparser` tokenizer for CSS token
validation and a small scope AST for deterministic transformation. The stylesheet crate consumes
the compiler facade; the compiler does not depend on the stylesheet layer.

Reason:

- `utilitycss-css-ir` remains a semantic output representation rather than a general-purpose CSS
  parser;
- comments, strings, nested functions, brackets, and blocks cannot be handled safely by a regular
  expression;
- the small AST preserves unrelated authored CSS while allowing selector-aware composition;
- `cssparser` is runtime-independent and builds for native and WASM targets;
- its MPL-2.0 dependency license is explicitly allowed by the repository dependency policy.

## Open decisions

Topics requiring future ADR/RFC:

- exact CSS ordering model,
- CSS-first configuration grammar,
- registry/plugin public API,
- N-API vs alternate Node binding implementation details,
- WASM support matrix,
- compatibility preset strategy,
- source map model.
