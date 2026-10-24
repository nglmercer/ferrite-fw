# Follow-up acceptance after PR #1

## Framework coverage and SSR routing

Linux x86_64, Rust 1.99.1, Node 24.22.1, Chromium headless shell 153.0.8010.0,
official Firefox 157.0. Tests are selected explicitly; ignored fixtures are never
counted as executed by a plain `cargo test`.

- 171 unit tests in ferrite/config/frameworks/server passed (three ignored excluded).
- `ssr_routes`: development source revalidation, emitted artifacts with scope
  hoisting on/off, lazy evaluation isolation, route precedence, query/method and
  status/header preservation, no-fallback 404 and prerender passed.
- `dev_hmr`: React/Vue/Svelte × JS/TS in both engines exercises nested props,
  keyed lists, memo/computed/derived values, hydration identity, child edits,
  CSS, state/reset contracts and syntax recovery. Vue script edits reset the
  component without replacing the document.
- Three standalone tests compile generated applications offline and exercise
  route-specific dotted URLs with buffered embedded, buffered process and streamed
  process runtimes, empty PATH and unrelated working directory.
- Official compiler resource/map/cache and library/server parity fixture passed.
- Strict all-target Clippy for config/server/frameworks/integration crates;
  formatting and whitespace checks.

These are bounded fixtures, not complete Vite or framework parity. See
[VITE_COMPATIBILITY.md](VITE_COMPATIBILITY.md) and the consolidated roadmap.


## Production correctness (dependent draft)

On the same Linux/tool/browser versions:

- 75 transform/bundler unit tests passed, including AST-accurate dynamic import
  wrapping, unmapped generated boundaries and native CSS import ownership.
- Both real browser matrices passed all minify/scope-hoist combinations. One
  combination uses nested JS/CSS output paths. Checks cover static cascade order,
  lazy CSS timing, failed stylesheet retry, CSS readiness before module execution,
  singleton shared state, deduplicated concurrent CSS, direct CSS imports and
  serving the actual emitted SVG asset.
- Two ordinary production cases verify original TypeScript positions after final
  import/CSS/helper/banner edits, CSS-driven importer hash invalidation and removal
  of stale maps following uncomposable code replacements.
- Thirteen side-effect/prerender/SSR-route/plugin-preview regressions passed.
- Official generated React/Vue/Svelte JS/TS production SSR fixtures passed again.
- Strict all-target Clippy, formatting, whitespace and four runner regressions.

[PRODUCTION_CORRECTNESS.md](PRODUCTION_CORRECTNESS.md) defines the bounded contract
and remaining gaps. The production acceptance profile selects these gates
explicitly; a zero-test exact fixture is a failure.

## SSR document and islands profile

The new `--profile ssr` selects 17 gates on Linux. It covers:

- 160 ferrite/config/server/SSR unit tests (three intentionally ignored unit cases
  excluded), including attribute escaping, input validation, original mounted
  request URLs, missing-media behavior and configuration merge precedence.
- Six ordinary document/route/prerender checks, including independent shells,
  selected CSS closures, source revalidation and legacy manifest-only loading.
- Both browser matrices hydrate official React/Vue/Svelte server HTML in dev,
  source-hidden preview and prerendered output. Load/interaction/visible/manual/
  idle triggers, original DOM identity, props, retries, deduplicated attempts,
  inserted roots, cleanup and an isolated admin document are checked.
- A generated standalone repeats those framework checks in both engines from an
  unrelated working directory with an empty PATH. Three additional standalone
  cases cover buffered embedded, buffered process and streamed process documents.
- Seven explicit streaming fixtures and a Node-free CLI process-streamed
  prerender case. The latter also regresses an explicit file-config renderer being
  accidentally overwritten by default programmatic configuration.

Versions remain Linux x86_64, Rust 1.99.1, Node 24.22.1, Chromium headless shell
153.0.8010.0 and official Firefox 157.0. Strict all-target Clippy, formatting,
whitespace and four runner regressions are checked separately. Earlier failed
attempts uncovered and fixed interaction scheduling, copied island ready-state,
missing favicon overlays and stripped mounted request URIs.

Rust 1.99.1 also produced a missing-rlib failure when generated standalone builds
reused the workspace build cache. Verification uses an isolated SDK target via
`FERRITE_STANDALONE_TARGET_DIR` and the existing external e2e codegen-units override;
no repository toolchain/profile change was made. The acceptance runner records
failures honestly and must finish all selected gates before reporting success.

[SSR_DOCUMENTS_AND_ISLANDS.md](SSR_DOCUMENTS_AND_ISLANDS.md) describes the implemented
contract and remaining limitations. This does not promote all framework versions,
nested island ownership, arbitrary native addons or cross-platform isolation.

Final snapshot rerun: the ordinary document/route/prerender gates passed; the
Chromium fixture then stopped while requesting React from registry.npmjs.org
(network request failure). The complete 17-gate profile is not claimed green for
this snapshot. Prior browser, streaming and standalone runs above remain
individual evidence. A subsequent environment lacked the Rust/browser toolchain,
so a complete rerun still requires the documented prerequisites.
