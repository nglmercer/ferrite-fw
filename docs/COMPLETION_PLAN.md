# Ferrite completion plan

This is the current consolidated backlog. `IMPLEMENTATION_STATUS.md` is an
append-only history: its older “remaining” paragraphs describe earlier commits.
A checked item means the named behavior has implementation and regression
coverage. It does not promote a whole framework or platform profile.

## Current application profiles

| Profile | Status | Principal remaining acceptance |
|---|---|---|
| Vanilla JS/TS client | Experimental | Full clean-directory platform/release matrix |
| React 19.2.0 JS/TS client | Experimental | Complete Refresh/component/hook matrix |
| Vue 3.5.22 JS/TS client | Experimental, explicit Node compiler | Granular HMR and wider component coverage |
| Vue 3.5.22 JS/TS SSR | Experimental, Node compiler + napi-vm runtime | Broader streaming/version/platform acceptance |
| Svelte 5.39.6 client | Experimental, explicit Node compiler | Wider runes/component/HMR coverage |
| Svelte 5.39.6 JS/TS SSR | Experimental, Node compiler + napi-vm runtime | Wider component/state and cross-platform release matrix |
| React 19.2.0 JS/TS SSR | Experimental, native compiler + napi-vm | Wider HMR/component and cross-platform release matrix |
| Preact/Solid/Qwik | Adapter unavailable | Explicit ownership/delegation and independent adapter acceptance |

The registry's `tested_versions` remains empty until a complete declared
application profile passes. Compiler-only evidence belongs in compiler profiles.

## 1. Accurate progress and reproducible gates

- [x] Replace stale Vue/Svelte placeholder documentation in English, Spanish and Chinese.
- [x] Separate compiler, client, renderer and standalone claims.
- [x] Add explicit local acceptance runner with per-command logs, revision/platform,
  dirty-tree status, failures and elapsed time.
- [x] Exact ignored-test gates reject successful Cargo runs that executed zero tests.
- [ ] Extend capability evidence to a complete version/host/OS/compiler/runtime matrix.
- [ ] Run every required gate on one release commit, including both browsers.

## 2. Runtime compatibility and lifecycle

- [x] Pin the current separate napi-vm core and explicit timer host, avoiding the Node binding facade.
- [x] Regress the Svelte `for (name in attrs)` syntax and named-function self-reference.
- [x] Carry one request deadline across runtime queueing and guest execution.
- [x] Cancel active guest execution when the awaiting future is dropped.
- [x] Expose explicit shared-runtime shutdown and reject new requests on every clone.
- [x] Bound last-owner thread shutdown waiting; cancel active work before closing the queue.
- [x] Regress deadline/cancellation recovery on the same worker and expired queue rejection.
- [x] Expose deadline and shutdown settings through config, merge, SSR adapters and diagnostics.
- [x] Execute the official Svelte server renderer through its emitted graph with meaningful HTML assertions.
- [ ] Complete browser hydration and generated-application acceptance before enabling an SSR profile.
- [x] Measure parser preflight overhead and add versioned syntax-result caching if justified.
- [x] Explicit Linux process profile with address-space/CPU ceilings, deadline, cancellation and reaping tests.
- [ ] Extend process isolation to Windows/macOS and validate native-addon fixtures.

The embedded profile uses cooperative engine cancellation. Uninterruptible native calls are not
forcibly killed; shutdown detaches after its wait bound. These controls are not a
sandbox or a hard memory limit.

## 3. Package graph and installation

- [x] Root/transitive npm registry aliases retain actual package identity and importing edge names.
- [x] Validate alias targets in lockfile edges; preserve editor projection and frozen replay.
- [x] Normalize legacy registry SHA-1 shasums into integrity metadata retained in the lock.
- [x] Verify multiple SRI hashes using the strongest supported algorithm and alternatives.
- [x] Reject missing integrity metadata for downloaded tarballs instead of silently trusting bytes.
- [x] Use rustls without reqwest's implicit OpenSSL/native-TLS build dependency.
- [x] Transitive peer-context propagation and contextual ancestor instances.
- [x] Workspace discovery, local links, importer identity and project/editor resolution.
- [x] Root optional dependencies and a portable all-platform frozen graph.
- [x] Checked exports/imports target validation, dependency-backed imports, nearest scopes and exact map files.
- [ ] Package-map URL query/fragment targets and remaining CJS helper interoperability.

