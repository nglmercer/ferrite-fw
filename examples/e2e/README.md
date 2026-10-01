# e2e demo

Minimal consumer of `ferrite e2e`: a counter app plus a Rust test suite.

```bash
cd examples/e2e
ferrite e2e --check                # verify Chromium launches
ferrite e2e --check --engine firefox
ferrite e2e                        # boot dev server + run tests/e2e.rs
ferrite e2e --engine firefox       # same suite on Firefox
```

`ferrite e2e` boots an in-process dev server on the `[e2e.web_server]` URL
(or reuses a running one), sets `FERRITE_E2E_BASE_URL`, and runs
`cargo test --test e2e`. Artifacts land in `test-results/`.

Runner limits can be configured in `ferrite.toml`:

```toml
[e2e]
global_timeout_ms = 120000
max_failures = 3
fail_on_flaky_tests = false
forbid_only = false
cleanup_timeout_ms = 5000
```

The corresponding flags are `--global-timeout`, `--max-failures` and
`--cleanup-timeout` (timeouts are milliseconds). Zero disables each limit.
The runner bounds setup/hooks/fixtures/body together, then shares one cleanup
budget across each cleanup scope. Already active tests finish when max failures is
reached; global timeout interrupts them and still runs cleanup.

Enable `fail_on_flaky_tests` to fail the overall run when retries recover an
unexpected attempt. The recovered test remains passed and flaky, with its
original attempt history. This policy does not consume `max_failures` or stop
later tests. Expected failures and skipped tests alone do not trigger it.
Repetitions/projects are counted separately; timeout/interruption remains a
failure independently. `Runner::fail_on_flaky_tests(bool)` overrides the config.

`forbid_only` rejects registered test/suite focus before filters, project filters
or shards can hide it, including focused skipped/fixme/expected-failure tests.
Focus remains separate from the run mode: `.only().skip()` still skips when
focus is allowed. `CI=1` or `CI=true` continues to force focus protection, even
with `forbid_only=false`. This inventory audit is stricter than Playwright's
pinned `grep` behavior. The CLI supports `--fail-on-flaky-tests` and
`--forbid-only`, plus `=false` to override configured true outside CI. The child
gets the complete JSON and explicit boolean environment values; library callers
can use `config_from_env()` / `Runner::from_env()`. Accepted legacy boolean values
are `true`/`false` (case-insensitive) and `1`/`0`; invalid values fail validation.

Live `on_end`, `TestReport::ok()` / `exit_code()`, list/dot summaries, JSON and
HTML expose aggregate policy failure without adding tests or attempts. JSON
adds derived `status`, `exit_code` and `flaky_policy_failed`; historical JSON
without configuration retains its original success rule. JUnit emits a
`FlakyTestPolicy` failure on each rejected flaky case so CI consumers also fail,
with `ferrite.final_status`, `ferrite.flaky` and `ferrite.attempts` properties
preserving its outcome. JUnit's failure count includes these policy violations;
`TestReport::failed()` continues to count actual unexpected test/run failures.
This JUnit behavior intentionally differs from pinned Playwright, which exits
unsuccessfully for rejected flakiness while reporting zero JUnit failures.

Source migration for exhaustive struct literals: add `fail_on_flaky_tests` and
`forbid_only` to `E2eConfig`, `fail_on_flaky_tests` to `ResolvedRunConfig`, and
`focused: false` to `Test` (legacy `mode: TestMode::Only` is still recognized).
Prefer config defaults and `test` / `test_with_context` constructors. Existing
serialized configuration/reports continue to deserialize with default policies.

Named project configuration is carried into `Runner::default()` by the CLI:

```toml
[e2e]
repeat_each = 2
output_dir = "test-results"
snapshot_dir = "baselines"
grep_invert = "slow"
selected_projects = ["desktop"]

[[e2e.projects]]
name = "desktop"
grep = "greeting"
repeat_each = 3
retries = 1
timeout_ms = 10000
output_dir = "test-results/desktop"
snapshot_dir = "baselines/desktop"

[e2e.projects.viewport]
width = 1280
height = 720
```

Add these settings to the existing `[e2e]` table rather than declaring it twice.
`--repeat-each`, `--output-dir`, `--snapshot-dir`, `--grep-invert`, `--shard` and
`--project` override global CLI inputs. Project values override global defaults;
test/suite retries, timeout and whole context overrides take precedence over
project defaults. Filters use name/tag substrings. Zero repetitions normalize
to one; zero timeout disables the deadline. Snapshot assertion options can
override the resolved directory/update mode.

Inside a `test_with_context` closure, inspect or attach effective settings:

```rust,no_run
let settings = ctx.info.settings();
let project_defaults = ctx.info.project_config();
println!("{:?} {:?}", settings.browser, project_defaults.map(|p| &p.name));
println!("run output: {}", ctx.info.config().output_dir);
ctx.info.attach("effective settings", format!("{settings:#?}").as_bytes(), "text/plain")?;
```

These snapshots describe actual selection, native browser versions and absolute
artifact/snapshot directories. Copies cannot change scheduling; `settings()`
reflects `set_timeout` updates. The final report stores run/attempt settings;
HTML shows them in expandable sections. `resolve_config(&browser).await` offers
a plan before native project startup, with dedicated versions still absent.

For API authentication, save `context.request().save_storage_state(path)` and
restore through `ContextOptions::storage_state`, or configure a standalone
`ApiClientOptions::storage_state`. Context clients inherit transport defaults
and share cookies across browser requests and HTTP redirects.

For challenge-only API credentials and a per-request redirect limit:

```rust
let api = ApiClient::with_options(ApiClientOptions {
    base_url: Some("https://example.test".into()),
    credentials: Some(HttpCredentials::new("user", "password")),
    credential_origin: Some("https://example.test".into()),
    credential_send: ApiCredentialsSend::Unauthorized,
    ..Default::default()
})?;
let response = api.fetch_with("GET", "/resource", ApiRequestOptions {
    max_redirects: Some(0), // inspect a redirect instead of following it
    max_retries: 1, // peer resets before headers; HTTP error statuses are not retried
    ..Default::default()
}).await?;
```

