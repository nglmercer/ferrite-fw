# Framework support implementation status

## Generated Vue SSR JS/TS sources build and hydrate in both browsers

A new real-package acceptance case generates each Vue SSR source variant into a
clean temporary project, installs its actual generated package.json through
Ferrite, writes the resolved lock, and uses create_builder to load the generated
ferrite.toml and declarative compiler profile. Default production settings build
both environments without the hand-written fixture's minification override.
The emitted renderer must produce the generated heading/counter/scoped markup
and retain component styles. Preview then hydrates each generated app in Chromium
and Firefox, retains the original server DOM button, updates count zero to one,
applies the generated scoped color, and emits no page/console errors or hydration
diagnostics. Source instrumentation adds only a pre-module DOM snapshot and a
mount-completion flag; component and rendering logic remain generated.

The expanded explicitly selected test PASSED all JS/TS and both-browser cases.
The earlier install/build/render-only run also passed. Final feature-enabled
compiler-test Clippy with warnings denied, formatting and diff checks passed.
An initial Config field-name compilation error was corrected to use CLI overrides
before final execution. The test is ignored by default for its registry, Node
compiler-host and browser requirements; actual execution supplies this evidence.

CLI create exposure, generated dev SSR/source-edit/HMR/recovery, frozen clean
reinstall, full editor/type-checker support, streaming/SSG, other frameworks and
release matrices remain unfinished. No whole profile is promoted until its
complete acceptance cycle executes. The full mission remains unfinished.

## Explicit Vue SSR source generator

The new library `scaffold::vue_ssr_files` builds on existing canonical Vue client
scaffolding and preserves its component, pinned dependency, public assets,
interaction fixtures and editor files. It requires explicitly selected Node
compiler host and separate napi-vm runtime, rejects unsupported combinations and
languages, and generates a real renderToString server entry, createSSRApp hydration
entry, HTML outlet and explicit SSR/runtime configuration. Generated documentation
identifies this as experimental and calls out the runtime feature requirement
and remaining acceptance/type-checking limitations. It does not start a host,
install dependencies or infer a runtime.

Framework tests passed 34, with five existing resource-dependent cases ignored;
all-target framework Clippy with warnings denied, formatting and diff checks
passed. Tests cover both JS/TS wiring and rejected host/runtime/language inputs.
These generation tests do not prove the complete generated application cycle.
The new source API is deliberately not yet advertised by CLI template selection;
generated-profile install/dev/HMR/build/preview browser acceptance must follow,
then CLI host/runtime selection and registry exposure. Existing aliases/profiles
and configuration formats remain unchanged. The full mission remains unfinished.

## Actual Vue SSR preview hydrates in Chromium and Firefox

The real Vue 3.5.22 production SSR conformance fixture now shares its HTTP/CSS
checks with two real-browser cases using Ferrite's existing Rust E2E tooling.
The client entry uses createSSRApp mounting; an inline pre-module snapshot records
the original server-rendered button. Each browser must hydrate while retaining
that exact DOM node, update count zero to one on click, retain computed red scoped
styles, and report no page errors, console errors or hydration diagnostics.
The existing HTTP checks still require meaningful markup before client execution,
isolated request headers, published matching CSS and private artifact rejection.

Both new ignored-by-default cases were actually selected with `--ignored --exact`
and PASSED: Chromium used the explicitly configured cached executable, Firefox
used the installed executable. Final feature-enabled compiler-test Clippy with
warnings denied, formatting and diff checks passed. An initial test compilation
failure from an ambiguous evaluate generic was corrected with an explicit result
type before executing the final cases. The HTTP-only entrypoint remains available
without browser requirements. These fixtures install official pinned packages
through Ferrite and explicitly use Node for compilation and napi-vm for rendering.

This proves production hydration/interaction for this Vue fixture, not the entire
generated-template acceptance cycle, dev SSR HMR, streaming/SSG or cross-platform
release behavior. Full create/install/editor/type-checker flows, other frameworks,
the Svelte parser defect and the remaining complete mission stay unfinished. No
whole framework profile was promoted based on these partial acceptance cases.

## Official component SSR styles survive through HTTP preview

The hosted framework adapter previously appended generated stylesheet imports
only for clients, dropping server stylesheet ownership. It now preserves CSS
resource edges in both targets. The existing SSR CSS pipeline produces inert
JavaScript shims; production extraction publishes CSS and removes those imports
from executable output. The Vue server pipeline regression now requires the
scoped style edge rather than incorrectly requiring it to disappear early.
The Svelte server check still excludes browser DOM injection.

The actual Vue 3.5.22 renderer conformance now builds both client and server with
an actual createSSRApp mounting entry, starts Ferrite preview, and asserts
meaningful HTML and isolated request headers over HTTP. It fetches each published
SSR stylesheet, matches its scope identifier to rendered markup, requires the
actual red declaration, and rejects private artifact access. The initial run
FAILED because the artifact had no CSS; after the adapter fix, this exact test
PASSED. The actual Vue/Svelte compiler pipeline test was also explicitly selected
and passed. Framework unit tests passed 33; five other framework tests remain
ignored and do not establish support. Feature-enabled framework/test all-target
Clippy with warnings denied, formatting and diff checks passed.

Migration: rebuild prior server output to capture component styles in renderer
artifacts and publish ssr-assets; artifact schema/lockfile formats are unchanged.
Client mounting code compiles here, but browser hydration/interaction is still
unexecuted. Generated SSR profiles, the Svelte interpreter parser defect, release
matrices and the complete remaining mission remain unfinished. No whole framework
profile was promoted.

## Actual Vue renderer checks concurrent guest-global isolation

The official Vue 3.5.22 server fixture now increments a guest-global request
counter before awaiting renderToString and returns the counter through response
headers. The compiled renderer case requires count one. The saved-artifact case
now launches three concurrent requests on the same adapter and requires count
one for each, alongside meaningful count-zero component HTML and scoped markers.
This catches shared guest globals across actual framework renderer invocations,
rather than inferring isolation from repeated component output.

The real-package compile/build/render case was explicitly selected and passed.
Its updated saved artifact was then tested from the current compiled test binary
with PATH empty; all three concurrent requests passed. Final feature-enabled
compiler-test Clippy with warnings denied, formatting and diff checks passed.
This establishes this fixture's guest-global isolation, not arbitrary native
state/filesystem isolation or complete framework support. Hydration, generated
SSR profiles, browser interactions, the unresolved Svelte parser failure and the
full remaining mission remain unfinished; no profile was promoted.

## Official Vue renderer executes with a rootless embedded runtime

The new real-package Vue conformance case uses Vue 3.5.22 createSSRApp and
vue/server-renderer renderToString, compiles a script-setup TypeScript SFC with
scoped/module styles through the explicitly selected Node compiler host, builds
the actual server graph, and executes it through napi-vm. It was explicitly
selected and PASSED, requiring a rendered button, initial count and scoped marker.
`FERRITE_VUE_SSR_ARTIFACT` optionally saves that actual emitted artifact.
A second case reads the saved artifact and invokes its rootless adapter for three
requests. It was executed from the current compiled test binary with PATH empty
and PASSED, after the original temporary source project had been removed. This
separates Node compiler-host requirements from in-process runtime execution.
Feature-enabled compiler-test Clippy with warnings denied, formatting and diff
checks passed. Both cases are explicit slow/resource-dependent tests; their
actual executions, not their ignored default status, supply this evidence.

Upstream napi-vm HEAD was inspected at 881cc8f1cec262049b6d31fa499b0d601ba673c5;
its parser statement implementation is identical to the pinned implementation
for the Svelte failing assignment-form for-in loop. No ineffective pin update,
vendored rewrite or runtime fallback was introduced. That Svelte failure remains.

This proves Vue rendering for these fixtures only. It does not establish
hydration, browser interaction, CSS HTTP delivery, complete request isolation,
streaming, release matrices or generated SSR template acceptance. No complete
framework profile was promoted; the full framework-support mission remains
unfinished.

## Engine parser preflight identifies the actual Svelte module failure

Before graph registration changes, napi-vm graph evaluation now lexes/parses each
supplied module with the pinned engine's own public lexer and parser. Errors name
both graph entry and failing module and retain line/column plus the original
parser diagnostic. This does not rewrite or substitute framework compiler output.
The syntax regression now distinguishes an importing entry from its broken
transitive dependency, checking both identities. Final runtime tests passed 22;
feature-enabled all-target runtime Clippy with warnings denied, formatting and
diff checks passed. This adds parsing work before evaluation; throughput and
syntax-preflight caching remain unverified.

The actual Svelte renderer test was executed again and still FAILED, now locating
`/assets/index-7416ddf8.js` at 137:20. Inspection of the saved emitted artifact
shows `for (name in attrs)` on that line: valid framework JavaScript rejected by
the selected pinned interpreter. This supplies a concrete upstream parser
compatibility defect to resolve; it does not establish SSR support or hydration.
No test assertion was weakened and no alternative runtime was started. The
Svelte rendering failure and the complete remaining mission remain unfinished.

## Actual Svelte renderer exposes embedded-runtime failure

A new explicitly selectable real-package test imports `svelte/server` render,
compiles a Svelte 5.39.6 component through the production server pipeline, loads
the emitted renderer artifact, and calls it through napi-vm. The meaningful HTML
assertions remain mandatory. This test was executed twice and FAILED: embedded
evaluation reported `SyntaxError: expected RBrace, found RParen at 137:20`.
Compilation success therefore does not establish Svelte rendering support. The
case remains ignored by default for its registry/Node compiler requirements;
ignoring it does not convert this failure into support. No alternate runtime is
spawned or substituted. `FERRITE_SVELTE_SSR_ARTIFACT` optionally retains emitted
artifact bytes for diagnosis; a diagnostic run produced the actual graph.

Graph evaluation errors now identify the napi-vm backend and graph entry and
retain the original error with a runtime-compatibility diagnostic. A regression
executes malformed graph syntax and verifies those fields. Final runtime tests
passed 22, feature-enabled runtime/test all-target Clippy with warnings denied,
formatting and diff checks passed. Initial test compilation mistakes involving
RuntimeEnvironment construction were corrected before final verification.
The real Svelte renderer failure remains unresolved and must be fixed before
advertising this embedded rendering profile. Pinned official Svelte package exports
were inspected alongside its documented server render API; no compatibility
profile was promoted. Framework hydration and the full mission remain unfinished.

## Official Svelte server compilation conformance

The real-package shared-pipeline test now checks server compilation of Svelte
5.39.6 components and runes-bearing TypeScript modules alongside existing Vue
3.5.22 server checks. It asserts the exact final server dependency identity,
absence of client runtime imports/HMR/generated style imports, retained server
maps, and removal of TypeScript/runes syntax. The official packages are installed
through Ferrite and compiled on the explicitly selected persistent Node host.
The expanded ignored-by-default test was explicitly executed and passed; final
standalone compiler-test Clippy with warnings denied, formatting and diff checks
passed. Its first run exposed an incorrect assertion expecting an unrewritten
bare import; the final check uses final analyzed dependency identities instead.

This proves server compiler output for these fixtures, not framework renderer
execution, hydration, browser conformance, native compiler hosting or generated
SSR template support. No compatibility profile was promoted. Those requirements
and the full remaining framework-support mission remain unfinished.

## Embedded SSR asset HEAD headers

Generated SSR servers now include the embedded asset length for GET and HEAD,
with an empty HEAD body. The real generated SSR binary acceptance test now checks
client-script HEAD length against its GET bytes and verifies asset POST returns
405 with the Allow header. Its existing renderer GET/POST/HEAD, private-file and
empty-PATH checks remain intact. The expanded slow test was actually selected,
compiled offline and passed; feature-enabled standalone-test Clippy with warnings
denied, formatting and diff checks passed. This validates custom-renderer binary
HTTP behavior, not framework hydration or browser interaction. The full mission
and its remaining framework/runtime/release requirements remain unfinished.

## Embedded static standalone HTTP methods

The generated embedded static server now permits GET/HEAD and rejects other
methods with 405 and `Allow: GET, HEAD`. HEAD preserves the asset content type
and byte length while returning no body. The existing real Cargo build/boot test
was extended, not weakened, to check HEAD body/content length and POST rejection
beside the prior GET and missing-route checks. The final generated binary test
was explicitly executed and passed. Facade library tests passed 29 with one
ignored; final facade/test all-target Clippy with warnings denied, formatting and
diff checks passed. No configuration migration is needed; POST to static assets
now fails explicitly. SSR assets need the same HEAD length check, and framework
SSR/hydration, release matrices and the broader mission remain unfinished.

## Executed generated-binary acceptance for staged packaging