Existing locks with SHA-256/384/512 remain valid. Old entries lacking integrity
need a non-frozen install to obtain registry integrity/shasum before downloading.
Portable graphs now write v3 locks. v1/v2 remain readable but require one
non-frozen install before portable frozen replay. Optional OS/CPU/libc branches
remain locked and inactive subtrees are omitted from download/projection.
Metadata/download/integrity failures remain fail-closed for optional packages.
Contextual registry peers propagate through transitive ancestors and cycles.
Workspaces and in-project file links keep live source identities and relative
editor views across relocation. Each local directory must have one peer context;
conflicting views fail explicitly rather than overwriting one source directory.
Use project-root installation for workspaces and file links.
Standalone SSR scaffolds now retain the selected SDK lock, when available, and
write `SDK-PROVENANCE.json` with manifest/source-lock hashes. The generated root
may update its lock for its own package; these hashes identify inputs, not a signed
SDK source snapshot or final binary.

## 4. Vue as the first complete framework profile

- [x] JS/TS generation, official compilation, emitted SSR graph and HTTP rendering foundations.
- [x] Existing browser fixtures for hydration, watched edits, syntax/style recovery and `/app/`.
- [ ] Slots, async components, directives, scoped/module styles, imported types and map diagnostics matrix.
- [ ] Granular HMR behavior and explicit state/reset contracts.
- [ ] Production dynamic style/preload tracking and no development code leakage.
- [x] Generated standalone framework binaries with pre-JS HTML and real Chromium/Firefox hydration on Linux.
- [ ] Run clean-directory acceptance on every advertised platform before promotion.

## 5. React and Svelte application completion

- [ ] Finish React Refresh boundary/hook/component matrix, including unsupported patterns.
- [x] Implement React JS/TS synchronous production rendering and Chromium/Firefox hydration.
- [x] React/Svelte JS/TS development SSR edit, syntax-error and recovery checks.
- [x] Generated Vue/React/Svelte JS/TS standalone hydration/interaction/CSS in Chromium and Firefox on Linux.
- [x] React/Svelte JS/TS development hydration, component/CSS HMR and syntax recovery in Chromium and Firefox on Linux.
- [ ] Wider component/hook/state matrices and cross-platform development acceptance.
- [x] Verify Svelte SSR with official output, retaining meaningful HTML assertions.
- [x] Add Svelte SSR JS/TS scaffolds only after renderer/hydration prerequisites pass.
- [ ] Expand Svelte runes, component resources, diagnostics and state/reset coverage.

## 6. Plugin and production contracts

- [x] Existing explicit foreign hook subset, host identity/dependency tracking and bounded Node transport.
- [x] Verify side-effect metadata against actual emitted production elimination and execution.
- [ ] Extend foreign context and lifecycle contracts one tested capability at a time.
- [x] Flatten and compose embedded indexed source maps with nested offsets and unmapped boundaries.
- [ ] Complete all emitted chunk/runtime map chains and external section loading contracts.
- [x] Executed lazy CSS timing/retry, ordered static styles, shared ESM state, direct CSS imports and image closure with minify/scope hoisting on/off in both browsers.
- [x] Compose final native import/CSS/preload/wrapper map edits; invalidate stale maps from uncomposable code hooks.
- [ ] Broader cyclic/worker/Wasm and plugin-driven output closures, native scope-hoist bailouts and full render-hook map composition.
- [ ] Benchmark before selecting a Rolldown backend; retain the current backend until parity passes.

## 7. Rendering features

- [x] Explicit routing policy for dotted routes shared by dev, preview and standalone.
- [x] Base-relative route entries with lazy renderer selection shared by dev, built graphs, prerender and standalone.
- [ ] Independent per-entry shells/runtime policies and per-route CSS isolation.
- [x] HTTP disconnect drops pending SSR and cancels process-runtime invocation; recovery verified.
- [x] Rust response streaming backpressure, HEAD, producer errors and disconnect teardown.
- [x] Experimental Linux process-worker pull streaming bridge, deadlines, bounded bytes and teardown; official React/Vue production output.
- [ ] Full Web Streams compatibility, broader framework versions and cross-platform streaming acceptance.
- [x] SSG route enumeration, output paths, errors and deterministic builds.
- [ ] Actual framework islands/hydration acceptance and lazy component preload closure.
- [ ] Runtime external/native profile contracts and artifact/source-map versioning.

