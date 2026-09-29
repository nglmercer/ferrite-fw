# Ferrite E2E compared with Playwright

Ferrite does **not** provide equivalents for every Playwright API or feature.
It implements a substantial browser-testing subset in Rust, but several
similarly named APIs have different behavior. It is not a drop-in Playwright
replacement, and its Chromium and Firefox backends do not have equal coverage.

Audit date: **2026-09-29**. Upstream baseline: the latest stable release at audit
time, [Playwright 1.63.0](https://github.com/microsoft/playwright/releases/tag/v1.63.0),
released September 4, 2026. Local baseline: working tree based on
`0d34463b291ccd71e4a12f74aae9f23ded7c50f5`, including pre-existing local changes
in `ferrite-e2e`. These findings describe the inspected implementation, not just
the capabilities advertised in the README.

The complete [API member matrix](PLAYWRIGHT-API-MATRIX.md) inventories **73
classes and 1,018 distinct documented JavaScript-applicable members**:

| Classification | Members | Meaning |
|---|---:|---|
| Equivalent | 15 | Counterpart for the basic operation or value; not full options/class/engine compatibility |
| Partial | 458 | Related operation, field or manual composition with material differences |
| Idiomatic | 42 | Comparable operation expressed through Rust facilities instead of a Playwright API object |
| Missing | 503 | No dedicated public counterpart found |

These are inventory counts, **not a behavioral-parity percentage**. Repeated
configuration properties count separately, overloads are collapsed, inherited
members appear on their declaring class, and deprecated/experimental members
are included. Browser APIs, assertions, test APIs, reporter APIs, Android and
Electron are covered. Other language-specific wrappers are excluded. Argument
options are compared by feature below rather than enumerated individually.
Runtime checks here exercise Ferrite's own tests; this is not a differential
execution of the same suite against both frameworks.

## Feature comparison

| Feature | Ferrite counterpart | Assessment |
|---|---|---|
| Chromium and Firefox automation | `Browser`, CDP/BiDi drivers | Present; backend capabilities differ |
| WebKit | None | Missing |
| Browser installation, managed versions and named channels | System discovery, executable override | Partial; no bundled browser/dependency installer |
| Launch and remote connections | `LaunchOptions`, `Browser::connect` | Partial; connection accepts a loopback Chromium CDP port, no Playwright-protocol remote connection |
| Persistent contexts and browser servers | Temporary profiles; `keep_profile` | Missing persistent-profile API and `launchServer`; retaining a temporary profile is not profile reuse |
| Isolated contexts | `Browser::new_context` | Available manually; automatic runner isolation is absent |
| Navigation and readiness | `goto_with_options`, load states, history, URL waits | Partial; unit return, narrower options and substring URL matching |
| CSS/text/XPath/role selectors | `Locator`, `get_by_*`, filters and combinations | Partial; semantic differences detailed below |
| Frame automation | `Frame`, `document_frames`, name/URL lookup | Partial; no `FrameLocator` or lazy cross-frame selection |
| Actions and input | Click/hover/tap/drag, fill, select, checkbox, files, keyboard/mouse | Partial; actionability and event semantics differ |
| JS evaluation and references | `evaluate`, `evaluate_handle`, `JSHandle` | Partial; JSON/string bridge and smaller handle API |
| Exposed callbacks and init scripts | Page callbacks; page/context init scripts | Partial; exposed callbacks reset on navigation; no context bindings |
| Auto-retrying assertions | `PageExpect`, `LocatorExpect`, `expect_poll` | Present subset; regex/array/options and several matchers absent |
| Soft assertions | `SoftAsserts` collector | Partial; explicit result collection, not integrated `expect.soft`/test error tracking |
| Custom assertion extension and `toPass` | Custom Rust checks | No corresponding matcher registration or `toPass` API |
| Screenshot and text snapshots | Snapshot helpers, assertion builders | Partial; different comparison/update/path algorithms |
| ARIA snapshots/assertions | Page snapshot helpers and exact text assertion | Partial approximation; no locator snapshot API/full ARIA tree matcher |
| Fake clock | Page `clock_*` methods | Partial; several controls do not match Playwright semantics |
| Request routing and mocking | Rules/handlers, fulfillment, fallback, limits, abort reasons | Present subset; engine and option restrictions |
| Network observation | `RecordedRequest`, event subscription, URL waits | Partial; no rich live Request/Response object graph |
| HAR recording/replay | `save_har_with`, `route_from_har` | Partial; body/format/timing/update limitations |
| WebSocket observation | Chromium `PageEvent::WebSocket` | Partial; no live socket abstraction or Firefox socket events |
| WebSocket interception/mocking | None | Missing `routeWebSocket` and `WebSocketRoute` |
| API testing | `ApiClient`, `ApiResponse` | Partial; independent HTTP client, no browser cookie sharing |
| Dialogs, downloads and popups | Handler decisions, download files, adopted pages | Present subset; reduced lifecycle/event/value interfaces |
| Storage/auth reuse | Cookies and one origin's localStorage | Partial; incompatible Playwright state format, no IndexedDB/OPFS persistence |
| Device and environment emulation | `ContextOptions`, seven device metrics presets, setters | Partial; fewer descriptors/options and many Chromium-only controls |
| Tracing | Context and runner JSON traces | Partial; no Trace Viewer-compatible archive or DOM/source snapshots |
| Video/live frame streams | Page recording/frames, runner video policies | Present; Chromium uses ffmpeg, Firefox native recording and screenshot-polled live frames |
| Parallel execution/retries | Tokio worker tasks and per-test retry/timeout builders | Present subset; shared context and different worker/retry lifecycle |
| Grouping/hooks/fixtures | Name-prefix `describe`, runner hooks, typed fixtures | Partial; no suite-scoped hooks, dependency graph or worker fixture scope |
| Projects/repetition/filtering/shards | `Project`, `repeat_each`, name/tag filtering, shards | Present subset; projects do not select independent browsers or context configurations |
| Test annotations/attachments | Test builders and `TestInfo::attach` | Present subset; fewer runtime mutators and metadata fields |
| Expected failures, focus and skip | `fail`, `only`, `skip`, `fixme`, `forbid_only` | Present subset; validation issue noted below |
| Named test locks | None | Missing Playwright 1.63 lock scheduling |
| Reports | Dot/list/JSON/JUnit/HTML serializers | Partial; no live custom Reporter/Suite/TestCase/TestStep API |
| Advanced reporting | None | Missing blob/merge/GitHub/Perfetto reporters and structured step timeline |
| Dev server lifecycle | `ferrite e2e`, `WebServer` | Present subset; one server configuration rather than full Playwright webServer options |
| CLI configuration forwarding | Environment variables to user cargo tests | Partial; several settings require explicit consumer code |
| Inspector/UI mode/code generation | None | Missing interactive debugging/recording/test explorer |
| Workers/service-worker automation | Worker blocking in Chromium | No Worker evaluation/events or service-worker enumeration API |
| JS/CSS coverage | Raw CDP possible | No dedicated Coverage API |
| File chooser interception | Input-file assignment | Missing FileChooser object/event; existing input uploads are available |
| Virtual credentials/WebAuthn | None | Missing Credentials API |
| Electron/Android/WebView automation | None | Missing experimental upstream backends |
| Component/story mounting | None | Missing `mount` fixture/integration |

Evidence: exported types in [lib.rs](crates/ferrite-e2e/src/lib.rs), browser
launching in [browser.rs](crates/ferrite-e2e/src/browser.rs), and the individually
linked implementations in the [member matrix](PLAYWRIGHT-API-MATRIX.md).

## Differences that affect correctness

### Test and page isolation

Playwright Test creates a fresh browser context for each test. Ferrite's
`Runner::run` obtains `browser.default_context()` once, then `run_one` creates
pages in that same context for tests and retries. Cookies, localStorage and
other context state can leak between tests, including parallel tests. Manual
`Browser::new_context` isolation exists, but the runner does not use it.
`Browser::new_page` also uses the shared default context; Playwright's convenience
`browser.newPage` creates a separate context.

Evidence: [runner.rs](crates/ferrite-e2e/src/runner.rs), `Runner::run` and
`run_one`; [browser.rs](crates/ferrite-e2e/src/browser.rs), `Browser::new_page`.
Upstream behavior: [test isolation](https://playwright.dev/docs/browser-contexts).

### Locator strictness and waiting

Ferrite actions select the first match unless `.strict()` is requested.
Playwright single-target locator operations reject ambiguous matches.
Ferrite's `first()` uses the same `Pick::First` as an un-narrowed locator;
its resolved set is not sliced, so `first().count()` or `first().all()` can still
include multiple matches.

The trusted click/hover/tap/drag paths check existence and visibility after
scrolling, but do not implement Playwright's full stability, enabled,
hit-target and retry behavior. The initial scroll can fail immediately before
the target exists. `fill`, `check` and several other actions execute DOM code
immediately. Consequently, a page that renders an input later can fail
`fill` rather than waiting for it.

`ClickOptions::force` invokes synthetic `el.click()`; Playwright's force option
changes actionability checks while retaining its input-action semantics.
Ferrite's overlay handlers check match count, rather than visibility, before
DOM actions; they do not run on every assertion retry or wait for the overlay
to disappear.

Evidence: [locator.rs](crates/ferrite-e2e/src/locator.rs), `Selector::resolve_with`,
`Locator::first`, `ready_state`, `click_with_options` and `fill`;
[page.rs](crates/ferrite-e2e/src/page.rs), `action` and
`run_locator_handlers_inner`. Upstream behavior:
[actionability checks](https://playwright.dev/docs/actionability) and
[locator API](https://playwright.dev/docs/api/class-locator).

### Selector, text and accessibility semantics

CSS selectors use `querySelectorAll` without shadow-root traversal or
Playwright-specific selector extensions. Text matching is a case-insensitive
substring search sorted by text length, capped at 20 matches. The helpers
do not expose regex or exact-match options.

`get_by_label` selects the `<label>` element itself, not its associated form
control. Role/name matching uses a limited role table and DOM text/attributes;
it does not implement the complete accessible-name computation. Hidden roles
are only filtered when the explicit include-hidden predicate is false.
Accessible name/description assertions are also approximations.

`Locator::text` returns trimmed `textContent`; it is used as the counterpart
for both `textContent` and `innerText` despite their different DOM behavior.
ARIA snapshots are a flat list of selected elements, capped at 200 nodes and
80-character names, with no full tree or depth/mode/boxes options. Locator
ARIA snapshots and the locator `toMatchAriaSnapshot` matcher are absent.

Evidence: [locator.rs](crates/ferrite-e2e/src/locator.rs), `resolve_leaf`,
`by_label`, `state_expression` and accessibility getters;
[page.rs](crates/ferrite-e2e/src/page.rs), `aria_snapshot` and
`aria_snapshot_json`. Upstream references: the pinned
[Locator definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-locator.md)
and [LocatorAssertions definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-locatorassertions.md).

### Clock, evaluation and frames

The fake clock applies only to the current document and is lost on navigation.
`clock_fast_forward` and `clock_run_for` are both aliases for advancing all
due timers. Playwright `fastForward` fires each overdue timer at most once.
Ferrite `resume` only flips a flag and does not restart wall-time progression.
`pause` has no target-time argument; fixed time and system time modify the
same internal timer clock, whereas Playwright provides separate semantics.

Evaluation uses strings and JSON deserialization, with no separate argument,
function or JSHandle argument bridge. `expose_function` must be installed
again after navigation; Playwright exposed functions survive navigation.
JSHandle has basic evaluation/property/disposal support, but no
`asElement`, `evaluateHandle` or `getProperties` methods.

Frame evaluation and locators exist, but frame names are empty on Firefox.
The implementation documents that coordinate actions may miss elements in
offset iframes. There is no lazy `FrameLocator`, `contentFrame` locator
conversion, or the selector-free cross-frame search introduced in 1.63.

Evidence: [page.rs](crates/ferrite-e2e/src/page.rs), `CLOCK_SCRIPT`, `clock_*`,
`expose_function` and `Frame`; [jshandle.rs](crates/ferrite-e2e/src/jshandle.rs).
Upstream: [clock semantics](https://playwright.dev/docs/api/class-clock), pinned
[Page definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-page.md),
and [1.63 release notes](https://github.com/microsoft/playwright/releases/tag/v1.63.0).

### Network, HTTP clients and persisted state

`RecordedRequest` merges captured request and response data. It does not expose
the full live Request/Response relationship, redirect chain, resource type,
failure/timing breakdown, security details or server address. Captured response
bodies are Chromium-only and capped at 1 MiB. Request/response waits match URL
substrings rather than general Playwright URL predicates.

`ApiClient` and `RouteInfo::fetch` use independent HTTP clients, without the
browser's cookie jar. There is no context/page request client with shared
cookies, API storage state, multipart builder, or full per-call timeout,
proxy/auth/redirect/retry configuration. API response JSON can be deserialized
into Rust types, but response URL/status text/security/timing metadata are
not retained by `ApiResponse`.

`StorageState` saves one origin and its localStorage. Context capture uses the
first open page, so it is not an aggregate multi-origin context snapshot.
Playwright's cookies/origins state format is not supported. IndexedDB can be
cleared on Chromium, but is not captured/restored; OPFS persistence is absent.
Permissions and HTTP credentials have fewer origin-scoping options.

Evidence: [api.rs](crates/ferrite-e2e/src/api.rs),
[page.rs](crates/ferrite-e2e/src/page.rs), `RecordedRequest`, `RouteInfo::fetch`,
`StorageState` and `storage_state`; [context.rs](crates/ferrite-e2e/src/context.rs),
`storage_state`. Upstream: pinned
[APIRequestContext definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-apirequestcontext.md)
and [BrowserContext definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-browsercontext.md).

### Runner, configuration and artifacts

Ferrite workers are concurrent Tokio tasks sharing one browser context.
`describe` prefixes names rather than defining suite execution scope. Hooks
run at runner scope, and typed fixtures are constructed per attempt without
fixture dependencies, worker scope, automatic fixtures or lazy resolution.
Projects only override name/filter/retries/timeout; all use the same Browser.
Filters match substrings instead of Playwright regular expressions.

`E2eConfig::expect_timeout_ms` exists but is not consumed by the assertion
implementation, which initializes its retry window to 5,000 ms. Explicit
assertion `.timeout(...)` is available. Distinct context/page navigation
timeout APIs and full test/suite timeout policies are absent.

The CLI passes some settings through environment variables to an arbitrary
user test command. `Runner::default` consumes filter/grep/shard/project variables,
but does not automatically consume the CLI's workers/retries/reporter values.
`Browser::launch_default` does not consume the browser variable. The bundled
example manually consumes browser, base URL and video; it does not consume
workers/retries/reporters. `--headed` and executable/config overrides are not
serialized into a complete test-process configuration. Explicit configuration
loading/builders are therefore needed for several advertised knobs.

Traces are custom JSON. They cannot be opened as Playwright Trace Viewer
archives and do not capture the same DOM/source/action snapshot data. Named
steps are page logs rather than a structured report tree. Screenshot
comparison uses an 8-bit per-channel threshold rather than Playwright's
perceptual threshold, and supports only missing/all/none update modes. The
available HTML report is not an equivalent interactive report/trace UI.

Evidence: [runner.rs](crates/ferrite-e2e/src/runner.rs),
[expect.rs](crates/ferrite-e2e/src/expect.rs),
[config](crates/ferrite-config/src/lib.rs),
[e2e command](crates/ferrite-cli/src/cmds/e2e.rs),
[example suite](examples/e2e/tests/e2e.rs),
[context tracing](crates/ferrite-e2e/src/context.rs), and
[snapshot.rs](crates/ferrite-e2e/src/snapshot.rs).
Upstream: [fixture model](https://playwright.dev/docs/test-fixtures),
[Trace Viewer](https://playwright.dev/docs/trace-viewer), and pinned
[snapshot configuration](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-testconfig.md).

## Engine restrictions

The table describes implementation support; Chromium was not installed for
this audit, so its entries were verified from source rather than runtime.

| Capability | Chromium | Firefox |
|---|---|---|
| Core navigation, locators, contexts, screenshots | Implemented | Implemented, with unresolved popup validation failure |
| User agent, proxy, insecure certificates | Launch and some context/page controls | Launch-wide; per-context overrides restricted |
| Locale/timezone/offline/global extra headers | Implemented | Unsupported |
| Basic/digest HTTP challenge credentials | Implemented | Unsupported |
| Device metrics/mobile/touch emulation | Implemented | Unsupported |
| JS disable, CSP bypass, service-worker blocking | Implemented | Unsupported |
| Color scheme/reduced motion | Implemented | Unsupported |
| Download policy and per-context directory | Implemented | Restricted to launch-wide directory; policy setters unsupported |
| Download cancellation and URL/failure metadata | Implemented | Unsupported / unavailable |
| Request body capture and response bodies | Implemented; response capture cap applies | Unavailable |
| Response modification and route URL replacement | Implemented | Unsupported; other routing actions have a subset |
| WebSocket frame observation | Implemented | Unavailable |
| Per-origin data clearing/IndexedDB clearing | Implemented | Unsupported |
| Request garbage collection | Implemented | Unsupported |
| Video | Frame capture assembled with ffmpeg | Native BiDi screencast |
| Live frame stream | CDP screencast frames | Screenshot polling |
| WebKit, client-certificate configuration | No public support | No public support |

These describe Ferrite's driver choices, not claims that the browser protocols
can never gain such capabilities. Evidence:
[driver.rs](crates/ferrite-e2e/src/driver.rs),
[context options](crates/ferrite-e2e/src/context.rs), and
[browser options](crates/ferrite-e2e/src/browser.rs).

## Validation

Existing tests were run without changing implementation or tests.

| Check | Result |
|---|---|
| Unit tests in `cargo test -p ferrite-e2e` | 98 passed |
| HTTP/download API integration tests | 2 passed |
| Initial browser suite | 84 passed, 1 failed (`runner_context_fixtures`) |
| Browser suite rerun using a larger `TMPDIR` | 83 passed, 2 failed |
| `runner_context_fixtures` alone with the larger `TMPDIR` | Passed |
| `page_wait_helpers_and_close` alone with the larger `TMPDIR` | Failed on Firefox |

The `/tmp` filesystem filled during the initial run; the rerun used
`TMPDIR=/home/meme/.cache/ferrite-playwright-audit`. The two failures in the
rerun were:

- `page_wait_helpers_and_close`: Firefox popup title was empty instead of
  `assert me`, despite waiting for a nonempty title. The isolated rerun also
  failed. See [browser tests](crates/ferrite-e2e/tests/browser.rs).
- `runner_context_fixtures`: expected-failure count was 2 instead of 1.
  This failed in the complete suite but passed alone. The cause is unresolved;
  the successful isolated run does not establish suite reliability.

Firefox was available; Chromium was missing. Browser tests return early or
skip individual engine/capability branches when unavailable, so a passing
test count does **not** mean every browser scenario ran. Doc tests were not
reached by the failed full commands. No runtime result establishes Playwright
behavioral equivalence.

## Priorities for closing the gaps

| Priority | Work | Why it matters |
|---|---|---|
| P0 | Fresh context per test and retry, complete cleanup | Prevent shared-state leakage and parallel interference |
| P0 | Consistent CLI/config propagation, wire assertion timeout | Ensure requested browser/runner settings affect the consumer suite |
| P0 | Investigate popup-title and suite expected-failure regressions | Existing validation currently fails |
| P0 | Strict locator defaults and complete action waiting | Prevent ambiguous actions and failures on delayed rendering |
| P1 | Correct label targeting, shadow DOM, role/name/text semantics | Match common Playwright locator behavior |
| P1 | Correct clock controls and callback navigation lifetime | Avoid misleading equivalents in timing/page lifecycle tests |
| P1 | Rich request/response objects and browser-linked API client | Support reliable API/auth/network test migration |
| P1 | Multi-origin Playwright-compatible state, IndexedDB/OPFS options | Support authentication and persisted application state |
| P1 | FrameLocator and accurate iframe coordinates | Support nested/cross-origin frame workflows |
| P1 | Suite/fixture/project scheduling and named test locks | Match isolation/configuration/lifecycle expectations |
| P2 | WebSocket routing, workers, coverage, file chooser, credentials | Close advanced automation API gaps |
| P2 | Structured reports/traces, UI mode, inspector, code generation | Close developer-tooling gaps |
| Scope decision | WebKit and managed browser distributions | Requires an additional browser/backend strategy |
| Scope decision | Experimental Electron, Android and component testing | Separate integrations rather than small API wrappers |

The priorities are an engineering judgment based on common test behavior and
the inspected gaps. They are not changes implemented by this audit.
