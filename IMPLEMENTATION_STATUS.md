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

## Declarative compiler-host startup (2026-10-02)

CLI dev/build and library `Config::resolve` now consume the same explicit
framework configuration. For the pinned installed compiler profiles:

```toml
[framework]
enabled = ["vue", "svelte"]
compiler_host = "node"
timeout_ms = 10000
# node = "/absolute/path/to/node" # optional; relative paths resolve from root

[runtime]
backend = "boa" # separate SSR runtime selection, not a renderer support claim
```

An omitted compiler host, unavailable host, unsupported framework, duplicate
owner, invalid deadline or unknown framework setting errors before host startup.
An explicit empty `enabled` list overrides inherited configuration and disables
this integration. Programmatic official plugins and declarative ownership for
the same framework conflict explicitly. Selected legacy disabled adapters are
replaced with official adapters; unrelated plugins remain. Build mode selects
production compilation. Configuration parse errors are propagated rather than
silently replaced with defaults. CLI transform uses this same configuration;
inspect reports framework settings and SSR runtime separately without starting
compiler workers, and dev prints the distinction with experimental status.

Validation:

- `cargo test -p ferrite-config --locked`: 22 tests and one documentation test
  passed, including explicit host validation, inherited settings, disabling,
  unknown fields and separate SSR runtime configuration.
- `cargo test -p ferrite-test --test framework_compilers official_components_resources_maps_cache_and_library_parity --locked -- --ignored --nocapture`:
  passed (19.59 seconds), including clean Ferrite installation, configured
  Vue/Svelte compilation, replacement of disabled adapters, unchanged runtime
  selection and explicit failure on malformed configuration.
- `cargo test -p ferrite-cli --locked`: all 11 existing CLI tests passed.
- `cargo clippy -p ferrite-cli -p ferrite-test --all-targets --locked -- -D warnings`:
  passed after the final CLI changes. Changed files formatted; diff check passed.
- Actual `ferrite transform CliCounter.vue` against the pinned installed fixture
  produced official render code, stripped TypeScript and generated CSS imports.
  Actual `ferrite inspect --json` reported compiler host `node`, SSR runtime
  `boa` and the selected official adapters; assertions passed.

No new advertised template or SSR rendering capability is established here.
Doctor, full version/host diagnostics, arbitrary explicit config-file selection,
scaffolds, framework HMR/checkers and the remaining mission remain unfinished.

## Shared, transactional client generation (2026-10-02)

Implemented `create --framework --language --rendering`, standalone
`create --list-templates`, `--no-install` and `--dry-run`, preserving the vanilla
legacy alias and global mode parsing. Unknown owners, languages and rendering
combinations fail before writing. Current generation profiles are
`vanilla/js/client` and `vanilla/ts/client`; Vue/Svelte scaffold acceptance and
real SSR profiles remain assigned rather than silently substituting vanilla.

Registry schema is now 3: framework descriptors include template variants,
and CLI selection/listing and the public library scaffold API read those
variants. Custom descriptor constructors must provide `template_variants`.
Lockfile schema remains 2. No dependency versions were upgraded; existing
rustix 1.1.5 is now a direct target dependency for no-overwrite publication,
and CLI tests use the existing tempfile dependency.

Generation creates real interactive source, HTML, CSS, package manifest,
configuration, editor/environment types, public icon, ignore rules and README.
It stages all output privately, installs through the existing Ferrite installer
and writes a resolved lock before publishing. Dry runs write nothing; no-install
retains the manifest and omits the lock. Linux publication uses an anchored
parent directory and atomic no-replace rename, with no unsafe overwrite
fallback. Traversal, symlink ancestors, dangling destinations, raced destination
creation and changed parent directories are rejected. Abandoned staging is
removed. Atomic publication also has a macOS implementation but is unexecuted;
other platform publication/release validation remains unavailable/unfinished.

Validation executed on Linux:

- `cargo test -p ferrite-frameworks -p ferrite-cli --locked`: 18 framework and
  12 CLI unit tests passed; the separately ignored actual compiler-host test
  was not executed by this command. New tests cover no-write dry runs, invalid
  profiles, preserved existing files, real installed locks, no-install manifests,
  raced publication, symlink/traversal rejection, anchored writes after parent
  replacement and staging cleanup.
- Fresh CLI build followed by
  `FERRITE_CLI_PATH=/home/meme/Documentos/challenges/ferrite-fw/target/debug/ferrite FERRITE_CHROMIUM_PATH=/home/meme/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome cargo test -p ferrite-test --test generated_apps --locked -- --ignored --nocapture`:
  both tests passed (7.20 seconds), covering both languages in Chromium and
  Firefox. Every Ferrite subprocess had an empty PATH: actual CLI create,
  frozen install with unchanged lock bytes, dev interaction, source-edit
  invalidation with an intentional reset, build with scope hoisting, and
  preview interaction all passed without page or console errors.
- Final targeted create tests and
  `cargo clippy -p ferrite-cli -p ferrite-test --all-targets --locked -- -D warnings`:
  passed; changed files formatted and diff check passed.

The registry retains experimental client status: these tests do not establish
all requested syntax-recovery, map, asset/base-path or platform matrices. Generated
standalone interaction-test packaging, actual TypeScript checking, full framework
templates, Doctor, SSR/hydration and all remaining mission work are unfinished.
The complete mission remains active.

## Real Vue/Svelte client scaffolds and compiler map regression (2026-10-02)

Implemented JavaScript and TypeScript client generation for pinned Vue 3.5.22
and Svelte 5.39.6. Components use official Vue refs/script setup and Svelte runes;
entries use `createApp` and Svelte 5 `mount`, respectively. Manifest dependencies,
component types, editor settings, public assets, scoped styles and configuration
are generated through the shared scaffold registry and existing transactional
installer. Node compilation requires explicit CLI opt-in:

```sh
ferrite create my-vue-app --framework vue --language ts --rendering client --compiler-host node
ferrite create my-svelte-app --framework svelte --language ts --rendering client --compiler-host node
```

Selecting these profiles without the host flag fails before writing. Generation
sets only the compiler host; it does not select Node as an SSR runtime. No-install
works without invoking a compiler. Profiles retain full-reload update behavior
with intentional state reset; SSR variants remain unavailable.

Registry schema is now 4. Template profiles report framework version and support
status in addition to host/language/rendering. Custom profile constructors must
supply `framework_version` and `support`. Listing reports exact pinned versions,
experimental client status and unavailable SSR. Vue/Svelte client descriptors now
report experimental support; no complete tested-profile or SSR claim is made.
Lockfile format is unchanged.

The standalone Svelte scaffold exposed a real upstream source-map defect:
Svelte 5.39.6 can emit negative original columns in multi-root component maps.
The focused compiler wrapper now uses the compiler's project-resolved
`@jridgewell/sourcemap-codec` dependency to retain all valid positions and represent
invalid original positions as explicit unmapped generated positions, with a
structured warning. It does not clamp to fabricated source coordinates or discard
whole maps. This follows ECMA-426's optional-error/null-original-position decoding:
https://tc39.es/ecma426/#sec-mappings-grammar . The upstream issue is
https://github.com/sveltejs/svelte/issues/16615 . Original location information for
these invalid positions remains unavailable and is reported explicitly.

Validation:

- `cargo test -p ferrite-frameworks -p ferrite-cli --locked`: 19 framework and
  13 CLI unit tests passed. Added tests verify version pins, real component
  mounting entries, language-specific source, explicit host selection,
  no-install behavior and absence of SSR runtime substitution. The ignored
  actual compiler-host integration is not established by this command.
- `FERRITE_COMPILER_FIXTURE=/tmp/ferrite-real-install-45z_otrh cargo test -p ferrite-frameworks --test compiler_host --locked -- --ignored --nocapture`:
  passed (0.77 seconds). The new map regression independently invokes the same
  official compiler and codec, then compares every emitted generated/original
  position and invalid-position barrier, including valid positions after an
  invalid one. Existing Vue/Svelte client/server compiler, runes, style and
  diagnostic checks also passed against the installed pinned fixture.
- Fresh CLI build followed by
  `FERRITE_CLI_PATH=/home/meme/Documentos/challenges/ferrite-fw/target/debug/ferrite FERRITE_CHROMIUM_PATH=/home/meme/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome cargo test -p ferrite-test --test generated_apps --locked -- --ignored --nocapture`:
  all four tests passed (74.46 seconds), covering all six generated profiles in
  both Chromium and Firefox. Actual CLI create, Ferrite installation, frozen
  replay with unchanged lock bytes, dev clicks, source edits/invalidation,
  updated scoped CSS, production build with scope hoisting, extracted CSS,
  preview clicks and absence of dev-only code/page/console errors were checked.
  Vanilla processes used an empty PATH; Node compiler profiles were exercised
  separately with Node enabled. No assertion was weakened for compiler failures.
- `cargo clippy -p ferrite-cli -p ferrite-test --all-targets --locked -- -D warnings`:
  passed after final registry/status changes. Actual `create --list-templates`
  printed all six profiles with versions, hosts and capability status.

Remaining: framework-specific HMR/state preservation, generated standalone test
packaging, complete checker/editor package resolution, Doctor, full source-map
and syntax-recovery matrices, real SSR/hydration/streaming, React Refresh,
remaining dependency/host capabilities, additional adapters and release/upstream
acceptance. The complete mission remains active.

## Portable package/editor projection during installation (2026-10-02)

Implemented relative links for importer-specific package views and a reusable
`project_editor_dependencies` service. Project-root `node_modules` points to
`.ferrite/npm/node_modules`, exposing the same locked graph to ordinary package
and editor resolution. Root and nested package edges retain their concrete
versions/peer identities; no second resolver, lockfile or lifecycle scripts run.
Generation projects the graph while staging, so views remain valid after atomic
publication. Install/add/remove/update maintain the same view. Empty manifests
also receive an owned empty projection, without requiring Node.

Owned absolute links migrate to relative links even when dependency edges are
unchanged. Existing unmanaged editor directories or mismatched links are rejected
before package projection; installation preflights the root editor destination.
No unrelated node_modules is deleted or replaced. Existing npm-managed views must
be moved aside explicitly before enabling this projection. Upgrade legacy owned
views before relocating a project; already-relocated absolute links are refused
as modified rather than guessed or overwritten. Generated ignore rules now cover
node_modules symlinks as well as directories. Lockfile/registry schemas are
unchanged. Per-view updates retain the earlier atomic replacement/rollback
behavior; graph-wide atomic projection and cross-platform validation remain
unfinished.

Validation:

- `cargo test -p ferrite-npm -p ferrite-cli --locked`: 20 package and 13 CLI
  tests passed. New projection tests cover relative scoped/concurrent versions,
  legacy-link migration with unchanged edges, relocation of the entire project,
  preserved importer-specific versions and unmanaged editor-view refusal before
  any package projection is created. Create tests verify installed and no-install
  output, including empty native manifests.