## 8. Packaging and distribution

- [x] Existing executed custom-renderer SSR and static standalone binary acceptance on Linux.
- [x] Seed standalone Cargo resolution from the explicit SDK lock and record manifest/source-lock hashes.
- [ ] Full SDK source provenance/signatures and final artifact attestation.
- [x] Concurrent and atomic final binary publication with failure/rollback tests.
- [ ] Windows/Linux/macOS release binaries, base paths, methods, assets and private-output checks.
- [ ] Cross-target toolchain/linker prerequisites; explicitly execute the musl fixture.
- [ ] Run promised Node-free profiles with empty PATH, no project checkout and no compiler host.

## 9. Performance and E2E scope

- [ ] Record cold/warm startup, transform/HMR latency, build time, SSR concurrency and RSS.
- [ ] Set budgets from measured baselines and attach reproducible fixtures/hardware metadata.
- [ ] Preserve the completed 51-task E2E scope and its engine-specific exclusions.
- [ ] Track wider Playwright members separately; completion of the 51 tasks is not full parity.
- [ ] WebKit and managed browser distribution remain separate expansion milestones.

## Running acceptance

```bash
# Uses installed Cargo. Runtime has no Node/browser prerequisite.
python3 scripts/acceptance.py --profile runtime

# View commands without claiming execution or passing status.
python3 scripts/acceptance.py --profile frameworks --list

# Explicit Node + installed Chromium/Firefox prerequisites.
FERRITE_CHROMIUM_PATH=/path/to/chrome FERRITE_FIREFOX_PATH=/path/to/firefox \
  python3 scripts/acceptance.py --profile frameworks

# Slow generated binary build and HTTP execution.
python3 scripts/acceptance.py --profile standalone
```

Profiles: `core`, `runtime`, `frameworks`, `standalone`, `e2e`, `all`. Reports and
logs default to `.ferrite/acceptance/`; select `--report /path/report.json` to
retain another run. `--timeout` bounds each command. Browser and package/compiler
prerequisites are explicit, never installed or substituted by the runner.
The standalone profile requires the Cargo toolchains/dependencies used by the
existing offline generated-binary fixture. Cross-platform execution is not inferred
from a successful host build.

## Runtime configuration

```toml
[runtime]
backend = "napi-vm"
request_timeout_ms = 5000
shutdown_timeout_ms = 1000
queue_capacity = 64
max_request_bytes = 8388608
```

A zero request timeout disables the wall-clock deadline; zero shutdown timeout
selects a 1000 ms last-owner wait. Queue and payload defaults are unchanged.
The runtime's deadline starts at runtime admission, not at the initial HTTP arrival.
`NapiVmRuntime::shutdown()` cancels the active request and permanently stops the
shared runtime; ordinary future cancellation leaves subsequent SSR requests usable.

## Static generation

`ferrite prerender --route / --route /products/item` builds the client and real
SSR renderer, renders each explicit route, and publishes the completed static
output only after every render succeeds. Alternatively set `ssr.prerender` to an
array of routes in ferrite.toml. Routes are relative to `base`; query strings,
encoded paths, traversal and output collisions are rejected. Renderers must
return status 200 without cookies. The deterministic `prerender-manifest.json`
records route paths and HTML hashes. Private server modules are removed.

Preview detects the manifest and serves nested index pages without an SSR
runtime. Missing paths return 404. Failed builds/renders retain previous output;
concurrent publication to the same destination fails explicitly. Directory
replacement uses staged publication with rollback; final executable replacement
uses a same-directory atomic file rename.

Embedded indexed source maps now flatten recursively before foreign-plugin map
composition. Section offsets, source contents, names and ignore lists survive.
External URL sections require the producer to fetch and embed them explicitly.

Validated on Linux in this PR: official Vue and Svelte hydration in Chromium
153 and Firefox 157; generated Svelte JS/TS production builds and preview hydrate
in both engines, retain the SSR button, respond to clicks and apply scoped CSS.
These are host-specific checks, not a complete platform release matrix.

## Explicit SSR routes and parser preflight