The existing preemptive Basic default remains `ApiCredentialsSend::Always`;
explicit Authorization headers take precedence. Cross-origin redirects strip
Authorization, and every redirect recomputes cookies from the shared jar.
Binary/multipart bodies replay through 307/308 and Basic challenges. Retries,
redirects and body reads share the total timeout/cancellation budget. New
`ApiClientOptions` credential fields and `ApiRequestOptions::max_redirects`
require updating exhaustive struct literals; `..Default::default()` preserves
existing defaults. TLS opt-out remains a client/context option.

Subscribe with `context.subscribe()` before triggering an action. Context events
include new pages/popups, console/errors, network, completed downloads and closure,
with source page IDs. `wait_for_event` provides a timed filter;
`wait_for_event_with_options` additionally accepts a cancellation token. A zero
operation timeout waits until success, cancellation or page/context disposal.

Page and context subscriptions also include native frame attachment/navigation/
detachment, main-document DOM/load readiness and dialog closure:

```rust,no_run
let (event, navigation) = tokio::join!(
    page.wait_for_event(ferrite_e2e::PageEventKind::FrameNavigated,
        std::time::Duration::from_secs(5)),
    page.goto("/account"),
);
navigation?;
if let ferrite_e2e::PageEvent::FrameNavigated(frame) = event? {
    println!("{} {:?}", frame.frame_id, frame.url);
}
```

Payloads contain native identity and optional metadata, without owning a live
Page/Frame. Same-document history events keep frame identity. Descendant removal
is reported once, child-first, with `detached_with_parent` when the ancestor's
native detach caused the observation. Readiness belongs to the main document;
repeated `set_content` calls retain distinct readiness events. Frame names are
unavailable on Firefox; CDP dialog closure omits frame identity/type. These
payloads are owned snapshots rather than upstream live Frame/Dialog objects.
New Firefox event subscriptions are individually probed; unsupported commit/
closure waits fail explicitly. Chromium observes its current target; OOPIF
adoption is deferred and a `swap` detach means leaving that session. Context
waits use live action-timeout defaults and also wake on transport loss.
Exhaustive PageEvent/Kind and ContextEventKind matches need the new variants or
a wildcard. Arm dialog handling before triggering a prompt.

When a route callback may still be running, remove routes with an explicit policy:

```rust
page.unroute_all_with(
    UnrouteOptions::default()
        .behavior(UnrouteBehavior::Wait)
        .timeout(std::time::Duration::from_secs(5)),
).await?;
```

Page and context removal also support pattern and shared-matcher companions.
Default removal releases active requests to the network while callbacks finish;
their later decisions are discarded and errors recorded. `IgnoreErrors` also
suppresses those errors. `Wait` awaits decisions within one shared budget;
timeout/cancellation stops the wait after removal, without canceling callbacks.
`Cancel` explicitly drops pending callbacks and aborts their requests. Closing
the page/context cancels its routing work. Handler hit limits now count every
invocation, including fallback, and remain shared across context pages.
Requests retain their dispatch-time registration order; removed entries cannot
start new calls, while new registrations apply to subsequent requests. Native
commands already issued may finish after cancellation.

Fetch and rewrite a real response through the route's owning context:

```rust,no_run
use ferrite_e2e::{RouteFetchOptions, RouteFulfillOptions};
use serde_json::Value;

page.route_with_handler("**/api/account", |route| async move {
    let response = route.fetch_with(RouteFetchOptions {
        max_redirects: Some(5),
        max_retries: 1,
        ..Default::default()
    }).await?;
    let mut account: Value = response.json()?;
    account["plan"] = Value::String("fixture".into());
    route.fulfill_with(RouteFulfillOptions {
        response: Some(response),
        json: Some(account),
        ..Default::default()
    }).await
}).await?;
```

Fetch inherits cookies, TLS/proxy/auth and HTTP defaults. Relative override URLs
use the context base URL. It leaves the browser request paused until the callback
returns an action. Options can replace method/headers/raw body/JSON and bound
redirects, reset retries, timeout and cancellation. Missing timeout uses the live
Page action setting; zero disables the local deadline.

Inspect `route.body_state()` before replaying an original payload: Chromium's
lossless binary entries have a 16 MiB cap, and Firefox's original bytes are
unavailable. Supply `body: Some(bytes)` or `json: Some(value)` explicitly when
needed; `Some(vec![])` means an intentional empty override. Text-only native
previews do not establish replayable bytes. `RouteInfo::new(...)` constructs a
detached record and replaces external struct literals now that native owner/state
fields are private.

Fulfillment supports response inheritance, status, duplicate-preserving replacement
headers, raw/JSON bytes and a regular-file path. Explicit content type wins;
truthy JSON then file MIME inference follow the pinned reference. A file wins
over body/JSON bytes, while supplying both body and JSON is invalid. Inherited
Content-Length is preserved even when bytes change. The request-aware companion
adds cross-origin CORS headers when no allow-origin value exists. Static legacy
helpers keep their contracts. An independent HTTP fetch URL override works on
both engines; native intercepted-URL/response-stage rewriting stays Chromium-only.

`page.with_cancellation(token)` and `locator.with_cancellation(token)` apply
cancellation to their clones. `with_timeout` overrides their action/protocol
budgets without changing siblings. Frame helpers include `page`, `set_content`,
`current_url` and function/URL/load/selector waits. See
[the parity audit](../../PLAYWRIGHT-PARITY.md) for engine and API limitations.

Define fixtures with explicit Rust dependencies and scopes:

```rust
use ferrite_e2e::*;
use std::time::Duration;

struct Token(String);
struct Profile(String);

let runner = Runner::default()
    .fixture_definition(
        Fixture::<Token>::new(|_| async { Ok(Token("session".into())) })
            .scope(FixtureScope::Worker),
    )
    .fixture_definition(
        Fixture::<Profile>::new(|dependencies| async move {
            Ok(Profile(dependencies.require::<Token>()?.0.clone()))
        })
        .dependency::<Token>()
        .setup_timeout(Duration::from_secs(2))
        .teardown_timeout(Duration::from_secs(1))
        .teardown(|_| async { Ok(()) }),
    );
let tests = Suite::new("account")
    .retries(1)
    .before_each(|page| async move { page.goto("/").await })
    .tests(vec![
        test_with_context("profile", |ctx| async move {
            assert_eq!(ctx.get::<Profile>().unwrap().0, "session");
            Ok(())
        })
        .fixture::<Profile>(),
    ]);
```

