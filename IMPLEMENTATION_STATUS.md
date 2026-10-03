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
