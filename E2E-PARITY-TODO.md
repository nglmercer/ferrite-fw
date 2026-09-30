# Ferrite E2E implementation TODO

Created: 2026-09-29, after `cbcfc8e`. Reference: Playwright **v1.63.0**.
Session plan refreshed: 2026-09-30, from verified B01 implementation `f8c12de`
and the current uncommitted B04 draft.
Uncommitted implementations are not counted complete.

Implement **G, then A, then B**, followed by the supported C extensions. This
backlog contains **45 tasks (27 complete, 18 remaining)**: four foundations, 16 core tasks, 19 follow-ups
and six optional extensions. All A tasks and B01/B02/B03/B05/B06/B07/B08/B15 are now verified;
continue with popup/error diagnostics, then the
remaining runner/capture/report work and supported C extensions. The ordering and effort assessments
are recommendations based on the current source and parity audit.

The target is a useful, reliable Rust API on the existing Chromium/CDP and
Firefox/BiDi backends. Full Playwright compatibility remains a separate,
substantially larger undertaking. The complete member inventory remains in
[PLAYWRIGHT-API-MATRIX.md](PLAYWRIGHT-API-MATRIX.md); current behavior and engine
limits remain in [PLAYWRIGHT-PARITY.md](PLAYWRIGHT-PARITY.md).

## Recommended next implementation

Finish **B04 — earliest popup diagnostics**, followed by
**B19 — structured errors/console data**. B01 is verified in `f8c12de`:
native frame/navigation/readiness/dialog-close observations preserve source
identity and forward once to the context. B06 remains verified in `c88c14b`;
retain its context-linked fetch and inherited fulfillment regressions.

The native-events phase passed all 325 E2E and 23 CLI/configuration checks,
with strict Clippy, formatting and 633 regenerated matrix source links. Three
event groups additionally ran on full Chrome 153 and Firefox 157; the four-case
pinned reference covers frame identity, history, readiness and dialog closure.
The complete inventory is 149 units, 173 integrations across all 23 targets,
three doctests and 23 CLI/configuration checks. Completed network/routing,
callback, popup, download and closure features remain regression requirements.

**Current handoff:** B01 is committed. B04 already has an uncommitted draft:
transport-ingress popup capture, bounded context diagnostics, request completion
retention, attempt/trace/HTML integration, four native regression groups and a
two-case pinned Playwright reference. Preserve and finish this work rather than
starting a second implementation. Earlier focused runs passed 152 unit checks
and all four popup groups on full Chrome/Firefox, but subsequent edits still
need verification. These draft runs do not replace the committed baseline above
or establish task completion.

Before checking B04, resolve the pending-capture overflow case: eviction must
not leave diagnostics reporting failed adoption if a queued adopter later
succeeds. Verify failed driver initialization followed by more native logs and
closure, request settlement after observation-budget exhaustion, cancellation,
and resource release. Complete public API/examples and serialization migration
notes, document capture limits, register the reference script, inspect the
expanded portable HTML report, then run the required regression gates and commit.
Keep B04 unchecked until these checks pass. B19 follows and must retain optional
native data through every attempt report.

The scope of the longer session is **all 18 open tasks below**. Completed tasks
remain regression requirements. The deferred projects are future work; completing
this checklist means practical parity within the stated engine capabilities,
not complete Playwright compatibility.

## Remaining work at a glance

This is an index of the existing checkboxes, not a second set of tasks. Follow
the detailed acceptance criteria later in this document and record evidence
there. G04 applies during every phase and closes after the final audit.

| Order | Task | Implementation result | Dependency or capability gate |
|---|---|---|---|
| 1 | B04 | Startup popup logs, errors and requests without adoption gaps | B01; existing popup and callback regressions |
| 2 | B19 | Structured errors and bounded console value previews | Native metadata; page/context/report serialization |
| 3 | B16 | Context owner access and reliable connection state | Weak ownership; disposal and transport-close tests |
| 4 | B17 | Locator descriptions in steps and operation errors | Existing automatic step/source recording |
| 5 | B12 | Attempt-owned soft assertions that affect test results | Retry and parallel-attempt isolation |
| 6 | B14 | Effective project settings and read-only resolved configuration | Library/CLI precedence and serialization |
| 7 | B13 | Fixture timeouts and one shared teardown budget | Runner cancellation, failed setup and reverse teardown |
| 8 | B09 | Supported screenshot options with reversible temporary changes | Native capture capabilities on each engine |
| 9 | B10 | Stable screenshot comparisons, paths and update modes | B09; project/path settings from B14 |
| 10 | B18 | Bounded ARIA snapshots with optional boxes and state | Existing DOM approximation; no full ARIA/YAML implementation |
| 11 | B11 | Searchable portable reports with per-attempt network diagnostics | B02/B03 complete; B19; browser visual inspection |
| 12 | C01 | Validated PDF options | Chromium; explicit Firefox unsupported result |
| 13 | C02 | Bounded captured-body bytes, text and JSON helpers | B02/B03 complete; Chromium body capture |
| 14 | C03 | Coverage navigation/source options and restart lifecycle | Chromium JS/CSS coverage |
| 15 | C04 | HAR matching, not-found and supported content options | A14 complete; B06; engine body/rewrite capabilities |
| 16 | C05 | Supported media/device emulation options and resets | Chromium native capabilities |
| 17 | C06 | Independently detachable CDP target sessions | Chromium transport and session ownership |
| 18 | G04 | Final lifecycle, compatibility and resource-release audit | Evidence from every completed phase |