New definitions are lazy; `.automatic(true)` opts into unconditional setup.
The legacy `Runner::fixture` helpers remain automatic and per-attempt.
Worker fixtures are shared within one worker/project. Test fixtures rebuild on
retries. Unexpected failures retire logical worker fixture and suite state before
reuse. Dependencies must be registered and worker fixtures cannot depend on test
fixtures. `.teardown(...)` cleans dependents first, including after partial setup.

Fixture setup and teardown limits are optional and separate. Without a local
limit, setup uses its enclosing hook/test budget and teardown uses the shared
cleanup budget. Explicit limits can shorten those clocks; they cannot extend
them. Zero disables only the selected limit, so a finite outer budget still
applies. Playwright's separate fixture accounting differs; Ferrite does not
pause the test clock during an explicit fixture setup.

An attempt shares cleanup time across afterEach hooks, reverse fixture teardown,
artifacts and native close, including worker retirement after an unexpected
failure. Suite completion/worker retirement and final run cleanup each use
their own enclosing clock. After exhaustion, immediately ready cleanup still
gets one poll; pending operations receive individual timeout errors. A dropped
page/context close wait leaves one disposal owner running, and a repeated close
waits for that same work. Firefox lifecycle serialization stays held until native
context disposal settles. Synchronous Rust hooks and filesystem writes remain
cooperative: blocking code cannot be preempted by an async deadline.

Nest suites by passing an inner `Suite::tests(...)` result into the outer suite.
Timeouts, retries, context options and tags inherit, with descendant overrides
winning. `beforeAll`/`afterAll` run once per participating worker/project;
`afterAll` runs after the remaining descendants. Before/after-each order follows
outer-to-inner/inner-to-outer nesting. Skipped and filtered suites do not run hooks.

Fixtures can also depend on `Page`, `BrowserContext`, `ApiClient` and `TestInfo`.
Worker fixtures can depend on `Browser` and `WorkerInfo`; test-scoped dependencies
are rejected for worker fixtures and suite-wide hooks.

```rust
let tests = Suite::new("authenticated")
    .before_each_with_context(ContextHook::new(|ctx| async move {
        ctx.info.set_timeout(std::time::Duration::from_secs(60));
        ctx.info.annotate("setup", "authentication");
        ctx.page.goto("/login").await
    }))
    .tests(vec![test_with_context("account", |ctx| async move {
        if !cfg!(target_os = "linux") {
            ctx.info.skip("Linux-only fixture")?;
        }
        ctx.info.slow("large account");
        ctx.page.step("open account", ctx.page.goto("/account")).await?;
        Ok(())
    })]);
```

Use `.fixture::<Profile>()` on `ContextHook` to request lazy fixtures before the
hook. `Suite::before_all_with_context` and `after_all_with_context` take a
`WorkerHook` with worker-only fixture requests. `ctx.context` is the fresh browser
context, and `ctx.request` is an isolated HTTP client; `ctx.context.request()`
shares browser cookies. Runtime `fail`, `skip`, `slow`, timeout and annotations
are shared across metadata clones. Use `effective_timeout()` to read live changes.

Implement `Reporter` and register it with `Runner::custom_reporter` to receive
live run, attempt, step, attachment and error callbacks. Callbacks are synchronous
and may run on different workers concurrently; queue slow uploads separately.
Retries have distinct `AttemptInfo.retry` values. Cleanup completes before attempt
end, and cancelled steps emit an interrupted end event. File reporters continue
to serialize the final aggregate report.

Use `ctx.info.status()`, `expected_status()` and `errors()` in cleanup hooks or
fixture teardown. Status is `None` during setup/body and is published before
`after_each`; later cleanup failures update shared metadata. Expected failures
retain a raw `Failed` status with `Failed` expected status, while an unexpected
pass is raw `Passed` and fails the aggregate result.

```rust,ignore
ctx.page.step_result("sign in", async {
    ctx.page.step_result("fill email", async {
        ctx.page.get_by_label("Email").fill("user@example.com").await
    }).await?;
    ctx.info.attach("account", b"signed in", "text/plain")?;
    Ok(())
}).await?;
```

`step_result` captures returned errors; `step` preserves arbitrary outputs and
records panics/interruption but cannot inspect Rust `Err` values. Awaited nested
steps retain source positions, timing, children and attachments. Concurrent
branches keep their parent scopes; detached Tokio tasks start root steps and
must be joined before the body returns to retain their completed diagnostics.

JSON and HTML include every executed attempt in `result.attempt_results`, with
its own steps, errors, annotations and artifacts. `result.flaky` and
`report.flaky()` identify successful retries after an unexpected failure. Handled
step errors stay in the step tree without making a successful test flaky. Existing
aggregate result fields remain available; older JSON without attempt history
still deserializes. Manual Rust result literals need the new history/step fields; `TestReport`
literals also need `run_steps` (empty when unused). `on_test_end` includes only the current attempt; the final report
includes the whole retry history.

Network events now distinguish response headers, request completion and transport
failure. `PageEvent::RequestFinished(NetworkRequest)` supplies request ID, method
and URL; `RequestFailed` additionally supplies error text and a backend cancellation
flag. HTTP 4xx/5xx responses finish normally. Context subscriptions forward both
kinds with the source page ID. `Request` and `Response` also include `request_id`;
use `..` in patterns when you only need URL/status. Firefox BiDi does not supply an
explicit cancellation flag (`cancelled` is `None`). Subscribe before triggering requests.

Controlled steps add local deadlines, skip reasons and live metadata:

```rust,ignore
use ferrite_e2e::{StepOptions, StepOutcome};
use std::time::Duration;

let outcome = ctx.page.step_with(
    "optional details",
    StepOptions::default().timeout(Duration::from_secs(2)).annotate("issue", "123"),
    |step| async move {
        if !show_details {
            step.skip("details are disabled")?;
        }
        ctx.page.get_by_role("button", "Details").click().await?;
        Ok(())
    },
).await?;
if let StepOutcome::Skipped(reason) = outcome {
    println!("{reason}");
}
```

Use `StepOptions::skip(reason)` to avoid even constructing the closure. Skipping
only affects that step; local timeout errors may be handled while the test keeps
running. Zero adds no local deadline; test cancellation and its enclosing timeout
still apply. `StepContext::annotations()` and `title_path()` read live metadata.

