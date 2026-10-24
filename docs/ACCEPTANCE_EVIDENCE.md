# Roadmap implementation evidence

Host-specific results for the changes in this PR. This is not full release or
cross-platform certification; unchecked work remains in COMPLETION_PLAN.md.

| Input | Validated selection |
|---|---|
| Host | Linux x86_64 |
| Rust | 1.99.1 |
| Native compiler | Oxc 0.151.0 |
| Explicit component compiler host | Node 24.19.0 |
| Framework versions | React 19.2.0, Vue 3.5.22, Svelte 5.39.6 |
| Embedded core + timer/web host | napi-vm revision 7362971e7781c0c57b36c5a11813fff0e6bc3521 |
| Chromium | 153.0.8010.0 |
| Firefox | 157.0 |

## Executed checks

- 309 unit checks across facade, CLI, config, frameworks, plugin, runtime,
  server, SSR and transform passed together. The 19 ignored checks were not
  counted as passes; selected slow fixtures below ran explicitly.
- All six generated-project fixtures in framework_compilers passed together:
  JS/TS Vue, Svelte and React build/preview hydration, Vue development graph
  invalidation/recovery, and Vue development hydration in each browser.
- Each framework's production fixture exercised both Chromium and Firefox,
  retained the SSR DOM node, clicked the counter, checked CSS and rejected
  hydration/console errors. Production preview used a dotted application route.
- Official Vue and Svelte emitted-renderer fixtures independently hydrated in
  Chromium and Firefox.
- Real prerender build/HTTP acceptance passed: sorted deterministic route
  reports, base paths, nested static pages, private renderer removal, missing
  route 404 and rollback after renderer failure.
- Generated SSR standalone binary compiled offline and passed HTTP execution
  with empty PATH and an unrelated working directory. Checks include dotted
  routes, request methods, response headers, asset HEAD/405 behavior and private
  renderer 404.
- Indexed-map flattening and composition passed nested-offset, equal-boundary,
  unmapped-region, contents/name/ignore-list and malformed-map regressions.
- Parser preflight measurement passed: 20 checks of 91,780 bytes measured
  858.21 ms uncached versus 7.24 ms cached in the debug test profile. This is not
  an SSR throughput measurement or a release performance budget.
- Strict all-target Clippy, formatting/diff checks and four acceptance-runner
  regressions passed. Runner reports now identify OS, architecture and tool
  versions; exact filtered gates reject zero-test false successes.

## Reproduction

Set FERRITE_CHROMIUM_PATH and FERRITE_FIREFOX_PATH to working installed browsers,
and select the explicit Node compiler host for Vue/Svelte. Use
`scripts/acceptance.py --profile frameworks`, `--profile runtime` and
`--profile standalone` for the corresponding gates and logs. The runner neither
installs prerequisites nor substitutes another browser/runtime.

The runtime/web-host integration uses real text codecs and structured clone;
it does not install fetch/network capabilities. Large official renderer graphs
use a 16 MiB native worker stack, independently of guest execution budgets.
Hard process/heap isolation, full framework development/standalone matrices,
streaming, contextual/workspace/portable optional package graphs and platform
release execution remain open. No entire roadmap or framework parity claim is
made by these results.

## Production side-effect follow-up

The next implementation batch adds actual effect-free module elimination,
including propagation of local/linked package metadata. Effect-only edges and
unused exports reach a fixed point before chunk and manifest generation.
Namespace re-exports preserve their complete namespace; static and dynamic imports
of the same URL retain distinct graph facts. Invalid sideEffects patterns remain
unknown, while documented glob forms use the shared glob engine.

Validation of this batch:

- 188 facade/bundler/resolver/server/transform unit regressions passed; the
  additional successive-removal map-chain regression also passed.
- Eight build regressions, prerender acceptance and both new side_effects
  integration tests passed (11 tests). The new tests each cover eight combinations
  of local/npm metadata, scope hoisting and tree shaking. Emitted graphs execute
  through napi-vm to verify effects and lazy evaluation; output checks verify
  retained CSS and elimination of dead subtrees.
- All six generated-project fixtures passed again, retaining actual Vue/Svelte/
  React production hydration and Vue development recovery in Chromium/Firefox.
- Strict all-target Clippy passed for the changed crates and integration targets.
- New removal transforms retain and chain maps, preserve import-like strings and
  dynamic calls, and protect live star/namespace re-exports. These focused tests
  do not establish completeness of every bundler source-map path or URL-asset
  elimination contract.

## Checked package-map follow-up

Package resolution now validates selected exports/imports before filesystem
access. Invalid target alternatives, null blocking, condition order and pattern
specificity are distinct; valid but missing targets do not trigger fallback file
probing. Traversal/encoded-separator targets, numeric conditions and mixed key
styles fail explicitly. Invalid manifests no longer fall back to index files.
Nearest package scope is enforced for #imports; dependency mappings resolve from
the owning package's locked edge and retain npm module identity.