## Concrete implementation deliverables

This expands the existing task IDs into implementation starting points. It adds
no duplicate checkboxes. Paths below are relative to `crates/ferrite-e2e/src`
unless another package is named. API names remain subject to the existing Rust
contracts and pinned behavior; prefer extending existing options and helpers.

| Task | Start in | Deliverable to review |
|---|---|---|
| B04 | `browser.rs`, `driver.rs`, `context.rs` | Buffer and adopt earliest popup observations once, including a popup that closes immediately; bound buffers and release listeners. |
| B19 | `event.rs`, `driver.rs`, `report.rs` | Preserve optional structured errors and bounded argument previews from native events through page/context history and every attempt report. |
| B16 | `browser.rs`, `context.rs`, `driver.rs` | Expose context owner access using weak ownership; report unexpected transport closure accurately and verify disposal remains safe. |
| B17 | `locator.rs`, `expect.rs`, `runner.rs` | Add locator descriptions with defined clone/chaining behavior; include labels in automatic steps and operation failures without duplicating steps. |
| B12 | `expect.rs`, `runner.rs`, `report.rs` | Collect soft failures per attempt, fail the attempt automatically and preserve source/message metadata through retries and cleanup. |
| B14 | `config.rs`, `runner.rs`, `report.rs`; CLI/config packages | Resolve project filters, repetition and output/snapshot paths consistently; expose immutable effective settings and verify library/CLI precedence. |
| B13 | `runner.rs`, `operation.rs` | Bound fixture setup/teardown and share the enclosing cleanup deadline across hooks and reverse teardown; preserve all relevant failure diagnostics. |
| B09 | `page.rs`, `locator.rs`, `driver.rs` | Validate supported clip/scale/background/mask/style options and restore temporary changes on success, failure and cancellation. |
| B10 | `snapshot.rs`, `expect.rs`, `config.rs` | Compare successive stable captures within one budget; resolve baseline paths/update modes and attach expected/actual/diff artifacts. |
| B18 | `dom.js`, `locator.rs`, `snapshot.rs` | Produce bounded structured ARIA snapshots with optional boxes/state and deterministic truncation; document DOM approximation limits. |
| B11 | `report.rs`, `bundle.rs` | Add search/status/project filters and per-attempt network summaries; verify escaping, retries, large/empty reports and relocated artifact links visually. |
| C01 | `page.rs`, `driver.rs` | Validate PDF option combinations and inspect actual generated page dimensions/content on Chromium; assert Firefox's unsupported result. |
| C02 | `network.rs`, `driver.rs` | Expose bounded captured-body bytes/text/JSON with distinct empty, missing, truncated and failed states; retain explicit Firefox unavailability. |
| C03 | `coverage.rs`, `driver.rs` | Define navigation reset/source options and stop/restart behavior; verify anonymous scripts and resource release on Chromium. |
| C04 | `har.rs`, `routing.rs` | Apply shared URL matchers and explicit miss policy to supported HAR content/timing options; test duplicate URLs, redirects and binary replay. |
| C05 | `page.rs`, `driver.rs` | Add only native supported media/metrics/user-agent options; verify observable values and resets, including explicit unsupported errors. |
| C06 | `cdp.rs`, `driver.rs` | Own target sessions independently; detaching one must settle its pending calls while other sessions/pages keep working. |
| G04 | `operation.rs`, affected tests and parity documents | Audit every new API's deadlines/cancellation/disposal/retries, resource release and serialization; reconcile the complete test inventory and matrix. |

The next delivery is the remaining diagnostics group **B04/B19** (B01 complete), then the runner group
**B16/B17/B12/B14/B13**, and the capture/report
group **B09/B10/B18/B11**. Implement the six C extensions after checking native
capabilities, and finish with G04. Each delivery should have usable public APIs,
examples and verified behavior before its implementation commit.

## Scope and tracking

### Session handoff

Resume at the first unchecked task in the remaining-work index. Review the
working tree before editing: an unfinished implementation may already exist.
Keep a draft task unchecked until its acceptance criteria and required gates
pass, even when its focused tests are green. Preserve its useful changes and
finish verification before starting a dependent task.

Use the pinned corpus to decide behavior differences and the capability table
to decide backend support. Every phase should leave a reproducible checkpoint:
implementation and examples, native evidence, documented Rust differences,
updated matrix, accurate checkboxes and a commit. Close G04 only after the final
audit across all phases. The copyable implementation request at the end of this
file includes the whole remaining scope.

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