Navigation/set-content, Locator operations and assertions record automatic steps,
as do hooks and typed fixture setup/teardown. Internal action calls and assertion
polling are suppressed. JSON/HTML retain categories, statuses and annotations;
run-wide cleanup appears in `report.run_steps`. Direct Page input and raw protocol
calls are outside this automatic coverage.

`report.write_bundle("report-folder")?` copies screenshots, videos, traces and
attachments into `artifacts/` and writes HTML, JSON and JUnit with relative links.
Move or upload the entire folder; links remain usable after source files are
removed. Selecting the `html` reporter performs this export automatically.
JSON/JUnit selected with HTML share its relative paths; JSON-only/JUnit-only
exports retain their prior source paths. Missing artifact files fail export.

URL/network matching accepts `UrlMatcher::exact`, `glob` and `regex`; legacy
string waits keep substring matching. Exact relative URLs resolve against base URL.

Reuse `UrlMatcher` for assertions, routing and HAR selection:

```rust,ignore
use ferrite_e2e::UrlMatcher;

let api = UrlMatcher::glob("/api/**/item")?;
ctx.page.route_matching(&api, |_| async {
    Ok(ferrite_e2e::RouteAction::fulfill(200, "fixture", "text/plain"))
}).await?;
ctx.page.unroute_matching(&api).await?;
ctx.page.expect().url_matching(&UrlMatcher::exact("/account")).await?;
ctx.context.route_from_har("fixture.har",
    ferrite_e2e::RouteFromHarOptions::default().matching(api),
).await?;
```

Relative glob paths resolve against base URL. Existing string routing/HAR filters
keep their globset semantics. `RouteRule::matching` opts a declarative rule into
the shared matcher; context matching handlers also apply to future pages. Manual
rule/handler literals need the new `matcher: None` field for legacy behavior;
HAR option literals need `url_matcher: None` or `..Default::default()`.

URL readiness options use one navigation budget for matching and loading:

```rust,ignore
use ferrite_e2e::{LoadState, UrlMatcher, UrlWaitOptions};

ctx.page.wait_for_url_matching_with_options(
    &UrlMatcher::exact("/account"),
    UrlWaitOptions::default().wait_until(LoadState::DomContentLoaded)
        .timeout(std::time::Duration::from_secs(5)),
).await?;
```

The same options are available for URL predicates and Frame waits. Defaults use
Load and the navigation timeout; legacy duration-based helpers wait only for the
URL. Zero disables the local timeout while cancellation/enclosing budgets remain
active. NetworkIdle tracks page HTTP quiet for 500ms after Load, excludes complete
worker/socket connectivity and is unsupported for frame-scoped waits.

```rust,ignore
let wait = ctx.page.wait_for_response_where(
    |r| r.url.ends_with("/api/save") && r.method == "POST" && r.status == 200,
    std::time::Duration::from_secs(5),
);
let click = ctx.page.get_by_role("button", "Save");
let (response, clicked) = tokio::join!(wait, click.click());
response?;
clicked?;
```

`wait_for_request_async` / `wait_for_response_async` support async predicates
returning `E2eResult<bool>`. Request waits resolve at start and response waits at
headers, including in-flight requests. Returned records are metadata snapshots;
body capture remains separate. Poll waits before triggering traffic. Timeout,
cancellation and disposal also bound pending predicates; lag is an explicit error.

Typed companions return live per-hop request/response observations. Receiving
headers and finishing a streamed body are separate operations:

```rust,ignore
use ferrite_e2e::{OperationOptions, UrlMatcher};

let save = ctx.page.get_by_role("button", "Save");
let (response, clicked) = tokio::join!(
    ctx.page.wait_for_response_handle(
        &UrlMatcher::exact("/api/save"), OperationOptions::default(),
    ),
    save.click(),
);
clicked?;
let response = response?;
response.finished().await?;
assert!(response.ok());
let request = response.request();
println!("{} {}", request.method(), request.url());
let cookies = response.header_values("set-cookie");
let snapshot = request.snapshot();
```

`subscribe_network()` yields typed Request/Response/Finished/Failed events;
`network_requests()` returns bounded recent metadata without enabling body
capture. `finished()` uses the owning Page action timeout; its options companion
supports zero timeout and caller cancellation. HTTP error statuses complete
successfully, while native transport failures return `E2eError::Network`.
Firefox may omit POST text/resource type or fold duplicate headers; inspect
snapshot completeness/truncation flags. Body helpers remain separate. API
responses also provide `headers_array`, `header_values` and `header_value`.
`ApiResponse::header` keeps its legacy first-value behavior. To forward a fetched
response, pass `response.headers().to_vec()` to `RouteAction::fulfill_full` with
its status/body; the pair array retains repeated names. Typed observations keep
acknowledged route-supplied pairs when the backend omits/folds them, identified
by `snapshot.response_headers_from_route`; raw completeness remains separate.
`finished()` settles the native completion and associated fulfillment reply.
Firefox may omit response/completion events for a synthetic redirect's earlier
hop: its request settles unavailable and `response()` remains None; the final
response and per-hop identity stay separate.

Pointer action options provide positions, modifiers, trial readiness and scoped
timeouts without changing Page defaults:

```rust,ignore
use ferrite_e2e::{ActionOptions, ClickOptions, DragOptions, KeyboardModifier};

let save = ctx.page.get_by_role("button", "Save");
save.click_with_options(ClickOptions::default().trial(true)).await?;
save.click_with_options(
    ClickOptions::default().position(8.0, 12.0)
        .modifiers(&[KeyboardModifier::Shift])
        .timeout(std::time::Duration::from_secs(2)),
).await?;
ctx.page.locator("#enabled")
    .check_with_options(ActionOptions::default()).await?;
ctx.page.locator("#source").drag_to_with_options(
    &ctx.page.locator("#target"), DragOptions::default().steps(12),
).await?;
```

Positions use CSS pixels from the padding-box top-left. Trial may scroll but sends
no input. Acquired modifiers/buttons are released on failure or cancellation;
previously held keys are preserved. `ClickOptions` struct literals need
`..ClickOptions::default()` for the additional fields. Same-origin frame offsets
and positive axis scaling are supported; see the audit for transform limitations.

Generated uploads do not require disk files:

```rust,ignore
ctx.page.locator("input[type=file]").set_input_file_payloads(&[
    ferrite_e2e::FilePayload::new("report.txt", "text/plain", b"generated content"),
]).await?;
```