The Rust standalone test suite now exposes a napi-vm feature and a real SSR
acceptance case. It builds both environments from TypeScript source, compiles the
published generated Cargo application offline, alters server source after the
build, and executes the binary from an empty directory with PATH empty. Assertions
cover meaningful pre-JavaScript HTML, status/custom headers, POST method/URL,
empty HEAD body, the emitted client script, and private renderer-file rejection.
The slow test remains explicitly selectable; it was actually executed and passed,
including its expanded request/asset assertions. This is custom-renderer SSR, not
React/Vue/Svelte hydration evidence.

Executing the existing real static-binary test exposed a previously unexecuted
mismatch: unknown extensionless routes returned the shell despite an existing 404
assertion. The generated static server now returns 404 for missing paths. The
assertion was preserved and the real build/boot test passed after correction.
Migration note: standalone static mode no longer substitutes index.html for
unknown extensionless paths; explicit files and directory index serving remain.
Test-owned servers now use cleanup guards on assertion failure.

Feature-enabled facade tests passed 32 with one ignored; the ordinary standalone
suite passed three with three ignored. Both real binary cases
were selected with `--ignored --exact` and passed; musl remains unexecuted.
Feature-enabled facade/test all-target Clippy and the final standalone-test Clippy
passed with warnings denied. SDK provenance, concurrent/atomic binary publication,
cross-platform releases, framework renderers/hydration, browser interaction and
the full remaining mission are still unfinished.

## Scaffold generation stages before publication

Standalone generation now writes and optionally builds in a temporary sibling
directory before replacing the existing scaffold. Publication retains the old
tree until rename succeeds, attempts restoration on publication failure, and
retains a named recovery directory if restoration fails. Internal staging and
backup trees are excluded from public asset collection. The failure regression
verifies a bad target preserves the old manifest and leaves no partial sources or
temporary directories. tempfile is now a runtime facade dependency; its existing
resolved version is unchanged. Default tests passed 29 with one ignored; final
feature-enabled tests passed 32 with one ignored; feature-enabled all-target
Clippy with warnings denied, formatting and diff checks passed. An initial Clippy
needless-borrow failure was corrected and the final check passed.

This stages scaffold publication, not the whole application build. Copied binary
publication still occurs separately, concurrent writers/filesystem replacement
are not isolated, and platform-specific rename behavior needs release tests.
Framework SSR/hydration and the remainder of the full mission remain unfinished.

## Existing scaffold write paths reject symlinks

Before any scaffold write, packaging now scans an existing generated tree with
symlink metadata and rejects links, including links in source, compressed assets
and Cargo output. This closes the separate overwrite path left by excluding the
old scaffold from asset collection. Unix regressions verify external source and
existing manifest contents remain unchanged for file/directory links. Default
facade tests passed 28 with one ignored; all-target Clippy with warnings denied,
formatting and diff checks passed. Concurrent filesystem replacement and atomic
scaffold publication remain unfinished; this is not a filesystem sandbox. The
complete framework mission remains unfinished.

## Standalone asset symlinks fail explicitly

Asset collection now rejects symlink output roots and symlink entries instead of
following them into external files, directories, or cycles. The failure occurs
before scaffold writes. Unix regressions cover external file and directory links
and verify external contents remain untouched. Default facade tests passed 27
with one ignored; all-target Clippy with warnings denied passed. Intentional asset
links must be copied into output. Concurrent filesystem replacement and full
transactional packaging remain unfinished, as does the broader framework mission.

## SSR SDK identity and exact-version requirements

Standalone SSR preflight now parses the explicitly supplied SDK Cargo manifest and
requires package `ferrite`, the generating tool's exact package version, and a
nonempty `napi-vm` feature. Unsupported/inherited/missing package versions fail
with an actionable error before packaging; `build_app` runs this preflight before
client output. Generated Cargo dependencies carry the same exact version along
with the explicit SDK path. The facade reuses the existing workspace TOML library;
Cargo.lock changes only to record that dependency, without package upgrades.

Validation: feature-enabled facade library tests passed 29 with one ignored;
all-target feature-enabled Clippy with `-D warnings` and offline cargo check
passed. The SDK rejection and scaffold generation tests passed from the current
built test binary with PATH empty. A broader initial package-test invocation used
an older binary and failed its fake Cargo shell fixture because `mkdir` was absent
from PATH; this does not establish whole-package Node-free support. The correctly
selected current binary was used for the two Node-free SDK tests. Regenerated
exact-version scaffold Cargo built offline, then the actual binary passed GET,
POST, HEAD, embedded JavaScript and private-file rejection checks from an empty
working directory with PATH empty. Formatting and diff checks passed.

Matching manifest identity/version is not source provenance or API compatibility
proof for arbitrary modified checkouts. Inherited workspace package versions are
currently unavailable with an explicit-version error. SDK provenance, transactional
scaffold replacement, automated generated-binary/CLI acceptance, release/platform
matrices, framework SSR/hydration, and the rest of the mission remain unfinished.

## Standalone app builds select the embedded SSR scaffold

`package.ssr_sdk` is an optional explicit SDK crate path (relative paths resolve
from the project root). With a server entry and standalone selection, `build_app`
requires this setting, the napi-vm feature/backend, embedded assets, a valid SDK
source directory and supported base before either environment writes output. It
builds client and server without intermediate static packaging, then embeds the
final renderer artifact with the existing standalone API. The CLI default build
uses this same library path. Explicit server-only standalone builds fail with an
action directing callers to build both environments. Explicit client-only builds
retain their static behavior; the static writer still rejects private SSR output.

Validation: feature-enabled facade tests passed 28 with one ignored; config tests
passed 29; default facade tests passed 25 with one ignored; standalone regression
tests passed three with two ignored. The new builder tests also passed with PATH
empty. The successful case compiles a TypeScript server through the production
pipeline, packages after both build reports, deserializes the embedded artifact,
and renders meaningful HTML using the rootless runtime. Failures assert that no
output directory was written. Feature-enabled all-target facade/config Clippy,
standalone-test Clippy, CLI feature compile check, formatting and diff checks
passed. Ignored real-build/cross-target tests do not establish additional support.

Existing TOML requires no migration; experimental SSR packaging opts in with
`[package] standalone = true, ssr_sdk = '/explicit/sdk/crates/ferrite'` and
`[runtime] backend = 'napi-vm'` (each setting on its own TOML line). SDK
identity/version/provenance checks, transactional replacement, CLI process-level
acceptance, cross-platform releases, framework renderer/hydration profiles and
the rest of the full mission remain unfinished. This tests custom-renderer SSR,
not framework SSR compatibility.

## Embedded standalone SSR scaffold with an explicit SDK

The new library API `package::write_ssr_standalone` validates a versioned renderer
artifact, shell, published styles, explicit SDK crate directory, embedded-assets
selection, URL base, and explicitly selected napi-vm runtime before generating a
Rust application. It embeds renderer bytes and public assets, reuses the existing
rootless artifact adapter and HTTP response conversion, and forwards request
metadata to the real renderer. SDK sources are build-time inputs only. Native
addons and nonembedded SSR profiles fail explicitly. Existing CLI/build guards
remain: this API is experimental and has not yet been wired to a configured SDK
source in standalone CLI packaging. No installed-tool SDK path is inferred.

Generated manifests now declare an independent workspace. Uncompressed embedding
also resolves asset paths from `standalone/src` to the actual output directory.
No configuration or lockfile migration is required by this library addition.

Validation: `cargo test -p ferrite --lib --locked` passed 23 tests with one ignored;
`cargo clippy -p ferrite --all-targets --locked -- -D warnings` passed. The fixture
was generated with `FERRITE_STANDALONE_SSR_FIXTURE` set, then its independent Cargo
manifest was built with `cargo build --offline`. The resulting Linux debug binary
ran from an empty temporary directory with PATH empty. HTTP checks passed for
meaningful GET/POST renderer HTML, status 202 and custom headers, empty HEAD body,
embedded client JavaScript, base redirect, private renderer/outside-base 404s,
and asset POST rejection. This tested a custom compiled renderer, not framework
hydration. Release/cross-platform builds, SDK version/provenance validation,
transactional scaffold replacement, automated generated-binary acceptance,
CLI packaging, and framework SSR/hydration remain unfinished. The complete
framework-support mission remains unfinished.

## Replacement compiled graphs remove omitted module registrations

The embedded graph worker now records supplied graph IDs and removes all prior
graph registrations before registering a validated replacement, including modules
omitted from it. Previously a replacement could accidentally import a dependency
left behind by an earlier graph. Structural validation still precedes mutation.
Registrations from a failed evaluation are tracked for cleanup on replacement.

The graph regression now omits a previously supplied dependency, requires an
evaluation error rather than previous exports, and verifies recovery with a
complete graph. All 18 napi-vm-enabled runtime tests passed; the expanded graph
test passed from the built test binary with PATH empty. Feature-enabled all-target
Clippy with `-D warnings`, changed-file formatting, and diff checks passed.
No public API, configuration, or lockfile migration is needed. This replaces graph
registrations; it does not establish isolation of globals, handles, built-ins,
filesystem module loaders, or concurrent requests. CLI graph wiring, framework
SSR/hydration, workspace/browser/host matrices, and releases remain unfinished.
The full mission remains active.

## Runtime accepts explicitly compiled module graphs

`CompiledModuleGraph` carries a canonical entry and compiled module list, with
validation for duplicate/empty/relative identities and a missing entry. The
`JsRuntime` graph method fails explicitly on unsupported multi-module backends;
single-module graphs can use the existing evaluation contract. Unavailable
backends identify the selected backend in graph errors.

The explicitly enabled napi-vm backend registers supplied modules on its existing
persistent owner thread before evaluating the entry. It removes earlier export
records for these identities before replacement, preventing stale evaluation of
updated supplied modules. The implementation reuses the pinned interpreter's
define_module/remove_module APIs, inspected at revision
0fa987d8860d620cd1008a84f2a17c9b67c495cd. Existing worker budgets, jobs, replies,
shutdown, and runtime selection remain in use; no Node subprocess is introduced.

Validation: 18 napi-vm-enabled runtime tests and three default-feature tests
passed. The compiled dependency graph test executes modules without filesystem
source files, checks replacement, duplicate IDs and missing entries. It also
passed from the built test binary with PATH empty. Feature-enabled runtime
all-target Clippy with `-D warnings`, CLI `cargo check --features napi-vm`,
changed-file formatting, and diff checks passed. No existing backend must
implement the new method immediately; unsupported graphs return actionable errors.

This is low-level graph execution evidence, not framework SSR/hydration support.
CLI adapter wiring still uses its old single-entry compilation path and must
consume shared compiled graphs. Full request isolation, removal of modules absent
from a replacement graph, source-map/URL propagation, graph cancellation/resource
limits, real framework package execution, full workspace/browser/host/release
matrices, and the full mission remain incomplete.

## Development and build share physical server-entry selection

`ferrite-config::resolve_js_server_entry` now owns explicit entry validation and
conventional JavaScript/TypeScript discovery. The builder and CLI SSR adapter
both use it. Development therefore honors explicit `[ssr].entry` with the same
errors as build, and build recognizes the same entry-server extensions as dev:
ts/tsx/mts/cts/js/jsx/mjs/cjs. Historical server.ts/server.js discovery and the
Rust-default sentinel remain. Selection identifies source only; it does not
establish a renderer or runtime capability. The stale dev comment suggesting
shell fallback was removed; explicit SSR initialization errors still propagate.

Discovery tests cover each extension, explicit overrides, and missing explicit
entries. Existing build tests retain override-over-invalid-conventional-entry
and rejection-before-client-output checks. Validation: 28 config tests, 16 CLI
tests, one config doctest, and eight build tests passed. Affected all-target
Clippy with `-D warnings`, formatting, and diff checks passed. Migration: dev now
honors configured physical entries previously ignored; more conventional script
extensions can trigger app server-module compilation. No schema/lock migration.
CLI SSR runtime execution, shared framework compilation in the CLI runtime
adapter, renderers/hydration, full workspace, browser/host matrices, and releases
remain unverified or incomplete. The full mission remains active.

## ModuleRunner invalidates canonical graphs behind recorded aliases

Successful runner URL records now retain their compiled canonical module ID.
Invalidating an imported alias uses that ID, invalidates its importer tree, and
removes every affected URL record, including other aliases and recorded importer
roots. Canonical IDs without their own URL record retain direct invalidation.
Unrelated records and graphs remain intact. Records store IDs, not independently
cached compiled graphs, preserving shared-pipeline validation on every import.

The regression imports two aliases, an importer and an unrelated module, then
checks canonical/importer invalidation, removal of affected aliases/roots,
preservation of the unrelated graph, fresh source through an alias after an edit,
and direct canonical invalidation. All 56 server unit tests passed, with two
explicit-host tests ignored. Server all-target Clippy with `-D warnings`, changed
file formatting, and diff checks passed. No public signature, config, or lockfile
migration is required. Synchronous invalidation of never-imported aliases still
uses its supplied graph ID; async resolution and concurrent runner lifecycle
semantics remain unfinished. Browser/full-workspace/host/release gates were not
rerun. Real SSR/hydration and the full framework mission remain incomplete.