Validation of this batch:

- 118 facade/resolver/server regressions passed, plus the added malformed-manifest
  regression (22 resolver unit tests now pass).
- 12 build, emitted-package-map, side-effect and prerender integration tests passed.
  The new emitted renderer selects dependency v2 from its owner's edge despite
  the root selecting v1, and preserves client/server conditions in both production
  bundling modes.
- The explicit Node reference fixture passed all 15 supported-case comparisons.
- All six generated-framework fixtures passed in Chromium/Firefox again.
- Strict all-target Clippy and acceptance-runner regressions passed.

Target URL queries/fragments, URL-scheme mappings and complete Node/CJS
interoperability remain outside this profile. These are explicit failures rather
than silent filesystem reinterpretations. Node-free distributed runtime claims
are unchanged; Node is needed only for the explicit reference/compiler fixtures.

## Linux process runtime and streamed HTTP lifecycle

The explicit `napi-vm-process` backend executes fresh child processes with Linux
address-space and CPU ceilings, bounded private-socket transport, shared admission
and shutdown. The CLI and generated SSR servers recognize the worker entry point.
See PROCESS_RUNTIME.md for configuration and containment limits.

A clean restored checkout passed 145 config/runtime/server/SSR unit tests, including
three new Rust-stream tests for HEAD, slow-consumer backpressure/disconnect drop,
and stream errors. Two explicit process acceptance tests passed after rebuilding
the CLI: request timeout/recovery, cancellation/reaping, shutdown, low-memory child
failure, payload bounds and queue deadlines. These do not establish JavaScript
framework streaming or cancellation during pending HTTP rendering.

Development and standalone framework acceptance is being extended in this draft;
new fixture definitions alone are not completion evidence.

## Portable project graph and framework/runtime acceptance

Portable v3 locks retain optional OS/CPU/libc branches and activate compatible
subgraphs without registry metadata during frozen replay. Required failure
propagates through cycles. Context partitioning preserves transitive peer providers
through dependency shadowing/cycles and stable IDs when unrelated roots are added.
Canonical metadata ordering makes fingerprints independent of HashMap iteration.

Project-aware CLI installation discovers workspace arrays/package patterns and
in-project file links, records importer identities and frozen manifest hashes, and
projects relocatable live source/editor views. Duplicate names, escaping paths,
symlinked source directories/manifests and conflicting peer views fail explicitly.
A single local source directory supports one dependency view; optional metadata,
download and integrity failures still fail closed. Legacy locks need one normal
install before portable frozen use.

The restored implementation passed 31 package and 22 resolver tests, 20 CLI and
38 framework tests. Four emitted integration tests passed, including workspace
SSR before/after source edits without reinstalling. Subsequent platform-list
matching coverage follows npm's installed checkPlatform implementation; final
counts are recorded by the final gate run.

React/Svelte JS/TS development rendering, edits, syntax failures and recovery
passed through real HTTP. Missing console globals exposed by React development
output are now supplied by the explicitly selected host. The generated standalone
matrix passed all Vue/React/Svelte JS/TS combinations in Chromium 153.0.8010.0
(headless shell) and official Firefox 157.0: empty PATH, server HTML, retained
hydration DOM, interaction, CSS and dotted application routes. This does not
establish a complete framework release/HMR/component/platform matrix.

A real HTTP disconnect test passed: dropping a pending request cancels its
process-runtime invocation before its ten-second deadline, then a following
request succeeds. An independent server test verifies pending handler teardown.
JavaScript framework streaming remains unimplemented; Rust response streaming
backpressure/error/drop checks do not imply ReadableStream support in the engine.

Final gates for this batch:

- 369 unit tests passed across facade, CLI, config, frameworks, npm, resolver,
  runtime, server, SSR, plugin and transform packages. The ordinary unit runs
  reported 22 ignored tests, which are not included in that pass count.
- Eight ordinary emitted/package-map/workspace/prerender/standalone integration
  checks passed; slow ignored fixtures were selected separately.
- Both process acceptance tests, real HTTP cancellation/recovery and the generated
  process-runtime standalone binary passed explicitly. The latter renders with
  empty PATH, times out an infinite render and serves the following request.
- The six generated framework/language standalone combinations passed in both
  browsers. React/Svelte development checks passed again after package changes.
- Strict all-target Clippy passed for the implementation crates; formatting,
  diff whitespace and four acceptance-runner regressions passed.

These are scoped Linux checks, not an execution of every release gate on every
host. Cross-platform process isolation, native-addon isolation fixtures, complete
framework development/HMR matrices and JavaScript framework streaming remain open.
