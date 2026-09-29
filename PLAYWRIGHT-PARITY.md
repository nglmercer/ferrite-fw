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
| Partial | 584 | Related exposed operation with material semantic, option or engine differences |
| Idiomatic | 42 | Comparable operation through Rust language/library facilities |
| Missing | 377 | No dedicated public counterpart |

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
| Browser lifecycle | Persistent Chromium/Firefox profiles, graceful close, Chromium HTTP/WebSocket CDP connections | No Playwright remote protocol, browser server, channels or managed browser installer |
| Configuration | Complete resolved CLI configuration forwarded as JSON; browser launch/runner consume it, with legacy overrides | Suite/test context, timeout, retry and tag inheritance; no named fixture option override hierarchy |
| Projects and scheduling | Independent project browsers/launch/context settings; parallel Tokio tasks, retries, repetition, filters and named resource locks | No process workers, project dependency graph or distributed locks |
| Artifacts | Per-attempt output directories, validated paths, unique attachment files, screenshots/video, live Reporter callbacks and portable HTML/JSON/JUnit bundles | Synchronous callbacks; no complete upstream reporter graph, stdout capture or status override |
| Step controls | Local timeout, skip with reason, live annotations/title paths and automatic action/assertion/hook/fixture trees | Explicit user steps have exact sources; automatic sources use test definitions; boxing and some Page/protocol actions remain absent |
| Runtime test controls | Shared TestInfo skip, expected failure, slow, annotations and timeout changes affect running attempts and final results | Result propagation for immediate skip; cooperative async cancellation and independent cleanup budgets |
| Locator selection | Strict single-target operations, genuine first/last/nth slicing, relative has/hasNot filters, exact/regex/visibility builders | No complete Playwright selector extension/custom-engine surface |
| Semantic locators | Associated labels target controls; roles and accessible names use shared DOM helpers; open shadow-root traversal | Full accessible-name specification and closed shadow roots remain outside this implementation |
| Actions | Retry readiness, visibility/stability/hit testing, trusted forced clicks, trusted checkbox/key input; delayed fill/select and contenteditable support | Some actions use DOM setters/events; full native input/event/layout semantics remain narrower |
| Uploads | Path and in-memory filename/MIME/binary payloads, multiple/empty batches and input/change events on both engines | DOM injection, 64 MiB total cap; native chooser and directory uploads remain deferred |
| URL/network waits | Exact, glob, regex and URL/request/response predicates, including async network predicates | Explicit Rust APIs and snapshot records; no waitUntil/URLPattern or complete live Request/Response objects |
| DOM access | Separate textContent/innerText, arrays, evaluate-all/JSON arguments, highlight removal; single-target getters wait and enforce strictness | JSON values only, without arbitrary JS/JSHandle argument serialization |
| Frames and handles | Same-origin lazy/nested/replacement `FrameLocator`, frame ownership, content/function/URL/load/selector helpers, remote handle evaluation/properties | Cross-origin/OOPIF lazy selection and ElementHandle are deferred |
| Assertions | Exact/regex page title/URL, raw regex/normalized or rendered text options, mixed lists, ordered text subsets, exact classes/class tokens, values, state/indeterminate options, native intersection ratios, accessible regex, custom predicates and `expect_to_pass` | Rust regex syntax; accessibility approximation; no custom matcher registry/asymmetric matchers or full options parity |
| Accessibility snapshots | Structured DOM role/name/state tree and locator/page exact snapshot assertions | Approximation, without complete ARIA/YAML matching or all upstream modes |
| Clock | Separate fixed Date/system time, run-for/fast-forward, promise/timer ordering, pause-at/resume and installation time | Page-local; navigation reinstalls initial state; idle callbacks approximate browser behavior |
| API testing | Query/headers/JSON/form/raw/multipart, cookies, TLS/proxy/auth, timeout, redirects, status checks and connect retries | IndexedDB state and all redirect/retry semantics remain deferred; returned response buffers are independently owned |
| Browser/API storage | Context-linked cookies in both directions; isolated protocol cookie partitions; Playwright cookies/origins localStorage JSON; Page/context API clients inherit transport defaults at creation | Redirect/partition/SameSite details remain narrower; no IndexedDB/OPFS snapshots |
| HTTP credentials | Browser challenge authentication on Chromium, preserving extra headers; explicit preemptive Basic helper | Firefox challenge credentials unsupported; cached-auth clearing is approximate |
| Callbacks and buffers | Page-exposed functions survive navigation; console/error source metadata, context history and per-attempt reporting | No context-wide bindings, async Rust callbacks or complete frame/worker dispatch |
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
  Cleanup has an independent per-operation budget rather than Playwright's shared
  afterEach/fixture-teardown budget.
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
  and artifact/close operations are bounded separately. Final unexpected failures
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
cleanup defaults to 5 seconds per operation.

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

`UrlMatcher::exact`, `glob`, `regex` and `contains` are reusable across
`Page::wait_for_url_matching`, `wait_for_request_matching` and
`wait_for_response_matching`; Frame supports matching URL waits too. Exact
relative URLs resolve against the configured base URL. Globs match the whole
URL: `*` excludes slashes, `**` includes them, `{a,b}` selects alternatives,
`?` is literal and backslashes escape characters. Regex anchoring follows the
supplied pattern. Invalid patterns fail at construction. Existing string waits
retain their explicit substring behavior.

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
changes and frame-scoped navigation. `waitUntil` and URLPattern remain absent.

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

- `ferrite-e2e`: **138 unit tests, 3 API tests, 93 browser tests, 7 attempt-diagnostics
  groups, 4 reliability groups, 4 runtime/reporter groups, 6 fixture/network
  groups, 4 step-control/bundle groups, 5 wait/upload/console groups,
  3 core conformance/capability groups and 2 doctests** (269 checks total).
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
  1 doctest (292 checks across E2E/CLI/configuration). This change adds no CLI
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
  after partial setup, errors or cancellation, with independent cleanup budgets.
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