Empty lists clear inputs. Multiple files require a `multiple` input; the total
64 MiB cap applies to both payloads and existing path uploads. Directory uploads
and native file chooser interception remain outside this API.

Console/error messages include optional source URL/zero-based line/column,
epoch-ms timestamp and owning page ID. `ctx.context.console_messages()` also
retains closed-page/popup output, independently of Page buffers. Attempt JSON,
HTML, trace and `on_test_end` preserve this history across cleanup and retries.

Popup startup observations are captured at transport ingress before asynchronous
page adoption. This also retains diagnostics for a popup that closes before a
usable Page exists:

```rust,ignore
ctx.page.evaluate("window.open('/popup'); true").await?;
// Read after the expected event/request has arrived; this accessor does not wait.
let history = ctx.context.popup_diagnostics();
for popup in &history.entries {
    println!("{} from {}: {:?}, {} console messages, {} requests",
        popup.page_id, popup.opener_id, popup.adoption,
        popup.console.len(), popup.requests.len());
    if popup.truncated {
        eprintln!("startup capture truncated: {:?}", popup.error);
    }
}
ctx.context.clear_popup_diagnostics();
```

Each attempt's `popup_diagnostics` and trace JSON retain the same owned history;
portable HTML lists popup ownership, adoption/closure and native requests. Normal
context console history retains startup logs exactly once. Clearing popup history
does not clear console history or stop live observation. History holds 64 entries
per context, with `dropped_popups` counting evictions; pending ingress captures are
bounded to 64 across the connection. Each projection permits 1,024 native events
and 2 MiB before it reports truncation. It covers all pre-adoption observations
and the initial document, then freezes after adoption and a later document commit.
Existing captured requests can still acquire completion after that point.

`closed` records actual native destruction. Setup failure and context disposal
can stop capture without proving native closure; inspect `adoption`, `error` and
request completion for those cases. Pending-slot eviction reports observation
loss independently of eventual adoption success. No Page/context/remote handles
are retained in returned snapshots. Exhaustive `AttemptResult` literals need
`popup_diagnostics: Default::default()`; older JSON without that field still loads.

`ConsoleMessage::arguments` contains owned native previews with explicit value
states, and `error` contains optional native page-error metadata:

```rust,ignore
use ferrite_e2e::ConsoleArgumentValue;
for message in ctx.context.console_messages() {
    if let Some(error) = &message.error {
        println!("name={:?}, constructor={:?}, message={:?}",
            error.name, error.class_name, error.message);
        for frame in &error.frames {
            println!("{:?} {:?}:{:?}:{:?}",
                frame.function_name, frame.url, frame.line, frame.column);
        }
    }
    if let Some(arguments) = &message.arguments {
        for argument in &arguments.values {
            match &argument.value {
                ConsoleArgumentValue::Json(value) => println!("JSON: {value}"),
                ConsoleArgumentValue::Preview(value) => println!("Native preview: {value}"),
                ConsoleArgumentValue::Unserializable(value) => println!("Special: {value}"),
                ConsoleArgumentValue::Unavailable => println!("No native by-value data"),
            }
        }
    }
}
```

JSON null has its own tagged value and survives roundtrips. Undefined, nonfinite
numbers, negative zero and bigints retain native special-value spelling. Function/
symbol or object references can be unavailable; object previews are partial native
CDP property lists or BiDi typed values, not live handles or complete JSON objects.
Remote ownership identifiers are removed, with `remote_reference` recording their
presence. Native object keys named `handle` remain intact when they are ordinary
by-value JSON data.

Argument previews retain at most 64 arguments and 32 KiB per message. Strings
are capped at 4 KiB, containers at 32 entries and traversal at 256 nodes. JSON
depth is capped at six; native encoding permits 24 levels to accommodate BiDi's
typed mapping wrappers. Truncation and omitted-argument counts are explicit. Error metadata has
an independent 32 KiB bound and at most 64 frames/eight supplied async stack
segments. Missing name/message fields stay None: Firefox currently supplies
uncaught-error text/frames rather than separate name/message fields. Chrome's
constructor class is not substituted for the mutable Error.name.

Page/context events, popup history, per-attempt JSON/HTML and console trace entries
retain the same structured data after closure and retries. The existing `text`
field keeps its rendering contract. Exhaustive `ConsoleMessage` literals need
`arguments: None, error: None`, and `TraceEntry` literals need `console: None`;
older console JSON omitting the new fields loads with None.

Typed synthetic events and richer assertions are available through explicit
options APIs:

```rust
use ferrite_e2e::{DispatchEventOptions, DomEventKind, TextAssertionOptions, TextMatcher};

ctx.page.locator("input[name=search]").dispatch_event_with("input",
    DispatchEventOptions::default().kind(DomEventKind::Input)
        .init(serde_json::json!({"data":"rust", "inputType":"insertText"})),
).await?;
ctx.page.locator(".status").expect().text_with(
    &TextMatcher::exact("ready"),
    TextAssertionOptions::default().use_inner_text(true).ignore_case(true),
).await?;
ctx.page.locator(".results li").expect().contains_texts_with(
    &[TextMatcher::exact("First"), TextMatcher::regex("Last.*")?],
    TextAssertionOptions::default(),
).await?;
ctx.page.locator(".card").expect().contains_class_tokens(&["selected", "ready"]).await?;
ctx.page.locator(".card").expect().in_viewport_with(0.5).await?;
```

Exact class assertions preserve order; token containment ignores order. Exact
strings normalize text whitespace, while regexes read raw values. Default viewport
assertions use native intersection and account for clipping ancestors. Typed
events are untrusted; use the native action APIs when trusted input matters.

See the [engine table](../../E2E-ENGINE-CAPABILITIES.md) and
[pinned conformance corpus](../../scripts/e2e-conformance/README.md) for native
verification and remaining limits.

Async callbacks can be registered per page or per context. Context registration
also covers future pages and popup startup scripts:

```rust,no_run
context.expose_function_async("double", |args| async move {
    Ok(serde_json::json!(args[0].as_i64().unwrap_or_default() * 2))
}).await?;
context.expose_binding("caller", |source, _args| async move {
    Ok(serde_json::json!({
        "page": source.page.target_id(),
        "frame": source.frame.id(),
        "url": source.frame.current_url().await?,
    }))
}).await?;
let result = page.evaluate_value("double(21)").await?;
context.remove_exposed_function("double").await?;
```