## ModuleRunner delegates cache validation to the shared pipeline

Every ModuleRunner import now goes through `ssr_load_module` and the existing
per-module pipeline cache instead of returning an independently cached graph.
That pipeline validates source and compiler-input state. The runner retains only
successful URL records for its existing cached-URLs/invalidate/clear API; failed
validation removes the URL record and never returns last-good graph output.

Regressions verify source edits without a watcher, a newly broken transitive
dependency, removal of its successful record, and correction/reload. A separate
preprocessing fixture verifies unchanged imports reuse the shared transform
cache and both declared and watched input edits trigger recompilation. These
tests exercise native TS lowering and plugin metadata, not official framework
renderer execution. All 55 server library tests passed, with two explicit-host
tests ignored; server all-target Clippy with `-D warnings`, formatting, and diff
checks passed. No public signature or configuration/lockfile migration is needed.

Imports now validate the dependency graph on every call, while reusing valid
compiled transforms. Browser acceptance, full workspace, compiler-host matrices,
and releases were not rerun. Runtime execution caching, alias invalidation,
concurrent close/import behavior, real SSR renderers/hydration, additional
frameworks, and the full mission remain incomplete.

## SSR graph loading preserves virtual modules and required failures

`ssr_load_module` now compiles plugin-owned virtual dependencies through the
shared pipeline and reports dependency errors with importer context instead of
logging and skipping them. Returned dependency IDs use compiled canonical IDs.
The graph walk still deduplicates cycles. SSR CSS lowering now emits module
exports without DOM style injection or dev-client/HMR imports, while retaining
the compiled stylesheet payload. Cache namespace v14 prevents reuse of earlier
SSR CSS wrappers containing browser behavior.

Regressions cover transitive virtual dependencies and a virtual cycle, a required
virtual syntax error, physical dependency compile failure and correction, and
development/production CSS-module SSR graphs without browser code. These are
graph compilation tests, not framework rendering, runtime cycle execution, or
hydration evidence.

Validation: 53 server and 17 facade unit tests passed, with three unrelated
explicit-host tests ignored; ten build/manifest integration tests passed. Server
all-target Clippy with `-D warnings`, changed-file formatting, and diff checks
passed. A broader workspace gate launched before this increment completed with
489 passed, zero failed, and 53 ignored across 87 targets; it does not replace a
final full-workspace gate after these edits. Browser/host feature matrices and
release checks were not rerun here. Migration: callers relying on incomplete SSR
graphs after required dependency failure now receive an actionable error;
virtual modules are no longer omitted. No lockfile/schema migration is needed.
ModuleRunner cache coherence, real SSR renderers, hydration, additional adapters,
and the full mission remain incomplete.

## Preview serves build output beneath the configured base

Configured preview now strips the build base path before resolving static output
and SPA routes. Root/base-without-slash requests redirect to the trailing-slash
base while preserving their query. Requests outside a non-root base return 404.
Explicit plugin mount/proxy prefixes still run before base handling. Plugin-less
`preview_dir` retains its root-path behavior. Empty/dot bases resolve at root;
absolute URL bases use their URL path. Invalid relative/traversal/query bases
and malformed URL bases fail before the listener starts.

Real build/HTTP regressions cover `/app`, `/app/`, and `/nested/app/`, verifying
generated HTML asset URLs, script/map serving, SPA routes, root redirects and
outside-base rejection. Invalid-base tests verify no listener starts. Existing
CommonJS Chromium/Firefox acceptance now builds at `/app/` and previews through
resolved configuration, checking root redirect, emitted application execution,
interaction, and console/page errors. Eight lifecycle/preview tests and three
CommonJS tests passed. Affected all-target Clippy and the final changed-test
Clippy check with `-D warnings`, formatting, and diff checks passed.

Migration: configured preview is now mounted beneath its base, with a root
redirect for non-root bases; assets outside that base no longer alias build
files. No schema/lockfile migration is needed. CDN-origin/browser delegation,
full relative-base routing, meaningful SSR, generated-profile browser matrices,
full workspace, feature matrices, and releases remain unverified here. The
full framework mission remains incomplete.

## Preview revalidates files that can change between builds

Static preview responses now default to `Cache-Control: no-cache`. The previous
hyphen-in-filename heuristic incorrectly granted year-long immutable caching to
unhashed public files and plugin-mounted files. Filenames alone do not establish
content-addressed identity, including names that resemble a hash. Explicit plugin
headers still override the default policy.

HTTP regressions request a hyphenated SVG, a hash-looking custom JS filename, and
a mounted text file, replace their contents in place, and verify updated bytes
and revalidation headers at the same URLs. Six lifecycle/preview tests and three
CommonJS tests passed, including Chromium/Firefox dev-edit-build-preview flows
with both browsers required. Facade/test all-target Clippy with `-D warnings`,
formatting, and diff checks passed. Migration: preview no longer infers immutable
caching from names; plugins can set an explicit policy when appropriate. No
configuration or lockfile schema migration is needed. Conditional-request/ETag
optimization, generated-profile browser coverage for in-place asset changes,
full workspace, feature matrices, and releases remain unexecuted here. SSR and
the full framework mission remain incomplete.

## Preview reports missing static resources and unsupported methods

Client preview now returns 404 for missing paths with a file extension, missing
plugin-mounted files, and missing routes requested with a non-HTML Accept header.
It previously returned the app HTML with 200 for these requests, masking missing
scripts/styles/maps. Extensionless HTML-compatible routes retain SPA fallback.
Static serving supports GET/HEAD and reports other methods as 405 with Allow;
proxy rules run first and retain their method handling.

Real HTTP tests cover missing JS/CSS/maps/mounted files, JSON requests, successful
HEAD with no body, POST rejection, SPA fallback and plugin headers, and proxied
POST. Six lifecycle/preview tests and three CommonJS tests passed, including real
Chromium/Firefox dev-edit-build-preview interactions with both browsers required.
Facade/test all-target Clippy with `-D warnings`, changed-file formatting, and
diff checks passed. Migration: missing dotted routes and missing mounted paths
now return 404 rather than falling back to index.html; unsupported static methods
return 405. No configuration or lockfile schema migration is required.

This is static client-preview behavior, not SSR rendering. Full Accept quality
negotiation, framework routing contracts, the complete generated-profile browser
suite, full workspace, feature matrices, and releases were not established by
this increment. The full mission remains incomplete.

## Production chunks reference maps relative to their own location

External `sourceMappingURL` comments now use the existing relative URL helper.
Previously `assets/main.js` referenced `assets/main.js.map`, resolving incorrectly
under `assets/assets/`. Regression builds cover default assets, nested chunk
patterns, and hidden maps; each referenced map exists relative to its chunk and
contains valid v3 sources and nonempty mappings. Hidden maps remain uncommented.
No configuration or lockfile migration is required.

Validation: eight build regressions, 18 bundler unit tests, and ten
manifest/shaking/plugin/preview tests passed. Bundler/test all-target Clippy with
`-D warnings`, changed-file formatting, and diff checks passed. Browser source
location accuracy, transform/wrapper map chaining, CSS maps, full workspace,
feature matrices, and release checks were not established by this increment.
SSR rendering, broader adapters, and the full mission remain incomplete.

## Explicit JavaScript server entries override conventional discovery

Production server-module builds now honor `[ssr].entry` for JavaScript/TypeScript
files instead of always choosing a conventional filename. Configured entries
must be project-relative files without parent traversal. Missing paths,
directories, and unsupported entry types produce actionable errors. App builds
validate the selected server entry before writing client output; direct SSR
builds validate it before creating the pipeline server. Conventional discovery
uses files rather than arbitrary existing paths and also recognizes
`src/server.js`. The historical default `src/server.rs` remains the legacy
sentinel for conventional JS discovery; this change does not introduce Rust
server compilation.

The regression creates a valid configured TypeScript server entry alongside an
invalid conventional JS entry, then verifies the app build selects the configured
module. Negative cases cover missing files, directories, parent/absolute paths,
and a custom Rust entry, with no client output on rejection.

Validation: seven build integration tests, 17 facade unit tests, and ten
manifest/shaking/plugin lifecycle/preview integration tests passed; one unrelated
explicit-host unit test ignored. Facade/test all-target Clippy with `-D warnings`,
changed-file formatting, and diff checks passed. Migration: explicitly configured
entries that were previously ignored now control server-module compilation and
fail clearly when invalid. No lockfile migration is needed.

This is compilation and entry-selection evidence, not SSR renderer execution,
hydration, server preview, or standalone SSR support. Browser acceptance, full
workspace, feature matrices, and cross-platform releases were not rerun.
Framework rendering and the full mission remain incomplete.

## Build environment validation and server hook resolver

`Builder::build` now rejects environment names other than `client` and `ssr`
before creating a pipeline server, invoking its hooks, or writing output.
Previously unknown names silently selected client compilation. The CLI parser
enforces the same supported names; named environment-file profiles continue to
use global `--mode`, independently of `--env`.

