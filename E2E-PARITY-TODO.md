# Ferrite E2E implementation TODO

Created: 2026-09-29, after `cbcfc8e`. Reference: Playwright **v1.63.0**.

Implement **G, then A, then B**, followed by the supported C extensions. This
backlog contains **45 tasks (9 complete, 36 remaining)**: four foundations, 16 core tasks, 19 follow-ups
and six optional extensions. Start the feature work with typed DOM events,
richer assertions and exposed callbacks. The ordering and effort assessments
are recommendations based on the current source and parity audit.

The target is a useful, reliable Rust API on the existing Chromium/CDP and
Firefox/BiDi backends. Full Playwright compatibility remains a separate,
substantially larger undertaking. The complete member inventory remains in
[PLAYWRIGHT-API-MATRIX.md](PLAYWRIGHT-API-MATRIX.md); current behavior and engine
limits remain in [PLAYWRIGHT-PARITY.md](PLAYWRIGHT-PARITY.md).

## Scope and tracking

Each checkbox is an implementation task with a completion criterion. API names
suggested below are proposals, unless explicitly described as existing.
**S** means a localized change; **M** means changes across several components;
**L** means a substantial lifecycle or integration change. These are relative
effort estimates, not promises about session length.

G/A/B target both engines unless the item explicitly describes an engine limit.
C items need a capability check before implementation. Existing unsupported
Firefox behavior must stay explicit; a missing native capability can move that
portion to the deferred list while supported, independent work continues.

For every completed task, record the implementation commit, focused regression
coverage and any remaining limitations beside its checkbox. Mark an existing
feature complete after confirming its behavior; extend its missing semantics
without introducing another equivalent API. Do not turn an unavailable field
into a fabricated value or mark a skipped browser test as native verification.

## Already implemented: preserve these features

- Exact/glob/regex URL matching, URL predicates and sync/async network predicates.
- Request-start and response-header waits, cancellation and operation deadlines.
- Disk and in-memory file uploads, including binary payloads and clearing inputs.
- Console/error source positions, epoch timestamps, page IDs and attempt history.
- Fresh contexts per attempt, retries, suites, fixtures, hooks and resource locks.
- Same-origin lazy frame locators, strict locators and actionability retries.
- Separate textContent/innerText getters, ordered text assertions and basic regex assertions.
- Navigation-persistent page callbacks, page clocks and Chromium coverage.
- Context-linked API cookies, storage-state helpers and persistent profiles.
- Step controls, live reporters, attempt artifacts and portable report bundles.
- Page/context action, navigation and assertion timeout settings; Page API clients.

Baseline evidence: the latest implementation passed 265 E2E checks plus 23
CLI/configuration checks, with strict Clippy. These are Ferrite regressions;
they do not constitute differential Playwright conformance coverage.

## Verified implementation checkpoints

- `5c2c7aa`: G01–G03 and A01–A06. All 269 E2E checks and 23 CLI/configuration
  checks passed; the expanded native conformance groups and Send runner integration
  passed on installed Chromium and Firefox, with strict Clippy and formatting checks.
- G04 remains open until callback and stream lifecycle coverage is complete.
  A14 has a verified zero-segment double-star correction; shared API integration
  and its remaining conformance cases are still open.

## G — Foundations for the larger session

Work in [the audit generator](scripts/playwright-parity/build_matrix.py),
[tests](crates/ferrite-e2e/tests) and the parity documents.

- [x] **G01 — Reconcile the matrix with current behavior (S).** Audit mappings
  against public APIs and native regressions before treating a Missing label as
  work to implement. In particular, check `Page::request`, Page/Frame text
  getters, context dialog/download/close events and timeout settings. Done when
  stale mappings/notes are corrected and every changed evidence link resolves.
  Evidence: Matrix regenerated in `5c2c7aa`: 73 classes / 1,018 members; corrected public API mappings and source evidence links.

- [x] **G02 — Maintain an explicit engine capability table (S).** Describe
  supported, partial and unsupported operations, including native metadata
  availability. Done when portable regression jobs require both installed
  engines and engine-specific jobs assert the unsupported result on the other
  engine. Browser absence must be visible in the validation report.
  Evidence: `5c2c7aa`; [engine capabilities](E2E-ENGINE-CAPABILITIES.md), fail-fast two-browser validation gate and native unsupported-result probes in `core_conformance.rs`.

