# Ferrite E2E implementation TODO

Created: 2026-09-29, after `cbcfc8e`. Reference: Playwright **v1.63.0**.
Session plan refreshed: 2026-09-30, with B10 capture, path, diagnostic, native font/animation, final-poll diagnostic, active image/read and staged baseline increments.
Expanded on request for a longer implementation session; B09/B13/B14 and D01/D02 are verified.
Uncommitted implementations are not counted complete.

The initial G/A foundations are complete except for the final G04 audit. Follow
the remaining-work index below through B/D improvements and supported C extensions.
This backlog contains **51 tasks (37 complete, 14 remaining)**: four foundations,
16 core tasks, 19 follow-ups, six optional extensions and six practical additions.
All A tasks, B01–B09/B12–B17/B19 and D01/D02 are verified; continue with
stable screenshot assertions, reports and supported C extensions. The ordering
and effort assessments are recommendations based on the current source and
parity audit.
D01–D06 add bounded improvements found in the missing/partial member inventory;
they do not imply that every upstream member needs a separate Rust API.

The target is a useful, reliable Rust API on the existing Chromium/CDP and
Firefox/BiDi backends. Full Playwright compatibility remains a separate,
substantially larger undertaking. The complete member inventory remains in
[PLAYWRIGHT-API-MATRIX.md](PLAYWRIGHT-API-MATRIX.md); current behavior and engine
limits remain in [PLAYWRIGHT-PARITY.md](PLAYWRIGHT-PARITY.md).

## Recommended next implementation

Start **B10 — stable screenshot assertions and snapshot paths**, then **B18/B11**.
B09 is verified in `1c37e31`: page and locator captures accept supported clip,
scale, mask color and temporary style options with one capture budget. Owned
restoration survives cancellation and dropped waits; cleanup failures remain
visible. Chromium transparent PNG preserves the last acknowledged background
set through Page.call; Firefox rejects transparency and normalizes Css output
from its native raster. Captures preserve caller viewport/emulation settings.
The B10 follow-up now finishes finite animations and cancels/resumes infinite
animations through native objects; styles cover reachable same-origin documents
and open shadow roots. Application-realm helpers and traversal remain narrower.

Latest completed-task checkpoint inventory: **405 E2E and 28 CLI/configuration checks**,
433 combined: 180 units, 221 integrations across all 34 targets and four doctests.
Broad batches covered the initial 404 E2E inventory; after adding the trace-step
regression and fixing private trace capture, all 16 related capture/report/step
checks reran on final source. Final routing23, doctests, CLI/configuration,
strict all-target Clippy and formatting passed. Runner/native capture scopes
used full Chrome 153/Firefox 157; core203/browser93/routing23 used matching
Headless Shell/Firefox. This is combined verification evidence, not one fresh
405-check invocation. All native groups required both engines.

Fifty-two actual pinned screenshot cases passed on both engines. Native tests
verify image dimensions/pixels, DPR and scrolling, full-page masks, background
restoration, concurrent captures, typed validation, cancellation/drop/disposal,
real cleanup failures, same-origin iframe/open-root styles and one user trace
step. The reference documents Firefox's native Css/alpha limitations and Rust's
quality/finite-coordinate validation and retained-background differences. One
reference launch timed out during concurrent native work; a final run with a
30-second launch budget passed all cases. One unchanged redirected-header
regression failed initially, then passed in isolation and in the full routing
batch; its cause remains an explicit G04 audit item.

The matrix remains 73 classes/1,018 members: Partial635/Missing327/Equivalent15/
Idiomatic41. Capture mappings now describe these options and engine differences;
they do not imply full compatibility. Current matrix generation, 727 local
Markdown links across the three parity documents and two reference/fixture
READMEs, and 655 source anchors passed for the staged baseline increment.

**Current handoff:** B10 is in progress. The verified increment `499c274` in
`snapshot.rs`,
`snapshot_capture.rs` and `expect.rs` implements successive captures, bounded font
readiness waits, typed control failures, retained last-image artifacts and
`Changed` update mode. Page/locator capture defaults use B09 Css scale, animation
suppression and hidden carets; no unstable baseline is written on expiry.
The 56-case actual pinned runner reference records missing/update/negation and
never-stable behavior differences. Three required two-engine native groups and
virtual-time tests cover the draft; these are focused evidence, not final phase
gates. The original font-wait test uses a held readiness promise; the new
`tests/snapshot_fonts_and_animations.rs` exercises actual held HTTP font responses.

Verified path increment `b34174d` implements browser/project/platform and file/title path
templates, project/assertion precedence, CLI/config/env plumbing and immutable
resolved settings. TestInfo.snapshot_path resolves without creating files, and
snapshot_options supplies standalone helpers with the same identity. Two required
two-engine native groups verify page/locator/text paths and retries; 36 actual
pinned path-only calls document naming differences. This remains partial parity.

Diagnostic increment `b826c56` copies expected/actual/diff images into attempt and
assertion-step attachments, retains previous/stability-diff images for unstable
content and preserves portability after baseline updates/source removal. Native
groups cover retry/soft ownership, exact pixels/dimensions, live events, visible
I/O failures and cancellation/restoration. Generic retry probes suppress intermediate
report images. Verified deferred increment `0509518` now retains the last screenshot
mismatch from the last completed failing outer probe; nested polls transfer
images. Success/control errors and completed pending results discard retained
images; unfinished probes drop their own images without replacing the earlier
completed candidate. Weak attempt ownership and one retained mismatch per invocation
keep deferred retry history from accumulating.

Verified native font/animation increment `e81d744`: seven groups now verify page/locator captures in the main
document, open roots and same-origin frames, fallback opt-out, timeout/cancellation/
disposal, native finite/infinite/zero-rate animation behavior and finish/resume errors.
The original reproducible font fixture is independent of installed system fonts.
The 32 checked actual pinned font/animation observations document matching
finite/infinite/zero-rate pixels, stronger same-origin page font waits in Rust
and observed upstream Chromium locator-stability timeouts.