Original baseline evidence: the implementation passed 265 E2E checks plus 23
CLI/configuration checks, with strict Clippy. These are Ferrite regressions;
they do not constitute differential Playwright conformance coverage.

## Verified implementation checkpoints

- `5c2c7aa`: G01–G03 and A01–A06. All 269 E2E checks and 23 CLI/configuration
  checks passed; the expanded native conformance groups and Send runner integration
  passed on installed Chromium and Firefox, with strict Clippy and formatting checks.
- `ab28b99`: A07–A10. All 273 E2E checks and 23 CLI/configuration checks
  passed; expanded native callback regressions additionally passed on full Chrome
  and Firefox, including zero deadlines, enclosing runner budgets/retries,
  cancellation and context-init ordering. Strict Clippy, formatting, generated
  matrix and local evidence links verified. Fixed Firefox child-frame adoption
  and subscribed to Chromium main-world events before enabling Runtime.
- `8a47459`: A11. Native daily API group passed on full Chrome and Firefox;
  strict E2E/CLI Clippy, formatting and regenerated matrix passed. Combined
  verified regression inventory is now 274 E2E plus 23 CLI/configuration checks.
- `bb25a52`: A12. All 276 E2E checks passed on installed Chromium Headless Shell
  and Firefox, including action cancellation, native input, frame scaling and
  legacy click/drag regressions. Strict E2E/CLI Clippy, formatting and regenerated
  matrix passed; combined verified inventory is 299 checks. Transform/cross-origin
  limits and ClickOptions literal migration are documented.
- `d9d19c8`: A13. All 280 E2E checks passed on installed Chromium Headless Shell
  and Firefox; the three readiness groups additionally passed with full Chrome
  and Firefox. Strict E2E/CLI Clippy, formatting and regenerated evidence links
  passed. Shared deadlines/defaults, deferred scripts, redirects/replacement,
  frame readiness, short HTTP bursts, cancellation, disposal and runner retries
  verified; combined regression inventory is 303 checks.
- `a900523`: A14. Pinned Playwright 1.63.0 reference records 22 URL cases;
  shared assertion, route/removal, future-page context handlers and HAR filters
  passed native Chromium/Firefox groups. Existing seven network regressions,
  140 unit checks and related conformance/readiness/wait groups passed; strict
  E2E/CLI Clippy and formatting passed. Combined verified inventory: 283 E2E
  plus 23 CLI/configuration checks. Legacy string glob contracts remain intact.
- `c433d07`: A15. Five actual pinned Playwright function cases and three native
  function-wait groups passed on full Chrome/Firefox, along with the three core
  conformance groups. Strict E2E/CLI Clippy, formatting and generated links passed.
  Combined verified inventory: 286 E2E plus 23 CLI/configuration checks. Native
  promise-return truthiness, polling cadence, cyclic live handles, frame JSON,
  cancellation/drop cleanup, navigation and enclosing runner retries verified.
- `fd55008`: A16. Native frame lookup regression passed on full Chrome/Firefox,
  with strict E2E/CLI Clippy, formatting and generated evidence links. Main/native
  tree identities, nested frames, base URL matching, predicates, navigation,
  replacement/detachment, closed-page errors and Firefox name absence verified.
  Combined verified inventory: 287 E2E plus 23 CLI/configuration checks.
- `157ed99`: B15. Both completed-download groups passed: native 2 MiB binary
  downloads on full Chrome/Firefox and focused I/O lifecycle/error cases. Strict
  E2E/CLI Clippy, formatting and regenerated evidence links passed. Reading,
  chunked streaming, source identity after page closure, zero/caller cancellation,
  recorded failures, missing files and non-NotFound deletion errors verified.
  Combined verified inventory: 289 E2E plus 23 CLI/configuration checks.
- `1a9e875`: B02/B03. All 295 E2E checks and 23 CLI/configuration checks passed
  with strict E2E/CLI Clippy, formatting and generated evidence links. Four typed
  metadata groups additionally passed on full Chrome/Firefox. Concurrent requests,
  child frames, redirects, duplicate cookies, JSON/form text, streaming completion,
  HTTP/transport errors, page closure, disconnect, zero/caller/enclosing deadlines
  and runner retries verified. Bounded raw-header correlation, unavailable metadata
  and weak history are explicit; bodies remain C02 and route header forwarding B07.
- `eb14f7b`: B07. Native API/JSON/binary forwarding and same-URL synthetic
  redirect regression passed on full Chrome and Firefox 156/157. All 143 units,
  four metadata groups, seven legacy network groups and 16 related conformance/
  context/matcher/wait/upload/console groups passed, with strict E2E/CLI Clippy,
  formatting and generated links. Combined verified inventory: 297 E2E plus 23
  CLI/configuration checks. Native acknowledgement/drop/budget/ordering covered;
  route-provided headers have an explicit source flag and serde migration note.
  Firefox's absent earlier synthetic redirect response settles unavailable.
