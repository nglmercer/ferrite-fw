# Ferrite E2E compared with Playwright

Ferrite now covers more of Playwright's everyday Chromium/Firefox testing
workflow, with fresh contexts, strict locators, shared API cookies and a larger
assertion/runner API. **It does not provide full Playwright API or behavioral
parity.** The implementation deliberately defers substantial backend,
distribution, debugger and orchestration work.

Audit date: **2026-09-30**. Upstream baseline:
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
| Partial | 635 | Related exposed operation with material semantic, option or engine differences |
| Idiomatic | 41 | Comparable operation through Rust language/library facilities |
| Missing | 327 | No dedicated public counterpart |

These counts describe an inventory, **not a behavioral compatibility
percentage**. The earlier inventory had 458 Partial and 503 Missing members.
Overloads are collapsed by member kind, inherited APIs appear on the declaring
class, and deprecated/experimental members remain visible. Argument options
are discussed below rather than counted individually. Runtime checks exercise
Ferrite's tests; they are not a differential Playwright conformance suite.
The [generator](scripts/playwright-parity/README.md) pins official sources,
requires a complete inventory and validates local evidence links.

The [implementation TODO](E2E-PARITY-TODO.md) prioritizes the remaining practical
work, with dependencies, completion criteria and explicit substantial-work exclusions.
The [engine capability table](E2E-ENGINE-CAPABILITIES.md) and
[pinned conformance corpus](scripts/e2e-conformance/README.md) distinguish native
shared behavior, engine-specific operations and missing protocol metadata.

A follow-up audit corrected three false Missing entries: `Page.close`,
`BrowserContext.isClosed` and `Locator.visible` already have implementations.
They are classified Partial because their options or lifecycle/visibility
semantics remain narrower. This correction changes the inventory, not the
implementation.

## Practical parity implemented

| Area | Implemented behavior | Remaining limit |
|---|---|---|
| Test isolation | Fresh context/page/request per attempt; typed built-in dependencies, lazy fixtures, hook injection, worker/project lifetimes and reverse teardown | Explicit Rust requests; no named fixture overrides or callback parameter inference |
| Page ownership | `Browser::new_page()` owns a fresh context and closes its popups on disposal | Use `default_context().new_page()` for intentional shared storage |
| Browser lifecycle | Shared Browser owners, weak context owner access, native disconnection state, persistent profiles, graceful close and Chromium HTTP/WebSocket CDP connections | No Playwright remote protocol, browser server, channels or managed browser installer |
| Configuration | Complete resolved CLI configuration forwarded as JSON; browser launch/runner consume it, with legacy overrides | Suite/test context, timeout, retry and tag inheritance; no named fixture option override hierarchy |
| Projects and scheduling | Independent project browsers/launch/context settings; parallel Tokio tasks, retries, repetition, filters and named resource locks | No process workers, project dependency graph or distributed locks |
| Artifacts | Per-attempt output directories, validated paths, unique attachment files, screenshots/video, live Reporter callbacks and portable HTML/JSON/JUnit bundles | Synchronous callbacks; no complete upstream reporter graph, stdout capture or status override |
| Step controls | Local timeout, skip with reason, live annotations/title paths and automatic action/assertion/hook/fixture trees | Explicit user steps have exact sources; automatic sources use test definitions; boxing and some Page/protocol actions remain absent |
| Runtime test controls | Shared TestInfo skip, expected failure, slow, annotations and timeout changes affect running attempts and final results | Result propagation for immediate skip; cooperative async cancellation and separate setup/cleanup scopes |
| Locator selection | Strict single-target operations, genuine first/last/nth slicing, relative has/hasNot filters, exact/regex/visibility builders | No complete Playwright selector extension/custom-engine surface |
| Locator descriptions | Optional labels with pinned derivation rules; calling actions/assertions retain them in errors, steps, retry histories and owned traces | Automatic sources remain test definitions; Rust labeled errors differ from the pinned upstream timeout |
| Semantic locators | Associated labels target controls; roles and accessible names use shared DOM helpers; open shadow-root traversal | Full accessible-name specification and closed shadow roots remain outside this implementation |
| Actions | Retry readiness and requested-point hit testing, trusted input, positions/modifiers/trial/scoped timeouts for click/hover/check/drag; delayed fill/select and contenteditable support | Same-origin offsets and positive axis scaling; rotated/perspective frames and cross-origin coordinates unsupported. Some actions use DOM setters/events |
| Uploads | Path and in-memory filename/MIME/binary payloads, multiple/empty batches and input/change events on both engines | DOM injection, 64 MiB total cap; native chooser and directory uploads remain deferred |
| URL/network waits | Exact/glob/regex, predicates, URL document readiness and typed per-hop Request/Response companions; async snapshot predicates | Optional native metadata, bounded redirect history; frame network idle, URLPattern and full body/worker graph unsupported |
| DOM access | Separate textContent/innerText, arrays, evaluate-all/JSON arguments, highlight removal; single-target getters wait and enforce strictness | JSON values only, without arbitrary JS/JSHandle argument serialization |
| Frames and handles | Same-origin lazy/nested/replacement `FrameLocator`, frame ownership, content/function/URL/load/selector helpers, remote handle evaluation/properties | Cross-origin/OOPIF lazy selection and ElementHandle are deferred |
| Native lifecycle events | Frame attach/navigation/detach with native identities, main DOM/load readiness, dialog closure and single context forwarding; bounded earliest popup diagnostics | Owned metadata snapshots; optional native fields/subscriptions; Chromium current target only, with OOPIF adoption deferred; popup observation loss is explicit |
| Assertions | Attempt-owned soft mismatches, contextual assertion steps, exact/regex page title/URL, raw regex/normalized or rendered text options, mixed lists, ordered text subsets, exact classes/class tokens, values, state/indeterminate options, native intersection ratios, accessible regex, custom predicates and `expect_to_pass` | Rust regex syntax; accessibility approximation; no custom matcher registry/asymmetric matchers or full options parity |
| Accessibility snapshots | Structured DOM role/name/state tree and locator/page exact snapshot assertions | Approximation, without complete ARIA/YAML matching or all upstream modes |
| Clock | Separate fixed Date/system time, run-for/fast-forward, promise/timer ordering, pause-at/resume and installation time | Page-local; navigation reinstalls initial state; idle callbacks approximate browser behavior |
| API testing | Query/headers/JSON/form/raw/multipart, hop cookies, TLS/proxy, scoped preemptive/challenge Basic auth, bounded manual redirects, reset retries and status checks | Legacy preemptive default; client-scoped TLS settings, no automatic compression decoding or full upstream option surface; returned buffers are independently owned |
| Browser/API storage | Context-linked cookies in both directions; isolated protocol cookie partitions; Playwright cookies/origins localStorage JSON; Page/context API clients inherit transport defaults at creation | Redirect/partition/SameSite details remain narrower; no IndexedDB/OPFS snapshots |
| HTTP credentials | Browser challenge authentication on Chromium, preserving extra headers; explicit preemptive Basic helper | Firefox challenge credentials unsupported; cached-auth clearing is approximate |
| Callbacks and buffers | Page/context sync/async functions and bindings, startup preloads, navigation/removal lifecycle; structured console/error metadata and attempt history | JSON callbacks, polled bounded dispatch and main/same-origin frames; bounded native argument previews, optional error fields; no cross-origin/worker/handle dispatch |
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

- Legacy captured requests/responses remain records; typed observations add
  live per-hop metadata and native completion without a full worker/body graph.
  Chromium captured response bodies are capped; Firefox has no body capture.
  WebSocket observation is Chromium-only; interception/mocking is absent.
- HAR recording/replay has narrower timing, body, update and archive support.
  Routes do not reproduce every response/redirect/header transformation option.
  Page/context removal now has default, wait and ignore-errors policies plus
  explicit Rust cancellation. Default/ignore-errors release pending requests
  while callbacks finish; later RouteAction decisions are discarded. Wait
  preserves decisions and shares caller/enclosing deadlines. Dispatch is
  independent and bounded to 256 concurrent pause tasks; shared hit limits count
  invocations atomically, including fallback. Empty pumps release native
  interception after pending stages/calls settle. Rust route priority remains
  page-first/context-fallback and first-registered-first, rather than upstream's
  newest-route-first order. Per-handler callback-identity removal is absent.
  A dispatched request retains its registration snapshot: removed entries cannot
  start another call, and newly registered entries apply to later requests.
  Native decisions already issued may finish after cancellation.
- Traces are Ferrite JSON rather than Trace Viewer archives with DOM/source
  snapshots. Live Reporter callbacks expose attempt and named-step metadata,
  without the complete Suite/TestCase/TestStep graph, blob merging or all
  upstream report formats.
- Screenshot comparison, update modes and paths differ. PDF options, device
  descriptors, emulation and permissions have smaller surfaces.
- URL/network waits support exact/glob/regex and predicates, while returning
  narrower snapshots and explicit Rust option surfaces. Event waits retain
  enum/predicate APIs rather than full upstream emitter semantics. Zero disables
  the operation timeout; action/protocol defaults are shared across page clones.
  Cancellation drops the outstanding wait; already issued browser commands or
  JavaScript may still finish remotely. It is cooperative and cannot interrupt
  blocking synchronous Rust code.
  Cleanup has one independent enclosing budget per cleanup scope. Fixture setup
  and teardown have separate optional limits capped by their enclosing clock;
  this differs from Playwright's separate fixture accounting.
- Nested suites scope hooks and inherit settings. Workers remain Tokio tasks,
  with logical fixture/suite state retired after unexpected failures; no process
  restart. Runtime TestInfo controls use shared state; Rust fixture requests/dependencies
  are explicit type declarations rather than inferred callback parameters.

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
- Process-based workers, project dependencies, named fixture overrides,
  Trace Viewer archives and advanced report merging.
- Full accessibility/selector algorithms, YAML ARIA matchers, all JS value
  serialization, IndexedDB/OPFS state persistence and complete backend parity.

The matrix preserves each missing member so future work can be selected
without treating raw CDP, arbitrary evaluation or Rust assertions as evidence
that a dedicated feature was implemented.

## Reliability and API additions

- `RouteInfo::fetch_with(RouteFetchOptions)` replays through the intercepted
  request's owning context, including cookies, TLS/proxy/auth and HTTP defaults.
  It supports method/header/raw/JSON and HTTP(S) URL overrides, context-base
  relative URLs, redirect limits and reset-only retries. Fetch does not resolve
  the native interception; return a RouteAction separately. Weak owner lookup
  prevents a retained description from keeping a page/context alive. Defaults
  use the live page action timeout; zero, caller cancellation, page/context
  disposal, transport loss and enclosing runner deadlines remain effective.
- `RouteBodyState` distinguishes absent, captured and unavailable request bytes.
  Chromium binary entries are lossless within a 16 MiB cap. A text-only native
  fallback is a preview limited to 64 KiB and is marked unavailable for replay.
  Firefox supplies no original body bytes: a body/JSON override is required
  for requests whose payload is unavailable, including an explicit empty body
  when desired. An independent HTTP fetch URL override is available on both
  engines; Firefox native intercepted-URL/response-stage rewriting stays unsupported.
- `RouteInfo::fulfill_with(RouteFulfillOptions)` prepares owned API response
  inheritance and status/header/binary/JSON/regular-file overrides. Header sets
  preserve duplicates and replace the inherited set when supplied. Explicit
  content type wins; otherwise truthy JSON supplies application/json, then a
  file supplies its MIME type. A file wins over body/JSON bytes; body plus JSON
  remains invalid. Pinned behavior preserves inherited/explicit Content-Length,
  even when bytes change, and adds length only for a nonempty explicit payload
  lacking that header. Source inheritance with replaced headers may omit length.
  These are browser-observable fulfillment semantics, not a raw HTTP wire promise.
  The request-aware companion adds Origin/credentials/Vary headers for cross-origin
  fulfillment when no allow-origin header exists. Legacy static helpers keep
  their existing behavior; `RouteAction::fulfill_with` has no request-origin data.
