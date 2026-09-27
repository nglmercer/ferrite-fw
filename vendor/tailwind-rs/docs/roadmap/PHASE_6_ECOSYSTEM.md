# Phase 6 — Smart Extraction and Ecosystem

## Objective

Expand capabilities without compromising the simple compiler core.

## SWC smart extractor

The dependency-free extractor remains the low-cost fallback. The production JavaScript-family
path is now `utilitycss-swc`, pinned to the current SWC parser/AST/visitor releases used by the
workspace. It feeds candidate text and byte spans toward the same compiler pipeline and does not
evaluate expressions.

Potentially understand:

```tsx
className="p-4"
clsx("p-4", active && "bg-red-500")
cva(...)
```

Only statically provable candidates should be emitted.

Do not execute user code.

## Framework extractors

Available static adapters:

- React/JSX and Solid-like JSX through SWC,
- Vue `:class`/`v-bind:class` literals,
- Svelte `class:` directives,
- Astro `class:list` literals,
- plain HTML-like templates through the conservative extractor.

Framework adapters MUST remain conservative and MUST document unsupported dynamic forms.

The text scanner remains the baseline.

## Plugin system

Before implementing plugins, answer:

- trusted native vs portable plugins?
- can plugins define utilities?
- can plugins define variants?
- can they affect ordering?
- how are plugin versions isolated?
- how is determinism preserved?

Prefer declarative plugin descriptions where possible.

## IDE services

The `utilitycss-lsp` crate now provides:

- candidate parse API,
- validation,
- completion metadata,
- hover docs,
- basic candidate hover information.

Class-to-generated-CSS preview and workspace-aware configuration reload remain future increments.

## Compatibility presets

The config crate now provides a versioned `utilitycss` baseline and an explicitly named
`tailwind-v4-subset` profile, plus declarative utility and variant extensions. This is not full
Tailwind compatibility. Any broader compatibility MUST add a conformance suite before being
advertised.

Do not claim full compatibility without a conformance suite.

## Exit criteria

This phase is iterative. Features graduate when they have:

- stable semantics,
- tests,
- performance data,
- documentation,
- adapter/core boundary compliance.