- Fresh CLI build followed by
  `FERRITE_CLI_PATH=/home/meme/Documentos/challenges/ferrite-fw/target/debug/ferrite FERRITE_CHROMIUM_PATH=/home/meme/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome cargo test -p ferrite-test --test generated_apps --locked -- --ignored --nocapture`:
  all four tests passed (75.94 seconds), covering all six generated profiles in
  both browsers. New assertions verify root editor links immediately after
  creation and resolve Vue/Svelte to the exact concrete root importer edge before
  dev starts. Frozen replay, dev/edit/CSS/build/preview checks remain intact.
  Native profiles still run with Node absent from PATH.
- `cargo clippy -p ferrite-npm -p ferrite-cli -p ferrite-test --all-targets --locked -- -D warnings`:
  passed; changed files formatted and diff check passed.

This establishes the filesystem package view, not complete editor-language-server
or type-checker conformance. Checker execution, Doctor, framework HMR, syntax/map
matrices, generated test packaging, real SSR/hydration, React Refresh, remaining
package/host capabilities, other adapters and release/upstream acceptance remain
assigned. The complete mission remains active.

### Read-only capability doctor

Implemented shared `ferrite_frameworks::doctor::inspect` and `ferrite doctor
[--root PATH] [--json]`. Reports the native compiler/version, explicit compiler
host, separately selected SSR runtime, concrete root importer framework versions,
experimental compilation profiles, unavailable SSR/checkers, and root editor view.
Explicit framework configuration overrides manifest detection, including explicit
disabling. No worker is started, executable is invoked, package view is projected,
or project file is written. Located Node is explicitly `located-not-executed`;
this does not establish compiler execution support. Missing dependencies, stored
identity mismatch, disabled/missing hosts, and unmatched pinned compiler profiles
produce actionable errors and CLI exit failure. React client conformance remains
unavailable. Inactive/unavailable update profiles are not advertised as usable.
Editor verification covers the root view only, not the full nested graph.

Validation:

- `cargo test -p ferrite-frameworks doctor::tests --locked`: three tests passed;
  configuration precedence, dangling editor views, concurrent Vue versions with
  concrete root selection, stored identity mismatch, and read-only host detection.
- `cargo test -p ferrite-cli --locked`: thirteen tests passed.
- `cargo clippy -p ferrite-cli -p ferrite-frameworks --all-targets --locked -- -D warnings`:
  passed; formatting and diff checks passed.
- Fresh CLI build and real installed Vue 3.5.22/Svelte 5.39.6 project: JSON reports
  exact root versions and separate Node compiler/Boa runtime. Repeated with empty
  PATH: structured missing-Node diagnostics and nonzero exit verified. These are
  diagnostic tests, not additional framework browser or SSR acceptance evidence.

Doctor executable ABI probes, full graph/editor/checker validation, inspect report
unification, React Refresh, framework HMR, SSR/hydration/SSG, remaining package and
host capabilities, adapters and release/upstream matrices remain assigned. No
capability is promoted to tested by this diagnostic-only change.

### Explicit configuration selection in compiler-driven commands

Fixed `--config custom.toml` previously selecting only its parent directory and
silently loading a different default file. Added shared
`load_user_config_path` for exact TOML and statically parsed JS/TS files, and
`Config.config_path` for library parity. Explicit files replace automatic
configuration discovery (including implicit local overlays); explicit directories
retain existing discovery/overlay behavior. Missing, malformed and unsupported
selected files fail without fallback. Programmatic configuration and CLI
per-field overrides retain their precedence. Without an explicit library root,
the selected file's parent/directory becomes the project root hint.

Dev, SSR dev, build, preview, transform, E2E, inspect and doctor pass the selected
path. Package-management command dispatch still does not consume global config;
that remains assigned rather than claiming all-command parity. JS configuration
continues to use the existing static parser, not arbitrary execution or complete
Vite configuration compatibility.

Validation:

- `cargo test -p ferrite-config -p ferrite-cli --locked`: 23 config tests and
  initially 13 CLI tests passed, plus the config documentation test. Final CLI
  rerun after the library-parity regression: 14 passed.
- `cargo clippy -p ferrite-cli --all-targets --locked -- -D warnings`: passed
  after final changes; changed files formatted and diff check passed.
- Fresh CLI build and a temporary real project with malformed default TOML:
  explicit selected config controlled inspect/doctor, transform substituted its
  define, scope-hoisted production build used its output directory, and malformed
  selected TOML produced failure naming that file. No additional browser or SSR
  conformance is established by these checks.

Migration: callers constructing every public `Config` field explicitly must add
`config_path: None`; callers using `..Default::default()` remain compatible.
Selecting a file now uses that exact file, so callers relying on unintended
neighbor-file discovery must select the directory instead. Full framework work,
SSR, checkers, host/package matrices and remaining acceptance gates stay active.

### Package-command configuration and compiler lock parity

Install/add/remove/update now consume global `--config` and `--mode`, using the
same exact-file/directory semantics and root selection as compiler commands.
Registry, metadata cache, package store, manifest and selected lockfile stay in
one project context. Project creation retains its existing shared installer
context. Configuration is validated before editor-view or install mutations;
package commands do not start framework compiler workers. Frozen missing-lock
errors now name the actual selected path.

Fixed the official compiler host independently hardcoding `ferrite.lock`: it now
receives `ResolvedConfig::lockfile()`, matching installation, doctor and resolver.
Generated Vue/Svelte TS acceptance fixtures now rename the resolved lock to
`selected.lock` and configure it, with no default lock left to hide fallback.

Validation:

- `cargo test -p ferrite-cli --locked`: 15 tests passed. New regression checks
  selected registry/cache/store/manifest/lock paths, unrelated-root preservation,
  custom-lock frozen replay, malformed selection without lock mutation, and
  installation despite an unavailable configured compiler executable.
- Actual CLI temporary project: custom-config install/frozen replay/update/remove
  succeeded despite malformed default config; malformed selected config failed
  for all four package commands before resolving requested packages.
- Fresh CLI build, then
  `FERRITE_CLI_PATH=/home/meme/Documentos/challenges/ferrite-fw/target/debug/ferrite FERRITE_CHROMIUM_PATH=/home/meme/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome cargo test -p ferrite-test --test generated_apps --locked -- --ignored --nocapture`:
  four tests passed in 74.12 seconds. All six client profiles passed real Chromium
  and Firefox create/install/dev/interaction/edit/CSS/build/preview checks;
  Vue/Svelte TS passed against only the configured custom lock. Native profiles
  ran with Node absent from PATH. Output: `/tmp/ferrite-selected-lock-acceptance.log`.
- `cargo clippy -p ferrite-test -p ferrite-cli --all-targets --locked -- -D warnings`:
  passed; formatting and diff checks passed.

No lockfile schema migration. Existing commands without explicit configuration
retain directory discovery. Full Refresh/framework HMR, checkers, real SSR/SSG,
remaining host/package capabilities, adapters and acceptance matrices remain
unfinished; the complete mission remains active.

### Reject unavailable compiler backends at construction

Removed the public library facade's silent substitution of Oxc when an engine
was unknown. `Ferrite::new` now returns `Result<Ferrite>` and propagates compiler
selection errors. Updated existing facade conformance callers to handle that
result. The shared compiler factory rejects SWC immediately when its cargo
feature is absent, rather than returning an object whose transform/minify would
later fail. Doctor consequently reports unavailable compilation and no compiler
version for this build profile; dev server construction fails before serving.
Direct `SwcCompiler` parsing remains available with explicit transform/minify
errors, preserving the parser API without claiming a compiled backend.

Validation:

- `cargo test -p ferrite-transform -p ferrite-server -p ferrite --locked`:
  44 transform, 32 server and initially 11 facade tests passed, plus three facade
  documentation tests. Final facade rerun including new constructor regressions:
  13 tests and three documentation tests passed.
- `cargo test -p ferrite-transform --features swc --locked`: 52 tests passed,
  including actual SWC lowering/minification tests and compiler-version checks.
- `cargo test -p ferrite-server --features swc --locked cache_key_tracks_backend_and_defines`:
  the targeted backend/define cache test passed. Default-feature counterpart
  explicitly asserts construction rejection, while still testing define keys.
- `cargo clippy -p ferrite -p ferrite-cli -p ferrite-test --all-targets --locked -- -D warnings`:
  passed; formatting and diff checks passed.
- Fresh default CLI build: unknown engine and SWC without feature both produced
  nonzero doctor JSON with unavailable compiler support/no compiler version;
  transform rejected both with the expected actionable backend errors.

API migration: library callers must propagate or handle `Ferrite::new(...)` with
`?`/error handling; unknown engine configurations no longer compile through Oxc.
This establishes backend selection failure and the exercised SWC compiler/cache
matrix, not full SWC framework/browser, SSR or release conformance. Full framework
mission requirements remain assigned and active.

### Compiler selection follows component ownership

Replaced compiler-host `Lockfile::find` selection, which could accept a unique
transitive framework package when the owner had no dependency edge. Compiler
requests now select the concrete framework edge from the component's locked
package owner or nearest importer. Scoped store identities and concurrent
framework versions retain their own edges. A framework package can select its
own identity. Existing paths are canonicalized, missing paths normalized, and
outside-project or untracked store components fail with ownership diagnostics.
Missing edges cannot fall back to a root or unique transitive dependency; selected
versions still pass the existing exact compiler-profile validation before calls.

Validation:

- `cargo test -p ferrite-frameworks --locked`: 23 unit tests passed. The ownership
  regression covers distinct root/scoped-package/importer versions, missing root
  and package edges despite available framework packages, unknown store ownership
  and normalized outside-project rejection. The ordinary command's ignored real
  compiler test is not counted as support evidence.
- `FERRITE_COMPILER_FIXTURE=/tmp/ferrite-real-install-45z_otrh cargo test -p ferrite-frameworks --test compiler_host --locked -- --ignored --nocapture`:
  actual installed Vue 3.5.22/Svelte 5.39.6 compiler client/server/module fixture
  passed (0.65 seconds), including existing maps, styles and runes assertions.
- `cargo clippy -p ferrite-frameworks --all-targets --locked -- -D warnings`:
  passed after correcting the test initializer; formatted files/diff check passed.

No schema migration or new advertised version. This establishes concrete compiler
ownership selection and preserves the exercised compiler fixture; complete
workspace installation, cross-package browser/version/peer matrices, linked
external-package integration, SSR, HMR/checkers and remaining mission work are
still unfinished. The full goal remains active.

### JSX compiler settings through shared transforms