- Fulfillment preparation validates final status 200..=599, header values and
  optional status text. Paths must resolve to regular files; devices, FIFOs and
  directories fail before reading. Deadlines/cancellation bound the awaited
  preparation; an already issued regular-file blocking read is not synchronously
  interrupted. Exhaustive external `RouteInfo` literals must migrate to
  `RouteInfo::new(url, method, headers, post_data)` because native state/weak owner
  fields are private. Detached records use a standalone client and caller-known bytes.
  Public options are additive companions; existing route rule/action APIs remain.
- Shared API requests uppercase methods and compute Content-Length from the
  materialized payload. Client default Content-Type participates before JSON/raw
  inference; explicit request headers win, and raw bytes default to
  application/octet-stream when no content type is supplied. These adjustments
  also apply to context-linked route fetches. Compression decoding remains absent.
- Route-options evidence: 18 actual pinned Playwright 1.63.0 Chromium reference
  cases and eight [route-option groups](crates/ferrite-e2e/tests/route_options.rs)
  passed on full Chrome 153 / Firefox 157. Binary replay, original-body absence,
  response/file/JSON/header precedence, duplicate cookies, relative URLs, CORS,
  context TLS/proxy/auth, zero/live/caller/enclosing budgets, retries, disposal,
  transport loss and callback release are covered. Full regression inventory
  verified in bounded batches: 145 units, 170 integration checks across all 22
  targets, and three doctests (318 E2E), plus 23 CLI/configuration checks.
  Strict E2E/CLI Clippy, formatting and regenerated source links passed.
- API redirects use explicit 301/302 POST and 303 method-to-GET transitions;
  307/308 replay owned binary/multipart payloads. Client/request redirect limits
  default to 20; zero returns the redirect response. Cross-origin Authorization
  is stripped and Cookie is recomputed on every hop. Hop cookies synchronize
  with the browser before a later redirect, TLS or body failure.
- `ApiRequestOptions::max_retries` retries peer resets before response headers,
  including an incomplete HTTP message before headers, with 250 ms doubling
  backoff. Refused connections, TLS certificate errors, failed body reads and
  HTTP statuses are not retried. Redirects, preparation, retries and body reads
  remain bounded by the enclosing request/runner deadline and cancellation.
  `fail_on_status_code` rejects outside 2xx/3xx with a bounded diagnostic preview.
- `ApiCredentialsSend::Unauthorized` matches the pinned challenge-only default.
  Ferrite preserves its existing `Always` default and explicit Authorization
  header precedence. Credential origins validate HTTP(S) scheme/host/port and
  use URL origin normalization (host case/default ports/root slash), rather than
  Playwright's raw origin-string comparison. Exhaustive client/request option
  literals need the new credential fields / `max_redirects`, or struct defaults.
  Returned Rust response buffers remain owned after client disposal; explicit
  response disposal releases their bytes. TLS opt-out remains client-scoped;
  compressed response decoding and arbitrary request streams are not added.
- API evidence: 19 actual pinned Playwright 1.63.0 HTTP cases and five
  [API fidelity groups](crates/ferrite-e2e/tests/api_fidelity.rs), including native
  full Chrome 153 / Firefox 157 context-cookie and TLS checks. All 307 E2E checks
  and 23 CLI/configuration checks were verified, with strict Clippy, formatting
  and regenerated matrix links. The final broad run recorded 300 passing checks
  before process termination; the remaining five integration checks and two
  doctests passed separately. An initial pointer-cleanup timeout passed isolated
  rechecking and the final broad run. No HTTP implementation change was needed
  for that timing failure.

- `CancellationToken`, `OperationOptions`, page/locator clones with cancellation
  and timeout overrides, and cancellation on page/context/client disposal.
  Protocol futures reclaim their pending-command entries when dropped. Zero
  disables API, protocol, wait and assertion deadlines; a hanging predicate/check
  remains bounded by the outer finite operation budget.
- `ApiClient::storage_state`, `apply_storage_state`, save/load helpers and
  `ApiClientOptions::storage_state`. Cookies preserve domain, path, session/expiry,
  HttpOnly, Secure and SameSite, including redirect cookies and expiry deletions.
  Context-linked clients inherit headers, Basic credentials, timeout, TLS and
  proxy settings at creation; cookie operations never create temporary pages.
  Browser auth challenges and browser emulation restrictions remain unchanged.
- Frame owning page, document content replacement, function/URL/load/selector
  waits and live `current_url`. Promise predicates are awaited in the frame.
  Frame NetworkIdle tracking and full wait argument/result options are deferred.
- `Runner::global_timeout`, `max_failures`, `cleanup_timeout` and cancellation.
  A single attempt budget covers context/page setup, beforeEach, fixture setup
  and the test body. Teardown runs after setup/body timeout or panic; callbacks
  and artifact/close operations share the attempt cleanup deadline. Final unexpected failures
  count after retries, pending tests are reported skipped, and already active
  workers finish on maxFailures. Global interruption still produces failure.
- `BrowserContext::subscribe`, enum event kinds, `wait_for_event` and options.
  Current/future pages and adopted popups forward console/error, network,
  download and page-close observations with their source page ID. Page/context
  closure emits once; caller cancellation and zero timeout are supported.
  Download lifecycle events use CDP and recent Firefox BiDi; older Firefox
  versions retain the explicit filesystem download-wait fallback. Dialogs still
  require handling to be armed; WebSocket events remain Chromium-only.

The runner serializes its Firefox attempt context/page creation and close operations to avoid
stock Firefox discarding a newly created tab when another worker closes a window.
Test bodies and hooks remain parallel. The same failure was reproduced on the
pre-change commit `9cfe075`; the existing parallel fixture/retry regression checks
the protected lifecycle.

Configuration/CLI additions: `global_timeout_ms` / `--global-timeout`,
`max_failures` / `--max-failures`, and `cleanup_timeout_ms` /
`--cleanup-timeout`. Global timeout and maxFailures default to zero (disabled);
cleanup defaults to 5 seconds per cleanup scope.

## Context-aware fixtures, live reporters and runtime controls

Test-scoped fixture dependencies may request `Page`, `BrowserContext`,
`ApiClient` or `TestInfo`; worker fixtures may request `Browser` and `WorkerInfo`.
Reserved built-in types cannot be redefined. `ContextHook` declares lazy fixture
roots for before/after-each hooks, with shared metadata and fresh page/context/
request resources. `WorkerHook` gives suite-wide hooks only worker resources;
invalid test-scoped requirements fail validation before global setup.
The standalone request fixture inherits context transport defaults and has an
isolated cookie jar. `context.request()` explicitly shares browser cookies.

`Runner::custom_reporter` adds synchronous thread-safe callbacks for run begin/end,
attempt begin/end, user/action/assertion/hook/fixture step begin/end, attachments and errors. Per-attempt
callbacks execute on workers before subsequent tests start; retry, repetition,
project and worker identity are explicit. Step futures dropped by cancellation
emit an interrupted end event. Each retry keeps its own trace file, with the
original trace filename retained as a latest-attempt alias; repeated attachment
names get a suffix instead of overwriting earlier files. Reporter panics are
contained. Final reports retain each attempt alongside the aggregate result; begin receives discovered definitions before filtering, and
static skips have no attempt callbacks. Asynchronous uploads, stdout capture and
upstream status overrides remain deferred. Run-wide lifecycle steps are retained
in the final report rather than emitted through attempt callbacks.

`TestInfo::skip(reason)?` aborts a closure while cleanup still runs. Shared control
also interrupts pending async setup/body work. `fail(reason)` marks expected
failure; an unexpected pass fails immediately, and expected failures do not
retry. Timeout, global cancellation or cleanup errors cannot count as expected
failures. `slow(reason)` triples the budget once, and `set_timeout` changes the
total elapsed context/page/setup/body budget (zero disables it). Worker fixture
setup and beforeAll have independent setup budgets. `effective_timeout` reads live
state; the public `timeout` field is the initial value. Dynamic annotations and
skip/failure reasons reach attempt callbacks and aggregate reports. Cleanup keeps
its independent budget, so test controls cannot suppress cleanup failures.

