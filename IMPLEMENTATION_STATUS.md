# Framework support implementation status

Status: incomplete. No new framework/version/host/rendering profile has passed
clean-directory browser acceptance. Unit tests alone do not establish support.

## Baseline and preservation

Started at d9eb5f3; compared with c4dc667a910e31d91a82f185103a898d20bcb889.
The historical changes concentrate on E2E tooling; existing E2E fixes remain.
No repository-level AGENTS.md was found. The vendor instructions apply only
inside vendor/tailwind-rs, which this change does not modify.
Other workspace edits appeared during execution and are not part of this change.

Baseline command: `cargo test -p ferrite-frameworks -p ferrite-cli --locked`:
10 CLI and 13 framework unit tests passed. The baseline included a test asserting
that the Vue stub returned markup, demonstrating that green unit tests did not
establish actual framework support.

## Completed in this change

- Unknown create templates fail before filesystem writes rather than silently
  generating vanilla. The legacy ssr template fails explicitly because its
  renderer/hydration profile is unvalidated; its shell scaffold is removed.
- Vue/Svelte load hooks reject compilation with official compiler package and
  validated host requirements. Markup-returning compiler stubs are removed.
  Existing parser utilities remain for API compatibility, not compiler support.
- Vue/Svelte ownership checks inspect path suffixes, ignoring query/hash values;
  Svelte module extensions are recognized and rejected explicitly as unavailable.
- Registry schema v1 describes current compiler requirements and rendering
  evidence: React experimental, Vue/Svelte unavailable, no tested package versions.
  This is the initial registry slice, not the full descriptor or CLI integration.
- Explicit dev SSR propagates runtime/adapter errors and never substitutes a shell.
- Final import analysis refreshes module syntax and HMR usage after plugins.

Migration: Vue/Svelte stubs and the legacy ssr scaffold now fail closed. Public
`template_stub`/`markup_stub` functions were removed because their output was not
valid framework compilation. No lockfile migration is implemented.

## Validation

`cargo test -p ferrite-frameworks -p ferrite-cli -p ferrite-server --locked`:
11 CLI, 14 framework, 29 server unit tests passed; doc tests passed (zero cases).
Formatting was applied to changed files. Workspace-wide formatting reveals
pre-existing differences; no workspace formatting compliance is claimed.

## Remaining / not validated

The mission remains assigned and incomplete. Required work includes full registry
use across create/install/dev/build/SSR/inspect/doctor; explicit ownership and
configuration; transform ordering and complete metadata/map propagation; lockfile
identity/edge migration and npm interoperability; validated persistent compiler
hosts and official project-matched Vue/Svelte compilers; complete React Refresh;
transactional scaffolding/install/editor/type checking; real SSR/hydration/SSG;
additional adapters and upstream delegation; adapter SDK; browser acceptance,
feature matrices, lint/workspace tests, release checks, and scheduled upstream
compatibility testing. Compiler versions are not pinned because no official
compiler execution was implemented or tested in this slice.

No Chromium/Firefox acceptance, Node-free profile, SSR request isolation,
stream cancellation, production CSS/chunk verification, or cross-platform release
check has executed. These are outstanding requirements, not skipped support proof.

## Shared pipeline slice (continued implementation)

Implemented pre/core/post ordering in both the server pipeline and programmatic
`Ferrite::transform_request`. `Enforce::Pre` defaults to BeforeLowering;
Normal/Post default to AfterLowering, and adapters can explicitly select their
phase. Pre transforms can override the resulting module type. The legacy
container API still runs all transforms in order for callers that need it.

LoadResult now retains maps and side-effect declarations. Plugin map chains are
composed rather than replaced. Import rewriting, HMR injection, and the React
registration footer produce generated-to-input maps with UTF-16 positions;
inserted code stays unmapped. Composition respects unmapped boundaries instead
of attributing generated text to a previous source token. Plugins changing code
without maps produce a diagnostic and remove invalid prior mappings.

Compiler dependencies and request-local add_watch_file registrations survive
through pipeline/cache/graph. Cache hits validate dependency content hashes,
including deletion; compiler identity hooks, loaded metadata, React configuration,
production mode and map settings participate in cache identity. Resolved and
loader-provided side-effect information survive. Cached syntax flags are retained.
Final analysis runs after plugin transforms. Declared component module types
prevent imports from being replaced with asset URL shims.

API migration for native adapters: LoadResult literals need `map` and
`side_effects` (or `..Default::default()`); plugin TransformResult literals need
`module_type: None` unless changing type. `dependencies` are watched filesystem
paths, absolute or project-root-relative, never browser URLs. Plugin cache_key
must include compiler/version/options that affect output.

Validation executed:

- `cargo test -p ferrite-server -p ferrite-plugin -p ferrite-transform -p ferrite-frameworks -p ferrite -p ferrite-cli --locked`: 118 unit tests passed,
  3 facade doc tests passed; 1 existing real-Node plugin test remained ignored.
  This ignored test establishes no Node compatibility.
- `cargo test -p ferrite-server -p ferrite pipeline_tests --locked`: 3 new
  pipeline/parity regressions passed after the final watch/map changes.
- `cargo test -p ferrite-server --features swc pipeline_tests --locked`: 2 tests
  passed, exercising both Oxc/SWC and development/production profiles.
- `cargo test -p ferrite-test --test plugins_virtual --test vite_parity --locked`:
  8 integration tests passed (plugin order, virtual modules, build lifecycle,
  configuration, and preview HTTP hooks/proxy).
- `cargo clippy -p ferrite-server -p ferrite-plugin -p ferrite-transform -p ferrite-frameworks -p ferrite -p ferrite-cli --all-targets --locked -- -D warnings`:
  passed.
- Changed files formatted with rustfmt; `git diff --check` passed.

This does not complete pipeline support: complete compiler resource/diagnostic
contracts, CJS wrapper maps/interoperability, production asset/chunk map handling,
CSS extraction parity, configuration/peer ownership, and concurrent input-change
handling still require implementation and acceptance tests. No official Vue or
Svelte compiler has executed yet. No new framework support claim is made.

The concurrent documentation reorganization was committed separately as 4ae1d13
and preserved. Framework safety changes were committed as ccbc64d.