Added `[react] import_source` for automatic runtime and `factory`/`fragment` for
classic runtime. Shared transform requests carry these options through facade,
dev/build pipeline and existing SSR entry lowering; Oxc 0.151.0 and SWC React
transform 55.0.1 APIs were checked against their installed source and exercised.
Unknown runtimes, incompatible combinations and empty settings produce explicit
errors. No alternative framework adapter is claimed by changing JSX settings.
The existing cache key includes the complete React configuration. Default
programmatic overlays now preserve file-level JSX settings and disabled Refresh
instead of unconditionally replacing the React configuration with defaults.

Example classic TOML: `[react]`, `runtime = 'classic'`, `factory = 'UI.h'`,
`fragment = 'UI.Fragment'`. Automatic: `runtime = 'automatic'`,
`import_source = './selected-runtime'`. The selected runtime must exist and provide
its actual JSX runtime; these settings do not install or implement a runtime.

Validation:

- `cargo test -p ferrite-transform -p ferrite-config -p ferrite --locked`:
  45 transform, 24 config and 13 facade tests passed, plus four documentation
  tests. Shared compiler regression checks automatic dev/production imports,
  classic factory/fragment output and invalid request rejection. Final config
  regression rerun passed after empty-setting validation was added.
- `cargo test -p ferrite-transform --features swc --locked`: 53 tests passed;
  the new regression exercises both real Oxc and SWC compilers. An initial SWC
  borrowed-string lifetime compile failure was fixed with owned pragma strings.
- `cargo clippy -p ferrite-cli -p ferrite-test --all-targets --locked -- -D warnings`
  and `cargo clippy -p ferrite-transform --features swc --all-targets --locked -- -D warnings`:
  passed; formatting/diff checks passed.
- Fresh actual CLI: classic file configuration emitted `UI.h`/`UI.Fragment`
  without default React factories; invalid runtime configuration failed loudly.

API migration: explicit TransformRequest literals add the three optional JSX
fields (None preserves defaults). ReactConfig literals add optional fields.
Existing default-value merge semantics still cannot distinguish an intentional
programmatic reset to automatic/Refresh-enabled from an unset default; complete
presence-aware configuration remains assigned. Per-file/package JSX ownership,
complete React Refresh, adapter runtime/browser conformance, framework HMR,
checkers and real SSR remain unfinished. The complete goal remains active.

### Explicit default React configuration overrides

React configuration now retains whether runtime and Refresh were explicitly set.
A local/programmatic overlay selecting automatic JSX or enabling Refresh overrides
inherited classic/disabled settings; an unset default overlay still inherits.
Changing runtime clears inherited factory/fragment or import-source settings for
the other runtime, while contradictory options explicitly supplied in the same
profile still fail validation. JSON serialization/deserialization preserves
explicit resets without making unset defaults explicit. Unknown React fields now
fail parsing rather than silently losing misspelled settings.

Rust callers can use `react.set_runtime("automatic")` and
`react.set_refresh(true)` to express resets to defaults; existing nondefault
field assignments continue to merge. `inspect --json` includes React configuration
so selected JSX settings are reviewable.

Validation:

- `cargo test -p ferrite-config -p ferrite-cli --locked`: 25 configuration and
  15 CLI tests passed, plus the config documentation test. Final config rerun
  passed after test initializer cleanup. Regression covers explicit default
  overrides, unset inheritance, both runtime switches, JSON round trips, Rust
  setters and unknown-field rejection.
- `cargo clippy -p ferrite-cli --all-targets --locked -- -D warnings`: passed
  after final changes. Formatting/diff checks passed.
- Fresh actual CLI build: main classic/Refresh-disabled config plus explicit
  automatic/Refresh-enabled local config reported the reset and cleared inherited
  classic options; an empty local React section preserved main settings.

Migration: external ReactConfig struct literals now need default construction
and public field assignments/setters because presence metadata is private.
Serialized unset React defaults may be omitted; deserialization retains their
existing automatic/Refresh-enabled effective values. This resolves the recorded
runtime/Refresh reset limitation, not complete presence handling for other config
sections or React Refresh conformance. SSR, JSX ownership, HMR, checkers and the
remaining framework mission are still unfinished and active.

### Keep React Refresh out of server and disabled profiles

React plugin transform now checks both SSR request flags and environment/context
kinds before adding development instrumentation. SSR HTML is never given the
Refresh preamble. Explicit Refresh virtual imports/resolution and loads outside
enabled client development fail with actionable errors, including production and
`react.refresh = false`; they no longer silently return browser-only code.

Validation:

- `cargo test -p ferrite-frameworks --locked`: 24 unit tests passed. New hook
  regression covers independent SSR flag/request-kind/context-kind signals,
  SSR virtual loading/HTML, production and disabled Refresh. Final
  `cargo test -p ferrite-frameworks react::tests --locked` passed all six React
  tests after adding positive enabled-client loading/transformation assertions.
  The ignored compiler fixture from the ordinary command is not execution evidence.
- `cargo clippy -p ferrite-frameworks --all-targets --locked -- -D warnings`:
  passed after final changes; formatting/diff checks passed.

Official runtime revalidation against React v19.1.1
(`https://raw.githubusercontent.com/facebook/react/v19.1.1/packages/react-refresh/src/ReactFreshRuntime.js`)
confirmed the current footer's `isLikelyComponentModule` call is not an exported
runtime API. Fixing boundary validation with actual pinned runtime execution,
hook signatures/registration, mixed exports and syntax recovery remains assigned;
these hook exclusion tests do not establish functioning Refresh or SSR rendering.
React client conformance remains unavailable in Doctor. JSX ownership, checker,
framework HMR, real SSR and the full mission remain active.

### React Refresh export boundary uses real runtime APIs

Removed the footer's nonexistent `isLikelyComponentModule` call. A development
module imports its current export namespace and compares it with the update using
`isLikelyComponentType`. Added/removed export keys, changed noncomponent exports,
component-to-noncomponent changes, empty boundaries and throwing getters invalidate
safely. Unchanged mixed constants can retain a component boundary. At least one
component must remain; null updates return without attempting refresh. Existing
server/production exclusion gates remain intact.

Added ignored-by-default, explicitly invoked actual-runtime conformance test using
Ferrite's installer and persistent Node plugin transport. It installs pinned
`react-refresh@0.17.0` in a clean directory, executes the generated preamble and
footer as real ESM (including self-namespace import), verifies component
registration and family advancement through the official runtime, then checks
six unsafe export shapes and null update handling. No runtime mock or placeholder
compiler is used; the test hot-context callback records invalidation decisions.

Validation:

- `cargo test -p ferrite-frameworks --locked`: 24 unit tests passed; separately
  ignored execution tests are not counted as support evidence.
- `cargo test -p ferrite-frameworks --test react_refresh_runtime --locked -- --ignored --nocapture`:
  actual runtime test passed, then passed again with the stronger registered
  family advancement assertion (1.67 seconds).
- `cargo clippy -p ferrite-frameworks --all-targets --locked -- -D warnings`:
  passed after final changes; formatting/diff checks passed.

This proves the exercised runtime API and export-boundary decisions, not complete
React Refresh. Hook signatures/custom hooks, anonymous defaults, hygienic
instrumentation, actual browser state preservation/recovery, self-import graph
behavior under browser HMR and broader version matrices remain assigned. React
client conformance is still unavailable in Doctor; no framework support profile
is promoted by this fixture. SSR, checkers, framework HMR and the full mission
remain active.

### Dispatch self-accept callbacks in the actual HMR client

Fixed the browser client's JS update branch importing modules without ever calling
registered accept callbacks. It snapshots the previous boundary callbacks and
disposers, runs disposers, imports the updated namespace and invokes the captured
self-boundary callbacks. Creating the next module context replaces registrations
while retaining hot data, preventing old callbacks/disposers accumulating across
edits. React's footer reuses injected `import.meta.hot` when present instead of
creating a second context that could clear earlier registrations.

Added explicitly invoked actual-client execution test through the persistent Node
transport. It loads the shipped `client.js`, uses a controlled WebSocket/DOM
transport harness and performs three real file-backed dynamic ESM edits. It
asserts exactly previous-to-next callbacks `[[0,1],[1,2],[2,3]]` and one disposer
per edit with retained data `[1,2,3]`. Browser transport controls are test fixtures;
the HMR client and ESM loading are the actual implementations, not stubs.

Validation:

- `cargo test -p ferrite-frameworks --test hmr_client --locked -- --ignored --nocapture`:
  actual client execution test passed (0.07 seconds).
- `cargo test -p ferrite-hmr -p ferrite-frameworks --locked`: two HMR and
  24 framework unit tests passed. Ignored execution cases from this ordinary
  command do not establish support.
- `cargo test -p ferrite-frameworks --test react_refresh_runtime --locked -- --ignored --nocapture`:
  pinned official runtime boundary/registration/family fixture still passed
  (1.64 seconds).
- `cargo clippy -p ferrite-hmr -p ferrite-frameworks --all-targets --locked -- -D warnings`:
  passed; formatting/diff checks passed.

This verifies self-boundary callback lifecycle, not dependency-accept arrays,
relative dependency normalization, concurrent update ordering, prune/custom-event
cleanup, syntax-error recovery or browser framework state preservation. Those
remain assigned alongside complete Refresh, framework HMR, SSR/checkers and the
full mission. No new template/support profile is advertised.

### Serialize incoming HMR message execution

Incoming WebSocket messages now enter one promise queue, so asynchronous module
imports/disposal/accept callbacks finish before the next message runs. This
prevents a slower earlier module evaluation calling stale boundary callbacks
after a newer update. A rejected message handler is reported and the queue
recovers instead of blocking every later update. Reconnects share the queue.

Extended the actual-client ESM fixture: two messages arrive simultaneously, the
older module deliberately waits before evaluation, and strict callback/disposer
history must remain ordered. A deliberate custom-handler failure rejects that
message; the following sixth module edit still advances the current boundary.
The expected failure is logged by the actual client, not suppressed as success.

Validation:

- `cargo test -p ferrite-frameworks --test hmr_client --locked -- --ignored --nocapture`:
  actual client fixture passed (0.11 seconds), with ordered six-edit transitions,
  retained data and post-error continuation.
- `cargo test -p ferrite-hmr --locked`: two tests passed.
- `cargo clippy -p ferrite-hmr -p ferrite-frameworks --all-targets --locked -- -D warnings`
  and final framework Clippy rerun: passed. Formatting/diff checks passed.
- Fresh CLI build then
  `FERRITE_CLI_PATH=/home/meme/Documentos/challenges/ferrite-fw/target/debug/ferrite FERRITE_CHROMIUM_PATH=/home/meme/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome cargo test -p ferrite-test --test generated_apps --locked -- --ignored --nocapture`:
  four tests passed (70.68 seconds), exercising all six client profiles in
  Chromium/Firefox through create/frozen install/dev interaction/edit/CSS/build/
  preview. Custom compiler locks and Node-free native profiles remain covered.
  Output: `/tmp/ferrite-ordered-hmr-acceptance.log`.