Bindings cover main/same-origin frames. JSON callbacks are polled and bounded;
errors/panics reject, navigation/closure cancels pending work and removal owns the
native preload. Duplicate names fail instead of replacing a live callback.

Clear matching native cookies without disturbing other names/paths/domains:

```rust,no_run
context.clear_cookies_with(
    CookieFilter::default()
        .name(TextMatcher::regex("^session_")?)
        .domain("example.test")
        .path("/admin"),
).await?;
```

Filters are ANDed. Linked API clients observe the deletion on their next request.
An empty filter clears all, retaining the existing clear-cookies behavior.

Wait for application state and retain the successful result:

```rust,no_run
use ferrite_e2e::{FunctionPolling, FunctionWaitOptions};
let state = page.wait_for_function_value(
    "key => window.app?.ready && {value: window.app[key]}",
    &"result",
    FunctionWaitOptions::default()
        .polling(FunctionPolling::Interval(std::time::Duration::from_millis(50))),
).await?;
let handle = page.wait_for_function_handle(
    "() => document.querySelector('.ready')", &(), FunctionWaitOptions::default(),
).await?;
let text: String = handle.evaluate("node => node.textContent").await?;
handle.dispose().await?;
```

The default polling mode follows native animation frames. Frame waits return
JSON values; live frame handles and remote handle arguments are unsupported.
Predicates/promises can fail directly. Cancellation removes the owned poller,
while user-created asynchronous work retains normal JavaScript behavior.

Lookup frame handles using the current native tree:

```rust,no_run
let main = page.main_frame().await?;
if let Some(frame) = page.frame_by_url_matching(&ferrite_e2e::UrlMatcher::glob("**/account")?).await? {
    let current = frame.current_url().await?;
    frame.get_by_role("button", "Save").click().await?;
}
let frame = page.frame_by_url_where(|url| url.contains("/checkout?")).await?;
```

Lookups return the first match, or None. Frame identity survives navigation but
never retargets a replacement iframe. `url()` is its lookup snapshot, while
`current_url()` is live. Firefox frame name metadata is empty.

Read a completed download without loading the whole file into memory:

```rust,no_run
use tokio::io::AsyncReadExt;
let download = page.wait_for_download_file(&download_dir, std::time::Duration::from_secs(15)).await?;
let owner = download.page_id();
let mut stream = download.create_read_stream().await?;
let mut buffer = [0u8;8192];
loop {
    let count = stream.read(&mut buffer).await?;
    if count == 0 { break; }
    // Consume buffer[..count].
}
drop(stream);
download.delete().await?;
```

Stream option timeouts/cancellation cover opening. Wrap the complete reading
future in `CancellationToken::run` for caller cancellation across reads.
Missing-file deletion succeeds; other filesystem errors propagate.

Retrieve the actual browser owner from a context:

```rust,no_run
let context = browser.new_context(ferrite_e2e::ContextOptions::default()).await?;
let owner = context.browser().expect("browser owner is retained");
let another_page = owner.new_page().await?;
context.close().await?;
assert!(owner.is_connected());
owner.close().await?;
assert!(!browser.is_connected());
assert!(another_page.is_closed());
```

Browser clones and retrieved owners share the process, profile, context registry
and default context. Closing one shuts down all; dropping a handle keeps the
browser alive while another owner remains. A context holds a weak reference,
so `context.browser()` returns None after the last owner is dropped. Explicit
close continues cleanup if its future is canceled. Final-owner release performs
bounded native shutdown and process/profile cleanup on the active Tokio runtime.
Persistent profile directories remain user-owned; remote Chromium attachment
disconnects Ferrite without terminating the remote browser.

`is_connected()` and context/page `is_closed()` reflect actual native transport
loss. A disconnected context's Closed wait can return the known terminal state;
an already-pending event wait reports a disconnection error. No native popup
closure fact is inferred from losing a transport.

Migration: `Browser::base_url()` now returns `Option<String>` so shared defaults
can change safely through another handle. Replace
`browser.base_url().map(str::to_string)` with `browser.base_url()`; bind the owned
value before using `as_deref()` when a borrowed string is needed. Setting the base
URL changes future contexts across handles; existing contexts keep their seed.

Describe the final locator to make its actions and assertions recognizable:

```rust,no_run
let checkout = page.get_by_role("button", "Checkout").first().describe("Checkout button");
assert_eq!(checkout.description(), Some("Checkout button"));
checkout.click().await?;
checkout.expect().text("Checkout").await?;
let selector = checkout.selector(); // Raw selector remains available.
let unlabeled = checkout.clear_description();
assert!(unlabeled.description().is_none());
```

`describe` returns a clone without changing the original or DOM resolution.
An empty description clears the label. Cloning and timeout/cancellation decorators
retain it; picks, filters, scoped/combined locators and matching builders clear
it, so describe after the final selector change. Content-frame conversion and
its `owner()` retain the iframe label; frame picks/children start unlabeled.
Rust Display/`to_string()` returns the description or raw selector.

Labels accompany the calling operation in action/assertion errors, live steps,
retry histories and owned trace entries without duplicate steps. Automatic step
locations remain the test definition; explicit user steps keep their caller
location. Timeout/cancellation/locator error variants and machine codes stay
intact. Typed I/O, HTTP or JSON failures from labeled operations use the new
`E2eError::Diagnostic { context, source }` variant, retaining the original typed
cause and code. Update exhaustive enum matches accordingly and inspect `source`
or the standard error chain when needed. Skip-control reasons remain unchanged.

Collect soft mismatches automatically for each runner attempt:

```rust,no_run
let test = ferrite_e2e::test_with_context("account", |ctx| async move {
    let soft = ctx.info.soft_asserts();
    soft.run("account heading", ctx.page.locator("h1").expect().text("Account")).await?;
    soft.run("save button", ctx.page.get_by_role("button", "Save").expect().visible()).await?;
    // Both checks execute. Collected mismatches fail this attempt automatically.
    assert_eq!(ctx.info.soft_failures().len(), ctx.info.errors().len());
    Ok(())
});
```

`run` creates one assertion step with the contextual message and its exact Rust
call site. A mismatch records a failed step and lets execution continue. For an
already awaited result, `check(result)?` or `check_with_message(result, message)?`
records the collection source and current user/fixture step without adding an
extra step. `SoftAsserts::for_attempt(&ctx.info)` returns the same attempt-owned
collector; clones share its failures. Use `?`: only E2eError::Expect mismatches
are softened, while operational timeouts, cancellation, disconnection, invalid
options and skip control propagate unchanged. Local assertion futures still work.

