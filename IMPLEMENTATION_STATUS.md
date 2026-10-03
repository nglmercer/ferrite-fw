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
valid framework compilation. At this initial slice, no lockfile migration was implemented; the later package
graph slice below implements migration.

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

At the initial checkpoint, none of the required browser/runtime acceptance had
executed. The CommonJS slice below subsequently exercises both browsers and a
Node-free fixture. Framework acceptance, SSR isolation/cancellation, production
CSS/chunk verification, and cross-platform release checks remain outstanding.

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


## Concrete package graph slice (continued implementation)

Implemented ferrite.lock schema v2 with concrete package identities and dependency
edges, root importer requests/edges, exact tarball URLs/integrities, deterministic
serialization and atomic lock writes. Multiple versions coexist; scoped package
paths and cycle edges resolve through the graph. Direct peer contexts use distinct
store identities; peer ranges are validated. npm range matching uses pinned
node-semver 2.2.0 (OR, comparator sets, hyphens, exact/zero-major/prerelease cases).
Root/transitive tags retain locked selections during ordinary installs; explicit
updates refresh metadata and select the requested newer version.

CLI add/remove/update/install and the single-package library API share manifest
installation. Failed resolution leaves the supplied graph unchanged. Removing a
root preserves reachable transitive packages. `ferrite install --frozen-lockfile`
checks manifest requests and concrete edges, replays locked tarballs without
metadata, and does not rewrite the lock. Cached archives are integrity checked;
extraction stages package contents before publishing and checks manifest identity.
Lifecycle scripts are not executed. Tar extraction supports the non-`package/`
root used by DefinitelyTyped; unsafe paths and link/device entries fail explicitly.

The resolver follows the importing package's exact dependency edge, including
self imports, or the project importer edge. A lock prevents undeclared imports;
without a lock, multiple matching store versions fail instead of selecting the
highest. Custom configured lock paths propagate into CLI/server/library resolution.
Lock content participates in transform cache identity and watch dependencies.
Inspect reports graph identities/importers and propagates malformed-lock errors.

Migration: read v1 edges into concrete name@version identities in memory. A
successful ordinary manifest install adds importer requests/edges and tarball URLs
and writes v2. Frozen installs require the complete v2 graph. Irrecoverable v1
collisions/dangling edges fail with instructions to back up/rename the old lock and
regenerate. Public LockedPackage adds fields (use `..Default::default()` where
appropriate); dependency values now mean concrete IDs. `Lockfile.find` selects a
root or a unique identity, never an arbitrary version. ResolverConfig adds the
lockfile path. Existing stores are retained, not deleted during migration.

Validation executed in this slice:

- `cargo test -p ferrite-npm -p ferrite-resolver -p ferrite-server -p ferrite -p ferrite-cli --locked`:
  77 unit tests and 3 facade doc tests passed; a subsequent npm rerun passed
  14 tests including the single-package API and unsupported-manifest guards.
- `cargo test -p ferrite-test --test config_resolver --test dev_pipeline --test build --test standalone --locked`:
  13 integration tests passed. Two pre-existing real standalone/cross-build tests
  remained ignored; no release/platform support follows from these skipped tests.
- `cargo clippy -p ferrite-npm -p ferrite-resolver -p ferrite-cli -p ferrite-server -p ferrite --all-targets --locked -- -D warnings`:
  passed.
- Actual CLI registry install in a temporary clean project with `vue=3.5.22` and
  `svelte=5.39.6`: 39 concrete packages installed with PATH=/nonexistent. After
  deleting the temporary store and metadata, frozen reinstall succeeded from
  tarball cache with the identical lock bytes and no metadata directory. This is
  installation evidence only: neither framework compiler was executed.

Remaining package requirements: full transitive peer context propagation and
contextual ancestors, workspace links, top-level optional dependencies, npm aliases,
all-platform frozen graphs, exports/imports condition parity, runtime deduplication,
CommonJS named/default interop and unsupported dynamic-require/native-addon errors,
SHA1-only/multiple-SRI compatibility and stricter absence-of-integrity handling.
Unsupported contextual ancestor collisions fail instead of overwriting another
instance; root workspaces/optionalDependencies fail instead of silently omitting
requests. Peer contexts and frozen replay are tested on the current host only.
Editor/compiler-host package projections are not implemented. No new framework
compilation, browser, SSR, or release compatibility profile is advertised.