- [x] **G03 — Add a focused conformance corpus (M).** Reuse small deterministic
  HTML/HTTP fixtures for events, text/class assertions, URL globs, redirects,
  callbacks and screenshots. Record pinned Playwright reference behavior and
  compare Ferrite results. An optional development-only Playwright job may use
  Node; Ferrite's library and normal Rust tests retain their current runtime.
  Done when failures identify the semantic difference and the corpus can be
  rerun against the pinned version.
  Evidence: `5c2c7aa`; [pinned corpus](scripts/e2e-conformance/README.md) records actual Playwright 1.63.0 Chromium results and compares shared native Chromium/Firefox behavior.

- [ ] **G04 — Apply lifecycle checks to every new API (M).** Reuse existing
  operation budgets and cancellation rather than adding independent timers.
  Done when representative new waits/callbacks/streams handle zero timeout,
  enclosing deadlines, caller cancellation, disposal and retries, with dropped
  protocol commands, listeners and tasks reclaimed.

## A — Core features to implement first

Main files: [locator.rs](crates/ferrite-e2e/src/locator.rs),
[expect.rs](crates/ferrite-e2e/src/expect.rs),
[page.rs](crates/ferrite-e2e/src/page.rs),
[context.rs](crates/ferrite-e2e/src/context.rs),
[driver.rs](crates/ferrite-e2e/src/driver.rs) and
[dom.js](crates/ferrite-e2e/src/dom.js).

### Events and assertions

