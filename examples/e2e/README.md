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
cleanup_timeout_ms = 5000
```

The corresponding flags are `--global-timeout`, `--max-failures` and
`--cleanup-timeout` (timeouts are milliseconds). Zero disables each limit.
The runner bounds setup/hooks/fixtures/body together, then gives each cleanup
operation its own budget. Already active tests finish when max failures is
reached; global timeout interrupts them and still runs cleanup.

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

`page.with_cancellation(token)` and `locator.with_cancellation(token)` apply
cancellation to their clones. `with_timeout` overrides their action/protocol
budgets without changing siblings. Frame helpers include `page`, `set_content`,
`current_url` and function/URL/load/selector waits. See
[the parity audit](../../PLAYWRIGHT-PARITY.md) for engine and API limitations.

Define fixtures with explicit Rust dependencies and scopes:

```rust
use ferrite_e2e::*;

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
        .dependency::<Token>(),
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