This establishes ordered client message handling and preserves exercised generated
profiles; it does not establish complete React hook/state conformance, dependency
acceptance, syntax-error recovery or bounded/cancellable update queues. Remaining
framework HMR, SSR/checkers, adapters and the complete mission stay active.

### Prune cleanup and custom-event ownership

Pruning now awaits disposal/prune callbacks, removes owned custom-event handlers,
releases hot data and removes owned style resources. Cleanup failures are reported
without preventing remaining callbacks/modules from cleanup; repeated prune paths
are harmless. Custom events await asynchronous handlers through the message queue.

Custom-event registrations are tracked by module and context generation. Repeated
registration of the same callback in one owner is deduplicated; separate modules
may register the same callback independently. `off` removes only the caller's
registration. Re-evaluation clears old handlers, and stale/pruned contexts cannot
register or remove handlers in a newer generation.

Validation:

- `cargo test -p ferrite-frameworks --test hmr_client --locked -- --ignored --nocapture`:
  actual shipped client execution fixture passed (0.11 seconds). It now checks
  current-generation asynchronous custom events after edits, disposal/prune once
  with retained data, absent handlers/data after duplicate pruning, independent
  shared-callback owners, stale/pruned contexts and cleanup continuation after a
  deliberate failing prune hook. Expected handler/cleanup errors are logged;
  assertions require later callbacks/modules to continue and remain cleaned up.
- `cargo clippy -p ferrite-hmr -p ferrite-frameworks --all-targets --locked -- -D warnings`:
  passed after final changes; diff check passed.

This establishes the exercised client cleanup APIs in the actual-client transport
harness, not additional browser/component state-preservation conformance. Server
prune emission and generated-resource ownership matrices, dependency acceptance,
syntax recovery, complete Refresh, SSR/checkers and remaining adapters/acceptance
work are still assigned. No support profile is promoted; the full mission remains
active.

### HMR planning covers all importer branches

Added `ModuleGraph::hmr_accepting_boundaries` and routed HMR planning through it.
It traverses every reachable importer branch, collects/deduplicates all accepting
boundaries, and sorts output deterministically. An unaccepted root or unknown
module forces full reload; a cycle with no accepting boundary terminates with
reload rather than looping. Explicit accepted-dependency edges stop their branch.
The existing nearest-chain query remains available for inspection, but no longer
controls actual HMR plans. Each planned update retains the changed path/timestamp
and identifies its accepting boundary.

Validation:

- `cargo test -p ferrite-graph -p ferrite-hmr --locked`: seven graph and three
  HMR tests passed. New assertions verify both accepting branches are emitted in
  sorted order, adding an unaccepted root changes the actual plan to full reload,
  and boundary-free cycles terminate.
- `cargo test -p ferrite-server --locked`: 32 server tests passed.
  Output: `/tmp/ferrite-all-boundaries-server.log`.
- `cargo clippy -p ferrite-graph -p ferrite-hmr --all-targets --locked -- -D warnings`:
  passed after final changes; formatting/diff checks passed.

This establishes graph/planner coverage, not complete dependency HMR. Precise
accept-call metadata (instead of generic import.meta.hot usage), dependency
callback path/array semantics and browser module-cache re-execution of importer
chains still require implementation and acceptance tests. Complete Refresh,
framework HMR, SSR/checkers and remaining framework mission stay active; no new
support profile is advertised.

### Self-accept metadata requires an accept call

Graph self-acceptance is no longer inferred from generic `import.meta.hot` usage.
Added AST-based analysis of explicit no-argument/function-callback self-accept
calls in final JavaScript, including the pipeline's lowered factory form when its
literal module identity matches. Data reads, dispose calls, dependency accepts,
strings/comments, unrelated logical contexts and unused function bodies do not
establish a boundary. Opaque aliases/callbacks are not treated as proven acceptance.
React's footer exposes its hot-context receiver to this analysis while retaining
actual-runtime fallback behavior.

Validation:

- `cargo test -p ferrite-transform -p ferrite-server --locked`: 46 transform and
  33 server tests passed. New real-file pipeline regression distinguishes hot
  usage from graph self-acceptance after lowering; parser regression covers false
  positives and recognized explicit forms. Final parser rerun passed with
  same-module versus other-module factory cases.
  Output: `/tmp/ferrite-explicit-hmr-verified-final.log`.
- `cargo test -p ferrite-frameworks --test react_refresh_runtime --locked -- --ignored --nocapture`:
  actual pinned runtime fixture passed (1.70 seconds).
- `cargo clippy -p ferrite-transform -p ferrite-server -p ferrite-frameworks --all-targets --locked -- -D warnings`:
  passed after moving the public export before the test module; formatting/diff
  checks passed.

The pipeline regression exposed another required fix: hot lowering currently
creates a context for each hot access, which can reset registrations during one
module evaluation. Singleton per-evaluation lowering remains assigned. Explicit
dependency/alias acceptance metadata, unsupported-form diagnostics, browser
state/recovery, complete Refresh, framework HMR, SSR/checkers and remaining
mission work stay active. No support profile is promoted by this analysis change.

### One hot context per lowered module evaluation

Hot lowering now initializes `import.meta.hot` once at module evaluation and
preserves original accesses instead of replacing every access with a fresh
factory call. Data reads and later registrations cannot clear earlier accept,
dispose or event handlers. No generated local binding is needed, and source-map
edits leave original access locations intact after the generated prefix. Plain
strings/comments remain untouched; AST detection also handles spaced accesses.
The mapped pipeline still rejects invalid JavaScript. Pipeline cache identity is
bumped to v6 to prevent persistent old lowering output from surviving upgrades.

Validation:

- `cargo test -p ferrite-transform -p ferrite-server --locked`: 46 transform and
  33 server tests passed. Existing lowering assertions now require one factory
  call instead of the prior incorrect per-access calls.
  Output: `/tmp/ferrite-hot-singleton.log`.
- `cargo test -p ferrite-frameworks --test hmr_client --locked -- --ignored --nocapture`:
  both actual-client execution tests passed (0.09 seconds). New fixture runs real
  mapped lowering output as ESM against the shipped HMR client, with repeated data
  reads plus accept/dispose/event registration, and requires correct callbacks,
  retained data and event ownership over three module updates.
- `cargo clippy -p ferrite-transform -p ferrite-server -p ferrite-frameworks --all-targets --locked -- -D warnings`:
  passed. After the cache-version change, the server cache-key test and server
  Clippy rerun passed; formatting/diff checks passed.

No lock/config schema migration. Cached transforms rebuild under the new pipeline
identity. This resolves the recorded per-access context lifecycle bug in exercised
lowering/client execution; actual framework browser state/syntax recovery,
dependency acceptance, complete Refresh, SSR/checkers and remaining mission work
are still assigned. No profile is promoted and the full goal remains active.

### Public scaffold profiles cannot silently change hosts or versions

The public `scaffold::files` API now validates the entire requested profile against
the registry, including compiler host, framework version and support status. It
previously discarded those fields and generated the canonical profile silently.
Mismatches now return an actionable configuration error directing callers to
`scaffold::select`. Canonical profiles and equivalent cloned descriptors remain
accepted. No configuration or lockfile migration is required.

Validation: `cargo test -p ferrite-frameworks --lib --locked` passed all 25 tests,
including host/version/support substitution checks across all six generation
profiles and the existing publication/symlink protections. Framework all-targets
Clippy with `--locked -- -D warnings` passed; formatting and diff checks passed.
No additional browser acceptance or support promotion is claimed. Complete
Refresh, framework HMR, SSR/checkers and the remaining mission remain assigned.

### Dependency acceptance through the pipeline, planner and shipped client

Final JavaScript now supplies AST-analyzed literal/string-array hot acceptance.
Dependency literals resolve through the plugin-aware resolver, become executable
dev URLs even with import maps, and retain chained edit maps. The graph records
accepted concrete dependency IDs and removes stale acceptance on new transforms
and cache hits. Dynamic acceptance arguments fail with an actionable error;
external dependencies cannot establish a local boundary. Deferred/aliased hot
contexts and computed acceptance forms remain outside this exercised subset.