Verified active image/read increment `92b5a66` moves comparison, Css normalization and baseline
reads off the async runtime with two active callbacks, cooperative cancellation,
shared immutable buffers and encoded/raster/intermediate-size guards. Five added
unit regressions bring the unit inventory to 200, including retaining the last
completed stability assessment while a newer native image awaits CPU work.
Opaque codec/resize/OS phases
are not hard-preempted, and admission does not bound queued bytes or blocking
threads. Verified staged baseline increment `4e35613` compares `Changed` against frozen expected
bytes using scalar tolerances and stages chunked temporary files. The foreground
owner checks the shared deadline before installing; `Missing` cannot overwrite
a competing file; a matching winner is accepted after bounded validation.
Existing/dangling symlink aliases and writable permissions are preserved.
Six added unit groups bring the unit inventory to 206. The 54 focused native
groups passed, including page/locator alias and parallel Missing generation;
22 font/path/artifact/polling groups replayed on final source. Workers never
capture whole SnapshotOptions, whose masks
can own locators/pages. Verified diagnostic increment `2c042e1` now shares immutable
buffers, moves diff rendering and chunked attachment writes into data-only workers,
and installs/publishes in the foreground under a short weak-attempt lifecycle guard.
Matching/update retain their assertion clock; final failure diagnostics share one
additional five-second clock, bounded by enclosing step/test waits and cancellation.
External filename winners survive; expired/sealed publication and the 1,024-name
collision boundary refuse installation and remove temporary handles. Two added
unit groups and the expanded real sink regression bring the unit inventory to 208.
All 54 focused native groups, four doctests and 28 CLI/config checks passed
(294 combined), along with strict Clippy, formatting and matrix/link validation.
The initial broader audit passed 24 remainder targets, then reproduced a native
large-console hang from undrained browser output pipes. Verified lifecycle fix
`ade139b` continuously drains both streams with a 4-KiB stderr tail, retains
readers through shutdown and aborts them without native-owner retention. All
211 units, seven native console/ownership groups and strict Clippy/format checks
passed. Complete phase gates are now rerunning on this source: the 54 focused
native groups plus all 27 remainder targets. B10 remains unchecked until that
audit finishes; G04 still covers the wider final lifecycle review.
Final-only deferred diagnostics for failing outer generic polls are now
implemented, including retry/soft/nested/parallel ownership and visible I/O failures.
The 12 actual pinned screenshot/toPass cases document upstream intermediate image
history, including eventual/nested success and expected-only missing-target images,
which differs from the Rust final-only policy.
Run final complete phase gates, update matrix/capability evidence and commit only
verified work before checking B10. Preserve entry points and document input-struct
migration. B18/B11 follow after B10 is complete.

Preserve D02's strict typed generic probe errors, immediate/final-repeat cadence,
5-second defaults, shared local/enclosing clocks, contextual last mismatches,
non-Send companions and final-only soft/report scopes. Bind an explicit context
cancellation token when an idle generic poll must wake on disposal. Unrelated
locator polling still needs its G04 error/cancellation audit; do not broaden
its semantics without separate evidence.

Preserve B13's ready cleanup after exhaustion, individual pending errors,
reverse dependency release, worker retirement draining, once-only native disposal
and Firefox lifecycle ownership until disposal settles. An already-lost transport
permits idempotent local close without confirming remote native release. Native
protocol budgets still apply; blocking synchronous Rust/filesystem work cannot
be preempted. Capture, trace and early-close errors must remain visible.

Preserve B12's only-Expect soft collection, body-only expected-failure handling,
setup/cleanup visibility, atomic sealing before artifact/context cleanup, weak
ownership and per-attempt modifier scope. AttemptResult now has a serde-defaulted
soft_assertions vector; TestError fields stay unchanged. Preserve B17's one
outer label/error/step/trace and Diagnostic typed causes, and B16's shared owner/
weak context graph and owned base_url getter. G04 remains open for channel lag,
partial setup, detached in-flight requests and the final cross-feature audit.

The scope of the longer session is **all 14 open tasks below**. Completed tasks
remain regression requirements. The deferred projects are future work; completing
this checklist means practical parity within the stated engine capabilities,
not complete Playwright compatibility.

## Remaining work at a glance

This is an index of the existing checkboxes, not a second set of tasks. Follow
the detailed acceptance criteria later in this document and record evidence
there. G04 applies during every phase and closes after the final audit.

| Order | Task | Implementation result | Dependency or capability gate |
|---|---|---|---|
| 1 | B10 | Stable screenshot comparisons, paths and update modes | B09; project/path settings from B14 |
| 2 | B18 | Bounded ARIA snapshots with optional boxes and state | Existing DOM approximation; no full ARIA/YAML implementation |
| 3 | B11 | Searchable portable reports with per-attempt network diagnostics | B02/B03/B19 complete; browser visual inspection |
| 4 | D03 | Explicit artifact retention policies with valid report links | B14/B11; final attempt classification and bundle export |
| 5 | D04 | Run/project metadata and configurable slow-test summaries | B14/B11; existing live/JSON/JUnit/HTML reporters |
| 6 | D05 | Local/session storage enumeration and typed bulk helpers | Existing page storage APIs on both engines |
| 7 | D06 | Socket identity, errors, closed state and bounded typed waits | Existing Chromium socket events; Firefox unsupported |
| 8 | C01 | Validated PDF options | Chromium; explicit Firefox unsupported result |
| 9 | C02 | Bounded captured-body bytes, text and JSON helpers | B02/B03 complete; Chromium body capture |
| 10 | C03 | Coverage navigation/source options and restart lifecycle | Chromium JS/CSS coverage |
| 11 | C04 | HAR matching, not-found and supported content options | A14/B06 complete; engine body/rewrite capabilities |
| 12 | C05 | Supported media/device emulation options and resets | Chromium native capabilities |
| 13 | C06 | Independently detachable CDP target sessions | Chromium transport and session ownership |
| 14 | G04 | Final lifecycle, compatibility and resource-release audit | Evidence from every completed phase |

Immediate delivery: **B10/B18/B11**. B09/B13/B14/D01/D02 are complete.
Capture/report work follows; D03/D04 consume its final artifact/report behavior.
D05/D06 and the supported C extensions can then proceed independently. G04 is
applied throughout and closed last.

### Verified B13 delivery sequence