## CommonJS and conditional-resolution slice (continued implementation)

Replaced regex collection/replacement and the default-only CommonJS wrapper with
Oxc AST/scope analysis. Comments, strings, escaped literals, whitespace and locally
shadowed require/exports/module bindings are handled as syntax. Static property
assignments, object literals, Object.defineProperty, Object.assign and direct
module.exports=require(...) chains expose named snapshots alongside the complete
module.exports default value. The __esModule marker does not discard the module's
other properties or unwrap its nested default. This follows the namespace model
in [Node's ESM documentation](https://nodejs.org/download/release/v22.11.0/docs/api/esm.html),
without claiming complete Node runtime compatibility.

Lazy factory resources (`?ferrite-cjs-factory`) close circular require edges and
preserve conditional execution. Facades and require calls share one factory/cache
per module; module.exports replacements and null values are preserved. Factories
cache before executing a body and clear failed executions for a later retry.
JSON require factories retain object identity. Generated helper text is unmapped;
original body locations survive wrapping, import rewriting and HMR injection.
Hashbangs are removed from the wrapped body with source-map edits.

Unresolved require, dynamic/aliased require, require.resolve/module.require,
native .node addons, mixed ESM/CommonJS and synchronous require(ESM) fail explicitly
with source/module context. No Node process is started or substituted. Namespace
property discovery remains a static subset; arbitrary dynamic exports and all
transpiler re-export helper patterns are not yet complete.

The resolver selects mutually exclusive import/require conditions and uses main
for legacy require entries. JSON object order is retained. Conditional keys follow
manifest order, nested null blocks fallback, arrays retain null/no-match semantics,
and wildcard selection prioritizes prefix length before total pattern length.
Root conditional exports do not accidentally expose private subpaths. These
regressions are grounded in the [Node 22.11 package resolution source](https://github.com/nodejs/node/blob/v22.11.0/lib/internal/modules/esm/resolve.js).
Invalid targets, external package imports maps and remaining full exports/imports
validation still need implementation.

Generated query resources now depend on their physical source in the graph/cache,
so a plain-file watcher event invalidates all owned resources. Re-export facade
name discovery watches contributing files; cached CommonJS facts survive replay.
The cache schema key is bumped to pipeline-v5. Compiler maps/watched dependencies
are preserved through the SSR text-transform pre/core/post phases too; caller-
supplied CJS text stays inline rather than referring to an invented source file.
Library CommonJS transformation reuses the server interop service, and SSR plugin
contexts/resolution use SSR conditions. This is transformation evidence, not an
SSR renderer/runtime/hydration claim.

API migration: ResolveHookRequest adds `kind: ResolveKind`; ordinary imports use
Import, CJS calls Require. PipelineModule/CachedTransform add `commonjs` static
facts; use code_only constructors or `commonjs: None` in literals. Factory query
resources are implementation IDs, not new user-facing rendering modes.

Validation executed:

- `cargo test -p ferrite-transform -p ferrite-resolver -p ferrite-plugin -p ferrite-server -p ferrite-frameworks -p ferrite -p ferrite-cli --locked`:
  134 unit tests and 3 facade doc tests passed. One pre-existing real-Node plugin
  test remained ignored and establishes no Node-host support. Final resolver
  rerun passed 14 tests after adding nested-null/pattern regressions.
- With FERRITE_CHROMIUM_PATH set to the installed executable,
  `cargo test -p ferrite-test --test commonjs --test config_resolver --test dev_pipeline --test build --test plugins_virtual --test vite_parity --locked`:
  23 integration tests passed, including both actual browsers. Browser tests fail
  when the executable is missing; they do not silently skip. One earlier run
  omitted the Chromium path and failed that prerequisite, then was rerun correctly.
- The CommonJS Rust test binary was executed with PATH=/nonexistent and explicit
  Chromium/Firefox executable paths: all 3 tests passed. Each browser asserted
  default/named/namespace/re-export values, circular require, shared identity,
  conditional nonexecution, JSON loading, interaction, actual watcher-driven
  source edit/reload and intentional state reset, production build/preview,
  absence of dev client code in emitted JS and absence of console/page errors.
  Tested host: Linux x86_64; Chrome for Testing 153.0.8010.12, Firefox 157.0;
  compiler Oxc 0.151.0. No framework template was used in this fixture.
- `cargo test -p ferrite-server --features swc pipeline_tests --locked`: 2 tests
  passed, including pre/post SSR transformation with maps on Oxc/SWC.
- Final library/server `pipeline_tests` rerun: 3 tests passed.
- `cargo clippy -p ferrite-transform -p ferrite-resolver -p ferrite-plugin -p ferrite-server -p ferrite -p ferrite-test -p ferrite-cli --all-targets --locked -- -D warnings`:
  passed. Changed Rust files formatted; `git diff --check` passed.

Framework compiler hosts, scaffold variants, complete React Refresh, Vue/Svelte
compilation, checker/editor projections, SSR renderers/hydration, remaining npm
contexts/platform/workspace requirements, additional/upstream adapters, SDK and
framework/release acceptance remain assigned. No entire-task completion or new
framework compatibility profile is claimed.

## Official compiler-host foundation

Implemented a typed NodeCompilerHost using the existing persistent NodeAdapterHost
transport, with no automatic Node spawn on an embedded-host failure. Constructing
this explicit library host projects the selected v2 lock graph and starts Node;
it is separate from any SSR runtime. Official project-matched `vue/compiler-sfc`
3.5.22 and `svelte/compiler` 5.39.6 execute in the worker. Package manifests and
actual compiler versions must match the locked selection. Other versions fail as
unvalidated, and a changed lock graph requires explicitly recreating the host.
Registry schema 2 records these compiler-only experimental profiles separately
from unavailable Vue/Svelte application and SSR rendering profiles. Neither
framework plugin nor CLI automatically enables this host yet.

Vue uses official parsing, compileScript, compileTemplate and asynchronous
compileStyle: script setup/TypeScript, binding metadata, nested templates,
scoped/module CSS, maps, style/type dependencies and template tips are retained.
Svelte uses whole-component compile and compileModule for runes-bearing modules;
.svelte.ts module input is lowered with Oxc and its source map chained. Typed
results retain code, CSS/modules, maps, dependencies, diagnostics and compiler
version. Compiler assembly inserts unmapped barriers for generated helper code.
Custom blocks, external Vue blocks and unsupported preprocessors fail explicitly;
preprocessor integrations, full macro/type-import coverage and asset integration
remain unfinished. Svelte HMR is explicitly disabled in this wrapper; no update
or hydration capability is inferred from compiler execution.

The Node/editor projection materializes importer-specific dependency edges as
owned node_modules symlinks inside Ferrite's package store, including concurrent
versions and scoped packages. It runs no alternate resolver or lifecycle scripts.
It validates every existing destination before writing, refuses unmanaged,
modified or unsafe links, and atomically replaces each owned view with rollback
on publish failure. Package contents remain intact. This is not a graph-wide
atomic projection transaction, and editor/installer CLI integration remains
unfinished. Node/editor and Windows release compatibility are not claimed.

Transport changes: typed asynchronous JSON exports, Tokio blocking-worker
isolation, private temporary worker scripts, 16 MiB request/reply limits, 256 MiB
Node old-space limit, separate ordinary log/protocol channels, request deadlines,
explicit cancellation, pending-call failure and child kill/reap on shutdown.
Timeout/cancellation never silently restarts or substitutes a backend. Node is a
normal subprocess, not a sandbox; complete RSS/descendant-process resource limits
and foreign-plugin factory/object-hook/config/server lifecycle support remain
unfinished. These changes do not establish full Vite compatibility.

Validation executed on Linux x86_64 with Node v26.10.0:

- `cargo test -p ferrite-npm -p ferrite-plugin -p ferrite-frameworks --offline`:
  46 unit tests passed; registry/Node integration tests were separately executed.
- `cargo test -p ferrite-plugin node_adapter::tests::real_node --locked -- --ignored --nocapture`:
  both real-Node tests passed, including async exports, process reuse, guest
  stdout/console logs, missing-export errors, timeout termination and cancellation.
- `cargo test -p ferrite-frameworks --test compiler_host --locked -- --ignored --nocapture`:
  passed from a clean directory, installing actual pinned compilers through the
  Rust Ferrite installer. Client/server output compiled and parsed, Vue development/production scoped, CSS-variable and
  module CSS validated, Svelte component/module runes and original .svelte.ts maps
  validated, syntax/preprocessor/custom-block failures and explicit cancellation
  checked. A prior run against the existing installed fixture also passed.
- `cargo clippy -p ferrite-npm -p ferrite-plugin -p ferrite-frameworks --all-targets --locked -- -D warnings`:
  passed; changed Rust files formatted and `git diff --check` passed.

These are compiler-only tests, not clean generated-app browser acceptance.
Official compiler integration into dev/build resources, watcher ownership, HMR,
scaffolds, checkers and real SSR renderers/hydration remains assigned. Native and
embedded compiler hosts, additional framework adapters and release/upstream
acceptance remain unfinished. The complete mission remains active.

## Official hosted components in the shared pipeline (2026-10-02)

Implemented explicit library opt-in through `VuePlugin::with_host` and
`SveltePlugin::with_host`, sharing the persistent NodeCompilerHost. Production
requires a production plugin instance; no implicit Node startup or SSR runtime
substitution is introduced. Official compiled components, runes modules and
owner-indexed CSS resources pass through dev and library pipelines with maps,
watched dependencies, diagnostics and side effects. Raw source requests retain
original component text. Removed styles fail explicitly instead of returning
stale resources. BuildLoader extracts generated CSS into production assets.
Compiler-default Vue flags and NODE_ENV are overridden by explicit user defines.

Fixed three production regressions uncovered by real framework acceptance:
canonical npm identities now collapse relative-path cycles; scope-aware AST
constant replacement preserves strings, comments and local bindings; named
re-export pruning and analysis after minification keep ESM links and chunk
metadata consistent. Source-map composition now uses binary lookup on dense
lines. Explicit compiler injection occurs before watcher startup.

Validation executed:

- `cargo test -p ferrite-resolver --locked`: all 15 tests passed, including
  cyclic relative imports under both preserve-symlink settings.
- `cargo test -p ferrite-transform -p ferrite-bundler -p ferrite --lib --locked`:
  45 transform, 18 bundler and 11 library tests passed.
- `cargo test -p ferrite-transform --features swc defines::tests --locked`:
  three shared define regressions passed; this is not a full SWC framework matrix.
- `FERRITE_CHROMIUM_PATH=/home/meme/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome cargo test -p ferrite-test --test framework_compilers --locked -- --ignored --nocapture`:
  all three tests passed (51.40 seconds), using actual pinned Vue 3.5.22 and
  Svelte 5.39.6 installed through Ferrite in clean fixture directories.
  Chromium 153 and Firefox 157 verified rendering, clicks, source edits,
  watcher invalidation, updated scoped CSS, production CSS extraction,
  builds with tree shaking/scope hoisting enabled, and preview interaction
  without page/console errors. The third test verified maps, cache refresh,
  generated-style ownership, removed-style errors, raw source and exact
  library/dev output parity. Server compilation was checked, not SSR rendering.
- `cargo clippy -p ferrite -p ferrite-bundler -p ferrite-test --all-targets --locked -- -D warnings`:
  passed after the final analysis changes.

These are opt-in library integration fixtures, not advertised `create` profiles.
Source updates currently use full reloads with intentional state resets; no
framework-specific HMR/state-preservation claim is made. CLI host configuration,
generated templates, checkers, real SSR/hydration, preprocessors, complete asset
coverage, React Refresh, additional adapters and all remaining mission work
remain assigned. Rendering support in the public registry is not promoted by
these narrower tests. No lockfile migration beyond the earlier v2 change is
required by this integration. The complete mission remains active.