The planner carries both the callback owner and accepted module. The client
re-imports/disposes the dependency without re-running its accepting parent,
deduplicates shared imports/disposals per update batch, and invokes array
callbacks once with entries for changed dependencies. This subset's callback
shape was checked against the official [HMR API](https://vite.dev/guide/api-hmr);
this does not claim complete Vite compatibility or syntax-error recovery.

Real-browser regressions exposed timestamp IDs creating unaccepted graph roots
after the first edit. Dev transport-only numeric `t` parameters now leave graph,
cache and hot-context ownership unchanged. Other resource queries retain their
identity and order; production/SSR requests do not use this normalization.

Validation:

- `cargo test -p ferrite-transform -p ferrite-graph -p ferrite-hmr -p ferrite-server --locked`:
  47 transform, 7 graph, 3 HMR and 35 server tests passed.
  Output: `/tmp/ferrite-dependency-hmr.log`.
- `cargo test -p ferrite-frameworks --test hmr_client --locked -- --ignored --nocapture`:
  all 3 actual-client execution fixtures passed, including shared dependencies,
  batched array callbacks, one disposal/import and retained parent registrations.
  Output: `/tmp/ferrite-dependency-client.log`.
- Explicitly executed `cargo test -p ferrite-test --test generated_apps --locked -- --ignored --nocapture`
  with the freshly built CLI, Chromium 153 and Firefox 157: all 6 tests passed
  (72.70 seconds). This includes clean create/install/dev/edit/build/preview for
  all six existing profiles in both browsers and two new Node-free dependency
  acceptance tests. Both edits preserve parent counter state and document
  identity, deliver updated exports to both owners, dispose once, and produce
  no page/console errors. Output: `/tmp/ferrite-dependency-all-browsers.log`.
- `cargo test -p ferrite-server --features swc --locked`: all 35 tests passed.
- All-target Clippy for transform/graph/HMR/server/frameworks/test with
  `--locked -- -D warnings`, formatting and diff checks passed.

No lock/config schema migration. Pipeline cache identity advances to v7. Custom
dev-protocol consumers must interpret `path` as callback owner and `acceptedPath`
as the module to import; restart/reload dev clients when upgrading. Transitive
self boundaries still require dependency cache-busting work; syntax recovery,
complete Refresh, component-specific HMR, SSR/checkers and remaining mission
requirements remain assigned. No framework profile is promoted.

### Transitive HMR imports use fresh dependency URLs

Dev import rewriting now adds the dependency's graph invalidation timestamp to
its browser URL while preserving canonical, untimestamped dependency edges.
Invalidated bare imports bypass the document's immutable import map and use the
resolved timestamped URL directly. Unchanged bare imports retain normal import
map behavior. SSR and production do not receive these dev URLs.

Importer invalidation participates in dev transform cache identity, preventing
unchanged importer source from replaying cached pre-edit dependency URLs. The
planner uses the accepted module's recorded invalidation timestamp so its entry
URL matches imports back to that module, including cycles. Repeated invalidation
advances a module's timestamp even within one clock millisecond.

Validation:

- `cargo test -p ferrite-graph -p ferrite-hmr -p ferrite-server --locked`:
  7 graph, 4 HMR and 36 server tests passed. The new pipeline regression covers
  a two-level chain, rewrite/import-map strategies, cache replay, canonical graph
  edges and timestamp-free SSR output. Output: `/tmp/ferrite-transitive-hmr-server.log`.
- Explicitly executed `cargo test -p ferrite-test --test generated_apps --locked -- --ignored --nocapture`
  with the freshly built CLI, Chromium 153 and Firefox 157: all 8 tests passed
  (78.67 seconds). The two new Node-free tests exercise a cyclic two-level chain,
  bare import map resolution and a self-accepting boundary. Both source edits
  produce fresh exports; affected modules execute once per edit, callbacks and
  disposal run once, and the outer interaction state/document survive without
  page or console errors. All six existing generated profiles still complete
  create/install/dev/edit/build/preview in both browsers.
  Output: `/tmp/ferrite-transitive-all-browsers.log`.
- `cargo test -p ferrite-server --features swc --locked`: 36 tests passed.
- All-target Clippy for graph/HMR/server/test with `--locked -- -D warnings`,
  formatting and diff checks passed.

No lock/config schema change; cached transforms rebuild under pipeline v8.
The exercised transitive self-boundary cache-busting gap is resolved. This does
not establish every virtual-resource/cycle shape or syntax-error recovery.
Complete Refresh, component-specific HMR, SSR/checkers, further adapters and the
rest of the full mission remain assigned; no framework support status is promoted.

### Compile failed edits before browser updates and recover after correction

Watcher and programmatic invalidation now validate changed modules, invalidated
compiled owners and newly reached dependencies through the existing pipeline
before publishing updates or full reloads. Compilation failure sends an actionable
diagnostic with a source ID, keeping the running client undisposed. Failed partial
validation restores previous HMR acceptance metadata and removes newly invented
boundaries, while retaining dependency edges so a later correction is tracked.
The watcher now coalesces to the latest event after a quiet window rather than
dropping the final save within its debounce interval.

Unresolved local imports now fail explicitly. This exposed CSS's formerly
unresolved HMR runtime import: `/@ferrite/client` is now a real built-in JavaScript
resolution/load result, unavailable to SSR/production imports. Explicit module
type prevents accidentally wrapping the runtime as an asset. The existing
overlay implementation required no change.

Validation:

- `cargo test -p ferrite-server -p ferrite --locked`: 39 server tests, 13 library
  tests and 3 library doc tests passed. New regressions cover diagnostics without
  update/reload, restoration after partial graph validation, correcting a new
  dependency, real notify-driven rapid saves, CSS runtime resolution and loud
  missing local imports. Output: `/tmp/ferrite-syntax-recovery-server.log`.
- Explicitly executed `cargo test -p ferrite-test --test generated_apps --locked -- --ignored --nocapture`
  with the rebuilt CLI, Chromium 153 and Firefox 157: all 8 tests passed
  (81.68 seconds). Every generated JS/TS vanilla/Vue/Svelte profile now includes
  an invalid edit, a source-named error overlay, retained running counter state,
  and correction before build/preview. Vue/Svelte use their pinned official Node
  compilers; vanilla and manual HMR fixtures run with Node absent from CLI PATH.
  Dependency and cyclic self-boundary fixtures additionally require zero disposal
  on failed edits, preserved document identity/state, cleared overlays and correct
  subsequent updates. Existing page/console-error assertions remain intact.
  Output: `/tmp/ferrite-syntax-recovery-browsers.log`.
- `cargo test -p ferrite-server --features swc --locked`: 39 tests passed.
- All-target server/test Clippy with `--locked -- -D warnings`, formatting and
  diff checks passed. Earlier browser failures from strict CSS resolution were
  fixed in implementation; assertions were retained.

No lock/config schema migration. Pipeline cache identity advances to v9 so
previously tolerated unresolved imports cannot survive as cached successes.
Initial-load failures, runtime execution failures, overlapping validation races
and the wider recovery matrix remain unproven; this establishes the exercised
edit/correction paths only. Complete Refresh, component-specific HMR, SSR/checkers,
additional adapters and the remaining full mission stay assigned. No support
profile is promoted.

### Required watcher/plugin hook failures stop HMR

The watcher now reports `watch_change`, modern `hot_update` and legacy
`handle_hot_update` failures through the existing diagnostic protocol, including
plugin/hook identity and source ID. It stops that event instead of logging and
continuing or silently using ordinary HMR as a fallback. Shared diagnostic
reporting also retains the source-ID behavior of compilation failures.

Validation: `cargo test -p ferrite-server --locked` and the same command with
`--features swc` each passed all 40 tests. The new real-notify regression exercises
each hook, requires a diagnostic with no fallback update/reload, checks that later
hooks and compilation did not run, and verifies recovery on a subsequent successful
edit. Outputs: `/tmp/ferrite-hmr-hook-errors.log` and
`/tmp/ferrite-hmr-hook-swc.log`. Server all-target Clippy with
`--locked -- -D warnings`, formatting and diff checks passed.

No lock/config/cache migration. This verifies the Rust plugin-container/watcher
failure path, not complete foreign-plugin compatibility or a new browser profile.
No support status is promoted; the full mission remains active.

### Serialize HMR validation and acceptance rollback

A shared async lock now serializes HMR validation, failure rollback and message
publication. An older failing validation cannot restore its acceptance snapshot
after a later HMR validation succeeds. Watcher and programmatic update publication
share the same gate.

Validation: default and SWC-enabled `cargo test -p ferrite-server --locked` each
passed all 41 tests. The new controlled-concurrency regression blocks the first
failed edit while a corrected edit arrives, requires one active validation,
ordered error/update publication, corrected code and corrected acceptance metadata.
Outputs: `/tmp/ferrite-serialized-validation.log` and
`/tmp/ferrite-serialized-validation-swc.log`. Server all-target Clippy with
`--locked -- -D warnings`, formatting and diff checks passed.

No schema/cache migration or additional browser compatibility claim. This gate
covers HMR validation jobs; concurrent HTTP transforms, hooks outside validation,
cancellation and the wider race matrix remain assigned. Complete Refresh,
framework HMR, SSR/checkers and the rest of the mission remain active.

### Track failed initial requests and replay diagnostics on connection

Failed local dev-module HTTP requests now create source graph nodes, allowing a
first correction or creation to trigger watcher validation and reload. Server
diagnostics are retained for clients connecting after the failure. The HMR socket
subscribes before its handshake and replays the retained diagnostic afterward,
avoiding the previous connection gap. Successful HMR validation clears the stored
diagnostic before publication, so later documents do not receive a stale overlay.

Validation:

- Default and SWC-enabled `cargo test -p ferrite-server --locked`: 42 tests passed
  in each build. New real HTTP/notify regression requires initial HTTP 500,
  source tracking, correction/creation, reload, cleared diagnostic and successful
  subsequent module response. Outputs: `/tmp/ferrite-initial-recovery.log` and
  `/tmp/ferrite-initial-recovery-swc.log`.
- Explicitly executed the generated-app browser suite with the rebuilt CLI,
  Chromium 153 and Firefox 157: all 10 tests passed (82.98 seconds). The two new
  Node-free tests first fail a module request before any socket exists, then
  require diagnostic replay, repair/creation recovery and working interaction.
  A fresh corrected document must have no stale overlay, page errors or console
  errors. Initial failing requests intentionally retain their real HTTP/browser
  errors. All six existing profiles still complete their acceptance flows.
  Command: `cargo test -p ferrite-test --test generated_apps --locked -- --ignored --nocapture`.
  Output: `/tmp/ferrite-initial-recovery-all-browsers.log`.
- Server/test all-target Clippy with `--locked -- -D warnings`, formatting and
  diff checks passed.

No configuration, lock or cache schema migration. The initial-entry JavaScript
paths above are verified; wider cold dependency graphs, query/virtual resource
startup, component startup matrices and multiple simultaneous diagnostics remain
assigned. Runtime-error recovery, complete Refresh, framework HMR, SSR/checkers
and the full remaining mission stay active. No support profile is promoted.

### Recover cold imports when their dependency is created

Failed client imports now retain temporary graph edges to the resolver's actual
local file candidates, including extension probes, aliases and directory entries.
Creating a candidate validates the previously failed importer and reloads the
document without requiring an importer edit. Successful compilation removes
obsolete candidate nodes. Queued events for removed candidates cannot cause a
later spurious reload. Query-resource responses now register graph ownership and
preserve loader/watch dependencies; subsequent edits to a recovered raw resource
invalidate its importer.

Validation:

- `cargo test -p ferrite-resolver -p ferrite-server --locked`: 16 resolver and
  43 server tests passed. The real-notify regression covers extensionless imports,
  aliases, directory indexes and missing raw resources, candidate cleanup and a
  second raw-data edit. Output: `/tmp/ferrite-missing-import-recovery.log`.
- `cargo test -p ferrite-server --features swc --locked`: 43 tests passed.
  Output: `/tmp/ferrite-missing-import-swc.log`.
- Explicit Chromium/Firefox startup recovery tests passed all four scenarios
  per browser with Node absent from the CLI's PATH: invalid entry, missing entry,
  missing extensionless dependency and aliased directory index. Dependency
  creation must recover interaction while the importer remains byte-identical;
  fresh documents must have no stale diagnostic or page/console errors.
  Output: `/tmp/ferrite-missing-import-browsers.log`.
- The full explicitly executed generated-app suite passed all 10 tests in
  Chromium and Firefox (86.36 seconds), including all six existing template
  profiles and dependency HMR flows. Command:
  `cargo test -p ferrite-test --test generated_apps --locked -- --ignored --nocapture`.
  Output: `/tmp/ferrite-missing-import-full-browsers.log`.
- Resolver/server/test all-target Clippy with `--locked -- -D warnings`, Rust
  formatting and diff checks passed.

The pipeline cache identity advances to v10, rebuilding older cached output;
lock/config schemas are unchanged. Bare-package installation recovery, cold CJS
and virtual resources, external watched dependencies, broader component startup
and multiple simultaneous diagnostics remain unverified. No framework support
profile is promoted. The rest of the implementation mission remains active.

### Observe configured lockfiles for cold package recovery

The watcher now observes the explicitly configured lockfile even when it lives
inside an otherwise ignored directory such as `.ferrite`. Failed bare imports
already retain a dependency edge to that lockfile. Publishing installed package
files followed by the concrete importer lock edge now revalidates the failed
importer without editing its source.

The new real-notify regression covers `selected.lock` and
`.ferrite/selected.lock`, concrete v2 importer/package records, successful reload,
compiled importer output, candidate cleanup and byte-identical importer source.
Default server tests passed all 44 tests (`/tmp/ferrite-cold-package.log`).
The SWC-enabled server run also passed all 44 tests
(`/tmp/ferrite-cold-package-swc.log`). Server all-target Clippy with
`--locked -- -D warnings`, formatting and diff checks passed.
This fixture writes installed package files and the lock directly; it does not
establish live installer/browser acceptance or frozen reinstall support beyond
the previously recorded tests. No cache, configuration or lock schema migration
and no framework support promotion. The full mission remains active.

### Register anonymous React default exports

The native Refresh registration pass now addresses anonymous default exports
through the self-imported module namespace. It preserves the declaration and
its source locations while assigning a stable `%default%` family identity.
Named exports retain their previous registration behavior; reexports are still
excluded. Boundary validation still uses the official runtime's component check,
so registration does not cause an anonymous function to bypass safe invalidation.
The React plugin cache identity now includes the registration implementation
version and enablement, invalidating prior plugin output without a lock/config
schema migration.

Validation: `cargo test -p ferrite-frameworks --locked` passed all 26 unit tests,
including anonymous arrows/functions/classes/memo expressions and cache identity.
The separately executed ignored `react_refresh_runtime` test installed and
executed actual `react-refresh@0.17.0` through the explicit Node transport. It
requires an anonymous default family, preserves the six mixed-export boundary
checks and verifies unsafe anonymous updates invalidate. Outputs:
`/tmp/ferrite-anonymous-refresh-all.log` and
`/tmp/ferrite-anonymous-refresh-runtime.log`. Framework all-target Clippy with
`--locked -- -D warnings`, formatting and diff checks passed.

This is registration coverage, not complete anonymous-component state
preservation or HOC instrumentation. Hook signatures, custom hooks, full HOC
transforms and actual React DOM renderer/browser state tests remain assigned.
Other ignored compiler/browser fixtures were not executed for this change and
do not add evidence. React remains unavailable as a complete advertised create
profile; no support status is promoted. The full mission remains active.

### Validate an explicit foreign factory/hook subset

`NodeAdapterHost::register_hook_plugin` now evaluates one default factory/object
or named-hook module with explicit JSON factory options. It validates supported
`resolveId`, `load` and `transform` declarations, including unordered `{handler}`
objects. Resolve calls preserve importer/options, and load/transform preserve
options. Missing optional hooks return null, while malformed hooks, required
unsupported properties/lifecycle hooks, arrays, order/filter metadata and plugin
context access fail with actionable errors. Calling an unconfigured default
factory through the old named-export hook path now fails rather than silently
skipping it. Separate typed-export compiler registration is preserved.

Validation: plugin tests passed 15 default tests; all three explicitly executed
real-Node tests passed, including async factory/options, persistent state,
importer/options, hook objects, context failure, unsupported lifecycle/metadata,
typed compiler calls, protocol/log isolation, timeout and cancellation. Commands:
`cargo test -p ferrite-plugin --locked` and
`cargo test -p ferrite-plugin --locked real_node -- --ignored --nocapture`.
Outputs: `/tmp/ferrite-hook-subset.log` and
`/tmp/ferrite-hook-subset-node.log`. Plugin all-target Clippy with
`--locked -- -D warnings`, formatting and diff checks passed.
The actual project-matched Vue/Svelte client/server/runes compiler fixture also
passed through the same transport: `FERRITE_COMPILER_FIXTURE=/tmp/ferrite-real-install-45z_otrh
cargo test -p ferrite-frameworks --test compiler_host --locked -- --ignored --nocapture`.
Output: `/tmp/ferrite-hook-subset-compilers.log`.

README documents this bounded contract. This is not complete Vite/Rollup support
or a Node sandbox. Ordered hooks, host context resolution/watch/asset methods,
configuration/server/HMR lifecycle integration and registration through CLI
configuration remain assigned. No lock/config/cache migration or framework
support promotion; the full mission remains active.

### Connect validated foreign hooks to the shared pipeline

The public `ForeignHookPlugin` adapter now implements native resolve/load/
transform hooks on an explicitly supplied persistent Node host. It preserves
importer/environment/options context, code, source maps, filesystem dependencies,
module types, resolution metadata and load/resolution side effects. Result
decoding rejects unknown fields and invalid maps. Transform side-effect overrides
are explicitly unavailable because the native transform result cannot preserve
them. There is no implicit Node startup or host substitution.

Cache identity includes the entry source and factory options. Entry changes cause
a cache miss and an actionable worker/registration restart error. Re-registering
an already loaded hook entry or replacing an active hook registration on the same
worker fails, avoiding Node module-cache reuse with changed code. The README
documents result fields and current limitations.

Validation:

- `cargo test -p ferrite-plugin -p ferrite-server --locked`: 17 plugin and
  44 server tests passed. New decoder tests require unknown metadata and invalid
  maps to fail; object and string maps retain equivalent content. Output:
  `/tmp/ferrite-foreign-pipeline-unit.log`.
- Explicit real-Node `foreign_factory_hooks_participate_in_the_shared_pipeline`
  passed with both default and SWC builds. Each test exercises dev and production
  configurations, a generated module, importer/options, final dependency graph,
  chained original source maps, watched dependencies, side effects, entry-change
  cache invalidation and explicit failure on stale worker reuse. Commands:
  `cargo test -p ferrite-server --locked foreign_factory_hooks -- --ignored --nocapture`
  and the same command with `--features swc`. Outputs:
  `/tmp/ferrite-foreign-pipeline.log` and `/tmp/ferrite-foreign-pipeline-swc.log`.
- All three explicitly executed real-Node transport tests passed
  (`/tmp/ferrite-foreign-pipeline-node.log`). Plugin/server all-target Clippy
  with `--locked -- -D warnings`, formatting and diff checks passed.
- The actual project-matched Vue/Svelte client/server/runes compiler fixture
  passed through the changed worker transport using `FERRITE_COMPILER_FIXTURE`
  and the ignored `compiler_host` test (`/tmp/ferrite-foreign-pipeline-compilers.log`).

Cargo.lock adds the plugin crate's edge to the existing pinned source-map crate;
no package versions, Ferrite lock/config schemas or global pipeline cache version
change. CLI plugin configuration, transitive plugin-import tracking, path-alias
registration deduplication, automatic worker reload, context methods and lifecycle
integration remain assigned. These Rust pipeline fixtures do not establish browser,
bundled-output or SSR rendering compatibility. No framework status is promoted;
the full mission remains active.

### Canonicalize foreign plugin entry identities

The Node driver now resolves local entry paths and file URLs to a canonical
filesystem path and imports its canonical file URL. Registered-entry checks use
that same identity, so encoded URLs, dot paths and symlink aliases cannot bypass
the hook worker's restart requirement. Non-file URLs and file-URL queries or
fragments fail explicitly instead of acting as module-cache bypasses. Generic
typed-export compiler workers retain their contract.

Validation: `cargo test -p ferrite-plugin --locked` passed 17 tests, and all four
explicitly executed real-Node tests passed. The new test registers a file with
spaces, obtains its actual `import.meta.url`, attempts encoded URL/dot-path/Unix
symlink aliases and cache-busting URLs, and requires exactly one factory call.
Outputs: `/tmp/ferrite-plugin-entry-identity-unit.log` and
`/tmp/ferrite-plugin-entry-identity-node.log`.
The real-Node shared pipeline fixture passed its dev/production configurations
(`/tmp/ferrite-plugin-entry-identity-pipeline.log`), and the actual project-matched
Vue/Svelte client/server/runes compiler fixture passed through the transport
(`/tmp/ferrite-plugin-entry-identity-compilers.log`). Plugin all-target Clippy
with `--locked -- -D warnings`, formatting and diff checks passed.

README records the local-file entry contract. No lock/config/cache schema
migration or support promotion. These checks ran on Linux; Windows/macOS release
checks, transitive plugin imports, automatic worker reload, CLI configuration and
the full remaining mission stay assigned and active.

### Select foreign hook profiles through shared project configuration

`foreign_plugins` now selects explicit local hook profiles from TOML, static JS
configuration and programmatic `Config`. Each profile names its entry, explicitly
selects `host = "node"`, supplies JSON factory options and may select a Node
executable/deadline separately from framework compiler and SSR runtime settings.
Shared `Config::resolve` registers persistent validated adapters for CLI/library
pipelines. Missing host opt-in, unsupported hosts, duplicate/empty names, empty
entries, invalid deadlines and unknown profile fields fail. An explicit empty
list disables inherited profiles. Dynamic JS profile values/methods/accessors
fail instead of being skipped. Startup failures identify the profile and relevant
configuration fields; a missing explicit executable never falls back to PATH.

Doctor reports selected plugin entries and executables without executing them,
with experimental support and unverified execution. Missing entries/executables
are actionable errors. Inspect reports selections without factory options.
README documents the configuration and bounded hook contract.

Validation on Linux with Node 26.10.0:

- `cargo test -p ferrite-config -p ferrite -p ferrite-frameworks --locked` passed
  27 config, 14 facade and 27 framework unit tests, plus four executed doc tests.
  Includes profile roundtrip/merge/validation, static JS/dynamic rejection,
  host opt-in/no fallback and read-only doctor regression.
  Output: `/tmp/ferrite-configured-hooks-unit.log`.
- The separately executed real-Node `configured_foreign_hooks` facade fixture
  passed development/production resolution with the selected config file and
  unchanged SSR runtime selection, on default and SWC builds. Outputs:
  `/tmp/ferrite-configured-hooks-node.log` and `/tmp/ferrite-configured-hooks-swc.log`.
- New Chromium/Firefox CLI fixtures passed rendering, trusted clicks, source
  edit/full reload, changed interaction, scope-hoisted build and preview
  interaction, with no page/console errors. Then all 12 generated-app acceptance
  tests passed with `--test-threads=2` (95.94 seconds), including all six existing
  templates, dependency HMR and startup recovery. Command:
  `cargo test -p ferrite-test --test generated_apps --locked -- --ignored --nocapture --test-threads=2`
  with explicit CLI/browser paths. Outputs: `/tmp/ferrite-configured-hooks-browsers.log`
  and `/tmp/ferrite-configured-hooks-bounded-browsers.log`.
- An additional rebuilt-CLI smoke check selected `selected.toml` explicitly,
  asserted inspect/doctor JSON, fetched transformed dev JavaScript and verified
  transformed production assets (`/tmp/ferrite-configured-hooks-cli.log`).
- Workspace compilation, affected-crate all-target Clippy with
  `--locked -- -D warnings`, formatting and diff checks passed.

The first full browser run concurrent with large SWC compilation failed one of
12 tests with an actual Node worker boot timeout; the remaining 11 passed.
Output: `/tmp/ferrite-configured-hooks-all-browsers.log`. Bounded concurrency
changed scheduling only; no timeout or interaction assertions were weakened.
High-concurrency startup under that resource load is not established.

Migration: the optional additive config field defaults to no foreign profiles;
existing projects retain their selections. Cargo.lock adds test-only edges to
the existing pinned tempfile crate. No Ferrite lock/global cache schema change.
Transitive plugin-import tracking, host/version cache identity, automatic worker
reload, ordered/context/lifecycle hooks, broader plugin conformance and
cross-platform checks remain assigned. No framework support status is promoted;
SSR, complete Refresh, framework HMR/checkers, additional frameworks and the full
remaining mission stay active.

### Include running Node host identity in compilation caches

The Node worker captures its actual version, canonical executable, platform,
architecture and underlying ABI/library versions before evaluating guests.
`NodeAdapterHost::profile` queries and caches that typed snapshot; incomplete,
inconsistent or unsupported responses fail without substituting a host.
`cache_identity` combines the snapshot with the bridge implementation.

Foreign-hook identities now include that running-host identity alongside source
and factory options, and advance their namespace to `foreign-hooks-v2`.
The official Vue/Svelte host now combines its lock graph, running-host identity
and compiler-wrapper implementation. Its public `host_profile` describes the
already-started compiler worker separately from SSR runtime selection. Worker
PIDs and temporary wrapper paths do not influence stable cache identities.
Doctor remains read-only and does not invoke these execution APIs.

Validation on Linux/Node 26.10.0:

- `cargo test -p ferrite-config -p ferrite -p ferrite-frameworks -p ferrite-plugin --locked`:
  27 config, 14 facade, 27 framework and 19 plugin unit tests passed, plus four
  doc tests. New regressions separate version/executable/platform/architecture/
  ABI keys and require malformed worker profiles to fail. Synthetic profiles
  prove key isolation only, not compatibility on those other hosts. Output:
  `/tmp/ferrite-host-identity-all-unit.log`.
- All five explicitly executed `real_node` transport tests passed. The new test
  compares reported metadata with an actual guest's process metadata and requires
  the same identity across distinct workers using the same executable. Output:
  `/tmp/ferrite-host-identity-node.log`.
- The actual project-matched Vue/Svelte client/server/runes compiler fixture
  passed, including equal cache identities across fresh compiler workers, with
  distinct PIDs and the same running-host profile. The shared foreign-hook
  dev/production pipeline fixture also passed. Outputs:
  `/tmp/ferrite-host-identity-compilers.log` and `/tmp/ferrite-host-identity-pipeline.log`.
- Rebuilt CLI; all 12 explicit Chromium/Firefox generated-app acceptance tests
  passed with `--test-threads=2` (93.87 seconds), including six existing templates
  and foreign-hook/dependency/startup flows. Output:
  `/tmp/ferrite-host-identity-browsers.log`.
- Workspace compilation, plugin/framework/server all-target Clippy with
  `--locked -- -D warnings`, formatting and diff checks passed.

Existing Node-compiled cache entries are rebuilt because their plugin identity
changes. No Ferrite lock/config/global pipeline schema migration. Executable
binary integrity, execution flags/environment inputs, transitive plugin imports,
additional Node versions/platforms and the wider cache matrix remain unverified.
No framework support status is promoted. SSR/renderers, complete Refresh,
framework HMR/checkers and the full remaining mission stay assigned and active.

### Isolate Node execution flags in compiler/plugin cache identities

The captured running-host profile now includes explicit execution arguments and
`NODE_OPTIONS`. Both foreign-hook and official Vue/Svelte cache identities
already consume this profile, so flags that change conditional package resolution
cannot reuse output from a differently configured worker. Profiles missing the
required execution-argument metadata fail instead of assuming default flags.
No additional worker is started to obtain this metadata.

Validation on Linux/Node 26.10.0:

- `cargo test -p ferrite-plugin -p ferrite-frameworks -p ferrite --locked` passed
  19 plugin, 27 framework and 14 facade tests plus three facade doc tests.
  Key-isolation tests cover explicit arguments and `NODE_OPTIONS`; the real
  process-profile probe checks those fields against the running guest. Output:
  `/tmp/ferrite-node-options-all-unit.log`.
- All six explicitly executed real-Node transport tests passed. The new Unix
  fixture launches the same actual Node executable with isolated wrapper
  environments, loads a real local conditional-exports package, transforms valid
  JavaScript and requires default/custom selections and different adapter keys.
  Node executable, ABI versions and explicit arguments remain equal while
  `NODE_OPTIONS` differs; no process-global test environment mutation is used.
  Output: `/tmp/ferrite-node-options-real.log`.
- Actual project-matched Vue/Svelte client/server/runes compiler and shared
  foreign-hook dev/production pipeline fixtures passed. Outputs:
  `/tmp/ferrite-node-options-compilers.log` and `/tmp/ferrite-node-options-pipeline.log`.
- Rebuilt CLI; all 12 explicitly executed Chromium/Firefox acceptance tests
  passed with `--test-threads=2` (95.27 seconds), retaining the six existing
  templates and foreign-hook/dependency/startup flows. Output:
  `/tmp/ferrite-node-options-browsers.log`.
- Workspace compilation, plugin/framework all-target Clippy with
  `--locked -- -D warnings`, formatting and diff checks passed.

The tested flag behavior matches Node 26.10.0's documented
[user conditions](https://nodejs.org/api/packages.html#resolving-user-conditions) and
[NODE_OPTIONS](https://nodejs.org/api/cli.html#node_optionsoptions) contracts.
Prior Node-derived cache identities change automatically;
no Ferrite lock/config/global cache schema migration. Other environment values,
working-directory inputs, executable integrity, transitive plugin state and
additional host/platform versions remain unverified. This fixture proves host
conditional resolution and key isolation, not Ferrite installer/browser support
for arbitrary custom-condition packages. No framework support status is promoted;
the full remaining mission stays active.

### Isolate Node startup environment and working directory

The running-host snapshot now includes canonical cwd and a deterministic SHA-256
hash of the startup environment, captured before guest evaluation. Foreign-hook
and official compiler keys consume both automatically. The profile does not
publish the environment dictionary. Persistent cwd/environment mutation during
registration or an awaited hook/export rejects the result with an actionable
host-restart error; subsequent protocol requests also reject the drift. Ferrite
does not silently reset the process, restart it or substitute another backend.

Validation on Linux/Node 26.10.0:

- `cargo test -p ferrite-plugin -p ferrite-frameworks -p ferrite --locked`:
  19 plugin, 27 framework and 14 facade unit tests, plus three doc tests passed.
  Output: `/tmp/ferrite-node-environment-unit.log`.
- All eight explicitly executed real-Node tests passed. Isolated executable
  wrappers vary environment/cwd without mutating Rust's global environment.
  Actual transforms and cache identities differ for both inputs, and the profile
  excludes the dummy private environment values. Registration, transform and
  typed-export mutation fixtures reject drift and require a new host.
  Output: `/tmp/ferrite-node-environment-real.log`.
- Actual project-matched Vue/Svelte client/server/runes compilation and shared
  foreign-hook dev/production pipeline fixtures passed after the final guard.
  Outputs: `/tmp/ferrite-node-environment-compilers.log` and
  `/tmp/ferrite-node-environment-pipeline.log`.
- Workspace compilation and plugin/framework all-target Clippy with
  `--locked -- -D warnings` passed; CLI rebuilt successfully.
- All 12 explicitly executed Chromium/Firefox generated-app acceptance tests
  passed with `--test-threads=2` (98.85 seconds), including the six existing
  templates and foreign-hook/dependency/startup flows. Output:
  `/tmp/ferrite-node-environment-browsers.log`. Formatting and diff checks passed.

Node-derived cache keys change automatically; no lock/config schema migration.
This is process-state consistency, not a sandbox or general purity guarantee.
Transient restored mutations, asynchronous changes after returning, arbitrary
files, transitive guest imports, executable integrity and other host/platform
versions remain unverified. No framework support status is promoted. Complete
Refresh, framework HMR/checkers, real SSR/SSG and the remaining mission stay active.

### Track loaded foreign-plugin module dependencies

The explicit Node worker now installs synchronous `node:module.registerHooks`
load observation before importing guests. It snapshots hashes of canonical file
modules actually loaded by ESM/CommonJS, exposes a typed dependency snapshot,
and rejects changed/deleted loaded files before protocol requests and after
successful guest execution. Foreign registration identities include the loaded
snapshot; `foreign-hooks-v3` keys also hash current file contents. Successful
load/transform results preserve these module paths as watched dependencies.
Hooks that first load new file dependencies while executing are persistently
rejected with instructions to recreate the host and import during registration,
including when guest execution throws. No stale-module reload or Node fallback.
Hosts without the required loader API fail startup with a tested-version hint.

The API contract was checked against the official
[Node 26.10.0 synchronous loader documentation](https://nodejs.org/api/module.html#moduleregisterhooksoptions).
This does not establish support on other Node releases.

Validation on Linux/Node 26.10.0:

- `cargo test -p ferrite-plugin -p ferrite-frameworks -p ferrite --locked` passed
  19 plugin, 27 framework and 14 facade unit tests plus three doc tests.
  Output: `/tmp/ferrite-transitive-unit.log`.
- All ten explicitly executed real-Node tests passed. New regressions use an
  actual imported CommonJS helper: edits change cache identity and reject the
  original worker, a fresh worker executes changed output, and deletion rejects
  execution. A late ESM import fails both first and subsequent hook calls.
  Output: `/tmp/ferrite-transitive-real.log`.
- Actual project-matched Vue/Svelte client/server/runes compiler fixture passed.
  Shared foreign-hook development/production fixture now checks imported helper
  watch metadata, cache-key invalidation and explicit stale-worker failure,
  retaining entry-change/map/import/side-effect assertions. Outputs:
  `/tmp/ferrite-transitive-compilers.log` and `/tmp/ferrite-transitive-pipeline.log`.
- All 12 explicitly executed Chromium/Firefox generated-app acceptance tests
  passed with `--test-threads=2` (104.24 seconds), covering the six existing
  templates and foreign-hook/dependency/startup flows. Output:
  `/tmp/ferrite-transitive-browsers.log`.
- Workspace compilation, rebuilt CLI, plugin/framework/server all-target Clippy
  with `--locked -- -D warnings`, formatting and diff checks passed.

Existing foreign/Node-derived cache identities change automatically. No lock or
configuration schema migration. This tracks loaded module files, not arbitrary
filesystem reads, package-resolution metadata, preloaded modules, worker-thread
imports, executable integrity or guest-created loader behavior. Whole-worker
snapshots conservatively include dependencies of other registrations on that
worker. File changes racing loading/checks, null-result watch ownership, automatic
restart, late dependency discovery and broader cache/concurrency matrices remain
assigned. No support status is promoted; the full framework mission remains active.

### Preserve foreign module watches when hooks decline a request

Foreign resolve/load/transform now use the native request-scoped watch context
before executing the hook. Their registration-time loaded module dependencies
therefore belong to the importer even when hooks return null or are absent,
without synthesizing a transform or changing its code/map. Existing successful
result metadata remains preserved. The shared pipeline fixture checks a real
Node plugin whose hooks all decline `/entry.js`: helper ownership appears in
returned dependencies and the graph, and remains present on a repeated cache
lookup. Development and production fixture profiles both passed.

Validation: `cargo test -p ferrite-plugin --locked` passed 19 unit tests; ignored
Node fixtures in that command do not establish support. The explicitly executed
`cargo test -p ferrite-server --locked foreign_factory_hooks -- --ignored --nocapture`
passed, using the actual Node host. Plugin/server all-target Clippy with
`--locked -- -D warnings`, formatting and diff checks passed. Logs:
`/tmp/ferrite-null-watch-unit.log`, `/tmp/ferrite-null-watch-pipeline.log`,
`/tmp/ferrite-null-watch-clippy.log`.

No migration or support promotion. Browser notification timing, watches outside
project roots, load races, late imports and the broader remaining mission remain
assigned; this regression proves pipeline/cache/graph ownership only.

### Invalidate explicitly tracked files in package/output directories

A real Node/notify regression reproduced a stale-plugin bug: the watcher
unconditionally discarded `node_modules` events, including retained foreign
compiler/plugin dependencies. Before the fix the new test timed out with no
HMR diagnostic (`/tmp/ferrite-foreign-watch.log`). The filter now checks graph
ownership first. Tracked dependencies in `node_modules`, `.ferrite`, `dist` or
`target` invalidate normally; unrelated output/store writes remain ignored.
The configured lockfile exception remains intact.

The regression loads an actual plugin importing an ESM helper under
`node_modules`, lets all hooks decline the application module, changes the
helper through the filesystem and requires a stale-dependency diagnostic with
explicit host-restart instructions. Both project-local and separately watched
outside-root helpers pass; no Node fallback or silent stale output is accepted.

Validation on Linux/Node 26.10.0:

- `cargo test -p ferrite-server --locked` and the `--features swc` matrix each
  passed 44 unit tests. Their two ignored Node fixtures are not support evidence.
- The new ignored real-Node watcher test was explicitly executed and passed
  under default and SWC lowering; outputs `/tmp/ferrite-foreign-watch-fixed.log`
  and `/tmp/ferrite-foreign-watch-swc-real.log`.
- Rebuilt CLI, server all-target Clippy with `--locked -- -D warnings`, formatting
  and diff checks passed. Outputs `/tmp/ferrite-foreign-watch-build.log` and
  `/tmp/ferrite-foreign-watch-clippy.log`.
- All 12 explicitly executed Chromium/Firefox generated-app acceptance tests
  passed with `--test-threads=2` (87.93 seconds); output
  `/tmp/ferrite-foreign-watch-browsers.log`. These retain the existing template,
  plugin, dependency HMR and startup-recovery assertions.

No migration or support promotion. Automatic plugin restart, stale-state recovery
without restart, browser overlay assertion for this precise helper-edit case,
other notify backends/platforms and the broader remaining mission stay assigned.

### Connect pinned native Refresh signature instrumentation

React's enabled client-development post transform now invokes the pinned
`oxc_transformer@0.151.0` React Refresh pass on lowered JavaScript, with JSX
lowering disabled. It reuses the existing semantic/transform/codegen service,
adds module-local registration/signature/runtime bindings, prefixes compiler
family keys by module ID and chains compiler maps through preamble/footer edits.
Generated local bindings avoid names already present in the source. Existing
export-boundary validation and anonymous-default registration are retained.
Production/SSR exclusion remains enforced before instrumentation. React cache
identity advances to `oxc-0.151.0-signatures-v1`; no lock/config migration.
The exact compiler API was inspected in the installed pinned crate's source and
validated by compilation, rather than assuming the latest docs matched it.

Validation:

- `cargo test -p ferrite-frameworks -p ferrite-transform --locked` passed 27
  framework and 48 transform unit tests. New assertions check actual hook
  instrumentation, syntactically valid plugin output, source-map content and a
  runtime-binding collision. Output `/tmp/ferrite-react-signatures-unit.log`.
- Transform tests with `--features swc --locked` passed 56 tests; this also
  exercises the native Refresh helper when SWC is available. The frameworks
  crate itself has no SWC feature; an initial invocation there was rejected and
  corrected to the crate that owns the feature. Output
  `/tmp/ferrite-react-signatures-swc.log`.
- The explicitly executed official `react-refresh@0.17.0` runtime fixture passed.
  It executes compiler-instrumented custom-hook/component functions, verifies a
  compatible edit appears in `updatedFamilies`, and a changed hook signature in
  `staleFamilies`, retaining the previous mixed-export and anonymous-default
  assertions. Output `/tmp/ferrite-react-signatures-runtime.log`.
- Workspace compilation, transform/framework all-target Clippy with
  `--locked -- -D warnings`, formatting and diff checks passed.

This runtime fixture proves signature/family decisions without a React DOM
renderer. Browser state retention, hook-only modules outside JSX ownership,
HOCs/anonymous declarations across the full conformance matrix, inserted binding
interactions, source-location accuracy and syntax-error recovery remain assigned.
No React template/profile is promoted. The full framework mission stays active.

### Verify React DOM Refresh and fix virtual dispatch/preamble ordering

New real Chromium/Firefox acceptance installs pinned React/React DOM 19.2.0 and
react-refresh 0.17.0 through Ferrite, renders with the official `createRoot` API,
clicks a stateful component, edits it, builds with scope hoisting and interacts
with the preview. Install/dev/build/preview run with Node absent from `PATH`.
This is a hand-authored conformance fixture, not a newly advertised template.

The first run mounted and clicked successfully but failed Refresh in both
browsers. A diagnostic rerun showed an overlay resolving a browser virtual URL
as a project filesystem path and zero registered Refresh renderers. Fixes:

- Shared resolve dispatch now decodes `/@id/` URLs before plugin/resolver hooks,
  as loading already does. Virtual IDs/URLs cannot acquire filesystem identity.
  React's internal virtual ID also goes through its client-development capability
  guard, preventing the internal alias from bypassing SSR/production rejection.
- JSX mounting entries with no component exports now prepend the Refresh
  preamble as their first ESM dependency, so the hook is installed before React
  DOM evaluates. Such entries do not become self-accepting Refresh boundaries.
  Cache identity advances to `oxc-0.151.0-entry-preamble-v2`.

Validation:

- Both new browser tests passed the full expanded flow (45.38 seconds): compatible
  edits preserve counter state/document identity; invalid syntax keeps the live
  UI and shows a source-named overlay; correction clears it and retains state;
  an added hook resets component state without reloading the document; subsequent
  clicks work. Build/preview interactions pass, with no page/console errors.
  Production JS is checked for absence of Ferrite Refresh instrumentation and
  `createSignatureFunctionForTransform`. Output `/tmp/ferrite-react-dom-recovery.log`.
- Initial failure logs `/tmp/ferrite-react-dom-browsers.log` and
  `/tmp/ferrite-react-dom-diagnostic.log` are retained; assertions/timeouts were
  not weakened. The earlier narrower post-fix run also passed both browsers.
- 27 framework and 45 server unit tests passed; the server SWC matrix also passed
  45 tests. Regressions check entry preamble/no false boundary, virtual dispatch,
  missing physical paths and SSR rejection. Logs `/tmp/ferrite-react-dom-unit.log`,
  `/tmp/ferrite-react-dom-frameworks.log`, `/tmp/ferrite-react-dom-swc.log`.
- Rebuilt CLI, workspace compilation, server/framework/test all-target Clippy
  with `--locked -- -D warnings`, formatting and diff checks passed.
- The complete explicit Chromium/Firefox acceptance suite passed all 14 tests
  with `--test-threads=2` (131.48 seconds), retaining the six existing generated
  templates and plugin/dependency/startup cases alongside the two React DOM
  fixtures. Output `/tmp/ferrite-react-dom-all-browsers.log`.

The renderer API matches the official
[React createRoot contract](https://react.dev/reference/react-dom/client/createRoot).
No lock/config migration or full-profile support promotion. Hook-only modules,
HOCs, anonymous/default/mixed-export browser matrices, JSX ownership, generated
React templates/checkers and real SSR remain assigned. The full mission stays active.

### Verify memo/custom-hook browser Refresh behavior

The real React DOM fixture now retains the plain-component case and adds a
memo-wrapped named component calling a local custom hook. Each browser variant
uses the existing pinned React/React DOM 19.2.0 and react-refresh 0.17.0 packages,
Ferrite installation, native compilation and CLI processes without Node on PATH.
The compatible edit preserves component/document state. The wrapped variant's
incompatible edit changes hook order inside the custom hook itself, requiring
recursive signature invalidation and a component reset without document reload.
Both variants retain syntax-error recovery, subsequent interaction, production
Refresh exclusion and scope-hoisted build/preview assertions. Existing compiler
instrumentation handled these cases; no runtime workaround was introduced.

Validation: explicitly executed all four `react_refresh_` Chromium/Firefox
acceptance tests with `--test-threads=2`; all passed in 83.75 seconds. Output:
`/tmp/ferrite-react-wrapped-final.log`. The initial wrapped/direct-hook-reset
probe also passed but is superseded by the stronger recursive-hook test.
`cargo clippy -p ferrite-test --test generated_apps --locked -- -D warnings`,
formatting and diff checks passed. Output `/tmp/ferrite-react-wrapped-clippy.log`.
The official [memo API](https://react.dev/reference/react/memo) and
[custom-hook guidance](https://react.dev/learn/reusing-logic-with-custom-hooks)
were checked; actual package/runtime execution establishes the stated fixture
version evidence, not current-doc compatibility with untested releases.

No migration or support promotion. These are hand-authored conformance fixtures;
generated React templates, imported hook-only modules, other HOC shapes,
anonymous/default/mixed-export matrices and the broader mission remain assigned.