The implementation followed five steps in `e2e3ecc`: pinned accounting cases,
local fixture limits, shared cleanup clocks, once-only native disposal and
integration/documentation verification. Preserve these outcomes as regressions.
Actual native target/user-context removal, dependency release after failures,
retry worker rebuilds, dynamic zero, cancellation and final run hook accounting
are covered. The upstream comparison remains explicit in
[fixture timeouts](https://playwright.dev/docs/test-fixtures#fixture-timeout),
[test timeouts](https://playwright.dev/docs/test-timeouts) and the pinned corpus;
Rust's separate local limits do not claim upstream's fixture clock semantics.

## Concrete implementation deliverables

This expands the existing task IDs into implementation starting points. It adds
no duplicate checkboxes. Paths below are relative to `crates/ferrite-e2e/src`
unless another package is named. API names remain subject to the existing Rust
contracts and pinned behavior; prefer extending existing options and helpers.

| Task | Start in | Deliverable to review |
|---|---|---|
| B04 | `browser.rs`, `driver.rs`, `context.rs` | Buffer and adopt earliest popup observations once, including a popup that closes immediately; bound buffers and release listeners. |
| B19 | `console.rs`, `page.rs`, `driver.rs`, `report.rs` | Preserve optional structured errors and bounded argument previews from native events through page/context history and every attempt report. |
| B16 | `browser.rs`, `context.rs`, `driver.rs` | Expose context owner access using weak ownership; report unexpected transport closure accurately and verify disposal remains safe. |
| B17 | `locator.rs`, `expect.rs`, `runner.rs` | Add locator descriptions with defined clone/chaining behavior; include labels in automatic steps and operation failures without duplicating steps. |
| B12 | `expect.rs`, `runner.rs`, `report.rs` | Collect soft failures per attempt, fail the attempt automatically and preserve source/message metadata through retries and cleanup. |
| B14 | `config.rs`, `runner.rs`, `report.rs`; CLI/config packages | Resolve project filters, repetition and output/snapshot paths consistently; expose immutable effective settings and verify library/CLI precedence. |
| B13 | `runner.rs`, `operation.rs` | Bound fixture setup/teardown and share the enclosing cleanup deadline across hooks and reverse teardown; preserve all relevant failure diagnostics. |
| B09 — complete | `screenshot.rs`, `page.rs`, `locator.rs`, `driver.rs` | Preserve `1c37e31` capture validation, coordinate/scale semantics, owned restoration and explicit native limitations. |
| B10 | `snapshot.rs`, `expect.rs`, `config.rs` | Compare successive stable captures within one budget; resolve baseline paths/update modes and attach expected/actual/diff artifacts. |
| B18 | `dom.js`, `locator.rs`, `snapshot.rs` | Produce bounded structured ARIA snapshots with optional boxes/state and deterministic truncation; document DOM approximation limits. |
| B11 | `report.rs`, `bundle.rs` | Add search/status/project filters and per-attempt network summaries; verify escaping, retries, large/empty reports and relocated artifact links visually. |
| C01 | `page.rs`, `driver.rs` | Validate PDF option combinations and inspect actual generated page dimensions/content on Chromium; assert Firefox's unsupported result. |
| C02 | `network.rs`, `driver.rs` | Expose bounded captured-body bytes/text/JSON with distinct empty, missing, truncated and failed states; retain explicit Firefox unavailability. |
| C03 | `coverage.rs`, `driver.rs` | Define navigation reset/source options and stop/restart behavior; verify anonymous scripts and resource release on Chromium. |
| C04 | `har.rs`, `routing.rs` | Apply shared URL matchers and explicit miss policy to supported HAR content/timing options; test duplicate URLs, redirects and binary replay. |
| C05 | `page.rs`, `driver.rs` | Add only native supported media/metrics/user-agent options; verify observable values and resets, including explicit unsupported errors. |
| C06 | `cdp.rs`, `driver.rs` | Own target sessions independently; detaching one must settle its pending calls while other sessions/pages keep working. |
| D01 — complete | `runner.rs`, `report.rs`; CLI/config packages | Preserve opt-in flaky-run failure and registered focus protection in `a2fd29b`, effective settings and original attempt outcomes. |
| D02 — complete | `expect.rs`, `operation.rs` | Preserve `312090d` polling companions, validated intervals/messages/cancellation, one shared window, typed probe errors and scoped final-only soft retries. |
| D03 | `runner.rs`, `bundle.rs`, `report.rs` | Apply retention only to runner-owned output after capture settles; represent removed artifacts explicitly and preserve portable links. |
| D04 | `resolved_config.rs`, `runner.rs`, `report.rs`; CLI/config packages | Carry user-supplied metadata and bounded slow-test summaries through existing reporter formats without adding a new reporter framework. |
| D05 | `page.rs`, `driver.rs` | Enumerate local/session storage and add typed bulk operations with deterministic serialization and normal operation guards. |
| D06 | `page.rs`, `driver.rs`, `network.rs` | Extend existing Chromium socket observations with identity, native errors and bounded lifecycle/wait helpers; do not implement interception. |
| G04 | `operation.rs`, affected tests and parity documents | Audit every new API's deadlines/cancellation/disposal/retries, resource release and serialization; reconcile the complete test inventory and matrix. |

The diagnostics group **B01/B04/B19**, ownership **B16** and locator labels
**B09/B17/B12/B13/B14/D01/D02** are complete; the next delivery is the capture/report
group **B10/B18/B11**, with D01–D06 placed as in the remaining-work index.
Implement the six C extensions after checking native
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
C items and D06 need a capability check before implementation. Other D items
target both engines. Existing unsupported Firefox behavior must stay explicit; a missing native capability can move that
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
- `1ec7e3e`: B04. All 332 E2E and 23 CLI/configuration checks passed, with strict
  Clippy, formatting and regenerated links. Four popup groups and seven runner/
  report targets ran on full Chrome 153/Firefox 157. Actual pinned popup behavior
  reproduced exactly; startup source identity, immediate/concurrent closure,
  independent clearing, cancellation/disposal/disconnect and retry/report retention
  verified. Wire fixtures cover pre-subscription bursts, failed initialization
  followed by logs/closure, budget exhaustion and pending-slot eviction/recovery.
  Expanded reports and seven relocated artifact links per engine verified.
  Combined inventory: 355 checks. Continue with B19; G04 stays open.
- `34890cc`: B19. All 338 E2E and 23 CLI/configuration checks passed, with strict
  Clippy, formatting and regenerated links. Three console groups and the runner/
  popup/report batch ran on full Chrome 153/Firefox 157. Native wire fixtures and
  two actual pinned cases cover argument states, null roundtrip, mutable names,
  constructor separation, field absence, stack/cap behavior and owned retry history.
  Expanded error/argument reports and seven relocated artifact links per engine
  verified. Final limits/trace checks preserve ordinary BiDi nesting and old JSON.
  Combined inventory: 361 checks. Continue with B16/B17; G04 stays open.
- `1e4d3bb`: B16. All 342 E2E and 23 CLI/configuration checks passed, with strict
  Clippy, formatting and regenerated links. Four native owner groups and two
  actual pinned cases cover shared owners, weak context references, convenience/
  persistent/default/remote identity, actual operations, storage failure cleanup,
  final-owner process/profile release, canceled/concurrent close, empty-context
  waits and native transport loss. A supplemental attached-client disconnect
  passed on full Chrome/Firefox. Shared base-URL getter migration documented.
  Combined inventory: 365 checks. Continue with B17/B12; G04 stays open.
- `8ae438c`: B17. All 347 E2E and 23 CLI/configuration checks passed, with strict
  E2E/CLI Clippy, package formatting and regenerated links. Inventory: 157 units,
  187 integrations across all 27 targets and three doctests; no ignored/failed
  groups. The 48-check runner/diagnostics batch ran on full Chrome/Firefox;
  browser93/core180/routing23 used Headless Shell/Firefox. Three actual pinned
  cases plus three native groups verify derivation/resolution, labeled errors,
  typed causes/codes, cancellation, local values/callbacks, retry/live/source/
  trace/JSON diagnostics. Expanded reports from both engines were inspected;
  all three links per relocated bundle resolved with escaped labels intact.
  Two error units preserve typed causes, identity and skip reasons. Combined
  inventory: 370 checks. B12 is next; G04 stays open. Whole-workspace fmt has
  unrelated existing differences, while required package formatting passed.
- `0ee8245`: B12. All 355 E2E and 23 CLI/configuration checks passed. Inventory:
  160 units, 192 integrations across all 28 targets and three doctests; no ignored/
  failed groups. Runner53 used full Chrome/Firefox; core183/browser93/routing23
  used Headless Shell/Firefox. Five actual pinned runner cases and five native
  groups cover automatic soft outcomes, continuation/retries/expected/cleanup/
  parallel isolation, sources/steps/retained handles, operational controls,
  failed setup/resource release, JSON defaults and escaped reports. Three units
  additionally cover local/zero/control behavior, concurrent sealing/weak release,
  late non-polled futures and interrupted Drop. Final 160-unit rerun passed.
  Initial Clippy found a test guard across shutdown; the fixed native group and
  final strict Clippy passed. Package formatting and regenerated links passed;
  expanded reports inspected and all three links per relocated engine resolved.
  Combined inventory: 378 checks. B14 is next; B13/G04 remain open.
- G04 remains open until lifecycle coverage across all additions is complete.

- `a7f1e45`: B14. All 365 E2E checks and 27 CLI/configuration checks passed,
  with strict E2E/CLI/config Clippy, package formatting and generated matrix links.
  Five native groups on full Chrome/Firefox compare four pinned runner observations
  and verify effective configuration, actual owner identity, precedence, artifacts,
  snapshot seeds, environment freezing, retries and backward-compatible report JSON.
  Expanded reports and relocated bundles inspected; final units reran after the
  empty legacy filter compatibility correction. Inventory: 164 units, 197
  integrations/all 29 targets and four doctests; combined total 392 checks.
- `e2e3ecc`: B13. All 377 E2E and 27 CLI/configuration checks passed, with
  strict all-target Clippy, package formatting and regenerated matrix links.
  Inventory: 170 units, 203 integrations/all 30 targets and four doctests;
  combined total 404 checks. Eight actual pinned observations document different
  fixture accounting. Six native groups on full Chrome/Firefox verify actual
  target/user-context release after dropped waits, setup caps and failure,
  cancellation, shared/zero cleanup, retry worker rebuilds, dynamic zero and
  soft-error retention. Six budget/lifecycle units cover one-poll release,
  intersections, disposal ownership/error replay, worker/suite retirement,
  run-final hooks and enclosing setup caps. Core/browser/routing batches used
  Headless Shell/Firefox. Initial stack growth, lost-transport idempotence and
  worker-fixture labels were corrected; final units/native groups and strict
  Clippy reran after late target-ID/run-final/setup-cap cases. Continue with D01;
  G04 stays open for the final cross-feature audit.

- `a2fd29b`: D01. All 386 E2E and 28 CLI/configuration checks passed; combined
  total 414. Inventory: 173 units, 209 integrations/all 31 targets, four doctests.
  Runner70 used full Chrome/Firefox; core196/browser93/routing23 used Headless
  Shell/Firefox. Fourteen actual pinned runner cases and twelve actual CLI cases
  verify policy/report/exit behavior and document upstream differences. Four
  native policy groups and isolated child checks cover unchanged successful
  retries, scheduling, repeated/projects, all statuses, interruption and focus
  hidden by filters/shards. HTML previews were inspected on both engines.
  B13's immediate target query was made a bounded actual-release wait; G04 must
  still audit ignored native close/detach command errors. Late units/policy/fixture
  tests passed after legacy-focus/reference changes; the policy target and final
  strict Clippy passed after narrowing a test mutex scope. Package formatting,
  regenerated matrix and links passed. D02 is next; 16 tasks remain open.

- `312090d`: D02. All 397 E2E and 28 CLI/configuration checks passed;
  combined total 425. Inventory: 180 units, 213 integrations/all 32 targets,
  four doctests. Runner74 used full Chrome/Firefox; core203/browser93/routing23
  used Headless Shell/Firefox. Seven added virtual-time and four native groups,
  13 actual pinned polling cases, soft/retry/report/lifecycle evidence and both
  HTML previews verified. Cached navigation image and short pointer acquisition
  assumptions were corrected; all native assertions remain. Strict all-target
  Clippy, formatting, matrix and links passed. B09 is next; 15 tasks remain open.

- `1c37e31`: B09. Current inventory is 405 E2E plus 28 CLI/configuration checks,
  433 combined: 180 units, 221 integrations/all 34 targets and four doctests.
  Broad batches covered the initial 404 E2E inventory; the later trace-step
  regression and private capture fix were verified by rerunning all 16 related
  capture/report/step checks on final source. Final routing23, doctests,
  CLI/configuration, strict Clippy and package formatting passed. Full Chrome
  153/Firefox 157 verified capture/runner scopes; matching Headless Shell/Firefox
  verified broad core/browser/routing scopes. Native capability and seven option
  groups cover pixels/dimensions, DPR/scrolling, masks, alpha, owned restoration,
  typed failures, concurrent captures, dropped/cancelled/disposed work, cleanup
  failures, open roots/iframes and trace step ownership. All 52 actual pinned
  screenshot cases passed on both engines after a concurrent Firefox launch
  timeout and final 30-second launch-budget run. Firefox alpha is unavailable;
  Css output uses raster normalization. Animation suppression, JPEG quality and
  coordinate validation retain documented Rust differences. A transient unchanged
  same-URL redirect header failure passed isolated/full routing repeats and
  remains unresolved under G04. Matrix and 735 links/652 source anchors passed.
  B10 is next; 14 tasks remain open.

- `92b5a66`: B10 active image/read work. Two active data-only callbacks, dropped-
  wait cancellation, shared encoded buffers, single-decode validation, bounded
  regular baseline reads and PNG/JPEG/resize guards. Five added unit regressions
  cover runtime progress, admission/input release, file/header bounds and expiry
  during pending validation. The current-source native gate exposed premature
  stability reset; the last completed assessment is now retained until the next
  pair is assessed. Its original outer-poll native assertions passed afterward.
  The 4,096/4,097-animation fixture now uses separate hidden native targets after
  its earlier stacked-target basic recovery capture hit a restoration timeout;
  limits, restoration assertions and budgets remain unchanged. Final focused
  gates: 200 units, 53 native integrations/12 targets, four doctests and 28 CLI/
  config checks (285 combined), full Chrome/Firefox required. Strict all-target
  Clippy, package formatting, matrix generation, 726 Markdown links and 655
  source anchors passed. B10 remains open for success commits, diagnostics and
  complete phase verification; 14 tasks remain open.

- `4e35613`: B10 staged baseline installation. Success assertions reuse validated
  stable bytes and frozen expected buffers, with scalar Changed comparisons and
  chunked temporary writes in data-only workers. The foreground owner checks the
  original deadline before replacing a baseline; Missing never overwrites a
  winner and accepts matching competitors after bounded validation, including
  read-only files/directories. Writable permissions and existing/dangling aliases
  are preserved. Six unit groups verify update/tolerance/file identity, real
  partial-write interruption, cancelled ready handoff, expired installation,
  invalid/corrupt/read-only inputs, permissions, aliases and race outcomes.
  One new native path group verifies page/locator alias updates, open readers and
  identical parallel Missing generation. Combined focused gates passed: 206 units,
  54 native integrations/12 targets, four doctests and 28 CLI/config checks
  (292 combined), full Chrome/Firefox required. Final-source replay of all 22
  font/path/artifact/polling groups passed after the last preparation changes.
  Strict Clippy, package formatting, matrix, 727 Markdown links and 655 source
  anchors passed. B10 stays open for diagnostic processing and complete phase
  verification; 14 tasks remain open. Public synchronous helpers are unchanged.

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
  Verify native context/page disposal after dropping close waits, including the
  shared-budget paths added by B13. Probe assertion cancellation/disconnection
  and interrupted step reporting: preserve the control error rather than
  retrying it as an ordinary mismatch. These are audit cases, not claims that
  every suspected failure has already been reproduced.
  D01's broad gate exposed an immediate-target-query timing assumption in B13;
  the regression now requires actual target disappearance within its existing
  finite budget. Audit native drivers' ignored page close/detach command errors
  and distinguish close acknowledgement from confirmed native destruction.
  B09's broad routing gate also exposed a transient same-URL synthetic redirect
  header metadata failure: `header_forwarding.rs:232` observed zero Set-Cookie
  pairs instead of two after response.finished, while x-hop matched. The unchanged
  strict two-engine target and subsequent full routing23 batch passed; no cause
  is proven and no assertion was relaxed. Audit native fulfillment acknowledgement,
  extra-event correlation and redirect-hop identity before closing this item.

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
- [x] **B04 — Earliest popup diagnostics (M).** Preserve console/error/network
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
  Evidence: `1ec7e3e`; `popup_diagnostics.rs` compares two actual pinned reference
  cases on full Chrome/Firefox and verifies four native groups. Three unit groups
  exercise transport bursts, initialization failure/late closure, request settlement,
  limits, eviction/recovery and weak ownership. Histories and attempt JSON have
  explicit truncation/closure/adoption states; portable retry reports were inspected.
  Request completion survives adoption/replacement. Native data remains optional;
  absent events for an aborted request are not fabricated. Full regression inventory
  and migration/capture limits are recorded above and in the parity report.
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
- [x] **B09 — Screenshot capture options (M).** Extend existing screenshot and
  locator-capture options with supported clipping, scale, transparent background,
  mask color and temporary styles. Done when coordinates/masks work for scrolled
  and full-page captures, and temporary DOM/style changes restore after failure.
  Check native engine support before accepting an option.
  Evidence: `1c37e31`; validated page ScreenshotOptions and Locator.screenshot_with
  support clip/scale/background/mask-color/styles with shared capture deadlines.
  Owned nodes, serialized captures and acknowledged background restoration survive
  success, errors, caller cancellation and dropped waits; restoration failures
  remain observable. Native capability plus seven option groups passed on both
  required engines, including actual dimensions/pixels, DPR/scrolling/full-page
  masks, concurrent styles, disposal/resource release, genuine cleanup failure,
  open roots/iframes and one explicit trace step. Fifty-two pinned cases passed.
  Final inventory 405 E2E/28 CLI checks is covered by broad initial 404 E2E batches
  plus 16 related final-source checks after the added trace regression; final
  routing/doc/CLI/Clippy/fmt passed. Firefox transparency is explicitly unsupported;
  Css uses raster normalization. The original CSS duration suppression (superseded by the B10 follow-up), reachable-root styles,
  quality 1–100 and finite-coordinate validation remain documented differences.
  Raw CDP background tracking is limited to acknowledged Page.call operations;
  cleanup has a separate bounded five-second window. Examples, migration notes,
  matrix, local links and source anchors verified. Route header flakiness stays G04.
- [ ] **B10 — Stable screenshot assertions and snapshot paths (M; needs B09).**
  Extend existing snapshot assertions with stable successive captures, browser/
  project/platform path templates and clearly defined update modes. Done when
  delayed fonts/animations, never-stable content, dimension differences and
  baseline changes are bounded and diagnosed; attach expected/actual/diff artifacts.
  Draft progress: `499c274`; successive stable capture and typed deadline/error handling,
  font readiness, `Changed` mode, PNG/ratio validation and last-image failures are
  implemented in this increment. The 56-case actual pinned reference verifies
  mode/negation/stability differences; three required two-engine groups and
  virtual-time checks cover the draft. Path templates now cover global/project/
  assertion precedence, frozen metadata, CLI/env and TestInfo access (`b34174d`); two native
  groups and 36 pinned path-only cases verify them. Attempt/step-owned expected/
  actual/diff and last-pair stability attachments (`b826c56`) now preserve immutable retry
  evidence and portable links, with four native diagnostic groups. `e81d744` adds seven additional
  native font/animation groups using real held HTTP fonts and native animation
  objects, including late CSS animation cancellation/resumption and visible
  finish/resume errors and the 4,096/4,097-object limit boundary. Final-only
  outer-poll diagnostics (`0509518`) now publish the last completed failing probe only,
  with four additional native diagnostic groups verifying nested/control/pending/
  unfinished/soft/parallel/retry/export and publication I/O behavior. Active
  image/read work (`92b5a66`) now has two callback slots, cooperative cancellation, immutable
  buffers, bounded regular baseline reads and PNG/JPEG/resize input guards, with
  five additional unit regressions. Success baseline commits (`4e35613`) now stage data-only
  temporary files and install in the foreground under the shared clock. Frozen
  Changed comparisons, non-overwriting Missing generation/matching race winners,
  writable permissions and existing/dangling aliases are verified by six more
  unit groups and one added page/locator/parallel native path group. Diagnostic
  finalization (`2c042e1`) now uses immutable buffers, data-only diff/chunked-write
  jobs and foreground non-overwriting attachment publication under a separate
  shared five-second clock. Sealing is excluded only during installation/metadata;
  cancellation, expired publication, external collisions and the 1,024-name cap
  are covered. Opaque phases and queued input memory are not hard bounded.
  Final complete phase gates are still required.
  Latest focused increment gates passed: 208 units, 54 native integrations/12
  targets, four doctests and 28 CLI/configuration checks (294 combined), strict
  Clippy, formatting, regenerated matrix and local links. Font waits now follow
  owned style preparation across reachable same-origin documents, with real-
  font HTTP delivery, cancellation/restoration and animation-boundary coverage.
  These are not full phase gates; keep this task unchecked until the remaining acceptance criteria are verified.
- [ ] **B11 — Report search, filtering and network diagnostics (M; needs B02/B03).**
  Extend the portable HTML report with test/status/project filters and per-attempt
  network summaries beside existing console output. Done when retries stay
  distinct, diagnostic text is escaped, empty/large reports remain usable and
  moving the bundle preserves all artifact links. Visually inspect representative
  expanded reports; no Trace Viewer archive implementation is required.
- [x] **B12 — Runner-integrated soft assertions (M).** Extend current SoftAsserts
  with an attempt-owned collector and contextual assertion messages. Done when
  collected failures affect the attempt result even without a final manual
  assert-all, include step/source metadata, preserve cleanup and stay isolated
  across retries and parallel tests. A custom matcher registry is deferred.
  Evidence: `0ee8245`; five actual pinned Playwright runner cases and five native
  `soft_assertions.rs` groups on full Chrome/Firefox. Weak collectors and one
  contextual assertion step, exact collection/wrapper source and owning paths,
  continuation, automatic failure/retry/expected classification, setup/cleanup/
  fixtures, failed setup dependency release, operational/control passthrough,
  retained-handle rejection, old JSON defaults and escaped reports verified.
  Three unit groups cover local/zero/control errors, actual concurrent sealing,
  weak release/late future non-polling and unfinished-attempt interruption.
  Standalone manual behavior remains intact. New AttemptResult vector migration
  and upstream persistent-skip versus Rust attempt-local modifiers are documented.
  All 355 E2E/23 CLI checks, strict Clippy, package formatting and generated links
  passed. A test-only await-held guard was fixed, with its native group and Clippy
  reverified. Expanded/relocated reports inspected; all three links per engine resolved.
- [x] **B13 — Fixture budgets and shared teardown accounting (L).** Add explicit
  fixture setup/teardown timeouts and define one enclosing cleanup budget with
  operation overrides bounded by it. Done when slow hooks/fixtures, failed setup,
  reverse teardown and cancellation produce all relevant errors without silently
  skipping cleanup. Preserve zero-timeout semantics and test existing defaults.
  Evidence: `e2e3ecc`; eight actual pinned runner observations and six
  `fixture_budgets` groups on full Chrome/Firefox. Local/zero/enclosing limits,
  setup failure and cancellation, reverse/ready cleanup after exhaustion,
  individual phase errors, actual removed native target/user-context IDs after
  dropped/repeated waits, worker rebuilds/retries, dynamic zero and soft errors
  verified. Six lifecycle units cover intersections, one-poll release, error
  replay/ownership, suite/worker retirement and final run scopes. Default values
  remain compatible; cleanup now shares one scope clock. Upstream independent
  fixture accounting and local close after lost transport remain explicit.
  All 377 E2E/27 CLI/config checks, strict Clippy, package formatting and generated
  links passed. Stack growth, lost-transport close and worker report labels were
  corrected before final gates; final 170 units/six native groups reran after
  target-ID/run-final/setup-cap additions.
- [x] **B14 — Effective project/configuration metadata (M).** Extend Project and
  report metadata with missing per-project grep-invert, output/snapshot paths and
  repetition settings; expose resolved configuration read-only. Done when global/
  project/suite/test precedence, defaults and explicit overrides are tested and
  CLI forwarding matches library configuration. Project dependency scheduling is deferred.
  Evidence: `a7f1e45`; five native `effective_configuration.rs` groups on full
  Chrome/Firefox and four actual pinned runner observations. Shared TOML/JSON,
  actual CLI child forwarding, filters/tag exclusion, zero defaults, repetition/
  sharding, whole-context/suite/test precedence, runtime timeout/retries, actual
  supplied/dedicated identities and released owners, project artifact/snapshot
  paths, startup errors, historical JSON and isolated environment freezing verified.
  Final 365 E2E/27 CLI/config checks, strict Clippy, package formatting and all
  generated evidence links passed. Final units reran after empty legacy filter
  compatibility was preserved. Expanded/relocated reports inspected; all 12 links
  per preview resolve. Project dependencies and B10 path/stabilization semantics
  remain excluded; current creation inputs are described accurately in parity notes.


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
- [x] **B16 — Context/browser ownership introspection (M).** Add a context owner
  accessor and review disconnection semantics of existing is-closed/is-connected
  APIs. Done when convenience-page contexts, explicit contexts and remote browser
  disconnection are covered, with no strong-reference ownership cycle. Include
  worker handles and final-owner cleanup in the ownership review. Reference:
  [context owner](https://playwright.dev/docs/api/class-browsercontext#browser-context-browser).
  Evidence: `1e4d3bb`; four `browser_ownership.rs` native groups on full Chrome/
  Firefox plus two actual pinned Playwright cases. Arc-shared real owners and
  weak context access, common defaults/registry, native context cleanup after
  storage failure, retained operations, last-owner process/profile release,
  cancellation/concurrent close, persistent profiles, empty-context wake-up
  and native/attached disconnection verified. All 342 E2E/23 CLI checks, strict
  Clippy/fmt/generated links and complete 26-target inventory passed. Getter
  migration to owned Option<String> and outside-runtime fallback are documented.
- [x] **B17 — Locator descriptions and diagnostic call sites (S).** Add optional
  locator descriptions and propagate them into automatic steps/errors. Done when
  cloning/chaining preserves intended labels and action/assertion errors identify
  the calling operation. Keep automatic-step deduplication and existing source data.
  Reference: [locator descriptions](https://playwright.dev/docs/api/class-locator#locator-describe).
  Evidence: `8ae438c`; three native `locator_descriptions.rs` groups on full
  Chrome/Firefox and three actual pinned Playwright cases. Replacement/removal,
  clone/decorator retention, derived-selector clearing and frame-owner conversion,
  unchanged resolution, early validation, typed JSON causes, cancellation,
  labeled assertions and non-Send values/callbacks verified. Error codes/identity
  and skip reasons are preserved; opaque errors retain their cause via the new
  Diagnostic variant. Retry/live-step/source data, one outer trace/step, JSON
  and escaped HTML verified. All 347 E2E/23 CLI checks, strict Clippy, package
  formatting and regenerated links passed. Expanded/relocated reports inspected;
  all three links per engine resolved. The pinned timeout omits labels; Rust's
  richer errors and enum migration are documented. Automatic locations remain
  test definitions, with explicit user-step caller locations unchanged.
- [ ] **B18 — Bounded ARIA snapshot options (M).** Extend the current structured
  DOM snapshots with depth limits and optional boxes/state fields. Done when
  nested/open-shadow/same-origin-frame cases are consistent, limits are explicit
  and snapshot assertions can consume the output. Full accessible-name conformance,
  upstream AI modes and YAML pattern matching remain deferred.
  Reference preparation: `aria-options-reference.mjs`/JSON record 26 actual pinned
  public cases on Chromium/Firefox, including role-depth trimming, native state,
  open-shadow/frame roots, text fragments and rounded viewport boxes. Zero/negative
  upstream depth is unbounded; fractional depth/type errors and frame-local versus
  main-viewport box coordinates are recorded. Rust implementation is still pending.
- [x] **B19 — Structured page errors and console value previews (M).** Add optional
  error name/message/stack frames and JSON-safe console argument previews where
  native events provide them. Done when page/context events, traces and attempt
  reports retain the same data through cleanup/retries, and unserializable values
  are identified explicitly. Remote argument handles and worker ownership are deferred.
  Evidence: `34890cc`; `structured_console.rs` verifies three native groups on
  full Chrome/Firefox, plus three wire/schema/limit unit groups. Tagged null and
  special values, native own error fields versus constructor class, optional
  Firefox fields, supplied async frames, caps, source forwarding, closure,
  popup retries, traces, JSON compatibility and escaped HTML verified. Typed
  TraceEntry export/deserialization added; exhaustive literal migration is documented.
  The pinned bigint handle difference remains explicit. Full 338 E2E/23 CLI gates,
  strict Clippy/fmt/generated links and expanded/relocated report inspection passed.

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

## D — Additional practical features for the longer session

These are new recommendations, all unchecked. Extend existing APIs and reports;
do not duplicate already implemented JUnit, storage access, socket observation,
focus protection, generic assertion polling or retry/flaky classification.
Current official documentation supports the feature comparisons below; differential
behavior claims must still use the repository's pinned Playwright v1.63.0 corpus.

- [x] **D01 — CI flaky-test policy and focus protection wiring (M; needs B14).**
  Add opt-in `fail_on_flaky_tests` through shared configuration, Runner and CLI.
  Reuse existing `forbid_only`, including suite focus, and expose its effective
  setting consistently. Default behavior must remain compatible. A retry that
  eventually passes remains a flaky pass in individual results; the enabled CI
  policy makes the aggregate run/CLI exit unsuccessful without fabricating a
  failed attempt. Define interactions with expected failures, skip, repetition,
  max-failures and global timeout. Done when library/child exit status, live
  reporters and JSON/JUnit/HTML agree, and focused tests cover policy on/off,
  retry success, ordinary success/failure and focus rejection before test bodies.
  Reference: [failOnFlakyTests and forbidOnly](https://playwright.dev/docs/api/class-testconfig).
  Evidence: `a2fd29b`; opt-in shared/config/Runner/CLI policy, immutable effective
  snapshots, registered test/suite focus before filters/shards and original
  successful retry outcomes. Live/list/dot/JSON/HTML expose aggregate failure;
  JUnit uses explicit FlakyTestPolicy failures with original status properties.
  Four native groups plus isolated child checks, 14 pinned runner cases and 12
  actual CLI cases cover on/off, retries, success/failure/expected/skip, repetition/
  projects, max-failures, timeout/interruption and hidden focus. Full inventory:
  386 E2E plus 28 CLI/config checks, strict Clippy and package formatting; HTML
  inspected on both engines. Upstream filtering/JUnit/expected-pass differences
  and public-field migration are documented. D02 is also verified; G04 remains open.

- [x] **D02 — Assertion polling options (M; needs B12/B13).** Extend existing
  `expect_poll` and `expect_to_pass` with options-based companions for interval
  sequences and contextual messages; retain existing helper defaults/signatures.
  Define empty/zero interval validation, exhaustion of the interval sequence,
  first-probe timing and last-mismatch diagnostics. All probes and sleeps share
  one caller/enclosing budget, including a hung probe and cancellation during
  a wait. Preserve supported non-Send assertion blocks. Retrying intermediate
  mismatches must not create separate soft failures or duplicate assertion steps;
  only the final mismatch may be collected by B12. Operational errors must
  remain distinct from assertion failures. Done when timed regressions verify
  cadence, success, exhaustion, zero/enclosing deadlines, cancellation, disposal,
  soft collection and a caller-provided message. Document Rust timeout/default
  differences instead of adopting upstream defaults silently.
  Reference: [polling and retry intervals](https://playwright.dev/docs/test-assertions).
  Evidence: `312090d`; seven virtual-time unit groups and four native
  `polling_options.rs` groups on full Chrome/Firefox. Exact immediate/final-reuse
  cadence, validation before probe construction, mutable/borrowed/non-Send values,
  hung probes, local/enclosing zero and finite clocks, explicit/owner cancellation,
  disposal with actual removed native context IDs and typed wrapped operational
  errors verified. Intermediate soft checks return mismatches without collection;
  one final failure, joined-work scope isolation, retry histories and escaped
  JSON/HTML reports verified. Thirteen pinned Playwright observations document
  deliberate defaults/validation/cutoff/soft/step/thrown-error differences.
  All 397 E2E and 28 CLI/config checks, strict Clippy, package formatting,
  matrix/links and both-engine HTML previews passed. Existing cached-image and
  input-acquisition fixture assumptions were corrected with stronger native
  evidence; no runtime navigation/input changes. B09 is next; G04 remains open.

- [ ] **D03 — Output retention policies (M; needs B14/B11).** Add validated
  always/never/failures-only policies for runner-owned attempt outputs, with a
  compatible default. Decide and document whether failures-only retains failed
  attempts of a flaky test, expected failures and interrupted attempts; compare
  against pinned observations where claiming matching semantics. Finish capture,
  reporter notifications and bundle copying before removing owned files that
  are no longer needed. Snapshot baselines and caller-owned attachment sources
  must survive. Restrict deletion to verified runner-owned attempt directories;
  handle overlapping project paths and symlinks without deleting unrelated data.
  Done when retries, cancellation and cleanup errors retain the declared files,
  JSON/HTML/bundles have no unexplained broken artifact links, and repeat runs
  do not remove each other's outputs. No automatic historical-run pruning.
  Reference: [preserveOutput](https://playwright.dev/docs/api/class-testconfig#test-config-preserve-output).

- [ ] **D04 — Run metadata and slow-test reporting (M; needs B14/B11).** Add
  optional JSON-safe user metadata/run name and project metadata, propagated
  into resolved configuration, worker/test read-only access and existing
  reports. Keep report schemas backward-readable with serde defaults. Add
  configurable duration threshold and bounded top-N slow-test summaries;
  explicitly document aggregation by Rust test/project rather than claiming
  upstream file scheduling semantics. Escape values in HTML/XML and preserve
  data through portable bundles; duplicate names and retries must not merge
  distinct tests accidentally. Done when configuration/CLI precedence, empty
  metadata, old JSON, custom/live reporters and relocated HTML agree. Automatic
  Git diff capture and process-wide stdout/stderr attribution remain deferred.
  Reference: [metadata and reportSlowTests](https://playwright.dev/docs/api/class-testconfig).

- [ ] **D05 — Typed Web Storage enumeration/bulk helpers (S–M, both engines).**
  Extend current local/session get/set/remove/clear methods with item enumeration
  and typed bulk set operations. Returned data is an owned snapshot of the
  current page origin; use deterministic serialization without promising native
  key ordering. Validate input before mutation and document that bulk writes
  are not transactional: quota/security failures can leave earlier writes.
  Done when both engines cover empty stores, Unicode/empty keys, overwrite,
  navigation, same-origin sharing, session page isolation, opaque-origin errors,
  quotas, cancellation and disposal. Preserve storage-state round trips; this
  does not add session storage to browser-context state or IndexedDB/OPFS support.
  Reference: [WebStorage operations](https://playwright.dev/docs/api/class-webstorage).

- [ ] **D06 — Chromium WebSocket diagnostic lifecycle (M; capability gated).**
  Extend current socket observations with native socket identity, error events,
  closed state and scoped typed wait helpers. Two sockets at the same URL must
  remain distinguishable. Preserve text versus binary opcode/payload semantics;
  cap retained payloads/history and expose truncation/lost observation explicitly.
  Do not invent a successful close when transport observation is lost. Waiters
  must settle on close, native error, channel lag, page disposal, disconnect and
  caller/enclosing cancellation. Done when Chromium tests cover concurrent
  sockets, binary/text frames, errors, closure and retries, and Firefox returns
  an explicit unsupported result. Any new public fields/enums need compatibility
  notes. Routing, message injection and service-worker sockets remain deferred.
  Reference: [WebSocket observation API](https://playwright.dev/docs/api/class-websocket).

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
| Workers/service workers and WebSocket routing | New evaluation/interception object graphs and uneven native backend capabilities. D06 extends current Chromium observation only. |
| Native file chooser and directory uploads | Event-driven chooser ownership, filesystem directory semantics and backend support. |
| Firefox response bodies, response rewriting and emulation gaps | Verify a supported native capability first; DOM approximations do not establish equivalent behavior. |
| Context-wide synchronized clock | Shared virtual-time semantics across documents/pages/frames need a separate design. Existing page clocks remain supported. |
| Complete accessibility algorithms and YAML/AI snapshots | Standards-level behavior and matcher language require dedicated conformance work. |
| IndexedDB/OPFS state | Storage schema/version, transactions and cross-origin persistence are separate projects. |
| Process workers, project dependency graph and fixture override hierarchy | Runner architecture and failure isolation changes; retain current Tokio workers and typed fixtures. |
| Trace Viewer archives, blob reports and distributed merging | New artifact formats, source/DOM capture and merge semantics. |
| Automatic Git diff capture and process-wide stdout/stderr attribution | Source capture and per-attempt attribution need a separate design with Tokio workers; user-supplied metadata is D04. |
| JavaScript transpilation, test-file discovery and npm reporter plugin compatibility | Rust compilation, registered tests and Rust reporter traits have different contracts; superficial aliases would not establish parity. |
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
| 3. Native event diagnostics | B01/B04/B19 — complete | Preserve frame/load/dialog observations (`f8c12de`), earliest popup traffic (`1ec7e3e`) and structured console/error data (`34890cc`) through ownership and runner changes. |
| 4. Runner and developer APIs | B16/B17/B12/B13/B14 — complete | Preserve shared ownership (`1e4d3bb`), labeled diagnostics (`8ae438c`), soft collection (`0ee8245`) and effective configuration (`a7f1e45`) and shared fixture/cleanup budgets with safe disposal (`e2e3ecc`). D01/D02 are complete; preserve the broader runner and polling regressions through capture/report work. |
| 4a. CI and assertion reliability | D01/D02 — complete | Preserve policy/focus wiring in `a2fd29b` and polling controls/scoped final-only soft retries in `312090d`. |
| 5. Captures and reports | B09 — complete; B10, B18, B11 | Preserve verified capture options and owned restoration (`1c37e31`); implement stabilized comparisons next, then bounded ARIA output and searchable per-attempt reports. |
| 5a. Practical storage and diagnostics | D03, D04, D05, D06 | Complete output retention, run metadata/slow summaries, typed Web Storage helpers and bounded Chromium socket diagnostics after their prerequisites. |
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
   order above, respecting dependencies. Complete independent B/D work as
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
cargo clippy -p ferrite-e2e -p ferrite-cli -p ferrite-config --all-targets -- -D warnings
cargo fmt -p ferrite-e2e -p ferrite-cli -p ferrite-config --check
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

> Implement all 14 open tasks (B10/B18/B11, D03–D06,
> C01–C06 and G04) in E2E-PARITY-TODO.md using the recommended remaining
> phases, dependencies and completion criteria. Preserve the already verified
> features. Implement all practical
> features supported by the existing Chromium/Firefox backends. Keep the deferred
> substantial projects outside scope; document any blocked subset and continue
> independent ready tasks. Verify native behavior, update the examples and parity
> matrix, keep the TODO accurate, and commit verified changes at phase boundaries.
