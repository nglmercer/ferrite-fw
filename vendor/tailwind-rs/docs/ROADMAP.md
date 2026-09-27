# Roadmap

The roadmap is organized around compiler risk, not marketing features.

## Phase 0 — Foundation

Goal: establish workspace, architecture, invariants, CI, benchmarks, and minimal IR.

Exit criteria:

- Cargo workspace exists.
- Core dependency direction is enforced.
- Formatting/lint/test CI passes.
- Benchmark harness exists.
- Error type and source span conventions exist.

See `roadmap/PHASE_0_FOUNDATION.md`.

## Phase 1 — Scanner + DSL parser

Goal: reliably discover and parse static utility candidates.

Exit criteria:

- text scanner works on representative HTML/JSX/TSX/Vue/Svelte-like input,
- escape rules are defined,
- candidate AST is stable enough for semantic work,
- fuzzing exists for parser/scanner boundaries.

See `roadmap/PHASE_1_SCANNER_PARSER.md`.

## Phase 2 — Utility compiler

Goal: compile a useful utility subset to deterministic CSS.

Initial families:

- display,
- spacing,
- sizing,
- color,
- border radius,
- typography subset,
- grid/flex subset.

Exit criteria:

- theme resolution works,
- deterministic ordering works,
- arbitrary values work for approved grammars,
- utility registry API is established.

See `roadmap/PHASE_2_UTILITY_COMPILER.md`.

## Phase 3 — Variants + incremental engine

Goal: support state/responsive variants and fast rebuilds.

Initial variants:

- hover,
- focus,
- active,
- disabled,
- dark,
- responsive breakpoints,
- group/peer subset.

Exit criteria:

- variant composition rules are specified,
- file-to-candidate diffing works,
- unchanged candidates are not recompiled,
- watch benchmark suite exists.

See `roadmap/PHASE_3_VARIANTS_INCREMENTAL.md`.

## Phase 4 — Native CLI + JS bindings

Goal: expose stable compiler behavior to native and JS runtimes.

Deliverables:

- CLI,
- N-API package for Node/Bun,
- WASM package where useful,
- stable serialization protocol.

See `roadmap/PHASE_4_BINDINGS_CLI.md`.

## Phase 5 — Vite + runtime adapters

Goal: make integration excellent.

Deliverables:

- Vite plugin,
- Node API,
- Bun-tested compatibility,
- Deno integration path,
- file invalidation bridge,
- dev/prod behavior parity.

See `roadmap/PHASE_5_ADAPTERS.md`.

## Phase 6 — Smart extraction + ecosystem

Goal: add optional AST-assisted extraction and extension points.

Delivered in the current production-readiness increment:

- SWC-assisted extractor,
- Vue/Svelte/Astro framework-specific static extractors,
- declarative utility/variant plugin descriptions,
- IDE/LSP diagnostics, completion, and hover services,
- explicitly scoped compatibility presets.

Remaining release work is broader compatibility coverage, workspace-aware IDE configuration, and
platform/package release verification. These MUST be accompanied by conformance fixtures before
being advertised as stable.

See `roadmap/PHASE_6_ECOSYSTEM.md`.

## Release milestones

Suggested:

- `0.1` — scanner/parser experimental,
- `0.2` — basic utilities,
- `0.3` — variants + incremental,
- `0.4` — CLI/bindings,
- `0.5` — Vite integration,
- `0.6+` — ecosystem hardening,
- `1.0` — grammar/API stability commitment.

## Feature gate policy

Experimental behavior should be behind explicit feature flags or unstable APIs until its semantics are documented.

## vNext implementation track

The LLM-first vNext track extends Phase 6 with the following release-oriented gates. Each gate is
implemented from the shared semantic registry; adapters and generated artifacts MUST delegate to
that registry rather than maintain parallel capability lists.

| Release | Gate | Workspace evidence |
| --- | --- | --- |
| `0.7` | Versioned candidate grammar, typed AST, arbitrary-property safety, and scanner reachability | `utilitycss-syntax`, `utilitycss-scanner`, `docs/CLASS_DSL.ebnf` |
| `0.8` | Declarative utility/variant metadata, typed values, dependencies, and validation | `utilitycss-utilities`, `utilitycss-variants`, `utilitycss-theme` |
| `0.9` | Core layout, spacing, typography, color, effects, grid/flex, SVG, and composable variants | semantic registries and CSS IR |
| `0.10` | CSS-first configuration, declarative JSON extensions, presets, and config fingerprints | `utilitycss-config` |
| `0.11` | Explain/validate/completion/hover, diagnostics, provenance, generated reference, schema, and LLM artifacts | `utilitycss-compiler`, `utilitycss-introspect`, CLI/LSP |
| `0.12` | Text/static/AST/hybrid extraction and executable compatibility fixtures | `utilitycss-extractor`, `utilitycss-protocol`, `utilitycss-compat` |
| `0.13` | Composite utilities, custom-property dependencies, arbitrary typed values, and modern CSS families | `utilitycss-utilities`, `utilitycss-css-ir` |
| `0.14` | Browser-target feature analysis and deterministic adapter transport | `utilitycss-css-ir`, compiler/browser diagnostics, N-API/WASM/protocol |
| `0.15` | Ecosystem hardening, reproducibility, benchmark coverage, and documentation freeze review | workspace checks and conformance fixtures |

These labels describe implementation gates, not compatibility promises. A Tailwind-like profile is
valid only for the behavior covered by its named fixtures. Native grammar and semantics remain the
default, and unsupported browser features produce structured findings instead of being silently
discarded.
