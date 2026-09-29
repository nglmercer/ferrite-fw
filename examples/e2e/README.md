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
