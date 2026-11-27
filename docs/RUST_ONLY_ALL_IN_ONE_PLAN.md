# Rust-only all-in-one frontend plan

This plan turns Ferrite into a complete Vite alternative delivered as one
self-contained Rust toolchain: one `ferrite` binary for creating, developing,
building, rendering, previewing and packaging frontend applications without
requiring Node.js, npm, bun, pnpm or `node_modules` in normal use.

It is a planning document, not a compatibility claim. Implementation and
acceptance remain governed by [COMPLETION_PLAN.md](COMPLETION_PLAN.md),
[VITE_COMPATIBILITY.md](VITE_COMPATIBILITY.md) and `scripts/acceptance.py`.

## Current baseline

Repository state checked on 2026-11-27:

- `main` is at `b0e37da` (PR #2). The newest reviewed work is the three-commit
  chain through `76ea5e9` on `fix/production-css-chunk-correctness`:
  production CSS/chunk/source-map correctness plus isolated SSR documents and
  selective framework hydration. That chain is merged within the stack but not
  yet merged back to `main`.
- Already implemented: Rust-native dev server, Oxc/SWC transform backends,
  module graph, resolver, npm registry installation and locking, HMR, HTML,
  CSS, assets, production bundling/manifests/source maps, SSR adapters,
  buffered/streamed rendering, static generation, embedded `rquickjs`, framework
  scaffolds and Rust-native browser E2E infrastructure.
- React uses the native compiler path, but Refresh coverage is incomplete.
- Vue 3.5.22 and Svelte 5.39.6 still require an explicitly enabled external
  Node compiler host for official compilation. This is the largest violation
  of the Rust-only goal.
- The SSR document/islands acceptance evidence is bounded to Linux. The final
  complete 17-gate SSR run was not green because a browser fixture could not
  reach the npm registry; that run must be repeated after networking is fixed
  or fixture inputs are cached explicitly.
- Production behavior has advanced substantially, but cyclic graphs, workers,
  WASM, nonliteral dynamic imports, complete plugin render-map composition,
  cross-platform isolation and performance budgets remain open.

## Meaning of “Rust-only”

The target has three strictness levels:

1. **Operational Rust-only** — required first. No external Node/npm/bun/pnpm
   executable is needed for `create`, `dev`, `build`, `ssr`, `preview`,
   `prerender`, `e2e` or standalone output. Browsers are test dependencies,
   not application runtime dependencies.
2. **Single-binary distribution** — the normal `ferrite` executable embeds its
   compiler frontends, runtime glue, framework compiler payloads, client HMR
   runtime, scaffolding templates and standalone server SDK metadata. Project
   dependencies still install into `.ferrite/npm/`; they are application data,
   not toolchain programs.
3. **Rust-native implementation** — the final preferred state. Framework
   compilation and all core transformations are implemented in Rust crates.
   Running official JavaScript compiler packages inside the embedded Rust
   runtime is acceptable only as a transitional compatibility bridge, not as
   the final strict native state.

The tier-3 external Node adapter may remain as an explicitly configured
compatibility escape hatch for foreign plugins, but it must never be required
by a supported application profile.

## Product target

A user should be able to run:

```bash
ferrite create app --framework react|vue|svelte|vanilla --language ts --rendering client|ssr
cd app
ferrite dev
ferrite build
ferrite preview
ferrite build --standalone
```

with only the `ferrite` binary installed. The binary must work from an empty
`PATH`, outside the Ferrite source checkout, and without a compiler sidecar.
Unsupported compatibility behavior must fail explicitly rather than silently
switching hosts or versions.

## Workstreams

### 0. Consolidate and reproduce the existing tip

**Goal:** make the current reviewed state the trusted baseline.

- Merge the `fix/production-css-chunk-correctness` chain into `main` or open a
  dedicated integration PR.
- Rerun `python3 scripts/acceptance.py --profile production` and
  `python3 scripts/acceptance.py --profile ssr` on one clean commit.
- Remove incidental network dependence from browser fixtures by using locked,
  explicitly cached package inputs or a local fixture registry.
- Record exact tool versions, host, command logs, dirty-tree state and failure
  output in the acceptance report.
- Keep framework/SSR changes separate from production-correctness changes in
  future PRs.

**Exit gate:** all selected production and SSR gates pass on one Linux release
commit without registry flakes, and the report is reproducible by a clean
checkout.

### 1. Finish the Vite-compatible core contract

**Goal:** complete the framework-independent behavior expected by frontend
applications.

Priority areas:

- Config/env resolution, aliases, conditions, package exports/imports and
  package-map URL query/fragment behavior.
- Dependency installation, lock replay, peer/optional dependency behavior,
  workspace links and remaining CommonJS interoperability.
- Dev module graph invalidation, cold-import recovery, watcher ownership and
  full HMR planning/rollback semantics.
- HTML rewriting, assets, workers, WASM, JSON, CSS modules, preprocessors and
  import maps.
- Plugin lifecycle/context contracts, with unsupported Vite/Rollup hooks
  failing explicitly.
- Stable programmatic Rust API for embedded dev/build/preview use.

**Exit gate:** a versioned compatibility matrix maps every advertised Vite
feature to `verified`, `partial`, `explicitly unsupported` or `missing`, with
one regression fixture for every verified row.

### 2. Replace external framework compiler hosts

**Goal:** remove Node from supported React, Vue and Svelte profiles.

#### React

- Complete the native Oxc React Refresh implementation: boundary detection,
  hook signatures, anonymous/default exports, memo/forwardRef, barrels,
  mixed exports and unsupported-pattern diagnostics.
- Expand browser state-preservation/reset matrices in Chromium and Firefox.
- Keep JSX/TSX transformation and production SSR on the existing native path.

#### Vue

Implement a Rust-owned Vue SFC compiler pipeline behind the existing framework
compiler abstraction:

1. SFC parser: blocks, attributes, custom blocks, source ranges and diagnostics.
2. Script analysis: normal/script-setup merging, imports, bindings, props,
   emits, exposes, slots, types and macro semantics.
3. Template compiler: AST, directives, slots, async components, caching hints,
   hydration metadata and SSR/client codegen.
4. Style compiler: scoped selectors, CSS modules, CSS variables, preprocessors,
   ordering and source maps.
5. HMR metadata: component identity, rerender/reload boundaries and state/reset
   contracts.

Evaluate existing Rust crates before writing new parsers/codegen. If an
embedded official JavaScript compiler is used temporarily, it must run inside
the embedded Rust runtime as a versioned, pure-computation payload with fixed
inputs/outputs, cache identity and provenance. That bridge is a milestone, not
final Rust-native completion.

#### Svelte

Implement the same staged replacement for Svelte 5:

1. Component parser and instance/module script separation.
2. Runes/state/derived/effect analysis and JavaScript transformations.
3. Template compilation for client and SSR output.
4. Style scoping, unused selector diagnostics and source maps.
5. Component resources, snippets, hydration and state/reset HMR contracts.

Pin one framework version at a time. Do not advertise version ranges until the
acceptance matrix passes for each version.

**Exit gate:** generated React/Vue/Svelte JS and TypeScript applications run
`create`, `dev`, `build`, `ssr` and `preview` with no Node executable present,
including with an empty `PATH`.

### 3. Complete application-level HMR and framework behavior

**Goal:** move from compiler success to real application behavior.

- Vue: slots, directives, async components, imported types, scoped/module
  styles, granular HMR and production leakage checks.
- React: finish Refresh matrix, including unsupported patterns and explicit
  full-reload fallback.
- Svelte: wider runes, component resources, state/reset semantics and
  diagnostics.
- Type-aware checking is not required for Vite parity because Vite transpiles
  without type-checking. If added later, it must be optional and must not block
  the operational Rust-only target.
- Every supported behavior needs dev, production, SSR and browser evidence.

**Exit gate:** the registry may declare a framework/version profile complete
only after the full generated-application matrix passes in Chromium and Firefox
on the advertised host platforms.

### 4. Finish production bundling correctness

**Goal:** dependable output for real applications, not only fixtures.

- Complete emitted chunk/runtime map chains and external source-map sections.
- Compose plugin `renderChunk` maps instead of dropping stale maps.
- Cover cyclic imports, nonliteral dynamic imports, workers, WASM and broader
  asset closure elimination.
- Validate scope-hoist bailouts and preserve execution order/side effects.
- Benchmark before choosing any Rolldown backend; keep the current backend
  unless parity and performance both pass.
- Add budgets for cold/warm dev startup, transform latency, HMR latency,
  production build time, SSR concurrency and RSS.

**Exit gate:** the production acceptance profile covers representative real
application graphs in all minify/scope-hoist combinations and publishes measured
performance baselines with hardware metadata.

### 5. Complete SSR, documents, islands and streaming

**Goal:** one coherent rendering model across dev, preview, prerender and
standalone binaries.

- Finish independent per-entry runtime policies and external/native contracts.
- Implement broader Web Streams compatibility and cancellation behavior.
- Expand automatic island ownership rules, nested-island diagnostics and lazy
  preload closure.
- Extend route shells, style isolation, streaming and selective hydration to
  all supported framework profiles.
- Keep artifact format versioning explicit and backward compatible.
- Extend process isolation beyond Linux and validate native-addon fixtures.

**Exit gate:** one SSR acceptance matrix passes for generated applications in
Chromium and Firefox on Linux, macOS and Windows, including streamed output,
disconnect cancellation and standalone execution with an empty `PATH`.

### 6. Package, plugin and ecosystem compatibility

**Goal:** support real frontend dependency graphs without promising universal
compatibility.

- Complete package-map URL semantics and CJS helper interoperability.
- Add native `.node` addon policy, integrity, versioning and per-platform
  runtime contracts.
- Define the stable Rust plugin API and versioned hook contract.
- If JavaScript plugins remain supported, run them in the embedded runtime by
  default with explicit capabilities and deterministic filesystem/network/process
  policy.
- Keep external Node hosting explicit, unsupported for core profiles and visible
  in doctor output.

**Exit gate:** a curated compatibility suite covers common plugin shapes
(aliases, virtual modules, transforms, assets, HTML injection, SSR transforms)
without external Node.

### 7. Single-binary packaging and release

**Goal:** make the all-in-one distribution real.

- Embed framework compiler payloads, templates, HMR client, SSR island client,
  runtime metadata and standalone SDK inputs in the release binary.
- Publish reproducible Windows/Linux/macOS binaries, including musl where
  feasible.
- Complete SDK provenance, signatures and artifact attestation.
- Validate cross-target linker/toolchain prerequisites with executable fixtures.
- Test installation from a clean directory, no project checkout, no compiler
  host and an empty `PATH`.

**Exit gate:** every advertised release artifact passes the same clean-machine
smoke matrix on its target OS/architecture.

## Recommended execution order

1. **Integrate current progress** into `main` and rerun the production/SSR
   acceptance gates on one commit.
2. **Remove fixture network flakes** so acceptance results are trustworthy.
3. **Finish React Refresh**, because React already has a native compiler path
   and the remaining work is incremental.
4. **Create the compiler abstraction boundary** required for Vue/Svelte native
   replacement while preserving current diagnostics and cache identity.
5. **Implement Vue SFC compilation first**, because Vue is already designated
   the first complete framework profile in the completion plan.
6. **Implement Svelte compilation second**, focusing initially on the pinned
   Svelte 5.39.6 generated-application matrix.
7. **Close production correctness gaps** in parallel after compiler output is
   stable enough to avoid repeated fixture rewrites.
8. **Expand SSR/islands/streaming and cross-platform acceptance** only after
   per-framework client behavior is reliable.
9. **Measure and optimize performance** after correctness contracts stop
   changing.
10. **Publish signed single-binary releases** once the clean-machine matrix is
    green.

## First concrete implementation batch

The first coding PR after this plan should be small and verifiable:

- integrate or rebase the current tip onto `main`;
- add a no-network package fixture mode to the SSR acceptance profile;
- add a `rust-only` acceptance profile that runs generated React/Vue/Svelte
  smoke checks with `PATH` empty and fails if any external Node executable is
  invoked;
- expose compiler-host usage in `ferrite inspect` so the profile can prove
  whether `native`, embedded or external compilation was used;
- document the exact transitional/final compiler choice for Vue and Svelte
  before writing large compiler implementations.

## Non-goals until the core matrix is green

- Universal Vite/Rollup plugin compatibility.
- Arbitrary framework version ranges.
- React Server Components or a full framework router.
- Full TypeScript type-checking inside the default dev pipeline.
- WebKit or managed browser distribution.
- Replacing the bundler backend before correctness and benchmark parity exist.