- `c644327`: B05. All 302 E2E and 23 CLI/configuration checks passed; five
  lifecycle groups additionally passed on full Chrome/Firefox 157. Six actual
  pinned Playwright route cases record removal release semantics and invocation
  limits. Native stage/shutdown races, independent handlers, errors/panics,
  weak capture release, cancellation, disposal, retries and disconnect verified.
  Strict Clippy, formatting and regenerated evidence links passed. Combined
  verified inventory: 325 checks. Continue with B08 then B06.
- `ffb5147`: B08. Pinned 19-case API reference and five API fidelity groups passed
  on full Chrome/Firefox 157. HTTP/HTTPS fixtures, replay, auth/origin filtering,
  redirects, retry/error classification, cookies, disposal and runner budgets verified.
  Complete target inventory: 307 E2E plus 23 CLI/configuration checks, with strict
  Clippy, formatting and regenerated links. Broad validation completed in two
  batches after process termination; combined verified inventory: 330 checks.
  Continue with B06, then B01. G04 remains open for the remaining additions.
- `c88c14b`: B06. All 318 E2E and 23 CLI/configuration checks passed in bounded
  complete-target batches, with strict Clippy, formatting and regenerated links.
  Eight route-options groups additionally passed on full Chrome 153/Firefox 157;
  18 pinned cases record actual Playwright fulfillment/fetch/CORS behavior.
  Weak context ownership, binary capture/absence, overrides, file/JSON/header
  precedence, TLS/proxy/auth/cookies, budgets/retries/disposal/disconnect and callback
  release verified. Migration notes and supported Firefox limits are documented.
  Combined verified inventory: 341 checks. Continue with B01, B04 and B19.
- `f8c12de`: B01. All 325 E2E and 23 CLI/configuration checks passed in bounded
  complete-target batches, with strict Clippy, formatting and 633 source anchors
  verified. Three event groups additionally passed on full Chrome 153/Firefox 157;
  four actual pinned Playwright cases compare native frame identity/history,
  readiness and dialog closure. Single context forwarding, optional fields,
  zero/live-default/caller/enclosing budgets, retries, disposal and transport
  wake-up (including empty contexts) verified. A protocol fixture verifies
  interrupted initialization releases its listener without closing transport.
  Owned snapshots, Firefox capability probes and current-target Chromium/OOPIF
  limits remain explicit. Combined inventory: 348 checks. Continue with B04/B19.
- G04 remains open until lifecycle coverage across all additions is complete.

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
  Include native event-channel lag/listener exit, partial page/popup setup,
  detached-frame in-flight requests and empty-context disconnect. Waiters must
  settle explicitly when their observation source is lost.
  Audit popup-pump lag and pending-capture eviction as well: every paused Chromium
  target must be resumed or closed, and retained adoption outcomes must agree
  with actual setup results. A bounded buffer must not create an unbounded wait.

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

- [x] **A07 — Async Page callbacks (M).** Add callbacks returning a future and
  `E2eResult` without blocking the dispatch pump. Done when JavaScript awaits
  results, Rust errors/panics reject predictably, concurrent calls remain
  independent and navigation/disposal cancels or settles pending work.
  Evidence: `ab28b99`; independent bounded async Page calls, Rust errors/creation and future panics, navigation, scoped caller cancellation and close tested in `callback_lifecycle.rs` on both engines.

- [x] **A08 — Context-wide exposed functions (L; needs A07).** Register callbacks
  for current/future pages and adopted popups. Done when the function is available
  to startup scripts and after navigation, registrations stay context-isolated
  and closing a context releases the associated pumps and callback state.
  Evidence: `ab28b99`; current/future pages, application and context-init startup scripts and adopted popups tested on Chromium/Firefox. Chromium pauses popup targets; Firefox requires native scoped preloads (136+), rejects unknown/older capability and isolates contexts.

- [x] **A09 — Binding caller metadata (M; needs A07/A08).** Add Page/context
  bindings carrying owning context, page and supported frame identity. Done when
  two pages and same-origin frames report the correct caller and async results
  and failures behave as in A07. Defer cross-origin/OOPIF binding dispatch.
  Evidence: `ab28b99`; Page/context async bindings return owning context/page/native frame identities; main and same-origin child callers on multiple pages/popups verified. Cross-origin/OOPIF and remote handle dispatch remain deferred.

- [x] **A10 — Callback registration/removal lifecycle (M; needs A07–A09).** Define
  duplicate-name behavior and add removal handles or named removal helpers.
  Done when removal affects current/future documents, pending calls settle and
  unrelated bindings keep working; removed preload scripts cannot reintroduce
  the callback after navigation.
  Evidence: `ab28b99`; duplicate names and competing page/context registration are defined; named removal aborts/rejects pending work and removes native preloads. Re-registration, navigation, unrelated callbacks and context capture release tested.