Soft failures immediately appear in TestInfo errors/status, reach cleanup hooks,
remain distinct across retries/workers, and survive in attempt JSON/HTML reports.
Expected failure can cover body mismatches, while setup/cleanup failures remain
unexpected. A later skip cannot erase earlier mismatches. Collection is sealed
after hooks/test fixtures finish, before artifact/context cleanup; a retained
collector cannot write into a completed attempt or another retry. Use the final
report for archived failures; collector snapshots require its runtime to remain.
Standalone `SoftAsserts::new()` still uses `check`/`failures`/`assert_all`; its
`check_with_message` adds contextual text without automatic runner ownership.

Migration: `AttemptResult` has a serde-defaulted `soft_assertions` vector of
`SoftAssertionFailure` records with error, message, step ID and title path.
Update exhaustive Rust AttemptResult literals with `soft_assertions: Vec::new()`.
Old JSON without this field loads as empty; TestError's fields remain unchanged.


Generic polling uses one window across all probes and waits:

```rust,no_run
use ferrite_e2e::{expect_poll_with, expect_to_pass_with, PollingOptions, Timeout};
use std::time::Duration;
let options = PollingOptions::default()
    .timeout(Timeout::secs(3))
    .intervals([Duration::from_millis(20), Duration::from_millis(40)])
    .message("API eventually ready")
    .cancellation(ctx.context.cancellation_token());
let page = ctx.page.clone();
let value = expect_poll_with("counter", &options, move || {
    let page = page.clone();
    async move {
        let value: u32 = page.evaluate("window.counter || 0").await?;
        Ok((value >= 3).then_some(value))
    }
}).await?;
assert!(value >= 3);
let soft = ctx.info.soft_asserts();
let heading = ctx.page.locator("h1").expect();
soft.run("eventual heading", expect_to_pass_with("heading", &options, || {
    heading.text("Ready")
})).await?;
```

The first probe is immediate. Intervals apply after each completed mismatch;
when the sequence ends, its last value repeats. Empty sequences or zero intervals
return Config before invoking the probe; a zero **timeout** remains valid and
removes only the local deadline. All probe time and sleeps count toward one
polling window. A hung future is dropped on expiry/cancellation. Caller/enclosing
runtime limits still apply, including runtime timeout changes. A cancellation
token interrupts both a hung probe and a long sleep; use a context's token to
bind disposal to a generic poll even when its callback does no browser work.

Only None (pending) and typed assertion mismatches retry. Operational errors,
including a Timeout returned by a probe, propagate with their original kind/code
and the helper's context. A timeout of the polling window itself becomes an
Expect failure with the last mismatch (or "no data yet"). Both description and
optional message appear in the single outer step and final error. Probes and
nested assertions do not create duplicate implementation steps.

Inside a retry probe, attempt-owned `soft.check(...)?` and `soft.run(...).await?`
return mismatches for retry without recording failures. Wrap the entire helper
in `soft.run` to collect its final mismatch once. Scope is local to the polled
future; unrelated joined work keeps normal soft collection. Collected final
failures are isolated by test retry as usual. Use `?` to propagate probe results;
intentionally discarded Results cannot drive retries. This differs from pinned
Playwright nested soft checks, which record a first-probe failure immediately.

Screenshot mismatches inside generic probes defer report images. A failed outer
assertion publishes the last screenshot mismatch from its last completed failing
probe into that outer step and attempt. Expected/actual and any unstable previous
image are frozen; nested polls transfer images to their enclosing probe. Successful
results, typed operational/control errors and a later completed pending result
discard retained images. An unfinished probe discards its own images and leaves
the earlier completed candidate available for final outer failure. Only one screenshot mismatch is retained per invocation,
replaced by a later screenshot mismatch. Baseline-adjacent `.actual.png` files
retain their earlier behavior. Publication I/O/encoding errors preserve the final
Expect code and add diagnostic context.

`expect_poll` and `expect_to_pass` keep their original signatures and 50 ms
cadence. Their callbacks now propagate operational errors immediately; use None
or Expect to signal "not yet", rather than Config/transport errors. Options
companions support mutable/borrowed and non-Send probes and values; Runner test
bodies still require Send. No new fields are required in existing option/config
structs. PollingOptions is a separate input type with defaults.

Rust Timeout/PollingOptions defaults remain 5 seconds for either helper.
Playwright's default poll intervals are 100/250/500/1000 ms; default toPass has
zero timeout and ignores configured expect timeout. Pinned empty intervals,
zero intervals, early deadline cutoff, nested soft collection and step emission
also differ. See the [actual reference corpus](../../scripts/e2e-conformance/README.md).


Screenshot options apply to page and locator captures:

```rust,no_run
use ferrite_e2e::{ElementRect, ScreenshotOptions, ScreenshotScale};
let bytes = page.screenshot(ScreenshotOptions {
    full_page: true,
    clip: Some(ElementRect { x: 0.0, y: 100.0, width: 300.0, height: 200.0 }),
    mask: vec![page.locator("[data-private]")],
    mask_color: Some("#202020".into()),
    style: Some(".loading-cursor { visibility: hidden !important }".into()),
    scale: ScreenshotScale::Css,
    ..Default::default()
}).await?;
let tile = page.locator(".card").screenshot_with(ScreenshotOptions {
    hide_caret: true,
    ..Default::default()
}).await?;
```

Page clips use viewport capture coordinates in CSS pixels; with full_page they
use document coordinates. Clips are intersected with the region and enclosed in
whole CSS pixels; an empty/outside region is Config. Finite coordinates and
positive dimensions are required. Locator captures scroll to the element and
translate its box to document coordinates; full_page/clip flags are rejected for
locators. The existing page/locator/screenshot_clip entry points remain usable.
Full-page masks now work and match scrolled document coordinates. Masks must
belong to the same owning page, including its supported same-origin frames.

Device output retains native pixel density. Css output is one output pixel per
CSS pixel: Chromium renders at that scale; Firefox normalizes the native device
raster with Lanczos3 and re-encodes PNG/JPEG. This is not identical rendering or
compression behavior. Capture allocation is limited to 64 million native/output
pixels. Document dimensions can exclude an engine's scrollbar; viewport captures
keep the viewport size. Chromium full-page capture no longer changes or clears
device metrics, preserving viewport, DPR, mobile state and caller overrides.

