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
| Vue 3.5.22 JS/TS SSR | Experimental, Node compiler + napi-vm runtime | Streaming, SSG, framework standalone/browser/platform coverage |
| Svelte 5.39.6 client | Experimental, explicit Node compiler | Wider runes/component/HMR coverage |
| Svelte SSR | Unavailable | Complete hydration and generated-application profile; official emitted renderer now passes |
| React SSR | Unavailable | Real renderer, hydration and production/standalone profile |
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
- [ ] Measure parser preflight overhead and add versioned syntax-result caching if justified.
- [ ] Hard heap/native/process resource isolation with a separate explicit process profile.

Cancellation is cooperative in the engine. Uninterruptible native calls are not
forcibly killed; shutdown detaches after its wait bound. These controls are not a
sandbox or a hard memory limit.

## 3. Package graph and installation

- [x] Root/transitive npm registry aliases retain actual package identity and importing edge names.
- [x] Validate alias targets in lockfile edges; preserve editor projection and frozen replay.
- [x] Normalize legacy registry SHA-1 shasums into integrity metadata retained in the lock.
- [x] Verify multiple SRI hashes using the strongest supported algorithm and alternatives.
- [x] Reject missing integrity metadata for downloaded tarballs instead of silently trusting bytes.
- [x] Use rustls without reqwest's implicit OpenSSL/native-TLS build dependency.
- [ ] Transitive peer-context propagation and contextual ancestor instances.
- [ ] Workspace discovery, local links, importer identity and project/editor resolution.
- [ ] Root optional dependencies and a portable all-platform frozen graph.
- [ ] Complete exports/imports validation, external imports targets and CJS helper interoperability.

Existing locks with SHA-256/384/512 remain valid. Old entries lacking integrity
need a non-frozen install to obtain registry integrity/shasum before downloading.
Aliases use existing v2 specifier and edge fields; no schema bump is required.

## 4. Vue as the first complete framework profile

- [x] JS/TS generation, official compilation, emitted SSR graph and HTTP rendering foundations.
- [x] Existing browser fixtures for hydration, watched edits, syntax/style recovery and `/app/`.
- [ ] Slots, async components, directives, scoped/module styles, imported types and map diagnostics matrix.
- [ ] Granular HMR behavior and explicit state/reset contracts.
- [ ] Production dynamic style/preload tracking and no development code leakage.
- [ ] Generated standalone framework binary with pre-JS HTML and real browser hydration.
- [ ] Run clean-directory acceptance on every advertised platform before promotion.

## 5. React and Svelte application completion

- [ ] Finish React Refresh boundary/hook/component matrix, including unsupported patterns.
- [ ] Implement and test actual React renderer/hydration in dev, preview and standalone.
- [x] Verify Svelte SSR with official output, retaining meaningful HTML assertions.
- [ ] Add Svelte SSR JS/TS scaffolds only after renderer/hydration prerequisites pass.
- [ ] Expand Svelte runes, component resources, diagnostics and state/reset coverage.

## 6. Plugin and production contracts

- [x] Existing explicit foreign hook subset, host identity/dependency tracking and bounded Node transport.
- [ ] Verify official side-effect metadata against actual emitted production elimination.
- [ ] Extend foreign context and lifecycle contracts one tested capability at a time.
- [ ] Indexed map flattening/composition and complete emitted chunk/runtime map chains.
- [ ] Dynamic imports, CSS order, shared chunks, scope-hoist bailouts and asset closure fixtures.
- [ ] Benchmark before selecting a Rolldown backend; retain the current backend until parity passes.

## 7. Rendering features

- [ ] Explicit routing policy for dotted routes and multiple server entries.
- [ ] Request disconnect cancellation propagated from HTTP through SSR and guest invocation.
- [ ] Streaming backpressure, abort/error behavior and request-resource teardown.
- [ ] SSG route enumeration, output paths, errors and deterministic builds.
- [ ] Actual framework islands/hydration acceptance and lazy component preload closure.
- [ ] Runtime external/native profile contracts and artifact/source-map versioning.

## 8. Packaging and distribution

- [x] Existing executed custom-renderer SSR and static standalone binary acceptance on Linux.
- [ ] SDK source provenance/digest validation beyond matching package name/version/feature.
- [ ] Concurrent and atomic final binary publication with failure/rollback tests.
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