### Daily API gaps

- [x] **A11 — Filtered cookie clearing (S).** Extend existing clear-cookies with
  name/domain/path exact and regex filters. Done when same-name cookies on
  different domains/paths are handled independently and browser/API linked
  stores remain synchronized. Preserve unsupported partition metadata honestly.
  Evidence: `8a47459`; `daily_api.rs` verifies native exact/regex AND filters, same-name domain/path independence, untouched attributes and linked API requests on both engines. Empty filters, zero timeout and cancellation covered. Opaque Chromium partition deletion fails explicitly; portable partition filter fields remain unsupported.

- [x] **A12 — Consistent action options (M).** Extend click/hover/check/drag
  options with supported positions, modifiers, trial readiness and scoped
  timeout overrides. Done when trial actions produce no input, modifiers are
  released after errors/cancellation and actionability stays consistent on both
  engines. Add options to existing methods rather than recreating timeout defaults.
  Evidence: `bb25a52`; `action_options.rs` verifies positions, modifiers, trial silence,
  checkbox/drag state, held-key preservation, scoped timeouts and cancellation cleanup
  on both engines. Same-origin offsets/positive axis scaling supported; cross-origin
  and rotated/reflected/perspective frame coordinates return explicit errors.
- [x] **A13 — URL wait readiness options (M).** Add a wait-until option to the
  existing matching/predicate URL waits. Done when URL matching and requested
  document readiness share one budget across redirects, same-document history,
  hashes and supported frames. Specify NetworkIdle limits explicitly.
  Evidence: `d9d19c8`; `url_readiness.rs` covers shared URL/readiness budgets on both
  engines. Defaults inherit navigation settings; legacy helpers retain URL-only
  readiness. DOMContentLoaded uses native navigation timing; NetworkIdle requires
  Load and 500ms observed page HTTP quiet, detects short bursts and rejects frames.
  Replaced Chromium documents leave idle accounting without fabricated terminal events.
- [x] **A14 — Shared URL matching across APIs (M).** Reuse UrlMatcher in page URL
  assertions, route selection and HAR filters while preserving legacy string
  contracts. Done when base-URL resolution, escaped globs, braces, zero-segment
  double-stars and regex anchoring have pinned conformance cases; invalid patterns
  fail before registration. Needs G03; coordinate with A13 and B05.
  Evidence: `a900523`; `shared_url_matching.rs` verifies shared matcher routing,
  hit limits, removal, assertions and HAR filters on both engines; the pinned
  corpus checks base paths, escaping, braces, double-stars and regex anchors.
  Relative matchers require a base URL; Rust regex and strict malformed-glob
  differences are documented. In-flight removal policy remains B05.
- [x] **A15 — Function-wait arguments, polling and results (M).** Extend existing
  function waits with JSON arguments and interval/animation-frame polling; add a
  result helper where existing handle support permits it. Done when promises,
  falsy-to-truthy transitions, returned values and cancellation are tested.
  Frame-scoped remote handles remain a capability-dependent extension.
  Evidence: `c433d07`; `function_wait.rs` compares the five recorded cases and
  verifies native RAF/interval scheduling, concurrent results, errors and lifecycle.
  Page handle/Frame JSON companions preserve legacy unit expression helpers.
  User-created promises/side effects cannot be force-canceled; owned pollers stop.
- [x] **A16 — Frame lookup conveniences (S).** Add a dedicated main-frame helper
  and URL matcher/predicate lookup over existing frame handles. Done when no
  match, navigation and detached frames have explicit behavior, and same-origin
  nested/replacement cases work on both engines. Check native name availability;
  selector-free OOPIF traversal stays deferred.
  Evidence: `fd55008`; `frame_lookup.rs` verifies dedicated main-frame and shared
  matcher/predicate helpers. Lookups are snapshots; existing current_url and
  is_detached semantics are explicit. Firefox supplies no native name metadata.

## B — Broader follow-ups on the existing backends