`[ssr] routes = ["/", "/users/*", "/feed.json"]` selects base-relative
application paths, including dotted paths. `*` matches all application paths;
an empty array disables rendering. Omitting routes retains extensionless
routing. Reserved asset, source and private-output prefixes bypass rendering.
The same matcher is used in dev, preview and generated SSR standalone servers.

Syntax preflight caches exact source text and parse errors under the pinned
engine revision. Its LRU is bounded to 256 entries and 8 MiB, survives guest
resets, and retains no parsed AST or guest state. On this Linux debug build,
20 checks of a 91,780-byte / 2,000-function module measured 858 ms uncached
versus 7.24 ms cached. Reproduce with `cargo test -p ferrite-runtime --features
napi-vm parser_preflight_cold_and_cached_measurement -- --ignored --nocapture`.
This measures preflight only, not total SSR throughput.

React JS/TS SSR now uses the public `react-dom/server.edge` synchronous renderer
and `hydrateRoot`. Its generated-source production test passes in both browsers,
including pre-JS HTML, DOM retention, clicks, styles and dotted routes. The runtime
explicitly installs the pinned pure-computation web host (text codecs and clone),
without installing network capabilities. The worker reserves a 16 MiB native
stack for large official renderer graphs; guest call-depth budgets remain active.
This establishes synchronous production acceptance, not streaming or full React
SSR/development/standalone parity.

Acceptance reports additionally retain tool executable paths/version results,
OS and architecture. A failed version probe remains explicitly failed; it does
not become browser/framework execution evidence. Release promotion still requires
running the full advertised matrix on each supported host.

## Production side-effect elimination

Production builds now remove static evaluation-only imports to JavaScript
modules explicitly marked effect-free. After unused exports are pruned, the
bundler repeats reachability and import pruning to a fixed point; dead barrels,
cycles and their CSS dependencies no longer force output chunks. Emitted imports,
chunk manifests and removal source maps are updated together. Namespace
re-exports retain their full dependency namespace, and a static import is kept
distinct from a dynamic import of the same URL.

Package metadata applies to local/linked files as well as locked npm packages.
`sideEffects` arrays support basename globs, path globs, globstars, alternatives
and character classes. Invalid patterns leave effects unknown. Required effects,
unknown metadata, live bindings, dynamic imports, entries, opaque importer facts
and directly imported CSS are retained conservatively. Disabling tree shaking
retains the original graph. This is not a claim of full Webpack/Rollup parity or
complete unused URL-asset elimination.

The side_effects integration target checks local and npm graphs with scope
hoisting and tree shaking independently enabled/disabled. It executes emitted
SSR graphs through napi-vm and verifies effect counts, lazy evaluation and
namespace values, rather than inferring correctness from chunk names alone.
Run `cargo test -p ferrite-test --features napi-vm --test side_effects`.

## Checked package maps

Resolver package maps now distinguish invalid targets/configuration from absent
or null mappings. Selected exports must be package-relative; imports may also
name dependency packages. Arrays skip invalid target alternatives but do not
retry another file after a valid target is missing. Conditional declaration order,
null blocking and wildcard specificity are retained. Numeric conditions and mixed
exports key styles fail explicitly. Decoded traversal, node_modules segments,
encoded separators and malformed percent escapes are rejected before filesystem
normalization; relative targets support encoded UTF-8 filenames. Malformed
package manifests fail instead of silently falling back to index files.

`#imports` stop at the nearest package scope and retain that package's locked
dependency/peer context. Relative imports inside the npm store retain canonical
npm identities. Package maps resolve exact files without extension/index probing.
Aliases and mapped dependencies have a bounded redirect depth to diagnose cycles.
URL-scheme targets and target URL queries/fragments remain explicitly unsupported;
this is not complete Node resolution or CJS interoperability parity.

The Node reference fixture compares supported exports cases using an explicitly
installed Node executable. The emitted integration fixture verifies distinct
client/server conditions and a mapped transitive dependency whose version differs
from the root, with scope hoisting enabled and disabled.


Route-specific entries are documented in [SSR_ROUTES.md](SSR_ROUTES.md).
[Vite replacement acceptance](VITE_COMPATIBILITY.md) tracks compatibility without
promoting partial fixtures to full framework or plugin parity.