These Rust counterparts follow the practical lifecycle described in the official
[fixture execution order](https://playwright.dev/docs/test-fixtures#execution-order),
[TestInfo controls](https://playwright.dev/docs/api/class-testinfo), and
[reporter callbacks](https://playwright.dev/docs/api/class-reporter), with the
explicit differences above.

## Structured step diagnostics and complete retry reporting

`Page::step_result` records returned `E2eResult` failures, including failures the
caller handles. Existing `Page::step` keeps arbitrary return types; it cannot
inspect a returned Rust `Err`. Both APIs record nested user steps, Rust call-site
file/line/column, start time, duration, panics and interruption. Attachments made
through `TestInfo::attach` inside a step belong to both that step and its attempt.
Concurrent awaited branches keep separate parent scopes; detached Tokio tasks
start root steps because task-local scope is not inherited. Finish or join spawned
work before returning from the test. Attempt completion seals the recorder and
ends any unfinished steps; later detached work cannot add late step events.
`Page::step_with(title, StepOptions, |step| async move { ... })` adds a live
`StepContext`. `StepOptions::timeout` bounds that step; zero adds no local deadline,
while the enclosing test timeout and cancellation still apply. Timeouts return an
`E2eError` and mark the step timed out even when the caller recovers.
`StepOptions::skip(reason)` records a skip without constructing the callback.
`step.skip(reason)?` aborts only that step and returns `StepOutcome::Skipped` to
its caller; it does not skip the parent or test. A cloned context can also cancel
pending step work. Dropped children are marked interrupted. `annotate`,
`annotations` and `title_path` expose live metadata; completed records reject
late annotations. JSON and HTML retain category, status, annotations and paths.

Page/Frame navigation and set-content methods, Locator async operations, and
Page/Locator assertions record automatic steps. Public calls produce one record;
internal delegations and assertion polling do not produce duplicate records.
Explicit user scopes can contain their own action children. beforeEach,
afterEach, beforeAll, afterAll, global hooks and actual typed fixture setup and
teardown record lifecycle scopes, including nested browser actions. Cached
fixtures do not create redundant setup records. Shared worker cleanup and global
hooks outside attempts appear in `TestReport::run_steps`, grouped by worker where
applicable. These records reach `Reporter::on_end`; live step callbacks cover
attempt-scoped operations. Automatic source locations use the enclosing test
file/line with unknown column zero, or `<run>` outside an attempt. Exact Rust
call sites remain specific to explicit user steps. Direct Page keyboard/mouse,
raw protocol calls and other unwrapped Page APIs are not automatically recorded.
Boxing, subtitle/parameter options and the full upstream category scheme remain
unsupported.

`TestInfo::status()`, `expected_status()` and `errors()` share live state across
hooks, fixtures and body clones. Status is `None` while setup/body is running;
its raw outcome is published before afterEach. Later hooks and fixture dependency
teardown see previous cleanup failures. `AttemptStatus` preserves failed, passed,
skipped, timed-out and interrupted outcomes; expected failure is a separate
expectation, and cleanup errors cannot redeem it. Unexpected passes retain their
raw passed status and add an expectation error. Structured `TestError` contains a
message, stable code, runner phase and source; runner-phase locations point to the
test definition (unknown column zero), while step errors point to the step call.
This does not provide JavaScript stack/cause objects or precise Rust throw sites.

`TestResult::attempt_results` retains every executed attempt's identity, raw and
expected status, expectation check, start/duration, errors, annotations, nested
steps, attachments and screenshot/trace/video paths. JSON serializes the complete
history; HTML exposes expandable attempts and step trees. A recovered successful
retry sets `TestResult::flaky`, with `TestReport::flaky()` and the summary showing
the count. A handled step failure alone does not mark a passing test flaky.
Legacy aggregate result/artifact fields remain available; old JSON without the
new history/flag fields still loads. Static skips and run-wide errors have no
executed attempt history. Adding public result fields requires manual Rust struct
initializers to supply those fields. Live `on_test_end` receives exactly one
attempt record, after cleanup, and the final run report retains all attempts.

`TestReport::write_bundle(directory)` copies every attempt and aggregate
screenshot, trace, video and attachment, including nested and run-step attachments,
into `artifacts/`. Canonical source paths are deduplicated; safe unique filenames
prevent basename collisions and preserve existing artifact files. HTML, JSON and JUnit
use relative artifact paths. The original in-memory report and source files are
unchanged. Relative input paths resolve against the current working directory;
missing, unreadable or non-file artifacts fail explicitly. Move or upload the
entire output folder, including `artifacts/`. Serving that folder over HTTP keeps
links usable after the original sources are deleted. Native traces remain
Ferrite JSON; they are not Playwright Trace Viewer archives.

Selecting the `html` reporter exports this portable folder and also writes
`results.json` and `junit.xml`. If selected together, JSON/JUnit use the same
relative paths. JSON-only and JUnit-only reporters retain the original serializer
behavior. `Runner::write_artifacts` reports bundle errors through `on_error` and
does not list a failed HTML export as written; explicit `write_bundle` returns
an error to its caller. Step fields and `run_steps` have serde defaults for older
JSON; manual Rust struct initializers need the new fields.

These controls follow [TestStepInfo](https://playwright.dev/docs/api/class-teststepinfo)
and the folder portability described by the [HTML reporter](https://playwright.dev/docs/test-reporters#html-reporter),
with the Rust result and artifact-format differences above.

These additions follow the official [TestStep metadata](https://playwright.dev/docs/api/class-teststep)
and [TestInfo outcomes](https://playwright.dev/docs/api/class-testinfo), with the
Rust schema and lifecycle differences described above.

## Typed events and assertion options

`Locator::dispatch_event_with` accepts `DispatchEventOptions`: explicit native
constructors or common event-name inference, JSON initialization and
bubbles/cancelable/composed flags (true by default). Canceled synthetic dispatch
still succeeds. Auto input uses Event as in the pinned Playwright version;
explicit `DomEventKind::Input` supplies InputEvent fields. The old CustomEvent
detail helper remains available. Events are untrusted; native mouse/keyboard
APIs provide trusted input. Live remote-handle arguments remain excluded.

`TextMatcher` and `TextAssertionOptions` add raw regex vs normalized exact/
contains text, case handling, rendered innerText, mixed ordered lists and
ordered contains-text subsets. Class equality preserves class order; separate
class-token containment ignores order. Multiple-select values can mix strings
and regexes. `MatchOptions` also applies to accessible name/description/error
assertions, with the existing DOM approximation. Regex syntax is Rust regex,
and inline pattern flags retain Rust's own semantics.

`CheckedOptions` supports checked/unchecked or indeterminate expectations;
combining the two is invalid. `state_with` accepts explicit expected states,
preserving absent-element behavior only for attachment/visibility. Negation
cannot convert ambiguous/missing input resolution into a passing state assertion.
`in_viewport_with(ratio)` uses native IntersectionObserver ratios and clipping,
including the supported same-origin frame scope; finite ratios from 0 to 1 are
accepted, and zero requires positive intersection. `intersection_ratio` exposes
the native sample. Default `in_viewport` getters/assertions also use positive
native intersection, replacing the previous rectangle-only overlap check.

Focused regressions compare native Chromium/Firefox outcomes with a recorded
Playwright 1.63.0 Chromium reference and separately check invalid options,
strict/negative assertions, delayed updates, zero deadlines, cancellation,
disposal and same-origin frame intersections. This is targeted differential
coverage; it does not establish complete behavioral parity.

## URL/network matching, generated uploads and browser diagnostics

### Typed native request and response observations

`Page::network_requests` exposes recent typed observations without enabling HAR
or body capture. `subscribe_network` returns Request/Response/Finished/Failed
events, while `wait_for_request_handle` / `wait_for_response_handle` and their
typed predicate companions resolve at request start/response headers. Existing
`RecordedRequest`, capture APIs, Page/context enum payloads and synchronous/async
snapshot waits retain their contracts.

Each redirect hop has a unique typed ID, a native ID, optional native frame and
page identity, weak previous/next links, and a serializable `RequestSnapshot`.
Request methods expose observed headers, method/URL, JSON or form POST text,
navigation/resource metadata, failure and an existing optional response. The
response exposes status/text/2xx, request/frame/page references and header lookup.
`Request::response` is a snapshot of whether headers exist, not an implicit wait.
Frame resolution uses the current native tree and may return None after detachment.

`Response::finished` awaits native completion independently of body capture on
both engines. HTTP 4xx/5xx finish successfully; native transport errors return
`E2eError::Network` (`FERRITE_E2E_NETWORK`). Pending waits obey the owning Page
action timeout, options, zero timeout, enclosing budgets and cancellation.
Closing/disconnecting wakes pending completion/event waits; already finished
observations remain readable after page closure. Dropping a wait leaves no
per-wait background task. Listener shutdown releases pending native metadata.

Header arrays preserve observed pairs; lookup is case-insensitive and joins
duplicate Set-Cookie values with newlines, other values with comma-space.
Chromium raw request/response headers use native extra-event availability flags
to correlate redirects even when extra rows arrive before or after base events.
Completeness is Some(true) only for received raw rows, Some(false) when missing
or unavailable, and None without a native signal. Failed pre-header requests
retain primary headers without claiming raw completeness. Firefox may fold
duplicates or omit POST text/resource destination; ambiguous comma-delimited
values remain intact. API responses share the array/value lookup helpers;
route fulfillment retains successfully acknowledged supplied header pairs when
native response events omit cookies or fold duplicates. The snapshot flag
`response_headers_from_route` identifies that source; it does not claim raw
native completeness. Supplied values are exposed only after both native headers
and a successful fulfillment reply. Full native raw headers take precedence.
Firefox 156/157 omits response/completion events for the first synthetic redirect
hop. Such a hop keeps no fabricated Response and settles as unavailable when
the native next-hop event arrives; its identity links remain available.
Per-hop native pause identity prevents same-URL redirects from editing an older
hop. Rejection/drop settles pending acknowledgements; `finished()` also settles
the associated acknowledgement within its existing budget. Pending correlation
is bounded to 4,096 records/16 MiB and is cleared at listener shutdown.
This field has a serde default for older serialized typed snapshots; Rust
`RequestSnapshot` literals need `response_headers_from_route: false`.

Metadata history is bounded to 4,096 hops and 16 MiB. Each POST/header list is
limited to 64 KiB and header lists to 256 pairs; snapshots identify truncation.
Evicted pending observations settle as unavailable, and weak redirect IDs survive
with an explicit history-truncated flag. Header correlation has a separate
16 MiB/4,096-ID budget and a 64-row per-ID backlog; overflow disables raw
correlation for that page rather than assigning rows to the wrong hop. Native
event buffers hold 256 entries and report lag explicitly. Bodies remain in the
existing bounded capture API; typed body helpers are still C02.

Evidence: [network_metadata.rs](crates/ferrite-e2e/tests/network_metadata.rs),
[header_forwarding.rs](crates/ferrite-e2e/tests/header_forwarding.rs) and focused
FIFO/budget/weak-history/acknowledgement units in [network.rs](crates/ferrite-e2e/src/network.rs).

### URL matchers and snapshot waits

`UrlMatcher::exact`, `glob`, `regex` and `contains` are reusable across
`Page::wait_for_url_matching`, `wait_for_request_matching` and
`wait_for_response_matching`; Frame supports matching URL waits too. Exact
relative URLs and relative glob paths resolve against the configured base URL.
Absolute matchers normalize URL host/scheme syntax; leading-star globs retain
their full-URL scope. Globs match the whole
URL: `*` excludes slashes, `**` includes them, `{a,b}` selects alternatives,
`?` is literal and backslashes escape characters. Regex anchoring follows the
supplied pattern. Invalid patterns fail at construction. Existing string waits
retain their explicit substring behavior.

`PageExpect::url_matching` / `url_where` share matching with URL waits; assertion
negation, retry budgets and cancellation apply. `Page` / `BrowserContext`
`route_matching`, `route_matching_times` and `unroute_matching` use the same
resolved matcher identity. `RouteRule::matching` replaces its legacy selection
pattern; `RouteFromHarOptions::matching` selects entries by the shared matcher.
Limits consume matching requests only. Context handlers are installed on current
and future pages even when no declarative rules exist.

Legacy string routing and HAR `url_filter` keep their globset contract; legacy
assertion strings stay exact/regex/contains as documented. Shared HAR matchers
and the legacy filter are mutually exclusive. Invalid shared matchers fail at
construction; legacy patterns are now checked before registration, including
contexts with no pages. Relative matchers require a base URL. Rust regex syntax
and strict trailing-backslash rejection remain explicit differences.

Migration: `RouteRule` and `RouteHandlerEntry` literals need `matcher: None` for
legacy behavior; prefer rule constructors/builders. `RouteFromHarOptions` literals
need `url_matcher: None` or `..Default::default()`. HAR replay still uses its
existing method+URL lookup and first-entry behavior; further replay/content
options and in-flight removal policy are tracked separately in the TODO.
The [22-case reference](scripts/e2e-conformance/url-reference.json) records actual
Playwright 1.63 Chromium wait/route behavior; [native regressions](crates/ferrite-e2e/tests/shared_url_matching.rs)
verify both Ferrite engines and assert invalid registration leaves no handler.

`wait_for_url_where` accepts a URL predicate. Network `_where` methods accept
synchronous predicates over `RecordedRequest`, and `_async` methods accept an
owned snapshot and return `E2eResult<bool>`. They can match method, URL, status,
request headers and response headers; predicate errors propagate. Timeout and
page/context/caller cancellation bound the entire wait, including pending async
predicates. Zero disables only its local deadline. Poll the wait future before
triggering traffic, for example `tokio::join!(wait, trigger)`; merely constructing
a Rust future does not arm it.

Live observations are separate from HAR/body capture: request waits resolve at
request start (status zero), response waits at headers and may match requests
already in flight when the wait begins. New waits do not consume past events.
Redirect hops preserve method/URL/status association. The observation channel
holds 256 events; lag fails explicitly instead of accepting incomplete traffic.
Returned snapshots have no response body; existing request capture remains
available with its documented engine limits. URL waits also handle hash/history
changes and frame-scoped navigation. URLPattern remains absent.

`Page` and `Frame` URL `_with_options` companions accept `UrlWaitOptions` for
Commit, DomContentLoaded, Load (default) or page-only NetworkIdle. Matcher and
predicate waits share one navigation budget across URL matching and readiness;
an omitted timeout inherits navigation settings, and zero disables the local
limit. Legacy duration-based helpers retain URL-only readiness and substring
strings. Readiness comes from the same document observation as the URL, including
native navigation timing for DOMContentLoaded (interactive state alone occurs
before deferred scripts finish). Transient execution-realm replacement is retried;
detachment, cancellation and unrelated protocol errors remain errors.

NetworkIdle requires Load and 500ms without observed page HTTP activity. An activity
counter detects requests that start and finish between polls. It does not represent
all socket, worker or OOPIF connectivity; supported frame waits reject this state
before waiting. Application assertions remain the stronger readiness signal.
Chromium document commits exclude replaced-document resources from idle accounting
without inventing terminal request events; later native completion/failure is still
reported. Existing load waits use the same DOMContentLoaded observation and idle
activity counter.
See the official [URL wait readiness options](https://playwright.dev/docs/api/class-page#page-wait-for-url)
and [navigation timing event timestamps](https://www.w3.org/TR/navigation-timing-2/).

`FilePayload::new(name, mime_type, bytes)` supplies generated files to
`Locator::set_input_file_payloads` or the Page selector helper. Bytes, Unicode
filenames and explicit MIME types reach the browser File and form submission.
Existing path uploads share the same transfer path, infer MIME from filenames
and read asynchronously. All uploads retain the 64 MiB aggregate cap. Empty
batches clear the input; multiple files require a `multiple` input. Complete
payload validation happens before DOM mutation; input/change events still fire.
Missing/non-file disk paths, invalid names/MIME characters and oversized batches
fail explicitly. This is DOM File/DataTransfer injection on both engines;
directory uploads and native chooser interception remain deferred.

`ConsoleMessage` now includes optional `ConsoleLocation` (source URL and
zero-based line/column), native epoch-ms `timestamp_ms` and owning `page_id`.
CDP Runtime/Log and BiDi log events supply metadata where available; unavailable
fields remain `None`. JavaScript exception descriptions retain the error text.
Page events/buffers and context events share the enriched message.
`BrowserContext::console_messages` retains context-wide history, including
closed pages and adopted popups; its clearing is independent of Page buffers.
Each `AttemptResult::console`, live `on_test_end`, final JSON/HTML and attempt
traces include context console/error observations with source/page identity.
Retries keep separate histories, and cleanup output is captured before attempt
end. HTML escapes console text and source metadata. These fields have serde
defaults for old JSON; manual Rust literals need the additional optional/history
fields. JSHandle arguments, worker ownership and complete JS error objects remain
outside this implementation.

These additions follow the official [URL waits](https://playwright.dev/docs/api/class-page#page-wait-for-url),
[request/response waits](https://playwright.dev/docs/api/class-page#page-wait-for-response),
[file payloads](https://playwright.dev/docs/api/class-locator#locator-set-input-files)
and [console metadata](https://playwright.dev/docs/api/class-consolemessage), with
the Rust/engine limits above.

## Async and context callback lifecycle

`Page` and `BrowserContext` provide `expose_function_async` and async
`expose_binding`; existing synchronous `expose_function` remains available.
Callbacks accept JSON and return `E2eResult<Value>`. BindingSource carries owning
context, page and native frame identity for main/same-origin documents. Rust
errors and callback creation/future panics reject JavaScript with an Error.
Calls execute independently through a bounded queue (256 pending per document
and 256 active per page), with polling latency around 25ms. Cross-origin/OOPIF
binding dispatch and remote handle arguments remain deferred.

Registrations persist through navigation. Context callbacks cover current,
future and adopted popup startup documents: native user-context preloads on
Firefox, and preload installation while Chromium popups are paused. Callbacks
are installed before context init scripts. Firefox context registration requires
[scoped preloads introduced in Firefox 136](https://bugzilla.mozilla.org/show_bug.cgi?id=1940927);
unknown/older versions return Config rather than installing a global callback. Duplicate
page/context names return Config errors. `remove_exposed_function` is idempotent
for missing names, removes the owning native preload and rejects pending calls;
context callbacks must be removed from the context. `clear_exposed_functions`
retains its legacy best-effort return type and clears page-owned registrations.
Removed callbacks cannot return after navigation, and unrelated callbacks survive.
A scoped caller cancellation rejects that registration's work while unrelated
callbacks retain their pump. Navigation aborts old-document futures using a realm
nonce; closure aborts pumps
and pending calls and releases captures. Dropped registration work removes
returned/known native preload IDs instead of retaining an orphan callback.

[Native callback regressions](crates/ferrite-e2e/tests/callback_lifecycle.rs) cover
startup, isolation, main/child identity, concurrency, errors/panics, registration
races, removal/re-registration, navigation, cancellation and closed-context capture
release. Firefox child frames are no longer adopted as popup pages. Callback
registration and native command cancellation remain bounded by existing lifecycle
and protocol budgets; runner attempt cleanup owns ongoing callback work.

## Filtered cookie clearing

`BrowserContext::clear_cookies_with(CookieFilter)` and the owning Page companion
accept exact/regex name, domain and path filters. Fields are ANDed, native strings
retain case/whitespace and an empty filter clears all. Chromium expires only
selected native keys, retaining supported partition keys; opaque partition keys
return Config before mutation. Firefox uses native exact cookie deletion scoped
to the user context. Cookie metadata on unrelated keys is preserved; deletion
never clears/rebuilds the full store or creates a scratch page. No partition-key
filter or new portable partition metadata representation is claimed.

Linked API clients refresh from the native store before the next request, so
filtered removals cannot resurrect their previously cached cookies. The
[native daily API regression](crates/ferrite-e2e/tests/daily_api.rs) covers same-name
domain/path independence, string and Rust regex filters, untouched attributes,
linked requests, zero timeout and cancellation on both engines.

## Pointer action options

`ClickOptions` retains click count, button and delay controls and adds position,
modifiers, trial and timeout. `ActionOptions` provides those shared controls for
hover/check/uncheck/set-checked; `DragOptions` adds a target position and move count.
Positions are CSS pixels from the padding-box top-left. Readiness hit-tests the
requested point, allowing an uncovered corner when the center is covered. Force
skips readiness checks while retaining strict resolution and trusted input.

Trial may scroll but sends no mouse/key input or checkbox state change. Modifier
cleanup releases only keys acquired by that action; previously held Ferrite keys
are preserved. Cancellation and errors release acquired modifiers and held mouse
buttons. Firefox drag pacing and click delays use cancelable Rust waits between
native input commands. Scoped timeouts do not change Page defaults; zero disables
the local timeout while enclosing deadlines and lifecycle cancellation still apply.

Main-document positions and same-origin nested frame offsets/positive axis-aligned
scaling are supported. Rotated/reflected/perspective frame transforms and cross-origin
coordinate translation return explicit errors. Explicit element positions require
positive axis-aligned transforms; SVG padding-box positions remain narrower. These
controls do not establish complete Playwright geometry/input parity.

Migration: `ClickOptions` has four additional public fields. Use builder methods
or `..ClickOptions::default()` in existing struct literals. Native regressions in
[action_options.rs](crates/ferrite-e2e/tests/action_options.rs) cover requested points,
trial silence, checkbox/drag behavior, frame scaling, scoped budgets, held-key
preservation and cleanup after cancellation on Chromium and Firefox.

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
FERRITE_E2E_REQUIRE_BOTH_BROWSERS=1 \
TMPDIR=/path/to/disk-backed-temp cargo test -p ferrite-e2e --no-fail-fast -- --test-threads=4
cargo clippy -p ferrite-e2e -p ferrite-cli --all-targets -- -D warnings
```

Validation for URL/network matching, generated uploads and browser diagnostics:

- Combined verified `ferrite-e2e` inventory: **143 unit tests, 3 API tests, 93 browser tests, 7 attempt-diagnostics
  groups, 4 reliability groups, 4 runtime/reporter groups, 6 fixture/network
  groups, 4 step-control/bundle groups, 5 wait/upload/console groups,
  3 core conformance/capability groups, 4 callback lifecycle groups,
  1 daily API group, 2 action option groups, 3 URL readiness groups,
  2 shared URL matching groups, 3 function wait groups, 1 frame lookup group,
  2 completed download groups, 4 typed network metadata groups, 1 header forwarding group
  and 2 doctests** (297 checks total).
  Headless Shell and Firefox were installed and exercised; unsupported-engine branches remain explicit.
- The five new groups additionally passed with full Chrome and Firefox, covering
  exact/glob/regex and predicate URL matching, frame history, request-start and
  response-header timing, in-flight requests, redirects, asynchronous predicates,
  errors, deadlines and disposal. Binary/Unicode/MIME upload payloads, multiple
  and empty batches, input/change events and native HTTP multipart submissions
  passed on both engines. Console/error source positions, epoch timestamps and
  page IDs survived closed popups, retries, cleanup, page-buffer clearing,
  trace export and portable JSON/escaped HTML reports.
- The existing four step/bundle groups cover
  preset/dynamic local skips, nested metadata and attachments, recovered and
  propagated step timeouts, enclosing budgets/cancellation, automatic action and
  assertion deduplication, hook/fixture lifetimes and shared worker cleanup.
  Native screenshots, traces, videos and attachments were exported, moved to a
  different folder and served over HTTP after original files were deleted; every
  HTML artifact link loaded successfully on both engines. Canonical aliases,
  basename collisions, relative source paths, re-export preservation and explicit
  missing/non-file errors have unit regressions. Run-step attachments share the
  same deduplicated copies as attempt/step attachments.
- Strict Clippy on all `ferrite-e2e`/`ferrite-cli` targets, package formatting, diff checks and
  matrix regeneration passed. Reporter panic containment and live timeout
  extension/shortening/zero/slow semantics have unit regressions.
- Existing trace and first-attachment names remain compatible; retry trace files
  and repeated attachment names preserve their individual contents.
- CLI/configuration checks passed again: 5 CLI tests, 17 configuration tests and
  1 doctest (320 checks across E2E/CLI/configuration). This change adds no CLI
  options. A real portable HTML report with expanded automatic/user/hook trees,
  skipped steps, annotations and run lifecycle was rendered in Chromium and
  visually inspected. The console section was also rendered and visually
  inspected with escaped text, source URL/position, native timestamp and page ID.

New regressions cover isolation/retries/locks, project engines and cleanup,
persistent profiles,
strict/shadow/semantic locators and delayed actions, callback navigation,
clock semantics, frame replacements/handles, storage/API cookie sharing,
HTTP payload/options, configuration forwarding and coverage. Existing tests
also exercise downloads, credentials, routing, snapshots, input and reports.

## Fixture scopes, suites and network lifecycle

- `Fixture<T>` definitions declare dependencies, test/worker scope, lazy or
  automatic setup and optional teardown. Tests request lazy values with
  `Test::fixture::<T>()`; setup reads only declared dependencies through
  `FixtureMap::require`. Legacy `Runner::fixture` remains automatic/per-attempt.
  Registration order is independent of dependency order. Missing/duplicate
  types, cycles and worker-to-test scope inversions fail before setup.
- Test values are rebuilt on retries. Worker values are reused by tests on the
  same worker/project and cleaned up after dependent test values. Unexpected
  attempt failures retire logical worker resources before reuse; the Tokio
  worker index stays stable. Teardowns run in reverse dependency order, even
  after partial setup, errors or cancellation, with one enclosing budget per cleanup scope.
- `Suite::tests` preserves nested identity. `beforeAll` runs once per participating
  worker/project; `afterAll` runs after its remaining descendants, inner suites
  first. Per-attempt hooks run outer-to-inner before the body and inner-to-outer
  afterward. Timeout/retry/context overrides and tags inherit through nesting;
  skipped or filtered suites do not execute hooks. BeforeAll uses its own budget.
- Page/context subscriptions expose request IDs and distinct `RequestFinished`
  and `RequestFailed` events with method, URL and backend failure text. Response
  means headers received on both engines; HTTP errors still finish successfully.
  Redirect hops finish separately, and unknown/duplicate terminal events do not
  underflow network-idle accounting. Chromium reports explicit cancellation;
  Firefox BiDi reports cancellation as unknown (`None`).
- Regression evidence: `tests/scopes_and_network.rs` runs both native engines,
  including worker/project isolation, parallel workers, retry lifetimes, nested
  hook ordering/settings, cleanup errors, redirects, streamed responses and
  transport failures. Unit tests cover graph validation, partial setup teardown
  and protocol failure/redirect accounting.

Full Chrome 153's unauthenticated Digest challenge timed out in the existing
`context_digest_auth` regression, identically on the previous `402bd89` commit;
Chrome Headless Shell passed that regression. This browser-variant limitation
is separate from the new fixture/suite/network tests, which run with full Chrome
and Firefox. Chromium launch now disables popup blocking, matching
[Playwright's automation defaults](https://github.com/microsoft/playwright/blob/v1.63.0/packages/playwright-core/src/server/chromium/chromiumSwitches.ts),
so context popup event waits also work in full Chrome.
The IndexedDB regression closes its setup connection before clearing storage,
preventing a live connection from blocking deletion.

### Function wait arguments and results

`Page::wait_for_function_value` and its Frame counterpart accept a function or
expression string, a JSON-serializable argument and `FunctionWaitOptions`.
Rust source strings representing functions receive the argument; other expressions
are evaluated as written. Playwright distinguishes JavaScript function objects
from expression strings; the reference passes function objects for function cases. JavaScript truthiness of the immediate predicate return controls success. A
returned promise is truthy and is awaited for its result, even if that result is
false, as in pinned Playwright. The JSON result is
captured at success without re-running the predicate. JSON uses native
`JSON.stringify` rules; cyclic/BigInt/top-level nonserializable values fail.
`Page::wait_for_function_handle` retains the actual successful value, including
objects that cannot serialize. Dispose the returned handle after use.

The default schedules in native `requestAnimationFrame`; interval polling uses
whole milliseconds in 1..=2147483647. Background/hidden document animation-frame
throttling is native behavior. Rust reads completion independently of scheduling.
Page action timeout defaults apply; zero disables the local budget, with enclosing
runner deadlines and cancellation preserved. Predicate errors fail directly.
Document replacement restarts polling within the same budget; detached frames
fail. Success, timeout, cancellation and dropped Rust futures reclaim owned
timers and result state. User-created promises/side effects cannot be canceled
by the library. Frame remote handles and handle arguments remain unsupported.
The legacy unit-returning expression waits retain their existing 50ms behavior.

The [official contract](https://playwright.dev/docs/api/class-page#page-wait-for-function)
and pinned five-case Playwright 1.63.0 corpus cover JSON arguments, promise
results, JavaScript falsy values and both scheduling modes.

### Native frame lookup

`Page::main_frame()` returns the current native root handle.
`frame_by_url_matching(&UrlMatcher)` and `frame_by_url_where(predicate)` search
the current native frame tree in tree order and return the first match or None.
Exact/relative glob patterns use the configured base URL; the existing substring
and name helpers preserve their contracts. Lookups are snapshots, not waits.

Frame identities survive navigation of the same native frame, and never retarget
a removed/replaced iframe. `Frame::url()` retains its lookup URL; `current_url()`
reads navigation updates. `is_detached()` checks identity in the current tree
and returns true after explicit owning-page closure; native disconnection errors
propagate. Firefox names remain empty because its native tree supplies no names.
Same-origin nested/replacement cases have native regressions on both engines.
Selector-free OOPIF traversal remains deferred.

### Native frame, document and dialog events

Page and context subscriptions expose `FrameAttached`, `FrameNavigated`,
`FrameDetached`, `DomContentLoaded`, `Load` and `DialogClosed`. Context events
retain source page identity and receive each page observation once. Frame
payloads are owned metadata snapshots, without retaining live Page/Frame owners.
They use native frame and document IDs; unavailable URLs/names/document IDs
remain optional. Navigation preserves the native frame identity, while removed
and replacement frames have distinct IDs. Fragment/history observations include
repeated updates to the same URL. Subtree detach emits children before their
parent, marks ancestor-driven removals and suppresses later duplicate native
notifications. Only live frame metadata is retained.

DOM/load events describe the main document. Repeated `set_content` operations
can emit readiness again without changing its document ID. Dialog-close payloads
retain accepted/user-text fields when supplied. Firefox identifies the prompt
context/type; CDP omits these fields, so Ferrite leaves them unavailable. These
are snapshots rather than Playwright 1.63's live `Dialog` payloads.

Firefox navigation/history/dialog-close subscriptions are probed individually.
Waiting for navigation or closure fails explicitly when its required native
subscription is unavailable; older engines can lack particular same-document
events. Firefox frame names remain unavailable. Chromium observes its current
target session: OOPIF session adoption is deferred, and a native `swap` detach
means leaving that session. Early popup observations use the ingress capture
described below.

Page/context event waits retain zero/caller/enclosing deadlines, disposal and
transport wake-up. Context waits use live action-timeout defaults. Interrupted
page initialization releases its listener without closing the shared browser
transport. The [native regressions](crates/ferrite-e2e/tests/lifecycle_events.rs)
compare four actual pinned Playwright cases on both engines, with separate
checks for identity, forwarding, metadata absence and wait/resource lifecycle.
Exhaustive matches on public event enums must handle the six new variants.

### Earliest popup diagnostics

Popup creation binds native source ownership at transport ingress before the
asynchronous adopter starts. Native console/errors, requests and lifecycle events
forward once to the context even when an immediately closed popup cannot produce
a usable Page. Successful Page adoption shares that observation state; its driver
listener handles its own runtime/download state without replaying general events.
Failed initialization releases the driver listener while bounded ingress capture
can retain later logs and native destruction.

`BrowserContext::popup_diagnostics()` returns owned metadata and startup history.
Adoption status, optional error, observed native closure and capture truncation
are separate facts. A setup failure does not imply native closure. Context history
holds 64 popups and reports evictions; the connection holds at most 64 strong
pending captures, then weakly observes adopted captures. Pending-slot eviction
settles observed in-flight requests unavailable, records truncation and preserves
the real adoption outcome if the queued adopter resumes. Retained history still
records destruction after its ingress slot is evicted.

Each projection is bounded to 1,024 native observation events and 2 MiB. It includes
all pre-adoption observations and the initial main document, then stops growing
after successful adoption and a later main-document commit. Captured requests
remain independently owned and can acquire completion afterward. Adopted Page
observation continues when this diagnostic projection reaches its limit. Clearing
popup history releases its records independently of context console history and
does not cancel live observation. Context disposal/transport loss settles pending
requests explicitly; if capture stops before destruction, `closed` can remain false.

Attempt results, traces and portable reports retain the owned history across
cleanup and retries. Migration: exhaustive `AttemptResult` literals require
`popup_diagnostics: Default::default()`; omitted fields in older JSON deserialize
as an empty history. This is a Rust diagnostic extension, not a live upstream
Popup/Page/Request object graph or complete remote value serialization.

The [native popup regressions](crates/ferrite-e2e/tests/popup_diagnostics.rs)
compare two actual pinned Chromium reference cases on both engines and exercise
retry/trace/report retention, independent clearing, cancellation/disposal and
disconnect. Deterministic wire fixtures additionally verify bursts before any
adopter subscribes, failed initialization followed by logs/closure, capture limits
and eviction/recovery. The immediate-close fixture dispatches a synchronous HTTP
request before closure; an aborted fetch with no native request event is not
fabricated. Upstream's earliest navigation `Request.frame()` can itself throw
before a frame exists, as recorded in the pinned reference.

Validation: all 332 E2E checks passed (152 units, 177 integrations across all
24 targets and three doctests), plus 23 CLI/configuration checks. The four popup
groups and seven runner/report targets ran on full Chrome 153 and Firefox 157;
the broad browser/routing/core batches used Chrome Headless Shell and Firefox.
Strict E2E/CLI Clippy, formatting and regenerated source links passed. The actual
pinned reference reproduced exactly. Expanded native retry reports were visually
inspected for both engines; all seven artifact links per relocated bundle resolved.

### Structured errors and console values

`ConsoleMessage::arguments` exposes up to 64 owned `ConsoleArgument` records.
The value enum distinguishes tagged JSON (including null), partial native previews,
unserializable primitive spelling and unavailable by-value data. CDP supplies
property previews; BiDi supplies typed nested values. These are not complete
JSHandle.jsonValue() results. Remote ownership identifiers are stripped, with a
boolean recording reference presence; cyclic references remain explicit partial
typed previews. Ordinary by-value JSON keys are preserved.

The argument payload has a 32 KiB serialized bound, with 4 KiB strings, 32-entry
containers and 256 traversed nodes. JSON depth is capped at six; native encoding
permits 24 levels for typed BiDi wrappers. Truncation/omitted arguments are
explicit. Existing rendered console text remains unchanged. Native bigints retain
their spelling (CDP `12n`, BiDi `12`): actual pinned Playwright 1.63 instead returned
an undefined console argument handle for this value. The corpus records that
difference and compares the remaining primitive values directly.

Uncaught errors expose boxed `PageErrorInfo` with optional own native name/message/
stack, constructor class, description, supplied stack frames and a thrown-value
preview when available. Constructor class is never substituted for Error.name.
Firefox's uncaught log supplies text/frames without separate name/message or thrown
object fields; these remain None. Error data has an independent 32 KiB bound and
at most 64 frames/eight supplied async stack segments; positions remain zero-based
and unavailable coordinates remain optional. No property getter/evaluation or
remote-handle lifetime is introduced to fill unavailable fields.

The WebError mapping also recognizes existing native page identity/source location
fields on ConsoleMessage; the prior Missing location label was an audit omission.
These optional snapshots remain narrower than upstream's live WebError/Page/Error
objects and always-present location accessor.

The owned payload forwards once through Page/context events, Page/context/popup
history, attempt reports and trace JSON. Console `TraceEntry` records also retain
the full message; other entries keep None. Portable HTML expands structured errors
and console arguments with escaped diagnostic text. Returned snapshots remain
usable after Page/context/browser disposal and retries retain separate source IDs.

Migration: exhaustive `ConsoleMessage` literals require `arguments: None` and
`error: None`; exhaustive `TraceEntry` literals require `console: None`. TraceEntry
is now exported and deserializable for typed consumers. Older console/trace JSON
missing the added fields deserializes with None. The [native regressions](crates/ferrite-e2e/tests/structured_console.rs)
compare the pinned corpus, forwarding/trace identity, live caps, closure/disposal
and popup retry/report retention. Actual normalized CDP/BiDi wire fixtures cover
null roundtrip, mutable error names, field absence, Unicode limits and async stacks.

Validation: all 338 E2E checks passed (155 units, 180 integrations across all
25 targets and three doctests), plus 23 CLI/configuration checks. Three structured
console groups and the runner/report/popup batch ran on full Chrome 153/Firefox
157; broad browser/core/routing batches used Headless Shell/Firefox. Strict Clippy,
formatting and regenerated matrix links passed. The two actual pinned cases compare
shared primitives and available Chromium error fields, with the observed bigint
handle difference explicit. Expanded reports/argument sections were inspected on
both engines and all seven artifact links per relocated retry bundle resolved.
Visual inspection exposed overly strict counting of BiDi encoding wrappers;
the adjusted native-depth limit preserves ordinary nested values, with final unit
and live-browser regressions verifying the byte/node/container limits and JSON.

### Completed download I/O

`Download::read()` asynchronously reads the completed file;
`create_read_stream()` opens a Tokio AsyncRead/AsyncSeek file for incremental
reads. Both reject recorded failures and expose missing-file I/O errors.
Options companions accept a local timeout and cancellation; None/zero disables
the local budget. Stream options cover opening only: subsequent reads use Tokio
I/O and can be wrapped in `CancellationToken::run` as one larger operation.
Dropping the stream releases its file handle; it does not delete the download.
Whole-file reads allocate for the file contents, so use incremental streaming
for large files.

`page_id()` retains only source identity, returning None for `from_path()` files.
Completed file access does not retain or require a live browser page. Deletion
is idempotent for NotFound and reports all other I/O failures. The earlier API
silently discarded those failures. Firefox URL/failure/active cancellation
metadata stays unsupported; it is not inferred from the downloaded filename.
These are completed-file companions to the [official download API](https://playwright.dev/docs/api/class-download#download-create-read-stream),
with a narrower active-download lifecycle.

### Shared browser owners and disconnection state

`BrowserContext::browser()` upgrades a weak reference to the actual shared
Browser owner. Explicit, convenience, default/persistent and attached Chromium
contexts all support it. A retained context can identify its owner after context
disposal, but returns None after the final Browser owner handle is gone.
Browser clones and retrieved owners share the process, profile, context registry,
base URL and default context. Worker/fixture handles use that same state.
Closing one handle shuts down all handles; dropping one retains the process
while another owner is alive. Context/page references alone do not retain it.

Explicit close starts one shutdown, which continues after its waiting future is
dropped; repeated/concurrent close calls await that shutdown. Temporary profiles
are released after the launched child exits. Final-owner drop uses bounded native
shutdown and reaping on an active Tokio runtime, with synchronous kill-on-drop
as the fallback outside a runtime. Persistent paths are never deleted. Attached
Chromium owners disconnect Ferrite without terminating the remote browser.

`Browser::is_connected()` includes shared shutdown and native transport state;
context/page `is_closed()` also reflect native disconnection. An empty context's
pending event wait wakes with Disconnected rather than waiting forever. A Closed
wait started after loss returns its already-known terminal state. Losing a
transport does not fabricate native popup closure observations or restore live
operations. The upstream emitter/reason surface remains narrower.

Migration: `Browser::base_url()` returns `Option<String>`, replacing its borrowed
string result so another shared handle can safely change future-context defaults.
Replace `.base_url().map(str::to_string)` with `.base_url()`; bind the returned
value before borrowing with `as_deref()`. Existing contexts retain their original
base-URL seed.

The actual pinned [ownership reference](scripts/e2e-conformance/ownership-reference.json)
records two Playwright 1.63.0 Chromium cases. Four
[native ownership groups](crates/ferrite-e2e/tests/browser_ownership.rs) passed on
Chrome 153/Firefox 157, covering retrieved-owner operations, defaults/registry,
convenience disposal, retained closed-context identity, last-owner release,
canceled/concurrent shutdown, persistent profiles, remote attachment and native
transport loss. Linux checks actual process reaping and temporary-profile release.
An initial test lookup failed because Chrome rewrites Linux argv; the corrected
lookup then exposed a profile leak with immediate process kill. Bounded graceful
final-owner shutdown fixed that leak, and the complete native group passed.

Validation: all 342 E2E checks passed (155 units, 184 integrations across all
26 targets and three doctests), plus 23 CLI/configuration checks. The runner/
diagnostics batch, including ownership and failed-storage setup, ran on full
Chrome 153/Firefox 157; browser/core/routing batches used Headless Shell/Firefox.
A final full-Chrome/Firefox check additionally verified an attached client's
unexpected source-browser disconnect. Strict E2E/CLI Clippy, formatting,
regenerated matrix links and local evidence links passed. B16 is complete;
G04 remains open for the final cross-feature lifecycle audit.

### Locator descriptions and operation diagnostics

`Locator::describe`, `description` and `clear_description` keep user labels
separate from DOM selectors. `describe` returns a new handle; empty labels clear
it. Cloning and timeout/cancellation decorators retain labels. Derived picks,
filters, scoped/combined selectors and matching builders clear them. Describe
after the last selector change. Content-frame conversion/owner retains its
iframe label; frame picks and child locators start unlabeled. Rust Display/
`to_string()` returns the label or raw selector; `selector()` remains raw.

The outer calling action/assertion includes its label once in operation errors,
automatic/live steps, attempt histories and owned `locator-operation` trace
entries, including errors raised by options validation. Automatic-step
suppression and user-step parent identity remain intact. Existing automatic
source locations point at the test definition; explicit user steps retain their
caller location. This does not add exact async Rust call-site attribution or
Playwright Trace Viewer compatibility. No report JSON schema changed.

Timeout/cancellation/locator/CDP errors retain their variants, codes and native
identity fields. Test/step skip reasons remain unchanged. Migration: the new
`E2eError::Diagnostic { context, source }` variant annotates opaque I/O, HTTP or
JSON failures while preserving their typed cause and machine code. Exhaustive
matches need this additional variant; inspect `source` or the standard error
chain for the original error. Typed JSON results/arguments and local assertion
callbacks retain their existing non-Send support.

The actual [pinned reference](scripts/e2e-conformance/locator-description-reference.json)
records three Playwright 1.63.0/Chrome 153 cases. Derived-label and DOM-resolution
rules agree. Its timeout error contains the operation and selector but omits
the label; Rust deliberately includes it. This observed difference is covered,
not claimed as equivalent. Three [native groups](crates/ferrite-e2e/tests/locator_descriptions.rs)
passed on Chrome 153/Firefox 157, covering derivation/frame owners, early
validation, typed causes, cancellation, assertions and non-Send values/callbacks,
plus retry/live reporter/source metadata, trace deduplication, JSON and escaped
HTML. Two unit groups verify error identity, codes, typed causes and skip control.

Validation: all 347 E2E checks passed (157 units, 187 integrations across all
27 targets and three doctests), plus 23 CLI/configuration checks. The 48-check
runner/diagnostics batch ran on full Chrome 153/Firefox 157; the 93 browser,
180 core and 23 routing checks used Headless Shell/Firefox. Strict E2E/CLI
all-target Clippy, E2E/CLI/config package formatting, generated symbol anchors
and local links passed. Expanded retry bundles from both engines were visually
inspected; all three artifact links per relocated bundle resolved and label text
remained escaped. An additional whole-workspace formatting probe reports
pre-existing formatting differences in unrelated crates; it is not counted as
a pass. B17 is complete; B12 is next and G04 remains open.

### Attempt-owned soft assertions

`TestInfo::soft_asserts()` and `SoftAsserts::for_attempt` return a weak, clonable
AttemptSoftAsserts handle for one attempt. Its `run(message, future)` owns one
assertion step and exact synchronous call-site source; a mismatch records a failed
step but returns Ok so later checks run. `check`/`check_with_message` consume an
already awaited result and retain the collection source/current step without
creating an extra step. Only E2eError::Expect mismatches are collected;
operational deadlines, cancellation, disconnection, invalid options, typed
causes and test/step skip control propagate unchanged. Local futures remain supported.
Standalone SoftAsserts keeps its manual check/failures/assert_all contracts and
adds contextual messages; there is no custom matcher registry.

Collection publishes errors and Failed status immediately, survives a successful
body result, and reaches subsequent hooks/fixture teardown. Body mismatches can
satisfy expected-failure annotations; setup/cleanup mismatches remain unexpected.
Prior soft errors remain alongside timeout/interruption/hard failures or a later
skip request. Every retry owns a fresh collector. Atomic sealing after hooks/test
fixture teardown includes all accepted failures before result classification and
rejects late writes before artifact/context cleanup. Dropping an unfinished
attempt retains both soft failures and interruption rather than treating a
previously published Failed status as completion. The handle does not retain its
runtime; archive/query final report records after runtime release.

Migration: `AttemptResult::soft_assertions` is a serde-defaulted vector of owned
SoftAssertionFailure records (error/message/step ID/title path). Rust struct
literals need the additional vector; old JSON loads it as empty. TestError fields
are unchanged. JSON, HTML and live final-attempt callbacks retain these records;
HTML adds an escaped soft-assertion section with source and step path. Collection
emits a live on_error notification per mismatch; the existing final failure
notification reports the combined result.

The actual [pinned test-runner reference](scripts/e2e-conformance/soft-assertion-reference.json)
records five Playwright 1.63.0 cases. Normalized errors, continuation, retries,
expected failures, cleanup and parallel isolation agree on both native engines.
The upstream skip-after-mismatch case first fails, then skips its retry before
its body runs because a runtime skip alters upstream test configuration. Rust
modifiers stay attempt-local; the comparison explicitly omits the mismatch on
its retry before requesting skip again. This does not claim persistent modifier
parity. [Native soft checks](crates/ferrite-e2e/tests/soft_assertions.rs) also cover
source/step ownership, retained handles, setup/cleanup/fixtures, operational
failures, failed setup and dependency release, JSON defaults and escaped reports.

Validation: all 355 E2E checks passed (160 units, 192 integrations across all
28 targets and three doctests), plus 23 CLI/configuration checks. The 53-check
runner/diagnostics batch ran on full Chrome 153/Firefox 157; core183/browser93/
routing23 used Headless Shell/Firefox. The final 160-unit rerun includes actual
concurrent writers racing atomic sealing, a late future that is never polled,
weak-runtime release and unfinished-attempt interruption. Strict Clippy initially
found a test-only mutex guard held across browser shutdown; releasing it before
the await fixed the issue, and the affected native group plus final strict
E2E/CLI all-target Clippy passed. Package formatting, matrix symbol anchors and
local links passed. Expanded reports from both engines were inspected and all
three links per relocated bundle resolved with escaped text intact. B12 is
complete; B14 is verified below, with B13 and G04 still open.


## Effective project configuration and execution settings

`E2eConfig` now carries named `E2eProjectConfig` values, global filter/grep/invert,
repetition, selection/shard and snapshot-directory inputs through TOML/JSON and
the CLI child-process bridge. `Project` adds inverted grep, repetition and
output/snapshot directories. The CLI adds `--repeat-each`, `--output-dir` and
`--snapshot-dir`; filters and project selection travel through child environment
and complete JSON without mutating the parent process. All scalar legacy bridge
values, including timeout/cleanup/expect/global limits and video FPS, are set on
the child so inherited values cannot override explicit CLI settings.

`Runner::try_from_config` validates configuration immediately; the existing
non-fallible `from_config` retains validation errors and reports them before
run setup. Unknown selected projects are checked at run resolution, allowing
consumers to register projects in Rust after loading CLI/environment inputs.
Duplicate project definitions and conflicting library engine/launch settings
fail before launch. Repeated selection names are deduplicated. Workers and
repetition zero normalize to one; retries zero stays zero, and timeout zero
remains unlimited. Name/tag inclusion and exclusion are substring matching;
this is not upstream regular-expression grep compatibility.

`Runner::resolve_config(&browser)` returns owned `ResolvedRunConfig` and selected
`ResolvedProjectConfig` snapshots without launching dedicated browsers. Runtime
versions are native: supplied owner identity takes precedence over unrelated
E2eConfig launch inputs, and planned dedicated versions are None until successful
startup. Dedicated launch options describe requests, not auto-detected executable
provenance. Explicit same-engine launch options still create a dedicated owner.
Projects without overrides reuse the supplied owner. The supplied owner's base
URL is authoritative, including changes made by startup hooks before attempts.

Precedence for retries/timeout/context is explicit test, inherited inner/outer
suite, project, then runner. Context overrides replace the whole ContextOptions
value; proxy inheritance and TLS acceptance use the same helper as actual native
context creation. These are resolved creation inputs, not a dump of every native
browser preference or later page/context mutations. Shared project configuration
currently includes viewport and launch overrides; other context fields remain
available through library Project/Suite/Test builders. Existing backend errors
for unsupported context settings remain effective.

Read-only `TestInfo::config()` and `project_config()` expose scheduling/default
snapshots. `settings()` returns an owned `ResolvedTestSettings` copy with actual
attempt engine/version, context creation options, repetition, retry settings,
resolved directories and current runtime timeout. Editing the copy or mutable
legacy TestInfo identity fields cannot alter scheduling/default snapshots.
Attempt settings are stored at finalization in `AttemptResult::settings`;
`TestReport::configuration` stores the resolved run snapshot. Static skip/fixme
results retain the existing one-entry-per-project/no-attempt behavior.

Filters, selected projects, shard, CI focus protection and snapshot fallback
inputs are fixed once at the run boundary. Explicit builders/configuration win
over fallback environment values; `from_env` first applies legacy overrides to
shared JSON. Optional unset filter/path/selection inputs may inherit legacy
values. Global snapshot directory overrides project-output-derived defaults;
an explicit project snapshot directory wins. Otherwise each project uses its
resolved output directory plus `/snapshots`. Directories resolve against the run
working directory to absolute paths, without requiring them to exist. Explicit
assertion SnapshotOptions override page seeds; standalone snapshot helpers keep
their environment fallback. B10 now implements stabilization and explicit path
templates; report diffs and final phase verification remain open.

Reporter `on_configuration` fires once after project startup and before attempts;
if startup aborts, it emits the available planned snapshot before cleanup.
Configuration validation failures have no fabricated snapshot. Attempt
`on_test_configuration` fires immediately before `on_test_begin` and describes
initial values, whereas the retained attempt settings include runtime timeout
updates. JSON serialization and escaped collapsible HTML retain both run and
attempt settings; portable bundles rewrite artifact links while preserving the
original configuration paths as execution metadata.

Migration: exhaustive Project and E2eConfig literals need the new fields or
`..Default::default()`. New `E2eProjectConfig` also supports defaults. Exhaustive
TestReport literals need `configuration: None`; AttemptResult literals need
`settings: None`. Both report fields have serde defaults so historical JSON
remains readable. Configuration snapshots, BrowserKind/LaunchOptions,
ContextOptions, ServiceWorkerMode, VideoMode and SnapshotUpdate now serialize;
serde's Duration format on LaunchOptions is `{secs,nanos}`, while resolved
runner/attempt timeout fields use milliseconds. New Reporter callbacks have
default implementations, preserving existing custom reporters.


B14 validation: all **365 E2E checks** passed (164 units, 197 integrations across
all 29 targets and four doctests), plus **27 CLI/configuration checks**. The
58-check runner batch used full Chrome 153/Firefox 157; core187/browser93/
routing23 used installed Headless Shell/Firefox. Strict E2E/CLI/configuration
all-target Clippy and package formatting passed. Five native configuration
groups and four actual pinned runner observations cover precedence, zero values,
project/tag exclusion, repetition/sharding, runtime timeout/retries, dedicated
owner identity/release, artifact/snapshot inputs and early setup errors. Legacy
environment fallback is frozen before hooks in an isolated child; empty legacy
filters stay ignored, while explicit empty JSON filters are retained. Four
additional E2E unit checks and shared TOML/CLI/actual-child checks cover conversion,
validation, old JSON and bridge values. Final units were rerun after the empty
legacy compatibility fix. Initial test compilation needed two borrows; the first
native run used the intentionally panicking invalid-shard builder instead of
shared configuration, and passed after that test setup was corrected. Expanded
run/attempt settings reports from both engines were visually inspected, all 12
artifact links per preview bundle resolved, and native relocated bundles retained
configuration paths as execution metadata. Regenerated matrix inventory is
73 classes/1,018 members: Partial630/Missing332/Equivalent15/Idiomatic41.


## Fixture limits and shared cleanup accounting (B13)

`Fixture::setup_timeout` and `teardown_timeout` provide separate optional limits.
Unset setup limits use the enclosing hook/test clock; unset teardown limits use
its cleanup scope. A local zero disables only the local limit. A finite outer
clock always applies, including runtime changes to the test timeout. Worker
setup and suite beforeAll retain their existing separate setup budgets.

The configured `cleanup_timeout_ms` now covers a cleanup scope rather than
renewing for every operation: attempt afterEach/fixtures/artifacts/native close
and failure-triggered worker retirement; suite completion and any resulting
retirement; final worker/project retirement; and dedicated-browser shutdown plus
run afterAll/global teardown. Defaults remain 5 seconds, with zero unlimited.
Immediately ready cleanup may complete in one poll after exhaustion; every
pending operation receives its own timeout error. Reverse teardown still
releases ready dependencies. Retired fixture state is drained before reuse.

Page and context disposal have one background owner once started. Dropping a
close wait cannot abandon disposal; concurrent/repeated calls await the same
result. Native page error kinds are replayed; context close aggregates callback,
page and native disposal failures into a configuration error. A timeout reports
unfinished cleanup, not completed native disposal. Firefox lifecycle ownership
stays held until its background context close settles. Native protocol limits
still apply. Closing after an already-lost transport remains idempotent local
cleanup; it does not confirm native release of a remote context. Synchronous Rust
callbacks and filesystem operations cannot be preempted; trace writing refuses to start after budget exhaustion and reports
serialization/filesystem errors. Screenshot/video cleanup errors also affect the
attempt instead of being silently discarded.

The eight actual pinned runner observations in
[fixture-budget-reference.json](scripts/e2e-conformance/fixture-budget-reference.json)
show material differences: Playwright's explicit fixture limit can run outside
the test clock and covers setup plus teardown together; Ferrite uses separate
local limits capped by the enclosing clock. In the pinned ordinary teardown
exhaustion case, the completed dependency's teardown was not observed. Ferrite
intentionally attempts every remaining cleanup and preserves its diagnostics.
This is practical bounded Rust behavior, not a claim of identical accounting.

The native `fixture_budgets` target exercises dropped/repeated close waits with
actual user-context release, explicit setup limits and partial setup, shared
cleanup exhaustion, zero/shorter limits, worker rebuilds on retry, dynamic zero
and soft-error retention, and cancellation during fixture setup. Lifecycle units
cover one-poll exhaustion, local deadline intersections, error replay/owner
release and suite-plus-worker retirement without repeating cleanup.

Verification: all 377 E2E checks (170 units, 203 integrations across all 30
targets and four doctests) and 27 CLI/configuration checks passed. Runner/native
B13 cases used full Chrome 153 and Firefox 157; core/browser/routing batches
used matching Headless Shell 153 and Firefox 157. Final units and six native
B13 groups reran after adding actual target-ID and run-final clock checks.
Strict all-target E2E/CLI/config Clippy, package formatting, regenerated matrix
and local evidence links passed. Initial stack growth, lost-transport idempotence
and worker-fixture report labels were corrected before final verification.

## CI flaky policy and focus protection (D01)

Shared `E2eConfig`, the Runner builder, effective run snapshots and CLI child
forwarding support opt-in `fail_on_flaky_tests` and configurable `forbid_only`.
Boolean flags can explicitly disable config values (`=false`); legacy environment
values are validated. Existing automatic focus protection for `CI=1`/`true`
remains enforced. Historical report/configuration JSON defaults both new policy
settings to false. TestInfo's owned run configuration exposes effective policies.

The flaky policy rejects aggregate success without fabricating a failed test or
attempt. Recovered tests remain Passed/flaky and retain their successful final
attempt; `failed()` still counts actual unexpected failures. The policy does not
increment scheduling's max-failures counter. Repetitions and projects contribute
separate results. Expected failures, skip/fixme and ordinary success do not
trigger it; ordinary failures and global/user interruption remain unsuccessful
regardless of the policy. Existing Rust unexpected passes still fail immediately
without retry.

Live `on_end`, exit status, list/dot summaries and HTML expose the aggregate
outcome. JSON adds derived `status`, `exit_code` and `flaky_policy_failed` alongside
unaltered results. JUnit marks each rejected flaky case as `FlakyTestPolicy` so
CI consumers detect the policy violation, preserving final status/flakiness/
attempt count in properties. No synthetic testcase or failed attempt is added.
JUnit failures count policy violations; Rust `failed()` keeps its original rule.

Focus is independent of skip/fixme/expected-failure modes. Registered test/suite
focus is audited before name/tag/project filtering or sharding, including focused
skipped descendants. Accepted focus preserves these modes during selection.
Empty suites without registered descendants have no runnable inventory entry.

[Fourteen pinned actual runner observations](scripts/e2e-conformance/ci-policy-reference.json)
verify upstream exits, attempts, statistics, focus errors and JUnit counts.
Material differences remain: Playwright's filtered-focus case succeeds; Ferrite
deliberately rejects hidden registered focus. Upstream rejected flakiness exits
1 while its JUnit failures count remains zero; Ferrite's XML explicitly reports
the policy violation. Upstream can retry an unexpected pass to an expected
failure and classify it flaky; Rust retains its immediate unexpected-pass rule.
These are documented practical policies, not identical Playwright semantics.

Verification: all 386 E2E checks passed (173 units, 209 integrations across all
31 targets and four doctests), plus 28 CLI/configuration checks. Runner/native
policy tests used full Chrome 153 and Firefox 157; core/browser/routing batches
used matching Headless Shell 153 and Firefox 157. Twelve actual `ferrite e2e`
process cases (six per engine) verified configured/flagged on/off policy, hidden
focus, CI enforcement, inherited environment overrides, child exits and parsed
JUnit counts. Fourteen actual pinned runner cases were regenerated. Native
HTML screenshots from both engines were inspected; policy markers remain
distinct from individual outcomes. Strict all-target Clippy, package formatting,
matrix regeneration and local links passed. Late units/native policy and fixture
budget checks reran after reference comparisons and legacy-focus preservation;
the policy target reran after narrowing a test mutex's scope. The close-release
regression now waits within its existing deadline for actual target disappearance
instead of assuming destruction follows the acknowledgement immediately. G04
still must audit ignored native page close/detach errors.

The current matrix remains a partial inventory: 73 classes, 1,018 members,
Partial635/Missing327/Equivalent15/Idiomatic41. Public struct-literal migration
and backward JSON defaults are documented in the E2E example guide.


## Generic assertion polling controls (D02)

`expect_poll_with` and `expect_to_pass_with` accept a separate PollingOptions input
with timeout, interval sequence, optional message and cancellation. Existing
helpers keep their signatures and 50 ms cadence. The first probe is immediate;
intervals follow completed mismatches and the final interval repeats. Empty or
zero interval values fail validation before constructing a probe. Zero timeout
only removes the local window; caller/runtime deadlines continue to apply.

One clock bounds all probes and sleeps, including hung futures. An explicit
cancellation token interrupts either kind of pending work; bind a generic helper
to a context's public cancellation_token when disposal must wake an idle poll.
A finite enclosing runtime limit caps zero/long local windows and still cleans
up. Probe None or Expect mismatches retry; operational/control errors retain
kind/code/cause, including wrapped errors and a probe's own Timeout. Expiry of
the polling window itself becomes Expect with the last mismatch or no-data
message. The optional message appears in the outer step and final diagnostic.

A scoped retry probe prevents attempt-owned soft.run/check from collecting
intermediate mismatches; these calls propagate mismatch Results for retry. Wrap
the complete helper in soft.run to collect one final mismatch. Probe factories
and async bodies share this rule. Scope does not bleed into unrelated joined
work. One outer assertion step suppresses nested implementation steps. Retries
retain first-attempt errors/soft records and isolate a successful final attempt.
Standalone soft collectors and unrelated locator assertion semantics are unchanged.

Options companions support borrowed/mutable/non-Send probes and values; native
local blocks were exercised outside the Runner, whose test bodies still require
Send. No existing DTO fields or serde schema change is needed. Behavior migration:
generic helper operational errors that were previously retried now propagate;
signal pending with None or an assertion mismatch instead. Defaults remain five
seconds when using Rust Timeout/PollingOptions defaults, including toPass.

[Thirteen actual pinned observations](scripts/e2e-conformance/polling-reference.json)
record material differences: upstream default intervals are 100/250/500/1000 ms;
default toPass has timeout zero and ignores configured expect timeout. Empty
intervals abort after an initial mismatch in the pinned case, while zero intervals
permit rapid probes; Rust rejects both. Upstream may stop early when the next
sleep would cross its deadline; Rust waits until its actual window expires.
Upstream toPass retries ordinary thrown errors; Rust propagates typed operational
errors. Upstream nested soft records an immediate failure and its normal
expectation probes emit intermediate steps; Rust enforces final-only collection
and one outer step. The hung-probe case catches an enclosing test timeout;
its measured helper duration reflects setup time already spent from that clock.
These differences are explicit policies, not full Playwright equivalence.


Verification: all **397 E2E checks** passed: 180 units, 213 integrations across
all 32 targets and four doctests. Runner74 used full Chrome 153/Firefox 157;
core203/browser93/routing23 used matching Headless Shell/Firefox. All 28 CLI/config
checks and strict all-target Clippy passed; package formatting, regenerated
matrix and local source links passed. The seven new virtual-time groups and
[four native groups](crates/ferrite-e2e/tests/polling_options.rs) verify cadence,
local futures, typed errors, hung/zero/enclosing budgets, cancellation/disposal,
soft/retry/step/serialization rules and actual native context removal. Expanded
HTML previews were inspected on both engines. The pinned generator completed
all thirteen real runner cases with checked process exits and outcomes.

The broad gate exposed two existing fixture assumptions. The navigation
replacement fixture reused a cached image, allowing its load to finish before
a scheduled redirect; it now uses a fresh image URL to keep the replaced
first document pending. A short pointer timeout required key-up without proving
key-down had happened; its finite budget now permits native acquisition and
explicitly requires key-down/mouse-down before release checks. Both native
fixtures passed after correction, as did the full remaining inventory. Runtime
navigation and input behavior was unchanged. G04 remains open for the separate
native protocol/lifecycle/error audit and final cross-feature verification.


## Supported screenshot capture options (B09)

Page ScreenshotOptions and Locator.screenshot_with add viewport/document clips,
Device/Css scale, transparent default background, custom mask color, temporary
styles and a shared optional timeout. Existing entry points remain; full-page
masks are now accepted. Source boxes translate CSS viewport positions to native
document coordinates; caller DPR stays intact. Clips are finite, positive-sized,
intersected and enclosed in whole CSS pixels; empty/outside clips, invalid color/
quality, other-page masks and conflicting locator regions are typed Config.

Captures have a shared page gate and owned temporary node references. Styles
visit reachable same-origin documents/open roots; CSS and masks restore without
removing application-owned IDs/classes. One outer automatic action step handles
capture diagnostics. Cancellation, timeout or dropping an active wait starts
independent bounded restoration with the gate retained until it settles. Native
restoration errors remain visible; original capture error kinds survive combined
failures. Deferred errors are drainable and reported by a subsequent capture
before it mutates the page. Native owner disposal releases the document; already-
lost transport cannot establish remote restoration. Separate cleanup has a
five-second bound; blocking native JavaScript/CPU work is not preemptible.

Chromium captures full documents through a region, without setting/clearing
metrics. This retains actual viewport/DPR/mobile state. Transparent PNG restores
the last acknowledged Page.call background override; separate raw CDP mutations
are outside that ownership. Firefox rejects transparent background before DOM
changes. Device pixels use native capture; Firefox Css output normalizes its
native raster via Lanczos3 and re-encodes PNG/JPEG, rather than modifying DPR or
media queries. Native/output images are capped at 64 million pixels. Default
page captures explicitly select the viewport, fixing the prior Chromium path
that could capture the document with captureBeyondViewport and no region.

The [52 actual pinned cases](scripts/e2e-conformance/screenshot-reference.json)
use Chromium 153 and Firefox 157 through Playwright 1.63.0's public moz-firefox
BiDi channel. Shared viewport dimensions, scrolled clips, full-document masks,
styles and restoration were measured at DPR 1/2. Pinned Firefox ignores Css scale
at DPR 2 and rejects transparency. Upstream clears a pre-existing background
color after transparent capture, whereas Rust restores its last acknowledged
Page.call value. Upstream accepts JPEG quality zero and infinite clip width;
Rust validates 1..=100 and finite bounds. CSS animation suppression remains a
narrower algorithm; temporary CSS removal does not rewind layout/scroll side
effects or application execution. Closed roots/cross-origin style traversal,
WebP format and all upstream specialized capture options remain outside this
supported surface. Public literal migration and examples are in the E2E guide.


Verification: the current inventory is **405 E2E and 28 CLI/config checks**:
180 units, 221 integrations across all 34 targets and four doctests. The broad
404-check E2E inventory passed with runner81 on full Chrome 153/Firefox 157 and
core203/browser93/routing23 on matching Headless Shell/Firefox. A late trace
regression first reproduced a duplicate automatic screenshot action; internal
trace capture now bypasses the public reporting wrapper. All 16 related native
scope checks passed afterward, including the additional regression, seven public
capture groups and the native capability group. Strict all-target E2E/CLI/config
Clippy and formatting passed on the final source; the pinned generator completed
52 checked cases after final assertions and an uncontended rerun.

The initial broad routing gate failed once when an acknowledged same-URL
synthetic redirect response exposed zero Set-Cookie pairs instead of two while
its x-hop value remained present. The unchanged strict header target then passed
in isolation and the entire 23-check routing gate passed. This is recorded for
G04's route-ack/native-extra-event correlation audit; no header assertion was
relaxed and no root cause is claimed here. A concurrent pinned Firefox launch
also timed out at 15 seconds; its terminal process was rerun after native gates
with a 30-second launch budget and all 52 cases passed. These observations do
not imply complete screenshot or backend equivalence.

### B10 draft: successive screenshot assertions

The working implementation now requires successive captures for page and locator
PNG assertions, including missing/all/changed generation; it waits for fonts under
the same assertion clock and never starts a new post-expiry artifact capture.
Underlying operation failures retain typed causes. `SnapshotUpdate::Changed`
creates missing values and rewrites only mismatches; corrupt PNGs/invalid ratios
are configuration errors, and comparison rejects more than 64 million pixels.
Defaults now use B09 Css scale, hidden carets and CSS animation suppression.

The [actual snapshot reference](scripts/e2e-conformance/snapshot-reference.json)
contains 56 runner observations on both pinned engines. Playwright's `missing`
policy writes and fails the test; Ferrite preserves its write-and-pass behavior.
Upstream `all`/`changed` can replace an existing baseline using the last image
when stable capture expires. Rust leaves that baseline intact and fails. Unlike
upstream's first matching capture shortcut, Rust always requires a successive
pair, and retains retrying initially stable mismatches within the shared window.
Per-channel comparison remains different from upstream's perceived-color/YIQ
algorithm. These are documented differences, not full matcher parity.

B10 remains unchecked. Expected/actual/diff report
attachments, real delayed-font/animation evidence and final broad verification
are still required. Focused draft checks do not replace the completed B09
checkpoint or prove completion of the expanded current test inventory.

Final focused validation for this B10 increment: 187 units, 27 native integration
checks across seven targets, four doctests and 28 CLI/configuration checks,
246 combined. The integration scope is new snapshot_stability3, browser3
(snapshot/ARIA selection), effective_configuration5, runtime_and_reporters4,
screenshot_capabilities1, screenshot_options7 and step_controls_and_bundles4.
All native targets required both full Chrome 153 and Firefox 157. Final strict
all-target Clippy, package formatting, regenerated matrix and local links passed.
This is focused increment evidence, not a complete current integration inventory
run. A timeout fixture initially used a missing mask (validly ignored); changing
it to a missing capture target verified a real typed native timeout and restoration
without relaxing capture behavior. Font wait ordering was corrected and the final
native scopes reran after that change. CPU/decode/filesystem budget work remains
part of B10; synchronous computation is not preempted by an async deadline.

### B10 draft: configurable snapshot paths (`b34174d`)

`Runner::snapshot_path_template`, `Project::snapshot_path_template`, shared
`E2eConfig`/project fields, `--snapshot-path-template` and
`FERRITE_SNAPSHOT_PATH_TEMPLATE` now supply one resolver for page/locator PNG
assertions and text/PNG helpers. Project templates override global templates;
explicit `SnapshotOptions` directory/template/context values override page seeds.
Runner metadata and the relative base are fixed at startup. `TestInfo::snapshot_path`
returns a path without writing; `snapshot_options()` supplies the same inputs to
standalone helpers. Retries/repeats share baseline identity. Failure `.actual`
files follow the resolved baseline; per-attempt report diff attachments remain open.

Supported tokens are `{arg}`, `{ext}`, `{platform}`, `{projectName}`,
`{browserName}`, `{snapshotDir}`, `{testDir}`, `{testFileDir}`,
`{testFileBaseName}`, `{testFileName}`, `{testFilePath}` and `{testName}`.
A single prefix character such as `{/projectName}` is emitted only for a nonempty
value. Missing metadata expands to empty. File metadata must be inside the root
when file tokens are used. Invalid templates and unsafe dynamic name components
fail before capture or writing; literal template paths are trusted configuration.

```rust,ignore
let runner = Runner::from_config(&config).snapshot_path_template(
    "{snapshotDir}/{browserName}{-projectName}/{platform}/{arg}{ext}",
);
// Within a test_with_context body:
let baseline = ctx.info.snapshot_path("nested/Card.png", SnapshotKind::Screenshot)?;
let options = ctx.info.snapshot_options();
assert_snapshot_text("nested/Body", "expected text", &options)?;
```

Without a template, existing `<snapshot-dir>/<whole-name-slug>.png|snap` paths
remain unchanged. With a template, dynamic argument components/project/title use
the existing lowercase ASCII slug rules; nested name components remain directories.
PNG/text extensions are canonical, and matching `.png`/`.snap` input suffixes are
removed before expansion. Relative templates use `SnapshotPathContext::root_dir`
or cwd; the runner freezes cwd and uses it for `{testDir}`. This differs from
Playwright's JS config-directory base and separate test discovery directory.

The [actual pinned path reference](scripts/e2e-conformance/snapshot-path-reference.json)
contains 36 public path-only runner observations. Upstream string names flatten
slashes, retain case, preserve unknown tokens and use a platform-suffixed legacy
layout; Ferrite recognizes the additional browser token and rejects unknown tokens.
Anonymous names, ARIA kinds, snapshotSuffix and upstream naming overloads remain
unsupported. See the [official token contract](https://playwright.dev/docs/api/class-testconfig#test-config-snapshot-path-template).

Input struct literals gain `SnapshotOptions::path_template/path_context`, shared
config/project template fields and `Project::snapshot_path_template`; use
`..Default::default()` or builders where appropriate. Effective run/project/test
metadata adds serde-defaulted template fields and a run/test frozen root, so old
report JSON remains readable. No full screenshot matcher parity is claimed.

Final focused path-increment gates passed: 192 units, 29 native integration checks
across eight targets, four doctests and 28 CLI/configuration checks (253 combined).
This reruns the stable-capture increment scopes and adds snapshot_paths2; all
native scopes require full Chrome 153 and Firefox 157. Strict all-target Clippy,
formatting, matrix generation, 738 parity-document links and 655 source anchors
passed. The final unit run also verifies visible failure-artifact write errors
without losing the original mismatch. Full current-inventory verification and
the remaining B10 acceptance criteria are still pending.