SSR build plugin contexts now reference the server resolver rather than the
client resolver. A real server-module build regression checks exact SSR resolver
conditions (including Ferrite's `node-compatible` condition, excluding browser)
and server output location. This is module compilation evidence, not proof of
SSR rendering or a Node runtime. The invalid-environment regression uses a
panic-on-configResolved plugin to prove rejection precedes server hooks and
checks that no output is created. CLI tests cover supported environments with a
staging mode and rejection of unknown/case-mismatched names.

Validation: 16 CLI tests and six build integration tests passed. Affected
CLI/facade/test all-target Clippy with `-D warnings`, changed-file formatting,
and diff checks passed. Migration: callers relying on arbitrary environment
names silently selecting client now receive an actionable error; custom build
environments are not implemented. No config or lockfile migration is needed.
Browser acceptance, full workspace, feature matrices, and release checks were
not rerun. Real SSR/hydration, additional adapters, and the full mission remain
incomplete.

## Resolved stylesheet references participate in invalidation

The shared CSS transform now registers filesystem inputs for resolved CSS
`@import`s and local `url()` assets. These registrations flow through existing
request-scoped watches into module dependencies, graph ownership edges, and cache
dependency snapshots. This also applies to direct CSS responses and the new
production extraction path. Previously references changed output URLs without
retaining their filesystem dependency state.

The development/production stylesheet regression now checks imported CSS and SVG
dependencies, graph ownership, stale cache snapshots after an SVG edit, and
invalidation reaching the owning stylesheet. Existing preprocessing-input and
extraction/browser-cache isolation assertions remain. The pipeline namespace
advances from v12 to v13 so previous cached transforms cannot omit these inputs.
No configuration or lockfile migration is needed.

Validation: 51 server library tests passed, two explicit-host tests ignored;
six build/manifest integration tests passed, including missing-input recovery and
shared pre/post transforms. Server all-target Clippy with `-D warnings`, changed
file formatting, and diff checks passed. This verifies dependency metadata and
graph/cache invalidation, not browser image-cache behavior after an asset edit.
Unresolved CSS reference recovery, complete public/virtual asset ownership,
final CSS maps, SSR, additional frameworks, and the full mission remain unfinished.
Browser acceptance, full workspace, feature matrices, and release checks were
not rerun for this increment.

## Production CSS extraction consumes the shared pipeline

Build CSS loading now uses `pipeline_stylesheet_module` instead of reloading raw
source. This runs the same resolve/load, pre transforms, CSS lowering, JavaScript
post transforms, final analysis, graph/watch metadata, and cache stages as module
compilation. Extraction emits the retained stylesheet text and avoids a second
CSS-module scoping pass; its production minification and asset/import rewriting
remain in the existing extractor. Wrapper maps and declared side effects are
retained. The extraction wrapper contains exports rather than runtime style
injection. Its cache identity is separate from the browser wrapper identity.

The bundler retains JavaScript added by stylesheet transforms and its resolved
imports through a new `CssExtract.keep_js` flag. Untouched plain-CSS export
wrappers still produce no JS chunk. A regression checks preprocessing to purple
CSS, a post-hook JS side effect and imported dependency in output, and no browser
style injection/dev-client imports. Existing missing-input recovery assertions
now select the emitted CSS extension explicitly because transformed stylesheets
can also produce JS. The shared-pipeline test verifies extraction/browser cache
isolation while preserving stylesheet metadata.

Validation: server/bundler library tests passed (69 passed, two ignored).
Facade/integration tests passed (72 passed, 36 ignored across 20 targets) with
explicit Chromium/Firefox paths and both browsers required. Six explicit
generated-profile tests passed: 16 Chromium/Firefox flows across Vanilla, React,
Vue, and Svelte JS/TS, preserving staging-mode checks, CSS, interaction, edits/HMR,
syntax recovery, reinstall, build, and Node-free preview. A fresh CLI build,
affected all-target Clippy with `-D warnings`, changed-file formatting, and diff
checks passed. Early regression failures exposed an unwanted plain-CSS JS chunk;
the implementation was corrected to retain only changed/import-bearing wrappers
without relaxing the existing no-chunk assertion.

Migration: direct Rust `CssExtract` literals must provide `keep_js`; choose false
only for wrappers intentionally safe to discard. The shared pipeline now supplies
production extraction, superseding the intermediate limitation below. Final CSS
source-map generation, full stylesheet graph/import metadata parity, SSR rendering,
additional adapters, and the broader mission remain incomplete. Full workspace,
SDK feature matrices, and cross-platform releases were not rerun in this increment.

## Shared pipeline retains compiled stylesheet output

`PipelineModule` now carries a typed `PipelineStylesheet` separately from the
JavaScript wrapper. It preserves CSS before browser URL rewriting, deterministic
CSS-module exports, the scoping flag, and the input source map supplied by
load/pre-transform stages. JavaScript post transforms and CommonJS conversion
retain this payload. Cache serialization and restoration preserve it too; the
pipeline cache namespace advances from v11 to v12 to prevent reuse of output
without stylesheet metadata.

The regression runs in development and production with CSS pre/post plugins.
It checks that preprocessing changes CSS, post hooks change the JS wrapper,
relative asset references and module exports remain in the stylesheet payload,
input maps survive independently of wrapper maps, cache round trips preserve
metadata, and changes to a declared preprocessing input invalidate cached CSS.

Validation: `cargo test -p ferrite-server -p ferrite --lib --locked` passed
68 tests, with three unrelated explicit-host tests ignored. Server/facade
all-target Clippy with `-D warnings`, changed-file rustfmt, and diff checks
passed. No browser, full-workspace, feature-matrix, or release gate was rerun.

Migration: public `PipelineModule` and `CachedTransform` struct literals gain a
`stylesheet` field; the new payload is exported by ferrite-server. Old serialized
payloads can deserialize without it, but the changed cache namespace rebuilds
compiled output. This is an intermediate implementation: production extraction
still uses its existing separate CSS loader and must be connected to the retained
shared-pipeline payload. The input map is not a claimed final CSS source map.
Direct CSS query responses currently do not carry this payload. Full CSS transform
and extraction parity, SSR, and the larger framework mission remain incomplete.

## Production rejects missing relative CSS resources

Production CSS extraction now fails on unresolved relative `url()` assets,
unreadable asset files, and unresolved relative `@import`s. Previously these
errors only produced warnings and left broken references in successful output.
Errors identify the reference and importing stylesheet; unreadable assets also
identify the resolved filesystem path. Existing asset emission and file-URL
hooks are reused. Remote and absolute references retain their current handling.

The regression covers missing SVG and imported CSS, verifies no output is written
on failure in a clean project, creates each input, and rebuilds with the same
builder. It asserts the emitted input contents and hashed reference in the
importing stylesheet. CSS content assertions account for minification while
retaining selector/declaration checks.

Validation: `cargo test -p ferrite -p ferrite-test --locked --
--test-threads=1` with explicit Chromium/Firefox paths and both browsers required
passed: 71 tests, zero failures, 36 ignored across 20 targets. This includes the
new regression, existing CSS/module/import/URL extraction, CommonJS browser
interaction, build shaking, and preview hook tests. Facade/test all-target Clippy
with `-D warnings`, changed-file formatting, and diff checks passed. The first
broader invocation omitted Chromium's executable path and correctly failed the
mandatory browser launch check; the corrected invocation above passed without
changing that assertion.

Migration: missing relative CSS inputs now fail builds rather than warning.
No lockfile/configuration migration is needed. This does not establish public
asset existence validation, complete CSS plugin/map parity, or SSR rendering.
Generated-profile browser acceptance, full workspace, feature matrices, and
cross-platform release checks were not rerun in this increment. The full mission
remains incomplete.

## Client preview separates compiler requirements from runtime requirements

Preview resolves configuration and existing plugin hooks without starting the
configured Vue/Svelte compiler worker. Compilation through `Config::resolve`,
development, and build still requires the explicitly selected compiler host and
installed compiler packages. Foreign plugins retain their explicit host during
preview; an unavailable foreign host remains an error rather than silently losing
preview hooks. Explicit Rust plugin instances retain their existing lifecycle.
No configuration or lockfile migration is required.

Validation: 17 facade unit tests passed, one unrelated real-Node test ignored.
The new test resolves Vue and Svelte preview configurations with a missing Node
executable and no installed packages, while source compilation fails. The foreign
host regression also covers preview resolution. The real HTTP preview test for
hooks, mounted files, and proxying passed. A fresh CLI build, facade/test
all-target Clippy with `-D warnings`, changed-file formatting, and diff checks
passed. All six generated-profile browser tests passed: 16 Chromium/Firefox
flows across Vanilla/React/Vue/Svelte JavaScript and TypeScript, now with an empty
PATH for every preview process. Existing interactions, staging environment flags,
development-code exclusion, CSS, HMR, syntax recovery, and reinstall assertions
remain in place.

This establishes Node-free serving of these built client fixtures, not Node-free
Vue/Svelte compilation or SSR rendering. Meaningful SSR preview, framework
semantic coverage, checker integration, additional adapters, cross-platform
release checks, and the broader mission remain incomplete. The full workspace
and feature matrix were not rerun for this increment.

## Named build modes use production compilation

Builds now separate the environment-file mode from production compilation.
`--mode staging build` loads staging environment values while setting `PROD=true`,
`DEV=false`, and the default `process.env.NODE_ENV` to production. Explicit
CLI, programmatic, and file modes retain their precedence; an unspecified build
mode defaults to production. Production and server transforms lower
`import.meta.hot` to undefined. Both the facade and direct builder enforce
production compilation, including after mutation of the public builder config.

Migration: `CliOverrides` gains `default_mode` and `is_production` fields.
Rust callers using exhaustive struct literals must supply these fields.
`create_builder` rejects an explicit nonproduction compilation request; use
`create_server` for development. Direct-builder compiler adapters must support
production configuration. Existing explicit environment defines retain their
precedence. No lockfile migration is needed.

Validation for this increment:

- Config/facade/build regression command: 45 tests passed, one unrelated real-Node
  test ignored. Tests cover mode precedence, production flags, staging environment
  values, and production removal of development hooks.
- Non-E2E workspace command, `cargo test --workspace --exclude ferrite-e2e
  --locked -- --test-threads=1`: 478 passed, zero failed, 53 ignored across 87
  targets. Ignored tests do not establish support.
- Fresh CLI build and affected config/facade/server/test all-target Clippy with
  `-D warnings` passed. Changed-file rustfmt and diff checks passed.
- Six explicit generated-profile tests passed: 16 Chromium/Firefox flows across
  Vanilla, React, Vue, and Svelte JavaScript/TypeScript profiles. Each now builds
  with staging mode and verifies staging environment values and production flags
  in preview, alongside existing interaction, source-edit/HMR, syntax recovery,
  reinstall, CSS, and development-code exclusion assertions. Native profiles use
  Node-free CLI PATH; Vue/Svelte use their explicitly enabled Node compiler host.

This does not promote any compatibility profile to tested. Framework semantic
coverage, real SSR/hydration, checkers, additional adapters, upstream integration,
and cross-platform release gates remain unfinished. The SDK test suite and
feature matrix were not rerun for this increment.

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

### Instrument imported React hook-only modules

The React post-transform filter now includes JavaScript/TypeScript module
extensions. Parsed direct React imports plus locally declared exported
`use[A-Z0-9]` names select hook instrumentation; package-store/virtual modules
and re-export-only barrels do not establish this ownership. Lowered hook code
uses the same pinned native Oxc signature pass and map chain. Hook-only modules
receive no component acceptance footer, so edits propagate through the existing
HMR graph to importing component boundaries. SSR/production guards remain
before detection. Cache identity advances to `oxc-0.151.0-imported-hooks-v3`.

The conformance fixture retains the plain and local memo/custom-hook cases and
adds a separate typed `hooks.ts` imported by a memo component. A hook-file-only
compatible edit preserves live counter state while visibly changing its value;
a hook-order edit resets the component without document reload. Syntax recovery,
subsequent clicks, build/preview and production Refresh exclusion remain asserted.
This is native client compilation with pinned React/React DOM 19.2.0 and
react-refresh 0.17.0; all CLI processes run without Node on PATH.

Validation:

- Both new explicit Chromium/Firefox imported-hook acceptance tests passed
  (42.20 seconds), output `/tmp/ferrite-imported-hooks-compatible.log`.
- 27 framework and 45 server unit tests passed. Regressions check hook signatures
  without false component boundaries and negative ownership cases. Server SWC
  matrix also passed 45 tests. Outputs `/tmp/ferrite-imported-hooks-unit.log`
  and `/tmp/ferrite-imported-hooks-swc.log`.
- Rebuilt CLI, workspace compilation, framework/server/test all-target Clippy
  with `--locked -- -D warnings`, formatting and diff checks passed.

No lock/config migration or support promotion. General per-file/package framework
ownership and ambiguous JSX rejection remain assigned, as do hook re-export
chains, namespace/alias/dynamic import matrices, mixed exports, anonymous defaults,
other HOC shapes, React generation/checking and real SSR. The full mission stays active.

The complete explicitly executed Chromium/Firefox acceptance suite passed all
18 tests with `--test-threads=2` (230.09 seconds), including all six React cases
and the existing template/plugin/dependency/startup flows. Output:
`/tmp/ferrite-imported-hooks-all-browsers.log`.

### Verify hook alias/star re-exports and barrel retargeting

The React browser conformance helper retains its previous plain, memo and direct
imported-hook cases and adds a typed named-alias re-export followed by a star
barrel. App imports the hook through that chain. Both browsers check compatible
hook edits preserve state and hook-order changes reset the memo component without
reloading the document. An additional barrel edit retargets its alias to a newly
loaded TypeScript hook implementation: the rendered value changes while live
state survives, and retargeting back restores the prior behavior without stale
output. Syntax recovery, later clicks, scope-hoisted build/preview and production
Refresh exclusion assertions remain active. No runtime workaround was required.

Validation: explicitly executed the two Chromium/Firefox barrel tests with
`--test-threads=2`; both passed in 44.51 seconds. Output
`/tmp/ferrite-hook-barrel-retarget.log`. The initial chain-only probe also passed
but is superseded by the retargeting assertions. Test-target Clippy with
`--locked -- -D warnings`, formatting and diff checks passed; output
`/tmp/ferrite-hook-barrel-clippy.log`. The same pinned React/React DOM 19.2.0 and
react-refresh 0.17.0 packages are installed by Ferrite, with install/dev/build/
preview CLI processes running without Node on PATH.

This proves the tested named-alias/star chain, not all hook re-export graphs.
No lock/config migration or support promotion. Cyclic/conflicting re-export
matrices, other namespace/default hook shapes, mixed exports, anonymous default
components, ownership rules, generated React profiles and the full remaining
framework mission stay assigned and active.

### Verify mixed component and constant exports in browsers

A new React fixture exports a component and a constant consumed by its mounting
entrypoint. Compatible component edits with the constant unchanged preserve
counter and document state. Changing the constant from 1 to 2 invalidates the
boundary: the current client reloads the document, the entrypoint observes 2,
and the reset component remains interactive. The same fixture retains signature
reset, syntax-error recovery, scope-hoisted build/preview interactions,
production Refresh exclusion and strict page/console error assertions. Existing
instrumentation handles this case without implementation changes.

Validation: explicitly executed both new Chromium/Firefox tests with
`--test-threads=2`; 2 passed, 0 failed in 43.62 seconds. Output
`/tmp/ferrite-mixed-exports-browsers.log`. Test-target Clippy with
`--locked -- -D warnings` passed; output
`/tmp/ferrite-mixed-exports-clippy.log`. Fixtures use React/React DOM 19.2.0 and
react-refresh 0.17.0 installed by Ferrite, with install/dev/build/preview CLI
processes running without Node on PATH. Formatting and diff checks passed.

This proves unchanged and changed primitive constant behavior for this fixture;
arbitrary mutable exports and broader boundary shapes remain unverified. No
migration or full-profile support promotion. React generation/checking,
ownership rules and the remaining framework mission stay assigned and active.

### Keep React footer bindings local and collision-free

React's plugin footer previously redeclared fixed `RefreshRuntime` and
`__ferrite_refresh_exports__` imports and called the ambient `$RefreshReg$`.
Valid application bindings with those names could conflict with generated imports
or intercept component registration. The plugin now supplies fresh namespace and
runtime bindings selected against the input source, registers via that runtime,
and uses those bindings throughout boundary validation and refresh execution.
Anonymous-default namespace expressions use the selected namespace while named
application bindings are retained. Compiler instrumentation already uses fresh
local registration/signature bindings; the footer now follows that ownership.
The public legacy footer helper retains its preamble-global contract for existing
callers. React cache identity advances to `oxc-0.151.0-local-bindings-v4`.

Every React browser conformance variant now declares the colliding namespace,
runtime name and a throwing local `$RefreshReg$`, requiring compilation, rendering
and Refresh to succeed without invoking the application's binding. The initial
plain Chromium/Firefox run passed the full state/syntax/reset/build/preview flow
(42.90 seconds), output `/tmp/ferrite-refresh-bindings-browsers.log`.

Validation:

- 28 framework unit tests passed. New assertions cover selected anonymous
  namespace expressions and retention of named application bindings. The old
  global-registration string assertion was updated to require the new direct
  runtime registration; browser bindings throw if ambient interception occurs.
  Output `/tmp/ferrite-refresh-bindings-unit.log`.
- The explicitly executed official react-refresh 0.17.0 runtime fixture passed,
  retaining legacy helper export-boundary, signature and anonymous assertions.
  Output `/tmp/ferrite-refresh-bindings-runtime.log`.
- Rebuilt CLI, workspace compilation, framework/test all-target Clippy with
  `--locked -- -D warnings`, formatting and diff checks passed.

No lock/config migration or support promotion. Intrinsic/global shadowing,
additional binding/anonymous component shapes, ownership, generated React
profiles and the full remaining framework mission stay assigned and active.

The complete explicit Chromium/Firefox acceptance run passed all 20 tests with
`--test-threads=2` (275.47 seconds), retaining the existing template/plugin/HMR/
startup flows and all eight React collision variants. Output:
`/tmp/ferrite-refresh-bindings-all-browsers.log`.

### Verify named and anonymous-memo default exports in browsers

The React conformance helper now uses named fixture variants rather than boolean
combinations, retaining the previous plain, memo, imported-hook and barrel cases.
Two additional variants mount a named default function and a default memo object
wrapping an anonymous function. They exercise namespace-based default
registration with the application's colliding runtime/namespace/global-helper
bindings still present. Compatible edits retain counter/document state;
hook-signature changes reset the component without reloading its document.
Syntax-error recovery, subsequent clicks, scope-hoisted build/preview, absence
of production Refresh instrumentation and no page/console errors remain asserted.
Existing instrumentation handled these cases without a runtime workaround.

Validation: explicitly executed all four new default-export Chromium/Firefox
tests with `--test-threads=2`; all passed in 84.16 seconds. Output
`/tmp/ferrite-default-refresh-browsers.log`. Test-target Clippy with
`--locked -- -D warnings`, formatting and diff checks passed; output
`/tmp/ferrite-default-refresh-clippy.log`. Fixtures keep pinned React/React DOM
19.2.0 and react-refresh 0.17.0, installed by Ferrite; install/dev/build/preview
CLI processes run without Node on PATH.

This covers the stated default shapes, including the anonymous memo inner
function, not arbitrary naked anonymous defaults or every HOC shape. No migration
or full-profile support promotion. Mixed exports, additional anonymous/HOC
matrices, ownership rules, React generation/checking and the entire remaining
framework mission stay assigned and active.

### Generate experimental native React client applications

The shared registry now exposes React JavaScript/TypeScript client templates,
pinned to React/React DOM 19.2.0 and react-refresh 0.17.0. Generation writes real
default-exported counter components, JSX/TSX mounting entries, an empty client
mount container, existing public assets/CSS, explicit native framework/automatic
JSX/Refresh configuration and package scripts. TypeScript profiles additionally
install @types/react and @types/react-dom 19.2.0 through Ferrite and select
react-jsx in editor settings. SSR requests remain rejected before generation;
transpilation does not claim checker execution.

The first actual browser attempt failed during creation because framework
configuration still only allowed Vue/Svelte on Node. This regression is retained
in /tmp/ferrite-react-scaffold-browsers.log. Shared configuration now accepts
explicit native React, and the library constructor installs its native plugin
without constructing a Node compiler host. Mixed native/Node owners in this
single-host configuration are rejected with an actionable error. CLI inspect
reports the native plugin rather than an invented official Node adapter.
Doctor exposes the experimental client capability only with installed concrete
React/React DOM/Refresh dependencies at the selected pinned versions; missing,
different or identity-mismatched packages produce errors. Broader host/version
and per-file ownership configuration remains assigned.

The generated-app acceptance helper now checks React state retention rather
than expecting the full reload used by other component templates. In clean
directories both languages execute create (including Ferrite installation),
frozen reinstall, dev rendering/clicks, invalid-source overlay with retained UI,
compatible component recovery preserving the counter, scope-hoisted build and
preview interaction with strict console/page checks. Every React CLI invocation
runs with an empty PATH.

Validation so far:
- Both explicit Chromium/Firefox generated React tests passed, covering four
  browser/language profiles in 84.80 seconds. Output:
  /tmp/ferrite-react-scaffold-browsers-final.log.
- Framework (30), configuration (27) and facade (14) library tests passed;
  one unrelated explicitly enabled Node fixture remains ignored by default.
  Output: /tmp/ferrite-react-scaffold-unit-final.log.
- All-target Clippy for configuration/framework/facade/CLI/test crates passed
  with --locked -- -D warnings. Output:
  /tmp/ferrite-react-scaffold-clippy-final.log.
- CLI build and workspace compilation passed; outputs:
  /tmp/ferrite-react-scaffold-build.log and
  /tmp/ferrite-react-scaffold-workspace.log.

No lock migration or Tested support promotion. These are experimental client
profiles, with no framework SSR/hydration/SSG renderer or integrated checker.
Complete Refresh conformance, CSS/cache/reinstall matrices, per-file ownership,
additional framework adapters, managed integrations and the full remaining
mission stay assigned and active.

Existing generated-template regressions also passed: all four explicit
Chromium/Firefox Vanilla/Vue/Svelte tests, covering twelve browser/language
profiles, completed in 109.05 seconds. This retains frozen/configured-lock,
editor-store, syntax recovery, scoped/generated CSS, build and preview checks.
Output: /tmp/ferrite-react-scaffold-regression-browsers.log. Formatting and
diff checks passed.

The full workspace test run is not green. The first invocation omitted browser
paths and stopped at ci_policy's isolated Chromium child; rerunning that target
with explicit Chromium/Firefox paths passed all six tests. A subsequent workspace
run with both paths and FERRITE_E2E_REQUIRE_BOTH_BROWSERS=1 reached browser.rs:
86 passed, seven failed (Firefox startup 404, three Chromium authentication/CDP
evaluation timeouts, a favicon/request event-order mismatch, runner modes and
missing video attachment). Output:
 /tmp/ferrite-react-scaffold-workspace-tests-final.log.
No assertions or capability guards were weakened. A full workspace rerun with
the same two-browser requirement and --test-threads=2 is still running, output:
 /tmp/ferrite-react-scaffold-workspace-tests-bounded.log.
Its process must be resumed and its terminal result inspected before claiming
workspace validation. Browser-tool failures and broader framework work remain
assigned; focused generated React acceptance is independently green as above.

### Cancel unattended native authentication challenges in acceptance tooling

The resumed bounded workspace run completed with 90 browser tests passing and
three repeatable Chromium authentication failures: Digest without credentials,
page credentials after clearing, and context credentials after clearing. Output:
 /tmp/ferrite-react-scaffold-workspace-tests-bounded.log.
These were native CDP evaluation timeouts, not component/compiler failures.

Chromium page/context initialization now activates the existing Fetch auth pump
even without credentials. The pump answers absent or cleared credentials with
native CancelAuth and configured credentials with ProvideCredentials; clearing
credentials no longer aborts the responder and leaves the browser awaiting an
unattended native prompt. Routing continues to own paused requests while active;
auth-only mode continues unrouted requests. Routing cleanup restores the active
challenge handler, and the pump exits when its page lifecycle is cancelled.
Default setup lives in context page wrapping (including popup adoption), preserving
the raw driver initialization/routing contract and all its existing strict tests.

The Digest browser regression now additionally authenticates, clears credentials
and asserts 401, restores credentials and asserts digest-ok. Existing Basic/page/
context assertions remain unchanged. Firefox authentication stays explicitly
unavailable; both browser executables are required by the test harness, and this
does not advertise Firefox Digest support. The native response contract was
checked against the official Chromium Fetch protocol definition:
https://raw.githubusercontent.com/ChromeDevTools/devtools-protocol/master/pdl/domains/Fetch.pdl

Validation:
- All 303 E2E unit tests passed with unchanged routing/startup/cleanup assertions.
  Output: /tmp/ferrite-auth-cancellation-unit-final.log.
- Both Basic/page/context credential browser tests passed (8.80 seconds), and
  Digest passed (6.71 seconds). The stronger clear/restore Digest regression
  passed in 4.51 seconds. Outputs:
  /tmp/ferrite-auth-cancellation-credentials-final.log,
  /tmp/ferrite-auth-cancellation-digest-final.log and
  /tmp/ferrite-auth-cancellation-digest-clear.log.
- E2E all-target Clippy with --locked -- -D warnings, CLI rebuild, formatting and
  diff checks passed. Outputs: /tmp/ferrite-auth-cancellation-clippy-final.log
  and /tmp/ferrite-auth-cancellation-cli-build.log.
- All 28 explicitly executed framework acceptance tests passed (553.79 seconds),
  retaining both browsers, all generated client languages, React Refresh shapes,
  startup recovery, hook/plugin/watch flows, production interactions and Node-free
  CLI profiles. Output: /tmp/ferrite-auth-cancellation-generated-apps.log.
- The workspace rerun with two threads passed 92 browser tests, including all
  authentication cases. Its remaining runner_modes_and_overrides failure was
  the 600 ms roomy-test success deadline while the separate acceptance run was
  also active. That test passed in isolation with unchanged assertions in 7.81
  seconds. Outputs: /tmp/ferrite-auth-cancellation-workspace.log and
  /tmp/ferrite-auth-cancellation-runner-modes.log.

A fresh complete workspace run is now active with both browser paths,
FERRITE_E2E_REQUIRE_BOTH_BROWSERS=1 and --test-threads=1, after the separate
acceptance run finished. Output:
 /tmp/ferrite-auth-cancellation-workspace-serial.log.
Resume its live process and inspect its terminal result before claiming the
workspace gate passed. No assertions, time budgets or capability guards were
weakened, and no compiler/rendering support promotion or lock migration occurs.
The full remaining framework mission stays assigned and active.

### Honor explicit React selection and remove duplicate Refresh instrumentation

The configured native React path attempted to remove ferrite:react, whereas
the actual adapter identifies itself as ferrite:react-refresh. Consequently the
CLI default instance remained alongside the configured instance. ReactPlugin
now exposes one shared NAME used by library selection and CLI inspect.
An explicit framework selection first removes the default React adapter; selecting
React adds exactly one configured instance, and selecting no frameworks leaves
none. An absent framework selection preserves the supplied default adapter.
Doctor now reports full-reload when React Refresh is explicitly disabled, and
does not require react-refresh for that disabled profile.

The new library regression checks exact adapter counts, and a Node-free CLI
regression creates/installs a real React project, compares inspect with transformed
output, requires exactly one createSignatureFunctionForTransform occurrence,
toggles refresh off and requires zero occurrences plus full-reload reporting,
then disables framework selection and checks both inspect and transform again.
The ordered plugin cache identity includes instance count, so removal of duplicate
instances changes compilation cache keys; no manual cache or lock migration is
needed. Inspect now exposes only the actual ferrite:react-refresh identifier.

Workspace validation also exposed outdated conformance expectations left behind
by earlier implementation changes. Tests now enforce the existing pipeline's
disabled-remote rejection with the explicit opt-in hint; Vue/Svelte without a
configured official compiler host reject main and legacy split-query requests
with actionable compiler-package/host errors. React structural tests require
local native registration/signature helpers and prohibit ambient registration
calls. Their resolver-only package fixtures do not establish runtime support.
The old CommonJS test expected the removed interop helper and used require(ESM)
as a positive fixture. It now checks a real CommonJS dependency's lazy factory
edge and named/default facade; a separate negative regression follows the actual
require(ESM) dependency factory and requires the unsupported-operation/import
hint. No runtime fallback, compiler stub or weakened success assertion was added.

Validation:
- Facade (15) and framework (30) unit tests passed; the unrelated opt-in Node
  fixture remains ignored by default. Output:
  /tmp/ferrite-react-selection-unit-final.log.
- The explicit CLI selection/inspect/transform/doctor regression passed in 4.82
  seconds. Output: /tmp/ferrite-react-selection-cli.log.
- Both generated React browser tests passed all four browser/language flows in
  94.10 seconds, including state preservation, syntax recovery, frozen install,
  build and preview, with Node absent from CLI PATH. Output:
  /tmp/ferrite-react-selection-browsers.log.
- Configuration/resolver (5), structural framework (6) and transform (6) tests
  passed. Outputs: /tmp/ferrite-react-selection-config-resolver.log,
  /tmp/ferrite-react-selection-framework-conformance.log,
  /tmp/ferrite-react-selection-transforms-final.log.
- The previous full serial workspace run verified all unchanged E2E crate
  unit/integration targets: 585 passed, zero failures/ignored in 52 targets,
  including the formerly timing-sensitive runner test. That run later stopped
  at the obsolete remote expectation. Output:
  /tmp/ferrite-auth-cancellation-workspace-serial.log.
- Current remaining workspace validation, with E2E excluded because its unchanged
  unit/integration targets were already executed above, passed 465 tests, zero
  failures, 52 ignored in 87 targets. Exact command:
  FERRITE_CHROMIUM_PATH=/home/meme/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome FERRITE_FIREFOX_PATH=/usr/bin/firefox FERRITE_E2E_REQUIRE_BOTH_BROWSERS=1 cargo test --workspace --exclude ferrite-e2e --locked -- --test-threads=1
  Output: /tmp/ferrite-react-selection-workspace-complete.log. Ignored/unexecuted
  feature/external-tool profiles do not establish support.
- All four E2E documentation tests passed separately. Output:
  /tmp/ferrite-react-selection-e2e-doc.log.
- CLI rebuild, affected facade/framework/CLI and test all-target Clippy with
  --locked -- -D warnings, formatting and diff checks passed. Outputs:
  /tmp/ferrite-react-selection-build.log,
  /tmp/ferrite-react-selection-clippy-final.log,
  /tmp/ferrite-react-selection-test-clippy-final.log.

This fixes standard adapter selection and reporting, not complete per-file/
package JSX ownership or ambiguity enforcement. Experimental support labels,
SSR/checker unavailability and the full remaining framework mission remain
unchanged and active.


### Explicit JSX ownership at shared lowering (2026-10-03)

Implemented a shared-pipeline rejection for `.jsx`/`.tsx` modules when explicit
`framework.enabled` excludes React. Empty, Vue-only and Svelte-only selections
now produce a module-specific error with the native React configuration and
plugin pre-lowering alternatives. They cannot silently receive React lowering.
Framework plugins that supply JavaScript before core lowering remain valid;
plain JS/TS continues to compile. This covers both client/server compilation
and development/production. It does not establish an SSR rendering profile.

Migration: projects explicitly disabling React while relying on implicit React
JSX lowering must select `enabled = ["react"]`, `compiler_host = "native"`, or
provide a plugin that lowers their JSX into JavaScript. Legacy projects without
explicit framework selection keep their existing lowering behavior. No lockfile
or registry schema migration. Per-package/file ownership, manifest ambiguity
detection and other JSX framework adapters remain assigned work.

Validation: seven shared transform tests passed, including the new multi-target
ownership regression; 45 server unit tests passed, two external-tool tests were
ignored and do not establish support. CLI create/install/inspect/transform
selection regression passed with Node absent from CLI PATH. Fresh CLI build and
affected server/test all-target Clippy with `--locked -- -D warnings` passed.
Logs: /tmp/ferrite-jsx-owner-transforms.log, /tmp/ferrite-jsx-owner-server.log,
/tmp/ferrite-jsx-owner-cli.log, /tmp/ferrite-jsx-owner-build.log,
/tmp/ferrite-jsx-owner-clippy.log.

Both generated React browser tests passed (Chromium and Firefox, JS and TS
profiles, four complete flows). They exercise real interaction, state-preserving
HMR, syntax-error recovery, build and preview with Node absent from Ferrite CLI
PATH. Output: /tmp/ferrite-jsx-owner-browsers.log. Formatting of changed Rust
files and diff checks passed. No full workspace rerun or new support promotion
in this increment; the broader mission remains active.


### Package-local inferred JSX ownership (2026-10-03)

Implemented direct dependency/devDependency/peerDependency JSX owner markers
in the shared framework registry, including unavailable Preact/Solid/Qwik
markers without adding supported adapter descriptors. Shared lowering examines
the nearest filesystem package.json for each JSX/TSX module. Multiple markers
produce an actionable ambiguity error; a sole unavailable marker fails instead
of receiving implicit React compilation. Explicit framework selection, JSX
import source or custom factory overrides inference. Package-local boundaries
prevent a root React dependency from claiming a nested Preact package. Plugins
that pre-lower to JavaScript do not enter core JSX inference.

Manifest candidates, including absent nearer package.json paths, participate in
transform dependency snapshots before cache lookup. Regression tests execute a
successful cached transform, change its owning manifest, and create a nearer
manifest, then verify neither request returns stale React output. No lockfile
or registry schema migration. Previously implicit JSX in ambiguous or sole
unavailable-owner packages now requires explicit selection or pre-lowering.

Validation: `cargo test -p ferrite-frameworks -p ferrite-server -p ferrite-test
--lib --test transforms --locked` passed 31 framework units, 45 server units and
8 transform integration tests. Two server external-tool tests remained ignored;
they establish no support. Affected framework/server/test all-target Clippy
with `--locked -- -D warnings`, changed-file formatting and diff checks passed.
Fresh CLI builds passed. Logs: /tmp/ferrite-jsx-inference-tests.log,
/tmp/ferrite-jsx-inference-clippy.log, /tmp/ferrite-jsx-inference-build.log.

Both Chromium/Firefox generated React tests passed four JS/TS create/install/dev/
interaction/HMR/recovery/build/preview flows in 88.34 seconds, with Node absent
from CLI PATH. Browser execution used the ownership implementation before its
unchanged marker logic was extracted into the shared registry; final registry
and pipeline tests were rerun afterward. Log:
/tmp/ferrite-jsx-inference-browsers.log. No full-workspace rerun or support
promotion in this increment. Source pragmas, explicit per-file ownership maps,
virtual-module ownership, inspector/doctor inference parity, unrecognized
ecosystems and unavailable framework compilers remain incomplete. The full
framework mission remains active.

The rebuilt final CLI also passed the explicitly executed create/install/
inspect/transform selection regression (4.32 seconds), with Node absent from
CLI PATH. Output: /tmp/ferrite-jsx-inference-cli.log.


### Recover first-load ownership failures from compiler inputs (2026-10-03)

Core-lowering failures now retain load/pre-transform dependencies and registered
watch files as graph inputs, using the existing failed-import tracking and
successful-transform cleanup. Previously a failed first JSX transform lost its
package manifest before graph recording; correcting the manifest alone could
leave the browser waiting indefinitely. Failed input edges now invalidate
the owner and run the normal HMR preflight. Successful compilation removes
the temporary tracking while retaining current declared dependencies. No
fallback output, lockfile migration or capability promotion was introduced.

A real HTTP/live-notify regression begins with a 500 for an ambiguous JSX
owner and separately malformed package.json, confirms the retained manifest
edge, corrects only that manifest, receives a full-reload message, checks the
cleared diagnostic, fetches successfully compiled output, and verifies
temporary tracking was removed. This tests compilation recovery, not framework
rendering or SSR support.

Validation: `cargo test -p ferrite-server --lib -p ferrite-test --test transforms
--test config_resolver --test frameworks --locked` passed 46 server tests and
19 integration tests (5 config, 6 structural framework, 8 transform). Two
server external-tool tests were ignored and establish no support. Affected
server/test all-target Clippy with `--locked -- -D warnings`, changed-file
formatting and diff checks passed. Outputs:
/tmp/ferrite-jsx-owner-recovery.log,
/tmp/ferrite-jsx-owner-recovery-tests.log,
/tmp/ferrite-jsx-owner-recovery-clippy.log.

Pre/post-hook failures and map-validation failures still need equivalent
first-load input retention; concurrent transforms and multiple diagnostics
remain incomplete. No browser acceptance or full-workspace rerun in this
increment. The broader framework implementation mission remains active.


### Retain inputs across resolved pipeline failures (2026-10-03)

Extended first-load failure retention from core lowering to the resolved module
pipeline: load hooks, cache hooks, pre/post transforms, map chaining and final
analysis now share one error boundary. Loaded dependencies enter the per-request
watch context immediately, and each transform result declares its dependencies
before map chaining or another plugin can fail. Errors retain those inputs as
graph edges using the existing recovery tracking. Relative inputs resolve from
the project root, and external inputs register extra filesystem watches.
Successful transforms retain the normal pipeline/cleanup/cache behavior. No
compiler fallback, output substitution or capability promotion was added.

A live HTTP/notify regression exercises five distinct failures (load, pre, post,
invalid chained map, invalid final JavaScript), including a dependency returned
by an earlier plugin and a relative load-hook watch. Every case checks the 500,
retained input edge, correction of only that input, reload message, cleared
diagnostic, valid compiled output and removal of temporary failure tracking.

Validation: affected plugin/server/test units and config/framework/transform
integration tests passed: 19 plugin units, 47 server units, 19 integration
tests. Ten real-Node plugin tests were then explicitly executed and all passed;
no remaining ignored tests establish support. The pinned real Vue 3.5.22 and
Svelte 5.39.6 compiler fixture also passed its client/server compilation and
runes checks; server compilation is not SSR rendering evidence. Two unrelated
server external-tool tests remained ignored. Affected all-target Clippy with
`--locked -- -D warnings`, changed-file formatting, fresh CLI build and diff
checks passed. Logs:
/tmp/ferrite-pipeline-failure-inputs-stages.log,
/tmp/ferrite-pipeline-failure-inputs-tests.log,
/tmp/ferrite-pipeline-failure-inputs-node.log,
/tmp/ferrite-pipeline-failure-inputs-compilers.log,
/tmp/ferrite-pipeline-failure-inputs-clippy.log,
/tmp/ferrite-pipeline-failure-inputs-build.log.

Resolution-hook failure retention, filesystem watch failures/missing external
inputs, asynchronous/concurrent pipeline races, post-graph lifecycle hook
failures and multiple diagnostics remain incomplete. The broader mission
remains active.

All six explicitly executed generated client browser tests passed in 155.69
seconds: Vanilla/React/Vue/Svelte, JS/TS, Chromium/Firefox (16 profile flows).
They execute create/install, dev interaction, edits/HMR or invalidation, syntax
recovery, build and preview interaction; native profiles exclude Node from CLI
PATH and Vue/Svelte use explicitly enabled Node compiler hosts. Output:
/tmp/ferrite-pipeline-failure-inputs-browsers.log. No full-workspace or
cross-platform rerun and no promotion from experimental support in this
increment.


### JSX ownership parity in inspect/doctor (2026-10-03)

Implemented shared registry ownership classification and package-manifest
search used by the compilation pipeline and developer tooling. Doctor/inspect
scan physical project JSX/TSX files, group them by nearest package manifest,
report declared markers, selected lowering and explicit/inferred ownership,
and distinguish ambiguous/unavailable/unowned input from custom unverified
lowering. The scan excludes source symlinks, package stores, node_modules,
generated output and public directories; its limited scope and unverified
package/host/plugin execution are included in the output. No compiler, Node
worker or plugin executes during the ownership scan.

Doctor emits actionable errors for standard native ownership conflicts.
Configured foreign-plugin lowering remains unverified and changes those
conditional native conflicts to warnings rather than claiming a working
adapter. Explicit framework/JSX settings override dependency inference;
malformed nested manifests are separately reported when explicit selection
applies, matching the pipeline's precedence without hiding the manifest
problem. Inspector exposes the same per-package ownership rows.

Migration: registry/report schema is now 5. Doctor JSON adds
jsx_ownership_scope/ jsx_ownership; inspector JSON adds
frameworkRegistrySchema/ jsxOwnershipScope/ jsxOwnership. Ownership rows
contain manifest, files, manifest_error, selection, status, declared_owners
and lowering. Consumers must recognize schema 5 and keep ownership selection
separate from tested compiler/rendering capability. No lockfile migration or
support promotion.

Validation: 33 framework and 47 server unit tests passed, including nested
peer-marker ownership, deterministic file grouping, generated-output exclusion,
symlink exclusion, explicit disabled/selected/custom policies and malformed
manifest precedence. Two server external-tool tests remained ignored and
establish no support. Nineteen config/framework/transform integration tests
passed. Two real CLI tests were explicitly executed with Node absent from CLI
PATH: inspect/doctor/transform conflict parity, and generated React
create/install/configured selection parity. Both passed. Fresh CLI build,
affected framework/server/CLI/test all-target Clippy with `--locked --
-D warnings`, changed-file formatting and diff checks passed. Outputs:
/tmp/ferrite-jsx-doctor-tests.log,
/tmp/ferrite-jsx-doctor-integrations.log,
/tmp/ferrite-jsx-doctor-cli-conflicts.log,
/tmp/ferrite-jsx-doctor-cli-selection.log,
/tmp/ferrite-jsx-doctor-build.log,
/tmp/ferrite-jsx-doctor-clippy.log.

No browser or full-workspace rerun in this tooling increment. Source pragmas,
per-file explicit mappings, virtual/linked source ownership, unrecognized
ecosystems, graph-reachable-file scanning and complete package-specific
installed/compiler capability checks remain incomplete. SSR/checkers, all
additional/upstream adapters and the broader framework mission remain active.


### External compiler-input watch recovery and graph roles (2026-10-03)

Implemented live watcher access for reconstructed HTTP/HMR handles through a
weak shared reference, avoiding a watcher/callback/inner ownership cycle.
Extra-file watches now attach to existing parent directories (or the nearest
existing ancestor of a missing tree), survive file replacement, and deduplicate
registered anchors. Relevant directory creation/removal events invalidate
tracked descendant inputs; unrelated events outside the project are filtered.
Actual filesystem paths remain the plugin watch-event payload. Closing the
server clears watcher registrations.

Added graph watch_input metadata so declared compiler/type/configuration inputs
invalidate their owners without being lowered as runtime application modules.
Actual imports and successful module compilation promote the same input to
a runtime module, preserving syntax validation. External absolute paths now
use distinct /@fs identities and round-trip through core URL helpers, including
lexical dot-segment normalization; this does not enable external module serving.

Migration: graph JSON adds default-false watch_input; older snapshots deserialize
with existing runtime behavior. Pipeline cache identity advances from v10 to
v11. External file_to_url results change from ambiguous project-looking paths
to /@fs identities. No lockfile or framework registry schema change.

Validation: real HTTP/notify tests begin with a failed first load and missing
external file or directory tree, reject unrelated external notifications, then
create only the declared input and verify reload, cleared diagnostics and valid
output. Further tests prove shared watcher identity/no reference cycle and
watch-only invalid JavaScript remaining data until an actual import, after
which syntax errors correctly fail preflight. Core external URL/dot-segment
round-trip tests passed. Scoped units passed 6 core, 7 graph and 50 server tests.

The final non-E2E workspace gate passed 476 tests, zero failures, 53 ignored in
87 targets using both browser paths and --test-threads=1. Those ignored tests
establish no support. The 303 E2E unit tests passed separately; SDK browser
integration tests were not rerun in this increment. All six explicitly run
generated client browser tests passed 16 Vanilla/React/Vue/Svelte JS/TS flows
in Chromium/Firefox in 159.91 seconds. Browser execution used the new watcher/
graph implementation before final lexical path normalization; its ordinary
project paths are unchanged, and final core/server/workspace tests were rerun
after normalization. Native flows exclude Node from CLI PATH; official SFC
flows explicitly enable their Node compiler. Fresh final CLI build and
ownership parity regression passed. Affected all-target Clippy with
--locked -- -D warnings, changed-file formatting and diff checks passed. Logs:
/tmp/ferrite-missing-external-watch-recovery.log,
/tmp/ferrite-missing-external-watch-tests.log,
/tmp/ferrite-missing-external-watch-workspace.log,
/tmp/ferrite-missing-external-watch-e2e-units.log,
/tmp/ferrite-missing-external-watch-browsers.log,
/tmp/ferrite-missing-external-watch-build.log,
/tmp/ferrite-missing-external-watch-cli.log,
/tmp/ferrite-missing-external-watch-clippy.log.

Environment-specific graph roles, directory dependency snapshots/replacement,
watch-registration failure propagation, stale/orphan watch cleanup, URL encoding
and linked external runtime-module serving remain incomplete. Linux was
executed; other-platform release checks were not. No support promotion; the
full framework mission remains active.

### Shared SSR graph payload and adapter consumption

- Added `DevServer::ssr_compile_graph`: returns the entry and transitive compiled pipeline modules, retaining final imports, source maps, CSS, watched inputs, and diagnostics through the existing shared pipeline. `ssr_load_module` now delegates to this traversal; canonical-ID deduplication handles aliases in cycles.
- Added `JsSsrAdapter::from_resolved_graph`, validating explicit compiled graphs and evaluating them through the selected runtime's graph API. Existing single-entry constructors remain available; no runtime substitution is introduced.
- Validation: `cargo test -p ferrite-server --lib --locked`: 56 passed, two explicitly ignored. `cargo test -p ferrite-ssr --features napi-vm --locked`: 14 passed, none ignored. The actual embedded adapter test imports a compiled-only dependency and verifies rendered HTML and shell preload injection; the server CSS test verifies graph payload retention in development and production.
- Remaining: CLI use of this graph, canonical/virtual runtime linking, dev revalidation, framework renderers/hydration, and request isolation. These API tests do not establish a framework SSR compatibility profile.

### CLI SSR uses the shared compiled dependency graph

- `ferrite dev --ssr` now awaits the shared SSR graph pipeline instead of reading and lowering only the physical entry. Compiler/plugin transforms and transitive dependency failures participate in initialization; the selected runtime consumes the compiled graph without a Node fallback.
- Added `DevServer::ssr_runtime_graph` to map internal virtual IDs into the resolved IDs already used by final import specifiers. Compilation metadata and canonical IDs remain available through `ssr_compile_graph`; runtime payload validation rejects conflicting identities.
- Validation: feature-enabled CLI tests: 17 passed. The actual embedded SSR adapter test compiles TypeScript entry/dependency sources, renders meaningful HTML, rejects a broken dependency, and recovers after correction. That exact test also passed with `PATH=''`. Server tests: 56 passed, two explicitly ignored; virtual dependency cycles assert matching runtime IDs and rewritten import specifiers. Clippy across CLI/server/SSR with napi-vm and all targets passed; changed-file formatting and diff checks passed.
- Limitations: the CLI adapter still captures an initialization graph; automatic dev revalidation, per-request isolation, framework renderer APIs/hydration, CSS delivery and complete source-map propagation remain unfinished. This establishes shared pipeline wiring, not complete framework SSR support.

### Development SSR revalidates each request

- CLI development SSR now revalidates the shared compiled graph before every request, using pipeline caches for unchanged sources. The same configured runtime worker is retained; graph replacement, evaluation, and render invocation are serialized together.
- Added validated adapter graph replacement and a non-owning server callback handle. The stored SSR callback avoids retaining a server/adapter ownership cycle; requests fail explicitly after owning server handles disappear.
- Validation: CLI/server/SSR feature-enabled tests: 17 + 56 + 14 passed, two server tests explicitly ignored. The same adapter now observes a dependency syntax error and recovers on the next render after correction without watcher notifications or adapter reconstruction, then rejects rendering after server ownership is dropped. The exact embedded regression also passes with an empty PATH. All-target Clippy and changed-file formatting passed.
- Limitations: this regression invokes the actual adapter rather than the HTTP transport. Serialization prevents graph/evaluation interleaving but does not isolate persistent guest globals or implement framework hydration, streaming, CSS delivery, or SSR preview/standalone execution. Complete framework support remains unfinished.

### SSR HTTP request and response contracts

- Extensionless application routes, including `/`, now reach the configured renderer before static index fallback. Proxy rules still run first; asset requests retain the shared pipeline.
- Forward actual request method, headers, URI/query, and body to the adapter/context. Preserve renderer HTTP status and response headers, including multiple Set-Cookie values; reject invalid status/header values instead of silently substituting. HEAD sends an empty response body. Full and streaming bodies use the same validated response metadata.
- Validation: server tests: 57 passed, two explicitly ignored; feature-enabled CLI tests: 17 passed. Real TCP tests cover root SSR despite an existing index.html, GET queries, POST payloads, request headers, custom status/headers, multiple cookies, HEAD, and JS asset serving. The persistent embedded CLI adapter now renders root HTML through HTTP, reports dependency compilation failure as HTTP 500, and recovers as HTTP 200 after correction without reconstruction. This exact HTTP regression also passed with an empty PATH. All-target Clippy and changed-file formatting passed.
- Remaining: framework renderer/hydration contracts, guest request isolation, HTTP stream cancellation/backpressure conformance, SSR CSS delivery, and preview/standalone renderer execution. The current route classification treats extensionless paths as app routes; dotted application routes require a future explicit routing policy.

### JavaScript renderer HTTP metadata contract

- JavaScript `render(url, request)` retains the existing URL argument and now receives `{ method, uri, headers: [[name, value], ...], body: [byte, ...] }` as its second argument. Raw body bytes survive the bridge without lossy text conversion.
- Accept legacy HTML strings or structured `{ html, status?, headers?: [[name, value], ...] }` responses. Status must be finite, integral, and within 200..599. Unknown result fields and malformed shapes fail explicitly; streaming is not inferred from JSON. Shell injection retains custom status/headers and duplicate cookies; explicit content type overrides the HTML default.
- Validation: feature-enabled CLI and SSR tests: 17 + 16 passed, none ignored. Real embedded renderer tests cover request bytes, method/URI/headers, structured HTTP metadata and cookies, malformed statuses/headers, missing HTML and unsupported stream fields. The real CLI HTTP regression sends binary POST data and receives renderer status 202, a custom header and rendered HTML; it also passes with an empty PATH. All-target Clippy and changed-file formatting passed.
- Remaining: framework runtime APIs, hydration, guest request isolation, JavaScript streaming/cancellation, production SSR execution and related full compatibility gates. No new framework profile is promoted by these contract tests.

### Load and execute emitted SSR chunks

- SSR builds now write `server/manifest.json` with output entry identities in addition to the existing SSR module-to-chunk manifest.
- Added public `load_built_ssr_graph(server_dir)`: selects exactly one renderer entry, reads its emitted static/literal-dynamic chunk closure, resolves relative chunk imports to explicit runtime IDs, and validates the graph. Missing chunks, external host imports, unsafe output paths, and symlinks outside server output fail explicitly. It uses emitted JavaScript without compiling project sources or spawning a compiler host.
- Validation: feature-enabled facade tests: 20 passed, one explicitly ignored; existing production manifest integration tests: two passed. A real TypeScript server build renders emitted HTML through napi-vm with scope hoisting both enabled and disabled; changing source to throw after the build does not affect output. The exact build/render test also passed with an empty PATH. Invalid/missing/external chunk and output-escape regressions pass. All-target Clippy and changed-file formatting passed.
- Remaining: wire this emitted-graph API into preview/standalone, define explicit external-runtime profiles and artifact versioning, preserve/chains maps during runtime import rewriting, verify nonliteral dynamic imports and framework renderer/hydration semantics. No framework SSR profile is promoted by this test.

### Preview executes emitted SSR renderers

- Configured preview now loads an emitted server manifest/graph and renders extensionless application routes, including the base root, using the explicitly selected napi-vm runtime. Explicit SSR entries or built server artifacts never silently degrade to static-shell preview when the runtime/artifact is unavailable. Framework compiler hosts are not started.
- Preview retains proxies, plugin mounts, base boundaries, client assets and GET/HEAD restrictions for static/mounted assets. SSR accepts request methods/body/headers and uses the shared validated HTTP response conversion. Server output URLs return 404 rather than exposing renderer code/manifests. Renderer invocations are serialized on a persistent runtime.
- Validation: feature-enabled facade tests: 21 passed, one explicitly ignored; existing preview/lifecycle integration tests: eight passed. Real emitted client+SSR builds run preview under `/app/` with scope hoisting both enabled and disabled, serve meaningful root HTML and POST-route HTML after source is changed to throw, and reject server artifacts/outside-base requests. Final focused preview and static regression reruns passed; the actual build/render/HTTP-preview test also passed with an empty PATH. All-target Clippy and changed-file formatting passed.
- Remaining: standalone SSR packaging, request isolation, framework hydration/rendering conformance, streaming cancellation, SSR CSS/preload completeness, richer routing and multi-entry selection, external-runtime contracts and artifact/map versioning. These custom-renderer tests do not establish framework SSR support.

### Published SSR styles and browser assets

- SSR builds publish extracted CSS and loader-emitted browser assets into `dist/ssr-assets/`, retaining their output subpaths. Server asset URLs use the configured base plus this public prefix; renderer JavaScript/maps remain private.
- Preview loads the server entry's stylesheet list, preserving manifest order and deduplicating files, and injects those links through SSR preload context. Missing published styles fail before preview listens with a rebuild instruction.
- Validation: feature-enabled facade tests: 22 passed, one explicitly ignored; client production-manifest and preview/lifecycle integration tests: 2 + 8 passed. Real client+SSR builds with both bundle strategies serve a server-only stylesheet and its SVG dependency through HTTP under `/app/ssr-assets/`; rendered shell links refer to that stylesheet while server artifacts stay private. The exact build/render/HTTP test passed with an empty PATH. Stylesheet ordering/deduplication/missing-file tests and all-target Clippy/format checks pass.
- Remaining: development SSR stylesheet delivery, dynamic component style/preload tracking, framework-specific CSS/hydration conformance, source-map composition, request isolation, standalone packaging and full release gates. These custom-renderer fixtures do not promote framework SSR profiles.

### Development SSR initial stylesheet delivery

- Added `ssr_runtime_graph_with_styles`, retaining the compiled graph plus direct-CSS URLs in dependency order. CLI development SSR passes those URLs through request preload context during graph revalidation.
- Shell preload classification now handles query/fragment-bearing CSS/JS URLs and escapes HTML attribute values. Direct CSS uses the shared existing CSS pipeline, preserving CSS-module scope while excluding DOM/HMR JavaScript from stylesheet responses.
- Validation: feature-enabled CLI/server tests: 18 + 57 passed, two server tests explicitly ignored; SSR tests after the preload regression: 17 passed. A real running embedded SSR HTTP fixture confirms that rendered CSS-module class names match linked direct CSS, then changes color and observes fresh HTML/CSS on the same server without watcher notifications. This exact HTTP test also passes with an empty PATH. Query preload classification and attribute escaping regressions pass; all-target Clippy and changed-file formatting pass.
- Remaining: browser HMR behavior for server-only styles, granular generated-style ordering/dynamic component tracking, framework hydration/request isolation, standalone SSR packaging, streaming conformance and full compatibility gates. No framework SSR profile is promoted by this HTTP fixture.

### Fresh embedded guest state for SSR evaluations

- napi-vm now honors `RuntimeEnvironment.ssr`: each SSR entry/graph evaluation constructs a fresh interpreter on the existing worker thread, retiring guest globals, prototypes, module caches and function tables. Configured fuel/loop budgets and filesystem-loader options are reapplied. Non-SSR evaluations retain their previous worker semantics.
- Function handles remain monotonically allocated across fresh interpreters, preventing retired handles from aliasing the next request's functions. `JsSsrAdapter` serializes its own evaluation/invocation pair, including direct library use. Native addons are rejected for this guest-isolation profile because native process state lacks a validated request-isolation contract.
- Validation: feature-enabled runtime/SSR/CLI tests: 19 + 18 + 18 passed, none ignored. Twelve concurrent requests through one actual embedded adapter each start with a fresh global counter and no previous Object.prototype mutation. Retired function handles fail while the fresh function still executes. Development HTTP regressions and the emitted build/HTTP-preview regression pass. The concurrent guest-isolation test also passes with an empty PATH. All-target Clippy/format/diff checks pass.
- Limits: this is guest-state isolation, not process sandboxing or native-addon isolation. Independent adapters sharing one runtime can still interleave separate evaluation/call operations and need an atomic runtime request contract. Cancellation, async resource teardown, host-side/global resources and full framework SSR conformance remain unfinished; no framework profile is promoted.

### Atomic compiled graph invocation for shared SSR workers

- Added explicit `JsRuntime::invoke_module_graph`: graph evaluation and entry-export invocation are one runtime operation. The default implementation reports unsupported capability; unavailable backends retain their requested name in actionable errors rather than composing unsafe separate jobs.
- napi-vm implements invocation as one worker job, including graph validation, fresh SSR guest state, registration, export validation, invocation and async result settlement. `JsSsrAdapter` uses this operation for both single-module and graph constructors. Independent adapters sharing one runtime no longer interleave evaluation and invocation.
- Validation: feature-enabled CLI/runtime/SSR tests: 18 + 19 + 19 passed, none ignored. Twenty-four independent adapters share one worker and the same entry ID with different async renderer code; each completes three requests with its correct owner/URL and fresh guest state. The exact shared-worker regression passes with an empty PATH. Default runtime/SSR tests: 3 + 13 passed; emitted build/HTTP-preview regression passes. All-target Clippy and changed-file formatting pass.
- Remaining: cancellation, queue/resource bounds, prompt teardown of abandoned async work, native/process isolation, framework runtime/hydration conformance, standalone SSR and full acceptance/release matrices. This atomic guest execution does not imply process sandboxing or complete framework support.

### Bounded embedded worker queues and abandoned queued requests

- napi-vm uses a bounded, nonblocking job queue. `NapiVmOptions.queue_capacity` selects capacity (zero = 64); full queues fail with an actionable retry/configuration error instead of accumulating requests or blocking an executor thread.
- `max_request_bytes` bounds encoded queued request payloads (zero = 8 MiB), including graph IDs/code/source URLs, export names and encoded arguments. Oversized requests and disconnected workers fail explicitly.
- The worker skips jobs whose reply receiver was dropped before execution, preventing abandoned queued requests from evaluating guest code. This does not interrupt work already executing.
- Validation: feature-enabled CLI/runtime/SSR tests: 18 + 21 + 19 passed, none ignored. Queue overload, oversized payload and disconnected-worker tests pass. A real embedded interpreter test confirms an abandoned queued job's global side effect never executes; that exact test also passes with an empty PATH. Final focused runtime rerun, all-target Clippy and formatting/diff checks pass.
- Remaining: in-flight cancellation/deadlines, hard heap/host resource limits, graceful bounded shutdown for executing work, exposing queue/payload tuning through resolved runtime configuration, compiler-host resource matrices, framework hydration and standalone SSR. Payload accounting is not a hard interpreter memory limit; no broader host-isolation capability is claimed.

### Resolved runtime worker limits and diagnostics

- Added `[runtime].queue_capacity` and `[runtime].max_request_bytes` to TOML/JSON configuration, configuration merging and resolved settings. Zero retains host defaults (64 queued jobs, 8 MiB request payload); explicit nonzero settings survive subsequent default merges.
- `JsSsrAdapter::from_resolved` passes both limits to the explicitly selected napi-vm runtime, so development and emitted-preview adapters share configuration behavior. CLI inspect and doctor include the runtime configuration and report the limit settings; reporting configuration does not prove host execution support.
- Example: `[runtime]` with `backend = "napi-vm"`, `queue_capacity = 8`, `max_request_bytes = 1048576` selects eight queued jobs and a 1 MiB encoded request payload limit.
- Validation: configuration/SSR/CLI tests: 28 + 20 + 18 passed, plus one config doctest; framework/doctor unit tests: 33 passed. A real embedded adapter created from resolved config rejects a request with a 16-byte payload limit and renders after selecting 4096 bytes; this exact limit-enforcement test also passes with an empty PATH. All-target Clippy/format/diff checks pass.
- Remaining: in-flight cancellation/deadlines, hard memory limits, bounded teardown, full compiler-host matrices, framework hydration/SSR conformance, standalone packaging and complete release acceptance. Queue/payload bounds do not imply a sandbox or full resource isolation.

### Guest call-depth and job-drain execution caps

- Added `[runtime].max_call_depth` and `[runtime].max_jobs_per_drain`, with zero retaining pinned-engine defaults. Configuration parsing/merging, napi-vm options and resolved adapters carry both settings; fresh SSR interpreters reapply them through the pinned interpreter's `execution_budget`/`set_execution_budget` APIs.
- Real embedded SSR tests exceed each cap (recursive calls and indefinitely rescheduled microtasks), require the corresponding limit error, then correct the graph and successfully render on the same worker.
- Validation after this increment: config/runtime/SSR tests: 28 + 21 + 21 passed, plus one config doctest. The execution-cap/recovery regression also passes with an empty PATH. All-target Clippy and changed-file formatting pass.
- Broader preceding-HEAD gate: `FERRITE_CHROMIUM_PATH=... FERRITE_FIREFOX_PATH=/usr/bin/firefox FERRITE_E2E_REQUIRE_BOTH_BROWSERS=1 cargo test --workspace --exclude ferrite-e2e --locked` passed 502 tests across 87 targets, with 53 explicitly ignored. This was run before the new cap fields; it does not establish skipped fixture support or replace the separate SDK/feature/release matrices. An initial run failed because Chromium was not discoverable; rerunning with the installed executable paths passed without weakening assertions.
- Compatibility finding: the pinned interpreter failed a self-reference in a named function expression used as a microtask callback (`ReferenceError: spin is not defined`); a function declaration was used to independently test the job cap. This remains a guest-language compatibility limitation requiring investigation, not a supported-framework claim.
- Remaining: wall-clock deadlines/in-flight cancellation (no interrupt API found in the pinned interpreter), hard heap/host limits, bounded shutdown, compiler-host/full framework conformance, hydration and standalone SSR. Call/job caps are deterministic execution limits, not deadlines or process sandboxing.

### Reject static-shell standalone substitution for SSR

- Found that `build_app` packages the client before producing SSR output; the generated standalone scaffold only serves static files. It could therefore appear to package an SSR application while omitting its renderer.
- Application builds selecting SSR plus standalone now fail before client output is written. Explicit SSR environment builds also reject standalone. The packaging API rejects existing server manifests before scaffold creation with an actionable instruction to use the built-renderer preview while standalone renderer execution is implemented.
- Public asset collection excludes the private server directory when an SSR manifest is present, so renderer code/manifests are not included as browser assets. Explicit client-only standalone builds remain available, including projects with an unused server entry.
- Validation: standalone integration tests: three passed, two explicitly ignored; facade/library unit tests: 21 passed, one explicitly ignored. Tests cover no-output SSR rejection for both application and SSR-environment builds, direct packaging rejection, exclusion of private server files, and preserved explicit client-only scaffolding. All-target Clippy and formatting/diff checks pass.
- Remaining: actual standalone SSR graph/runtime embedding and renderer execution, packaging after all environments, portable SDK/dependency provenance, binary HTTP/hydration conformance and cross-platform release checks. This rejection fixes an unsupported compatibility claim; standalone SSR remains unavailable and the overall mission remains unfinished.

### Versioned portable renderer artifact for embedding

- Added serializable compiled module/graph types and public `SsrRendererArtifact` version 1, with executable graph and ordered stylesheet output paths. `from_bytes` validates embedded payloads without filesystem dependencies; unknown schema versions, malformed data and missing compiled static/literal dependencies fail with rebuild instructions.
- Embedded-runtime SSR builds emit private `server/renderer.json` from final emitted chunks after bundle hooks. Chunk source URLs are portable output IDs. Graph loading and preview prefer a present artifact and never fall back after artifact corruption; older manifest-only output still uses the existing emitted-chunk loader. Artifact stylesheet metadata supports preview without the entry manifest.
- Private-output detection and standalone rejection also recognize artifact-only server directories, preserving server/client separation while standalone execution is implemented.
- Validation: feature-enabled facade tests: 23 passed, one explicitly ignored; production-manifest/standalone integration tests: 2 + 3 passed, two standalone tests explicitly ignored. Real emitted renderer artifacts round-trip through JSON bytes, then successfully render and serve HTTP preview after emitted JS chunks and the entry manifest are deleted. The same test passes with an empty PATH. Unknown-version/missing-dependency/corruption regressions and final focused validation pass; all-target Clippy and formatting/diff checks pass.
- Remaining: consume embedded artifact bytes in a real standalone renderer binary, SDK/build dependency provenance, artifact integrity/signatures and source-map metadata, nonliteral dynamic imports, framework hydration/conformance and cross-platform gates. Versioned payload availability does not establish complete SSR/standalone or framework support.

### Rootless renderer construction from embedded bytes

- Added `JsSsrAdapter::new_graph` for explicitly supplied runtimes and compiled graphs. Added `SsrRendererArtifact::into_adapter` to construct an SSR renderer directly from validated artifact data, an explicit runtime, a shell and base URL; it deduplicates artifact styles and adds their public links through preload context.
- The constructor does not resolve a project, load a manifest or install a filesystem compiler loader. Callers remain responsible for serving the published browser assets under the chosen base. Runtime selection remains explicit; graph invocation uses the existing atomic worker operation.
- Validation: feature-enabled facade/SSR tests: 24 + 21 passed, one facade test explicitly ignored. A byte-only artifact with a compiled dependency renders POST metadata, custom HTTP status/headers and one deduplicated stylesheet link using the default rootless napi-vm runtime. The real compiled-artifact regression also now renders through this rootless constructor after emitted chunks are removed, then verifies HTTP preview. Both embedding regressions pass with an empty PATH. All-target Clippy and formatting/diff checks pass.
- Remaining: generate and compile a standalone binary around this primitive, package public assets, establish portable SDK/dependency provenance and execute binary/browser acceptance; framework hydration/conformance, cancellation and release matrices remain unfinished. Rootless adapter construction is not a complete standalone profile.