Work through these after A; independent items may be completed earlier when
their dependencies are ready. References include
[network request metadata](https://playwright.dev/docs/api/class-request),
[response lifecycle](https://playwright.dev/docs/api/class-response),
[API requests](https://playwright.dev/docs/api/class-apirequestcontext),
[visual comparisons](https://playwright.dev/docs/test-snapshots) and
[reporters](https://playwright.dev/docs/test-reporters).

### Network and event lifecycle

- [x] **B01 — Missing frame/load/dialog lifecycle events (M).** Extend Page and
  context events for supported frame attachment/navigation/detachment, DOM/load
  readiness and dialog closure. Done when events have page/frame identity and
  ordered, deduplicated lifecycle tests. Preserve already implemented dialog,
  popup, download and page-close forwarding; audit it under G01 first.
  Evidence: `f8c12de`; `lifecycle_events.rs` compares four pinned cases on both
  engines and verifies stable native identities, main readiness, repeated
  same-URL history/set-content events, child-first deduplicated detach, replacement,
  dialog fields and single context forwarding. Wait/default/cancellation/
  disposal/retry/disconnect checks and interrupted-init listener release pass.
  Payloads are owned snapshots; optional Firefox subscriptions/names and CDP
  dialog metadata stay explicit. Chromium observes its current target, with
  OOPIF adoption deferred and `swap` meaning session departure. B04 remains open.
- [x] **B02 — Rich request/response metadata wrappers (L).** Build typed wrappers
  over current observations: headers/arrays, method, post-data JSON, status,
  resource type, frame/page references, failure and redirect links where native
  data exists. Done when concurrent requests and redirect hops retain identity,
  old RecordedRequest callers keep working and unavailable fields remain optional.
  Body retrieval is separate in C02; this item must work without Firefox bodies.
  Evidence: `1a9e875`; `network_metadata.rs` and bounded FIFO/history units verify
  live Request/Response wrappers, typed waits/events, concurrent per-hop identity,
  frame/page references, JSON/form parsing, redirects and legacy compatibility.
  Absent/folded Firefox fields, header completeness and history truncation stay explicit.
- [x] **B03 — Response completion helpers (M; needs B02).** Expose completion of
  a response separately from receiving its headers. Done when delayed streaming
  bodies, HTTP error statuses, redirects, transport failures and disposal settle
  correctly. Extend existing RequestFinished/RequestFailed observations; no
  response-body capture is required to implement completion.
  Evidence: `1a9e875`; the native streaming/failure/lifecycle and runner/disconnect
  groups verify independent header/completion phases, HTTP error success, failures
  before/after headers, disposal and already completed observations after closure.
  Defaults, zero/caller/enclosing deadlines, retries and transport wake-up covered.
- [ ] **B04 — Earliest popup diagnostics (M).** Preserve console/error/network
  observations emitted before a popup is fully adopted into the context. Done
  when startup-script logs, immediate requests and immediate closure retain
  source identity without duplicate forwarding or listener leaks.
  Bind source ownership before asynchronous driver/`finish_page` work and retain
  events emitted before adoption starts. Handle closed/failed adoption without
  dropping observations; bound pending state and expose truncation/failed capture.
  Verify startup console/errors, immediate HTTP requests, immediate closure,
  concurrent popups and cancellation/disposal, including page/context history
  and attempt-report retention. B01's event graph/forwarding is the baseline;
  do not replace missing native data with DOM-derived guesses.
  Draft handoff: `popup_capture.rs`, `tests/popup_diagnostics.rs` and
  `scripts/e2e-conformance/popup-reference.{mjs,json}` exist uncommitted. Finish
  the verification and documentation listed above before recording evidence.
- [x] **B05 — Route removal and in-flight handler behavior (L).** Extend existing
  unroute/unroute-all with documented wait/ignore-error behavior. Done when active
  async handlers settle or cancel according to policy, request ordering remains
  defined and disposal cannot hang. Reuse the completed A14 matching; apply G04
  while implementing this item. Required semantics:

  - Default removal returns without waiting for callbacks and releases active
    requests to the network; callbacks continue, late actions are discarded and
    later errors remain observable.
  - Wait removal awaits already running calls within the caller/enclosing budget.
  - Ignore-errors removal returns without waiting and suppresses later errors
    from the removed calls. It does not implicitly cancel those calls.
  - Any explicit cancellation extension must be identified as a Rust API choice.
    Removed handlers receive no new calls; unrelated routes continue working.
  - Route changes must not abort unrelated active handlers. Slow calls must not
    block unrelated requests; finite hit limits remain correct under concurrency.
  - Cover page/context removal, fallback ordering, handler errors/panics, deadline
    expiry, cancellation, disposal and release of retained callback state.

  Reference: [unrouteAll behavior](https://playwright.dev/docs/api/class-page#page-unroute-all).
  Evidence: `c644327`; six actual pinned Playwright 1.63.0 route cases and five
  native lifecycle groups on full Chrome/Firefox 157. All 302 E2E checks plus
  23 CLI/configuration checks passed, with strict E2E/CLI Clippy, formatting and
  regenerated matrix links. Independent dispatch, shared atomic limits/fallback,
  default/wait/ignore/cancel, registration churn, panics, page/context/native
  closure, zero/default/caller/enclosing budgets, retries and disconnect verified.
  Empty pumps await native shutdown; Chromium tracks response stages and Firefox
  drains queued pauses. Rust registration priority, no callback-identity removal
  and already-issued native command cancellation limits remain explicit.
- [x] **B06 — Route fetch/fulfill option fidelity (M).** Extend supported route
  operations with method/body/header overrides and bounded fetch options using
  existing ApiClient machinery. Done when redirect limits, retries, binary bodies,
  duplicate headers and status overrides have focused regressions. Distinguish
  out-of-band HTTP fetch URL overrides from native intercepted-request URL
  rewriting; Firefox's unsupported native rewriting must stay explicit. Implement after
  B08 so HTTP options share one transport contract and total operation budget.
  Verify header/body precedence, JSON content type, cookies/authentication and
  context-linked client behavior against the pinned corpus before documenting
  equivalence. Reuse the intercepted request's owning context without introducing
  an ownership cycle. When Firefox does not supply the original request body,
  keep that absence explicit and distinguish a supplied body override from
  unavailable bytes; never silently replay a missing payload as empty.
  Verify response/body/JSON/file/header precedence, inferred content types and
  content-length behavior against actual pinned results. Invalid combinations
  must fail before resolving the native interception. Fetch must not implicitly
  continue or fulfill the intercepted request. Retained request descriptions
  must not keep a disposed page/context alive.
  Preserve B07's duplicate-header and native acknowledgement tests.
  Reference: [route fetch options](https://playwright.dev/docs/api/class-route#route-fetch).
  Evidence: `c88c14b`; 18 actual pinned Playwright 1.63.0 Chromium cases and
  eight route-options groups on full Chrome 153/Firefox 157. Method/header/body/JSON
  overrides, base-relative/HTTP(S) URLs, original binary capture or explicit
  unavailability, inherited response/file/status/header precedence, cookies/CORS,
  context TLS/proxy/auth, budgets/cancellation/disposal/transport loss and runner
  retry capture release verified. Complete disjoint inventory: 145 units, 170
  integration checks across all 22 targets and three doctests (318 E2E), plus 23
  CLI/configuration checks. Strict Clippy, formatting and regenerated links passed.
  Regular-file paths/final-status validation, private RouteInfo constructor
  migration, source Content-Length retention and JSON truthiness are explicit;
  Firefox native rewriting and automatic compressed decoding remain unsupported.

### API testing, captures and reports

- [x] **B07 — Header convenience APIs with duplicate preservation (S).** Add
  headers-array, all-values and case-insensitive lookup helpers to API/network
  response types. Done when repeated Set-Cookie and other duplicate headers
  survive serialization, lookups and route forwarding. Coordinate with B02/B06.
  Evidence: `1a9e875` adds shared header helpers; `eb14f7b` verifies API transport,
  serialization, binary route forwarding, Set-Cookie with Expires commas and native
  cookie storage on both engines. Acknowledged route-supplied arrays supplement
  omitted/folded native fields with an explicit source flag; raw headers take
  precedence. Same-URL redirect identity, rejection/drop and bounds covered.
  Firefox 156/157 omits the earlier synthetic redirect's response/completion:
  that request has no fabricated Response and settles unavailable. Native
  generic comma folding remains intact; B06's fuller fetch/fulfill options are
  now verified.
- [x] **B08 — API request option/redirect fidelity (M).** Extend current requests
  where the audit shows missing semantics: incompatible body options, retryable
  transport errors, credential-origin rules, redirect methods and status failure
  handling. Done when local HTTP/HTTPS fixtures prove behavior, linked cookies
  stay synchronized and errors/cancellation do not retain buffers or connections.
  Mutually exclusive payload validation already exists in `ApiClient::fetch_with`;
  preserve and verify it rather than adding a duplicate API. Exercise 301/302/303
  versus 307/308 method/body handling, zero/exceeded redirect limits, cross-origin
  credential stripping, cookies at each hop and retryable transport failures.
  Distinguish transport retries from HTTP status failures and keep retries/body
  reads within one deadline. Decide any intentional Rust differences using the
  pinned reference, not reqwest defaults alone.
  Evidence: `ffb5147`; 19 actual pinned Playwright 1.63.0 HTTP cases and five
  API fidelity groups, additionally verified with full Chrome 153/Firefox 157.
  HTTP/HTTPS, binary/multipart replay, conflict validation, redirect limits/methods,
  cross-origin headers, hop cookies, Basic origin/challenge rules, reset backoff,
  status/body/TLS failures, caller/client/context cancellation and enclosing
  runner retries verified. Canceled retry fixtures release their captured state.
  All 307 E2E checks and 23 CLI/configuration checks verified, with strict Clippy,
  formatting and regenerated links. The complete target inventory was verified
  across a 300-check broad run and seven resumed checks after process termination;
  see the parity report for the initial pointer timing failure and successful
  rechecks. Legacy preemptive Basic/default-header precedence, normalized credential
  origins, client-scoped TLS and owned response buffers remain explicit Rust
  differences; no compression decoding or arbitrary request streams added.
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

- [x] **B15 — Completed download streaming and ownership (S).** Add async reading
  of completed files and owning-page identity without duplicating save-as/delete.
  Done when binary/large files, missing files and cancellation work on both
  engines; deletion is idempotent for missing files but reports other filesystem
  errors. Per-download active cancellation and early failure metadata require
  native capability checks; do not infer unavailable Firefox metadata from filenames.
  Evidence: `157ed99`; `download_stream.rs` verifies completed-file read/stream
  companions and owning page ID without retaining a live page. Options bound file
  reads/opening; subsequent Tokio stream reads can be wrapped in caller cancellation.
  Firefox active download metadata and per-download cancellation remain deferred.
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
| Cross-origin/OOPIF lazy selectors, bindings and unified target events | Realm/target routing and frame replacement/identity need a dedicated backend design. B01 observes the current Chromium target; session swap is not unified DOM-frame removal. |
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

### Recommended remaining phases

Use this order for the next long implementation session. The original task IDs
stay stable so commits, regressions and follow-up sessions can refer to them.
G04 is a check throughout the session and a final audit; it does not require
waiting for every future feature before starting independent work.

| Phase | Tasks | Outcome and reason for this order |
|---|---|---|
| 1. Network observations | B02, B03 — complete | Typed per-hop request/response identity and completion independent of body capture. Verified in `1a9e875`; continue with phase 2. |
| 2. HTTP and routing correctness | B05/B06/B07/B08 — complete | Context-linked fetch/fulfill options verified in `c88c14b`. Preserve shared HTTP, route lifecycle, duplicate-header forwarding and pinned precedence regressions. |
| 3. Native event diagnostics | B01 complete; B04, B19 remaining | Preserve verified frame/load/dialog observations (`f8c12de`); add earliest popup traffic and structured error/console data with consistent ownership and ordering. |
| 4. Runner and developer APIs | B16, B17, B12, B14, B13 | Add ownership and locator descriptions, attempt-owned soft assertions, effective configuration, then fixture/shared cleanup budgets. Changes to budgets need broader runner regressions. |
| 5. Captures and reports | B09, B10, B18, B11 | Implement capture options before stabilized comparisons; add bounded ARIA output and searchable per-attempt reports using the earlier network/error data. |
| 6. Supported backend extensions | C01, C02, C03, C04, C05, C06 | Extend PDF, captured bodies, coverage, HAR, emulation and scoped CDP sessions. Check capability before accepting each option; verify explicit errors on the other engine. |
| 7. Final lifecycle and compatibility audit | G04 | Verify cancellation, zero/enclosing deadlines, retries, disposal and released resources across the additions, then run the complete regression and documentation gates. |

Within a phase, prefer correctness and lifecycle work over convenience methods.
Independent tasks may move earlier when their prerequisites are verified. A
native capability gap should leave a precise remaining subset, rather than
blocking unrelated work or becoming a successful no-op. B15 is already complete
and does not need to be implemented again.

### Record for every phase

Keep the relevant checkbox evidence concise and include:

- Implementation commit and public API/example changes.
- Focused regression names and actual Chromium/Firefox execution.
- Timeout, cancellation, disposal and serialization compatibility results.
- Native limitations, remaining subsets and the next ready task IDs.

A phase is reviewable when its implementation, examples and evidence agree.
Finish with the full suite after cross-cutting transport/runner changes. Keep
completion counts based on checked tasks; an unsupported or blocked subset
does not count as complete unless the task explicitly excludes it.

1. Recheck current code and native capability evidence under G01/G02. Do not
   rebuild features added since this document was created.
2. All A tasks are complete. Continue with the remaining-work index and phase
   order above, respecting dependencies. Complete independent B work as
   prerequisites become available; implement C
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
export FERRITE_CHROMIUM_PATH=/path/to/chromium
export FERRITE_FIREFOX_PATH=/path/to/firefox
export FERRITE_E2E_REQUIRE_BOTH_BROWSERS=1
export TMPDIR=/path/to/writable-temp
cargo test -p ferrite-e2e --no-fail-fast -- --test-threads=4
cargo test -p ferrite-cli -p ferrite-config
cargo clippy -p ferrite-e2e -p ferrite-cli --all-targets -- -D warnings
cargo fmt -p ferrite-e2e --check
python3 scripts/playwright-parity/build_matrix.py
git diff --check
```

For long-running validation, run the same complete target inventory in bounded
batches and retain each command's exit status and result count. Include units,
every integration target and doctests; an interrupted command is not a pass.
Record any initial failure and the change or repeat that resolved it. A single
focused pass must not replace the broader checks required by a transport or
runner change. For documentation-only checkpoints, validate task counts, local
links and the diff; do not present historical browser runs as new verification.

The generator needs its pinned upstream documents; use its documented `--fetch`
or `--upstream` option when the cache is absent. Changes to CLI/configuration
also need formatting checks for the modified packages.

Copyable request for the implementation session:

> Implement every open task in E2E-PARITY-TODO.md using the recommended remaining
> phases, dependencies and completion criteria. Preserve the already verified
> features. Implement all practical
> features supported by the existing Chromium/Firefox backends. Keep the deferred
> substantial projects outside scope; document any blocked subset and continue
> independent ready tasks. Verify native behavior, update the examples and parity
> matrix, keep the TODO accurate, and commit verified changes at phase boundaries.