omit_background is supported on Chromium PNG and fails explicitly on Firefox
BiDi or JPEG. It changes the default canvas background; explicit CSS backgrounds
remain visible. Chromium restores the last acknowledged background override
submitted through Page.call/call_with_timeout. Changes made through a separate
raw CDP connection are outside that tracking. JPEG still uses quality to select
format; valid quality is 1..=100. Default format remains PNG.

Temporary styles reach the main document, reachable same-origin frames and open
shadow roots present at preparation. Closed roots/cross-origin frames are outside
that traversal. Application-owned nodes/IDs are retained. With disable_animations,
finite CSS animations/transitions and Web Animations API objects finish through
their native finish method. Infinite animations are cancelled for capture and played again during
cleanup; zero-playback-rate animations remain untouched. Listeners also settle
CSS animations/transitions started during font waits in prepared roots. Finite
completion events and application execution are not rewound. Native finish/resume
errors remain visible; helpers run in the application realm, and preparation
rejects more than 4,096 distinct handled animations.

Captures serialize per page. The optional timeout bounds queueing, preparation,
native capture and awaiting restoration; zero removes the local limit, while
caller/context cancellation and enclosing runner budgets still apply. Use the
existing with_cancellation scope on Page/Locator. Owned restoration continues
independently after a dropped/canceled/timed-out wait, retains the capture gate
and has a separate five-second bound. A following capture waits for restoration
and reports deferred failures before making changes. take_screenshot_cleanup_errors
also drains those diagnostics; immediate cleanup failures remain visible in the
returned error, preserving an original capture error's kind. Owner disposal
releases the native document, and already-lost transport cannot prove remote
restoration. Synchronous browser/CPU work cannot be preempted by dropping a Rust
wait; Firefox normalization uses bounded worker work with no native page ownership.

Migration for exhaustive ScreenshotOptions literals: add clip: None,
scale: ScreenshotScale::Device, omit_background: false, mask_color: None,
style: None and timeout: None, or use ..Default::default(). Default page captures
now explicitly select the viewport; full_page selects the document. Previously
Chromium captureBeyondViewport without a region could include the document.
See the [native pinned observations](../../scripts/e2e-conformance/README.md)
for CSS-scale, background restoration and validation differences.

### Stable screenshot assertions (B10 in progress)

```rust
use ferrite_e2e::{ScreenshotOptions, SnapshotOptions, SnapshotUpdate};

let options = SnapshotOptions {
    update: Some(SnapshotUpdate::Changed),
    capture: Some(ScreenshotOptions {
        style: Some(".timestamp { visibility: hidden !important }".into()),
        disable_animations: true,
        hide_caret: true,
        scale: ferrite_e2e::ScreenshotScale::Css,
        ..Default::default()
    }),
    ..Default::default()
};
page.locator(".card").expect().screenshot_with("card", &options).await?;
```

Page and locator assertions require two successive captures that agree within
SnapshotOptions tolerances. Existing baselines may become correct later within
the same assertion window. Generation under `missing`, `all` and `changed` also
requires stability. A changing page cannot replace a baseline after expiry.
`changed` creates missing baselines and replaces only values outside tolerance;
matching files remain untouched. This mode also works for text snapshot helpers
and runner/CLI configuration. `missing` retains Ferrite's existing write-and-pass
policy; Playwright writes the baseline and fails the test. Negated screenshot
assertions require an existing valid baseline and never generate or update one.

With `capture: None`, assertions use CSS scale, hidden carets and native animation
finishing/cancellation; a supplied ScreenshotOptions replaces those capture defaults.
`wait_for_fonts` defaults to true and awaits reachable same-origin documents'
`document.fonts.ready` after temporary style preparation, within the same
assertion window. Owned restoration covers the font wait. JPEG is unavailable for
PNG snapshot assertions. A caller-supplied capture timeout may shorten a capture;
the page's action timeout does not create fresh assertion windows. Zero assertion
timeout removes the local limit while caller/owner cancellation and enclosing
test budgets continue to apply. Operational timeout/configuration/cancellation/
disconnection errors retain their type. Assertion expiry uses the last completed
image for `.actual.png`; it does not open another capture window.

Pixel comparison still uses per-channel tolerance and both diff count/ratio
limits, rather than Playwright's perceived-color comparator. PNGs above 64 million
pixels, malformed bytes and nonfinite/out-of-range ratios fail explicitly.
SnapshotOptions gained `capture` and `wait_for_fonts`: migrate exhaustive struct
literals with these fields or `..Default::default()`. This is an input-only type.
Path templates and expected/actual/diff report attachments remain B10 work; the
whole task is still unchecked.

### Run metadata and slow results

Configure these values through `E2eConfig` / `Runner::from_env()` or explicitly
on a library runner:

```rust
use ferrite_e2e::{Runner, SlowTestOptions};

let runner = Runner::default()
    .run_name("checkout smoke")
    .metadata([("build".into(), "local".into())].into_iter().collect())
    .report_slow_tests(Some(SlowTestOptions { threshold_ms: 2_000, max: 5 }));
```

Inside `test_with_context`, `ctx.info.config()` and
`ctx.info.project_config()` expose the frozen run/project metadata. Projects
inherit the whole run map unless `Project::metadata` or project configuration
replaces it; an explicit empty map clears inherited metadata. WorkerInfo provides
owned copies. JSON, list, JUnit and portable HTML carry the same values. JUnit
`ferrite.run.name`, `ferrite.run.metadata`, `ferrite.project.metadata` and
`ferrite.slow_tests` property values use JSON encoding.

Slow summaries are opt-in. Durations must be strictly above `threshold_ms`;
`max` is capped at 1,000 and zero disables the summary. Each scheduled Rust
result remains distinct by result index/project/repetition, including duplicate
names; retries count once in total result duration. This differs from Playwright's
source-file aggregation and unlimited zero setting. Old report JSON remains
readable. Legacy environment overrides are `FERRITE_E2E_RUN_NAME`,
`FERRITE_E2E_METADATA` (JSON object or `null`) and `FERRITE_E2E_SLOW_TESTS`
(JSON options or `null`); explicit builders take precedence.