These extend the official [event dispatch API](https://playwright.dev/docs/api/class-locator#locator-dispatch-event)
and [locator assertions](https://playwright.dev/docs/api/class-locatorassertions).

- [x] **A01 — Typed DOM event dispatch (M).** Add an options-based companion to
  the existing CustomEvent helper: Event, MouseEvent, KeyboardEvent, FocusEvent,
  InputEvent and PointerEvent, with event-specific initialization and explicit
  bubbles/cancelable/composed defaults. Done when listeners observe the correct
  constructor and fields, shadow-boundary behavior and cancellation; retain the
  existing CustomEvent detail contract. Live JSHandle event arguments are deferred.
  Evidence: `5c2c7aa`; seven constructor/field samples, flags, cancellation, shadow composition and legacy CustomEvent compatibility in the native corpus.

- [x] **A02 — Text assertion options (M).** Support exact/contains/regex text,
  case handling and a rendered-text option using existing innerText getters.
  Done when exact-string whitespace rules and raw regex matching are specified
  separately and hidden text, newlines, negation and delayed updates are tested.
  Evidence: `5c2c7aa`; exact-string normalization, raw regex, rendered text, case/negation and delayed updates covered on both engines. Rust regex flags retain Rust semantics.

- [x] **A03 — Mixed list, class and value matchers (M).** Extend existing text,
  class and selected-value assertions with string/regex lists, class-token
  semantics and ordered subset matching for contains-text lists. Done when
  empty lists, ordering, duplicates, negation and retrying list changes match
  the documented contract and produce useful expected/actual diagnostics.
  Evidence: `5c2c7aa`; mixed exact/regex lists, ordered subsets, raw class order, native class tokens and selected values; empty/duplicate/negated/delayed cases covered.

- [x] **A04 — State assertion options (S).** Extend existing state matchers with
  explicit expected states and checkbox indeterminate handling. Done when true,
  false and mixed-state assertions work, invalid combinations/types fail clearly
  and negation does not hide a locator resolution error.
  Evidence: `5c2c7aa`; explicit state and indeterminate options; invalid types/combinations, strict resolution and missing-element behavior covered.

- [x] **A05 — Viewport intersection ratios (M).** Add a ratio option to the
  current overlap assertion using browser intersection observations. Done when
  partial visibility, clipping ancestors, scrolling and thresholds are tested;
  the default behavior and supported same-origin frame scope are documented.
  Evidence: `5c2c7aa`; native IntersectionObserver ratio, clipping/transforms, thresholds and same-origin frame coverage; default assertion/getter migrated to positive native intersection.

- [x] **A06 — Accessible assertion regex/options (S).** Extend accessible name,
  description and error-message assertions with common string/regex/case options.
  Done when they use the shared DOM computation consistently and test label
  changes and missing relationships. Keep the existing accessibility algorithm
  approximation explicit.
  Evidence: `5c2c7aa`; shared string/regex/case options with changing labels and missing relationships covered. Existing DOM accessibility approximation remains explicit.

### Rust callbacks

Reference: [page callbacks](https://playwright.dev/docs/api/class-page#page-expose-function)
and [context callbacks](https://playwright.dev/docs/api/class-browsercontext#browser-context-expose-binding).
The existing synchronous Page callback survives navigation; extend it.

- [ ] **A07 — Async Page callbacks (M).** Add callbacks returning a future and
  `E2eResult` without blocking the dispatch pump. Done when JavaScript awaits
  results, Rust errors/panics reject predictably, concurrent calls remain
  independent and navigation/disposal cancels or settles pending work.
- [ ] **A08 — Context-wide exposed functions (L; needs A07).** Register callbacks
  for current/future pages and adopted popups. Done when the function is available
  to startup scripts and after navigation, registrations stay context-isolated
  and closing a context releases the associated pumps and callback state.
- [ ] **A09 — Binding caller metadata (M; needs A07/A08).** Add Page/context
  bindings carrying owning context, page and supported frame identity. Done when
  two pages and same-origin frames report the correct caller and async results
  and failures behave as in A07. Defer cross-origin/OOPIF binding dispatch.
- [ ] **A10 — Callback registration/removal lifecycle (M; needs A07–A09).** Define
  duplicate-name behavior and add removal handles or named removal helpers.
  Done when removal affects current/future documents, pending calls settle and
  unrelated bindings keep working; removed preload scripts cannot reintroduce
  the callback after navigation.

### Daily API gaps

- [ ] **A11 — Filtered cookie clearing (S).** Extend existing clear-cookies with
  name/domain/path exact and regex filters. Done when same-name cookies on
  different domains/paths are handled independently and browser/API linked
  stores remain synchronized. Preserve unsupported partition metadata honestly.
- [ ] **A12 — Consistent action options (M).** Extend click/hover/check/drag
  options with supported positions, modifiers, trial readiness and scoped
  timeout overrides. Done when trial actions produce no input, modifiers are
  released after errors/cancellation and actionability stays consistent on both
  engines. Add options to existing methods rather than recreating timeout defaults.
- [ ] **A13 — URL wait readiness options (M).** Add a wait-until option to the
  existing matching/predicate URL waits. Done when URL matching and requested
  document readiness share one budget across redirects, same-document history,
  hashes and supported frames. Specify NetworkIdle limits explicitly.
- [ ] **A14 — Shared URL matching across APIs (M).** Reuse UrlMatcher in page URL
  assertions, route selection and HAR filters while preserving legacy string
  contracts. Done when base-URL resolution, escaped globs, braces, zero-segment
  double-stars and regex anchoring have pinned conformance cases; invalid patterns
  fail before registration. Needs G03; coordinate with A13 and B05.
- [ ] **A15 — Function-wait arguments, polling and results (M).** Extend existing
  function waits with JSON arguments and interval/animation-frame polling; add a
  result helper where existing handle support permits it. Done when promises,
  falsy-to-truthy transitions, returned values and cancellation are tested.
  Frame-scoped remote handles remain a capability-dependent extension.
- [ ] **A16 — Frame lookup conveniences (S).** Add a dedicated main-frame helper
  and URL matcher/predicate lookup over existing frame handles. Done when no
  match, navigation and detached frames have explicit behavior, and same-origin
  nested/replacement cases work on both engines. Check native name availability;
  selector-free OOPIF traversal stays deferred.

## B — Broader follow-ups on the existing backends

Work through these after A; independent items may be completed earlier when
their dependencies are ready. References include
[network request metadata](https://playwright.dev/docs/api/class-request),
[response lifecycle](https://playwright.dev/docs/api/class-response),
[API requests](https://playwright.dev/docs/api/class-apirequestcontext),
[visual comparisons](https://playwright.dev/docs/test-snapshots) and
[reporters](https://playwright.dev/docs/test-reporters).

### Network and event lifecycle

- [ ] **B01 — Missing frame/load/dialog lifecycle events (M).** Extend Page and
  context events for supported frame attachment/navigation/detachment, DOM/load
  readiness and dialog closure. Done when events have page/frame identity and
  ordered, deduplicated lifecycle tests. Preserve already implemented dialog,
  popup, download and page-close forwarding; audit it under G01 first.
- [ ] **B02 — Rich request/response metadata wrappers (L).** Build typed wrappers
  over current observations: headers/arrays, method, post-data JSON, status,
  resource type, frame/page references, failure and redirect links where native
  data exists. Done when concurrent requests and redirect hops retain identity,
  old RecordedRequest callers keep working and unavailable fields remain optional.
  Body retrieval is separate in C02; this item must work without Firefox bodies.
- [ ] **B03 — Response completion helpers (M; needs B02).** Expose completion of
  a response separately from receiving its headers. Done when delayed streaming
  bodies, HTTP error statuses, redirects, transport failures and disposal settle
  correctly. Extend existing RequestFinished/RequestFailed observations; no
  response-body capture is required to implement completion.
- [ ] **B04 — Earliest popup diagnostics (M).** Preserve console/error/network
  observations emitted before a popup is fully adopted into the context. Done
  when startup-script logs, immediate requests and immediate closure retain
  source identity without duplicate forwarding or listener leaks.
- [ ] **B05 — Route removal and in-flight handler behavior (M).** Extend existing
  unroute/unroute-all with documented wait/ignore-error behavior. Done when active
  async handlers settle or cancel according to policy, request ordering remains
  defined and disposal cannot hang. Needs G04; reuse A14 matching when available.
- [ ] **B06 — Route fetch/fulfill option fidelity (M).** Extend supported route
  operations with method/body/header overrides and bounded fetch options using
  existing ApiClient machinery. Done when redirect limits, retries, binary bodies,
  duplicate headers and status overrides have focused regressions. Chromium-only
  response rewriting/URL overrides remain explicit on Firefox.

### API testing, captures and reports

- [ ] **B07 — Header convenience APIs with duplicate preservation (S).** Add
  headers-array, all-values and case-insensitive lookup helpers to API/network
  response types. Done when repeated Set-Cookie and other duplicate headers
  survive serialization, lookups and route forwarding. Coordinate with B02/B06.
- [ ] **B08 — API request option/redirect fidelity (M).** Extend current requests
  where the audit shows missing semantics: incompatible body options, retryable
  transport errors, credential-origin rules, redirect methods and status failure
  handling. Done when local HTTP/HTTPS fixtures prove behavior, linked cookies
  stay synchronized and errors/cancellation do not retain buffers or connections.
- [ ] **B09 — Screenshot capture options (M).** Extend existing screenshot and
  locator-capture options with supported clipping, scale, transparent background,
  mask color and temporary styles. Done when coordinates/masks work for scrolled
  and full-page captures, and temporary DOM/style changes restore after failure.
  Check native engine support before accepting an option.
- [ ] **B10 — Stable screenshot assertions and snapshot paths (M; needs B09).**
  Extend existing snapshot assertions with stable successive captures, browser/
  project/platform path templates and clearly defined update modes. Done when
  delayed fonts/animations, never-stable content, dimension differences and
  baseline changes are bounded and diagnosed; attach expected/actual/diff artifacts.
- [ ] **B11 — Report search, filtering and network diagnostics (M; needs B02/B03).**
  Extend the portable HTML report with test/status/project filters and per-attempt
  network summaries beside existing console output. Done when retries stay
  distinct, diagnostic text is escaped, empty/large reports remain usable and
  moving the bundle preserves all artifact links. Visually inspect representative
  expanded reports; no Trace Viewer archive implementation is required.
- [ ] **B12 — Runner-integrated soft assertions (M).** Extend current SoftAsserts
  with an attempt-owned collector and contextual assertion messages. Done when
  collected failures affect the attempt result even without a final manual
  assert-all, include step/source metadata, preserve cleanup and stay isolated
  across retries and parallel tests. A custom matcher registry is deferred.
- [ ] **B13 — Fixture budgets and shared teardown accounting (L).** Add explicit
  fixture setup/teardown timeouts and define one enclosing cleanup budget with
  operation overrides bounded by it. Done when slow hooks/fixtures, failed setup,
  reverse teardown and cancellation produce all relevant errors without silently
  skipping cleanup. Preserve zero-timeout semantics and test existing defaults.
- [ ] **B14 — Effective project/configuration metadata (M).** Extend Project and
  report metadata with missing per-project grep-invert, output/snapshot paths and
  repetition settings; expose resolved configuration read-only. Done when global/
  project/suite/test precedence, defaults and explicit overrides are tested and
  CLI forwarding matches library configuration. Project dependency scheduling is deferred.

### Useful convenience and diagnostic APIs

- [ ] **B15 — Completed download streaming and ownership (S).** Add async reading
  of completed files and owning-page identity without duplicating save-as/delete.
  Done when binary/large files, missing files and cancellation work on both
  engines; deletion is idempotent for missing files but reports other filesystem
  errors. Per-download active cancellation and early failure metadata require
  native capability checks; do not infer unavailable Firefox metadata from filenames.
- [ ] **B16 — Context/browser ownership introspection (S).** Add a context owner
  accessor and review disconnection semantics of existing is-closed/is-connected
  APIs. Done when convenience-page contexts, explicit contexts and remote browser
  disconnection are covered, with no strong-reference ownership cycle.
- [ ] **B17 — Locator descriptions and diagnostic call sites (S).** Add optional
  locator descriptions and propagate them into automatic steps/errors. Done when
  cloning/chaining preserves intended labels and action/assertion errors identify
  the calling operation. Keep automatic-step deduplication and existing source data.
- [ ] **B18 — Bounded ARIA snapshot options (M).** Extend the current structured
  DOM snapshots with depth limits and optional boxes/state fields. Done when
  nested/open-shadow/same-origin-frame cases are consistent, limits are explicit
  and snapshot assertions can consume the output. Full accessible-name conformance,
  upstream AI modes and YAML pattern matching remain deferred.
- [ ] **B19 — Structured page errors and console value previews (M).** Add optional
  error name/message/stack frames and JSON-safe console argument previews where
  native events provide them. Done when page/context events, traces and attempt
  reports retain the same data through cleanup/retries, and unserializable values
  are identified explicitly. Remote argument handles and worker ownership are deferred.

## C — Optional extensions with native capability gates

These have lower priority. Implement the supported portion on the current
backend, with explicit errors on other engines. Do not add a new backend to
complete this section.

- [ ] **C01 — PDF options builder (S, Chromium).** Extend existing PDF export with
  validated paper/size/margins, background, scale and header/footer options.
  Done when real PDFs confirm page size/content and invalid options fail early;
  unsupported engines return a clear error. Reference:
  [PDF export](https://playwright.dev/docs/api/class-page#page-pdf).
- [ ] **C02 — Captured response-body convenience methods (M, Chromium; needs B02/B03).**
  Add bytes/text/JSON helpers over native captured bodies with bounded storage.
  Done when uncaptured, unavailable, empty, truncated and failed bodies are
  distinguishable and malformed JSON reports a useful error. Preserve the
  existing cap unless a reviewed bounded option replaces it. Firefox body
  capture remains deferred until a supported native mechanism is verified.
- [ ] **C03 — Coverage lifecycle/options (M, Chromium).** Extend current JS/CSS
  coverage with supported navigation reset and source inclusion controls.
  Done when navigation, anonymous scripts, stop/restart and disposal behave
  predictably. Reference: [Coverage](https://playwright.dev/docs/api/class-coverage).
- [ ] **C04 — HAR matching/recording options (M, supported subsets).** Extend
  existing HAR tools with shared URL matchers, explicit not-found behavior and
  supported content/timing options. Done when redirects, duplicate request URLs,
  binary bodies and replay failures are tested. Keep Firefox body/response
  rewriting limits visible; ZIP archive/update workflows remain deferred.
  Reference: [HAR routing](https://playwright.dev/docs/api/class-browsercontext#browser-context-route-from-har).
- [ ] **C05 — Additional emulation options (M, Chromium).** Extend current media/
  device descriptors with native forced-colors/contrast/media and custom metrics/
  user-agent metadata where supported. Done when application-observable values
  and reset behavior are verified, and unsupported Firefox options fail explicitly.
  Reference: [media emulation](https://playwright.dev/docs/api/class-page#page-emulate-media).
- [ ] **C06 — Scoped CDP session ownership (M, Chromium).** Add independently
  detachable target sessions over the existing connection. Done when detaching
  one session leaves other pages and browser transport working, pending calls
  fail cleanly and repeated disposal is safe. Cross-backend protocol session
  equivalence is deferred. Reference: [CDPSession](https://playwright.dev/docs/api/class-cdpsession).

## Deferred substantial work

Keep these outside the default large-session scope. Record a capability/design
finding here if a task above exposes a dependency on one of these projects.

| Feature | Reason to defer / prerequisite |
|---|---|
| WebKit backend | New driver, event/action implementation and engine regression suite. WebKit itself is available on Linux; Ferrite lacks its backend. |
| Browser installer, channels and dependency manager | Distribution, platform maintenance and install lifecycle beyond the current stock-browser approach. |
| Playwright remote protocol/browser server | New transport/server lifecycle, ownership and compatibility commitments. |
| Cross-origin/OOPIF lazy selectors and bindings | Realm/target routing and frame replacement/identity need a dedicated backend design. |
| Complete ElementHandle and arbitrary JS value serialization | Remote identity, nested handle arguments and object lifetime management across engines. |
| Workers/service workers and WebSocket routing | New evaluation/interception object graphs and uneven native backend capabilities. |
| Native file chooser and directory uploads | Event-driven chooser ownership, filesystem directory semantics and backend support. |
| Firefox response bodies, response rewriting and emulation gaps | Verify a supported native capability first; DOM approximations do not establish equivalent behavior. |
| Context-wide synchronized clock | Shared virtual-time semantics across documents/pages/frames need a separate design. Existing page clocks remain supported. |
| Complete accessibility algorithms and YAML/AI snapshots | Standards-level behavior and matcher language require dedicated conformance work. |
| IndexedDB/OPFS state | Storage schema/version, transactions and cross-origin persistence are separate projects. |
| Process workers, project dependency graph and fixture override hierarchy | Runner architecture and failure isolation changes; retain current Tokio workers and typed fixtures. |
| Trace Viewer archives, blob reports and distributed merging | New artifact formats, source/DOM capture and merge semantics. |
| Inspector/UI mode, codegen and component mounting | Dedicated tooling and framework integration projects. |
| Electron, Android/ADB/WebView and WebAuthn | Additional targets or specialized backend capabilities outside everyday web testing. |

The Linux distinction follows the official [browser support documentation](https://playwright.dev/docs/browsers).

## Large-session execution and completion

1. Recheck current code and native capability evidence under G01/G02. Do not
   rebuild features added since this document was created.
2. Implement A in order, respecting the callback and matching dependencies.
   Complete independent B work as prerequisites become available; implement C
   only within the existing native capabilities. Keep changes reviewable and
   update checkboxes after verification, preferably committing at phase boundaries.
3. For an unavailable capability or a substantial deferred dependency, record
   the precise reason and remaining subset beside the item and continue other
   ready work. An unchecked blocked/deferred item remains incomplete.
4. Add meaningful focused regressions for new semantics, failures and lifecycle
   behavior. Run Chromium and Firefox for shared features; for engine-specific
   APIs verify the supported engine and the other engine's explicit error.
5. Update examples, parity notes and manual matrix mappings for each API change.
   Regenerate source links and validate formatting, exports and serialization
   compatibility. New public fields need a migration note and appropriate serde defaults.
6. Run the final regression suite, strict Clippy and diff checks. Validate generated
   artifacts, and visually inspect changed HTML reports. Report actual native
   execution, unsupported portions and remaining unchecked items separately.

Suggested final commands, using installed browsers and a writable temp directory:

```bash
FERRITE_CHROMIUM_PATH=/path/to/chromium \
FERRITE_FIREFOX_PATH=/path/to/firefox \
FERRITE_E2E_REQUIRE_BOTH_BROWSERS=1 \
TMPDIR=/path/to/writable-temp \
cargo test -p ferrite-e2e --no-fail-fast -- --test-threads=4
cargo test -p ferrite-cli -p ferrite-config
cargo clippy -p ferrite-e2e -p ferrite-cli --all-targets -- -D warnings
cargo fmt -p ferrite-e2e --check
python3 scripts/playwright-parity/build_matrix.py
git diff --check
```

The generator needs its pinned upstream documents; use its documented `--fetch`
or `--upstream` option when the cache is absent. Changes to CLI/configuration
also need formatting checks for the modified packages.

Copyable request for the implementation session:

> Implement the open tasks in E2E-PARITY-TODO.md in G → A → B → C order,
> following the dependencies and completion criteria. Implement all practical
> features supported by the existing Chromium/Firefox backends. Keep the deferred
> substantial projects outside scope; document any blocked subset and continue
> independent ready tasks. Verify native behavior, update the examples and parity
> matrix, keep the TODO accurate, and commit verified changes at phase boundaries.
