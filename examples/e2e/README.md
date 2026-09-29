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

Subscribe with `context.subscribe()` before triggering an action. Context events
include new pages/popups, console/errors, network, completed downloads and closure,
with source page IDs. `wait_for_event` provides a timed filter;
`wait_for_event_with_options` additionally accepts a cancellation token. A zero
operation timeout waits until success, cancellation or page/context disposal.

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

Network events now distinguish response headers, request completion and transport
failure. `PageEvent::RequestFinished(NetworkRequest)` supplies request ID, method
and URL; `RequestFailed` additionally supplies error text and a backend cancellation
flag. HTTP 4xx/5xx responses finish normally. Context subscriptions forward both
kinds with the source page ID. `Request` and `Response` also include `request_id`;
use `..` in patterns when you only need URL/status. Firefox BiDi does not supply an
explicit cancellation flag (`cancelled` is `None`). Subscribe before triggering requests.
