# Ferrite E2E compared with Playwright

Ferrite now covers more of Playwright's everyday Chromium/Firefox testing
workflow, with fresh contexts, strict locators, shared API cookies and a larger
assertion/runner API. **It does not provide full Playwright API or behavioral
parity.** The implementation deliberately defers substantial backend,
distribution, debugger and orchestration work.

Audit date: **2026-09-29**. Upstream baseline:
[Playwright v1.63.0](https://github.com/microsoft/playwright/releases/tag/v1.63.0).
The initial inventory and existing changes were committed in `d93365a`; this
report describes the subsequent practical parity implementation. The original
findings remain available in that commit's history.

The [complete API matrix](PLAYWRIGHT-API-MATRIX.md) covers **73 classes and
1,018 documented JavaScript-applicable members**, including browser, test,
reporter, Android and Electron APIs:

| Classification | Members | Meaning |
|---|---:|---|
| Equivalent | 15 | Counterpart for the basic operation/value, without full options or engine compatibility |
| Partial | 508 | Related exposed operation with material semantic, option or engine differences |
| Idiomatic | 42 | Comparable operation through Rust language/library facilities |
| Missing | 453 | No dedicated public counterpart |

These counts describe an inventory, **not a behavioral compatibility
percentage**. The earlier inventory had 458 Partial and 503 Missing members.
Overloads are collapsed by member kind, inherited APIs appear on the declaring
class, and deprecated/experimental members remain visible. Argument options
are discussed below rather than counted individually. Runtime checks exercise
Ferrite's tests; they are not a differential Playwright conformance suite.
The [generator](scripts/playwright-parity/README.md) pins official sources,
requires a complete inventory and validates local evidence links.

## Practical parity implemented

| Area | Implemented behavior | Remaining limit |
|---|---|---|
| Test isolation | Fresh context/page for every test attempt and retry; teardown on errors, timeouts and body panics | Fixtures remain per-attempt, without dependency graphs or worker scope |
| Page ownership | `Browser::new_page()` owns a fresh context and closes its popups on disposal | Use `default_context().new_page()` for intentional shared storage |
| Browser lifecycle | Persistent Chromium/Firefox profiles, graceful close, Chromium HTTP/WebSocket CDP connections | No Playwright remote protocol, browser server, channels or managed browser installer |
| Configuration | Complete resolved CLI configuration forwarded as JSON; browser launch/runner consume it, with legacy overrides | Custom suites must use the consuming constructors; no suite-scoped fixture options |
| Projects and scheduling | Independent project browsers/launch/context settings; parallel Tokio tasks, retries, repetition, filters and named resource locks | No process workers, project dependency graph or distributed locks |
| Artifacts | Per-attempt output directories, validated `TestInfo::output_path()`, attachments, screenshots and video policies | No Playwright snapshot templates or full reporter object model |
| Locator selection | Strict single-target operations, genuine first/last/nth slicing, relative has/hasNot filters, exact/regex/visibility builders | No complete Playwright selector extension/custom-engine surface |
| Semantic locators | Associated labels target controls; roles and accessible names use shared DOM helpers; open shadow-root traversal | Full accessible-name specification and closed shadow roots remain outside this implementation |
| Actions | Retry readiness, visibility/stability/hit testing, trusted forced clicks, trusted checkbox/key input; delayed fill/select and contenteditable support | Some actions use DOM setters/events; full native input/event/layout semantics remain narrower |
| DOM access | Separate textContent/innerText, arrays, evaluate-all/JSON arguments, highlight removal; single-target getters wait and enforce strictness | JSON values only, without arbitrary JS/JSHandle argument serialization |
| Frames and handles | Same-origin lazy/nested/replacement `FrameLocator`, frame ownership conversion, remote handle evaluation/properties | Cross-origin/OOPIF lazy selection and ElementHandle are deferred |
| Assertions | Exact/regex page title/URL, normalized ordered texts, classes, values, role/error message, custom predicates, `expect_to_pass` | No custom matcher registry/asymmetric matchers or full options parity |
| Accessibility snapshots | Structured DOM role/name/state tree and locator/page exact snapshot assertions | Approximation, without complete ARIA/YAML matching or all upstream modes |
| Clock | Separate fixed Date/system time, run-for/fast-forward, promise/timer ordering, pause-at/resume and installation time | Page-local; navigation reinstalls initial state; idle callbacks approximate browser behavior |
| API testing | Query/headers/JSON/form/raw/multipart, cookies, TLS/proxy/auth, timeout, redirects, status checks and connect retries | No full APIRequest lifecycle/storage-state API or all redirect/retry semantics |
| Browser/API storage | Context-linked cookies in both directions; isolated protocol cookie partitions; Playwright cookies/origins localStorage JSON | API transport options configured separately; redirect/partition/SameSite details remain narrower; no IndexedDB/OPFS snapshots |
| HTTP credentials | Browser challenge authentication on Chromium, preserving extra headers; explicit preemptive Basic helper | Firefox challenge credentials unsupported; cached-auth clearing is approximate |
| Callbacks and buffers | Page-exposed functions survive navigation; console/error retrieval and clearing | No context-wide bindings, async Rust callbacks or complete frame/worker dispatch |
| Downloads | Chromium download behavior/cancellation scoped to the owning context; completed-file deduplication | File-based lifecycle, limited Firefox URL/failure/cancellation metadata |
| Coverage | Dedicated Chromium JS/CSS coverage controller with sources and usage ranges | Native V8/CSS ranges, without Playwright flattening/navigation options; Firefox unsupported |

Evidence is linked per member in the matrix. The main implementation is in
[browser.rs](crates/ferrite-e2e/src/browser.rs),
[context.rs](crates/ferrite-e2e/src/context.rs),
[locator.rs](crates/ferrite-e2e/src/locator.rs),
[runner.rs](crates/ferrite-e2e/src/runner.rs),
[api.rs](crates/ferrite-e2e/src/api.rs),
[expect.rs](crates/ferrite-e2e/src/expect.rs), and the new
[frame locator](crates/ferrite-e2e/src/frame_locator.rs),
[clock](crates/ferrite-e2e/src/clock.js),
[DOM helpers](crates/ferrite-e2e/src/dom.js),
[coverage](crates/ferrite-e2e/src/coverage.rs) and
[configuration bridge](crates/ferrite-e2e/src/config.rs).

## Existing features with narrower equivalents

Routing/fulfillment/fallback/HAR, dialogs, popups, request capture, device and
permission controls, screenshots/PDF, video/live frames, JSON traces, soft
assertion collection and fixed report serializers remain available. Their
presence does not establish full Playwright behavior:

- Captured requests/responses are records rather than a rich live object
  graph. Chromium response bodies are capped; Firefox has no body capture.
  WebSocket observation is Chromium-only; interception/mocking is absent.
- HAR recording/replay has narrower timing, body, update and archive support.
  Routes do not reproduce every response/redirect/header transformation option.
- Traces are Ferrite JSON rather than Trace Viewer archives with DOM/source
  snapshots. Reporter output is aggregate, without the live Suite/TestCase/
  TestStep plugin graph, blob merging or all upstream report formats.
- Screenshot comparison, update modes and paths differ. PDF options, device
  descriptors, emulation and permissions have smaller surfaces.
- URL/event/network waits retain narrower matching/result options. Timeout
  defaults can be shared across pages, but protocol calls also retain backend
  timeouts; zero-timeout behavior is not uniformly Playwright-compatible.
- Hooks are runner-wide rather than describe-suite scoped. Fixture dependency
  resolution, worker lifecycle and runtime test metadata mutation remain partial.

## Deliberately deferred substantial work

These are exclusions from the practical implementation, not implemented APIs:

- WebKit backend, browser/dependency distribution, named channels and
  Playwright-protocol remote/browser-server operation. Ferrite has no WebKit
  driver; this is an implementation limit, not a claim that WebKit cannot run
  on Linux. [Playwright supports WebKit on Linux](https://playwright.dev/docs/browsers).
- Cross-origin/OOPIF lazy frame traversal, selector-free frame search and full
  ElementHandle identity/lifetime support.
- Worker/service-worker evaluation and event graph, WebSocket routing,
  file-chooser interception and virtual credentials/WebAuthn.
- Inspector/UI mode, code generation, component mounting, Android/ADB/WebView
  and Electron backends.
- Process-based workers, project dependencies, suite execution scopes, fixture
  dependency/worker graphs, global timeout/max-failure orchestration, live
  custom reporters, Trace Viewer archives and advanced report merging.
- Full accessibility/selector algorithms, YAML ARIA matchers, all JS value
  serialization, IndexedDB/OPFS state persistence and complete backend parity.

The matrix preserves each missing member so future work can be selected
without treating raw CDP, arbitrary evaluation or Rust assertions as evidence
that a dedicated feature was implemented.

## Engine and validation evidence

Chromium uses CDP and Firefox uses stock WebDriver BiDi. Firefox accepts user
agent/proxy/TLS settings at launch; offline, extra headers, HTTP challenge
credentials, locale/timezone/media/device/JS/CSP emulation, download policy and
response modification remain Chromium-only and fail explicitly on Firefox.
Coverage is Chromium-only. Firefox recordings are native; Chromium recordings
require ffmpeg. Individual tests contain explicit unsupported-engine branches;
a passing two-engine suite therefore does not imply that every operation runs
on both engines.

The final validation run uses Firefox from `/usr/bin/firefox` and official
Chrome-for-Testing headless shell 153.0.8010.12. `FERRITE_CHROMIUM_PATH` points
to that binary so Chromium tests actually execute, rather than being skipped.
`TMPDIR` points at a disk-backed directory because this environment's `/tmp`
is full.

```bash
FERRITE_CHROMIUM_PATH=/path/to/chrome-headless-shell \
TMPDIR=/path/to/disk-backed-temp cargo test -p ferrite-e2e -- --test-threads=2
cargo clippy -p ferrite-e2e -p ferrite-cli --all-targets -- -D warnings
```

Validation completed successfully:

- `ferrite-e2e`: **100 unit tests, 3 API tests, 93 browser tests and 2 doctests**.
  Chromium and Firefox were both installed and exercised. Unsupported-engine
  branches remain explicit; these are not full cross-engine conformance claims.
- CLI/configuration: **4 CLI tests, 17 configuration tests and 1 doctest**.
- A repeat run of all **8 practical parity regressions** passed;
  the strict bounding-box getter and shorter missing-element waits also passed
  their targeted two-engine tests.
- `cargo clippy -p ferrite-e2e -p ferrite-cli --all-targets -- -D warnings`
  passed, as did formatting checks for both changed packages and diff checks.
- Matrix regeneration was deterministic, and incomplete source sets were
  rejected explicitly.

New regressions cover isolation/retries/locks, project engines and cleanup,
persistent profiles,
strict/shadow/semantic locators and delayed actions, callback navigation,
clock semantics, frame replacements/handles, storage/API cookie sharing,
HTTP payload/options, configuration forwarding and coverage. Existing tests
also exercise downloads, credentials, routing, snapshots, input and reports.
