# E2E parity completion evidence index

Implementation checkpoint: abandoned Fetch cleanup receipt repair following `6502ff7`. This indexes every requirement in
[E2E-PARITY-TODO.md](E2E-PARITY-TODO.md); its acceptance criteria remain authoritative.
All 51 task IDs are represented. Evidence locations are not completion claims.

Cargo metadata identifies all 51 integration targets. The preceding complete
replay passed 302 units, all 282 integration tests and four doctests: **588 checks**,
with no failures, ignored or filtered cases. Its process exited successfully.
It predates the Fetch cleanup receipt repair and is retained as checkpoint evidence.

The new final-source complete build and replay are **pending**. Current scoped
evidence is 303 unit tests, 30 mandatory Chromium/Firefox native groups across
six targets and four doctests. E2E/CLI/configuration strict Clippy and formatting
passed. The separately recorded CLI/configuration gate passed 30 tests and one
doctest; all 32 recorded reference JSON files identify Playwright 1.63.0.
The expected new E2E inventory is 589 checks (303 units, 282 integrations, four
doctests), pending reconciliation and terminal verification. This expected total
is not a report of the pending replay's result.

## Requirement evidence locations

Regression links lead to actual assertions and their native capability policy.
Existence or compilation alone does not prove passing behavior. The final replay
must execute every current integration target, including those outside this
feature index that protect earlier behavior.

| Task | Required deliverable | Implementation or artifact | Regression or generated evidence |
|---|---|---|---|
| G01 | Reconcile the matrix with current behavior (S). | [build_matrix.py](scripts/playwright-parity/build_matrix.py) | [PLAYWRIGHT-API-MATRIX.md](PLAYWRIGHT-API-MATRIX.md) |
| G02 | Maintain an explicit engine capability table (S). | [E2E-ENGINE-CAPABILITIES.md](E2E-ENGINE-CAPABILITIES.md) | [core_conformance.rs](crates/ferrite-e2e/tests/core_conformance.rs) |
| G03 | Add a focused conformance corpus (M). | [README.md](scripts/e2e-conformance/README.md) | [core_conformance.rs](crates/ferrite-e2e/tests/core_conformance.rs) |
| G04 | Apply lifecycle checks to every new API (M). | [operation.rs](crates/ferrite-e2e/src/operation.rs) | [fixture_budgets.rs](crates/ferrite-e2e/tests/fixture_budgets.rs), [route_lifecycle.rs](crates/ferrite-e2e/tests/route_lifecycle.rs), [browser_ownership.rs](crates/ferrite-e2e/tests/browser_ownership.rs) |
| A01 | Typed DOM event dispatch (M). | [event.rs](crates/ferrite-e2e/src/event.rs) | [core_conformance.rs](crates/ferrite-e2e/tests/core_conformance.rs) |
| A02 | Text assertion options (M). | [assertion_options.rs](crates/ferrite-e2e/src/assertion_options.rs) | [core_conformance.rs](crates/ferrite-e2e/tests/core_conformance.rs) |
| A03 | Mixed list, class and value matchers (M). | [assertion_options.rs](crates/ferrite-e2e/src/assertion_options.rs) | [core_conformance.rs](crates/ferrite-e2e/tests/core_conformance.rs) |
| A04 | State assertion options (S). | [assertion_options.rs](crates/ferrite-e2e/src/assertion_options.rs) | [core_conformance.rs](crates/ferrite-e2e/tests/core_conformance.rs) |
| A05 | Viewport intersection ratios (M). | [locator.rs](crates/ferrite-e2e/src/locator.rs) | [core_conformance.rs](crates/ferrite-e2e/tests/core_conformance.rs) |
| A06 | Accessible assertion regex/options (S). | [assertion_options.rs](crates/ferrite-e2e/src/assertion_options.rs) | [core_conformance.rs](crates/ferrite-e2e/tests/core_conformance.rs) |
| A07 | Async Page callbacks (M). | [callbacks.rs](crates/ferrite-e2e/src/callbacks.rs) | [callback_lifecycle.rs](crates/ferrite-e2e/tests/callback_lifecycle.rs) |
| A08 | Context-wide exposed functions (L; needs A07). | [context.rs](crates/ferrite-e2e/src/context.rs) | [callback_lifecycle.rs](crates/ferrite-e2e/tests/callback_lifecycle.rs) |
| A09 | Binding caller metadata (M; needs A07/A08). | [callbacks.rs](crates/ferrite-e2e/src/callbacks.rs) | [callback_lifecycle.rs](crates/ferrite-e2e/tests/callback_lifecycle.rs) |
| A10 | Callback registration/removal lifecycle (M; needs A07–A09). | [callbacks.rs](crates/ferrite-e2e/src/callbacks.rs) | [callback_lifecycle.rs](crates/ferrite-e2e/tests/callback_lifecycle.rs) |
| A11 | Filtered cookie clearing (S). | [cookie_filter.rs](crates/ferrite-e2e/src/cookie_filter.rs) | [daily_api.rs](crates/ferrite-e2e/tests/daily_api.rs) |
| A12 | Consistent action options (M). | [action_options.rs](crates/ferrite-e2e/src/action_options.rs) | [action_options.rs](crates/ferrite-e2e/tests/action_options.rs) |
| A13 | URL wait readiness options (M). | [url_wait.rs](crates/ferrite-e2e/src/url_wait.rs) | [url_readiness.rs](crates/ferrite-e2e/tests/url_readiness.rs) |
| A14 | Shared URL matching across APIs (M). | [url_matcher.rs](crates/ferrite-e2e/src/url_matcher.rs) | [shared_url_matching.rs](crates/ferrite-e2e/tests/shared_url_matching.rs) |
| A15 | Function-wait arguments, polling and results (M). | [function_wait.rs](crates/ferrite-e2e/src/function_wait.rs) | [function_wait.rs](crates/ferrite-e2e/tests/function_wait.rs) |
| A16 | Frame lookup conveniences (S). | [page.rs](crates/ferrite-e2e/src/page.rs) | [frame_lookup.rs](crates/ferrite-e2e/tests/frame_lookup.rs) |
| B01 | Missing frame/load/dialog lifecycle events (M). | [lifecycle_events.rs](crates/ferrite-e2e/src/lifecycle_events.rs) | [lifecycle_events.rs](crates/ferrite-e2e/tests/lifecycle_events.rs) |
| B02 | Rich request/response metadata wrappers (L). | [network.rs](crates/ferrite-e2e/src/network.rs) | [network_metadata.rs](crates/ferrite-e2e/tests/network_metadata.rs) |
| B03 | Response completion helpers (M; needs B02). | [network.rs](crates/ferrite-e2e/src/network.rs) | [network_metadata.rs](crates/ferrite-e2e/tests/network_metadata.rs) |
| B04 | Earliest popup diagnostics (M). | [popup_capture.rs](crates/ferrite-e2e/src/popup_capture.rs) | [popup_diagnostics.rs](crates/ferrite-e2e/tests/popup_diagnostics.rs) |
| B05 | Route removal and in-flight handler behavior (L). | [routing.rs](crates/ferrite-e2e/src/routing.rs) | [route_lifecycle.rs](crates/ferrite-e2e/tests/route_lifecycle.rs) |
| B06 | Route fetch/fulfill option fidelity (M). | [route_options.rs](crates/ferrite-e2e/src/route_options.rs) | [route_options.rs](crates/ferrite-e2e/tests/route_options.rs) |
| B07 | Header convenience APIs with duplicate preservation (S). | [network.rs](crates/ferrite-e2e/src/network.rs) | [header_forwarding.rs](crates/ferrite-e2e/tests/header_forwarding.rs) |
| B08 | API request option/redirect fidelity (M). | [api.rs](crates/ferrite-e2e/src/api.rs) | [api_fidelity.rs](crates/ferrite-e2e/tests/api_fidelity.rs) |
| B09 | Screenshot capture options (M). | [screenshot.rs](crates/ferrite-e2e/src/screenshot.rs) | [screenshot_options.rs](crates/ferrite-e2e/tests/screenshot_options.rs), [screenshot_capabilities.rs](crates/ferrite-e2e/tests/screenshot_capabilities.rs) |
| B10 | Stable screenshot assertions and snapshot paths (M; needs B09). | [snapshot.rs](crates/ferrite-e2e/src/snapshot.rs) | [snapshot_stability.rs](crates/ferrite-e2e/tests/snapshot_stability.rs), [snapshot_paths.rs](crates/ferrite-e2e/tests/snapshot_paths.rs), [snapshot_fonts_and_animations.rs](crates/ferrite-e2e/tests/snapshot_fonts_and_animations.rs), [snapshot_artifacts.rs](crates/ferrite-e2e/tests/snapshot_artifacts.rs) |
| B11 | Report search, filtering and network diagnostics (M; needs B02/B03). | [report.rs](crates/ferrite-e2e/src/report.rs) | [report_diagnostics.rs](crates/ferrite-e2e/tests/report_diagnostics.rs) |
| B12 | Runner-integrated soft assertions (M). | [expect.rs](crates/ferrite-e2e/src/expect.rs) | [soft_assertions.rs](crates/ferrite-e2e/tests/soft_assertions.rs) |
| B13 | Fixture budgets and shared teardown accounting (L). | [runner.rs](crates/ferrite-e2e/src/runner.rs) | [fixture_budgets.rs](crates/ferrite-e2e/tests/fixture_budgets.rs) |
| B14 | Effective project/configuration metadata (M). | [resolved_config.rs](crates/ferrite-e2e/src/resolved_config.rs) | [effective_configuration.rs](crates/ferrite-e2e/tests/effective_configuration.rs) |
| B15 | Completed download streaming and ownership (S). | [page.rs](crates/ferrite-e2e/src/page.rs) | [download_stream.rs](crates/ferrite-e2e/tests/download_stream.rs) |
| B16 | Context/browser ownership introspection (M). | [browser.rs](crates/ferrite-e2e/src/browser.rs) | [browser_ownership.rs](crates/ferrite-e2e/tests/browser_ownership.rs) |
| B17 | Locator descriptions and diagnostic call sites (S). | [locator.rs](crates/ferrite-e2e/src/locator.rs) | [locator_descriptions.rs](crates/ferrite-e2e/tests/locator_descriptions.rs) |
| B18 | Bounded ARIA snapshot options (M). | [aria_options.rs](crates/ferrite-e2e/src/aria_options.rs) | [aria_options.rs](crates/ferrite-e2e/tests/aria_options.rs) |
| B19 | Structured page errors and console value previews (M). | [console.rs](crates/ferrite-e2e/src/console.rs) | [structured_console.rs](crates/ferrite-e2e/tests/structured_console.rs) |
| C01 | PDF options builder (S, Chromium). | [pdf.rs](crates/ferrite-e2e/src/pdf.rs) | [pdf_options.rs](crates/ferrite-e2e/tests/pdf_options.rs) |
| C02 | Captured response-body convenience methods (M, Chromium; needs B02/B03). | [captured_body.rs](crates/ferrite-e2e/src/captured_body.rs) | [captured_bodies.rs](crates/ferrite-e2e/tests/captured_bodies.rs) |
| C03 | Coverage lifecycle/options (M, Chromium). | [coverage.rs](crates/ferrite-e2e/src/coverage.rs) | [coverage_options.rs](crates/ferrite-e2e/tests/coverage_options.rs) |
| C04 | HAR matching/recording options (M, supported subsets). | [har.rs](crates/ferrite-e2e/src/har.rs) | [har_options.rs](crates/ferrite-e2e/tests/har_options.rs) |
| C05 | Additional emulation options (M, Chromium). | [emulation.rs](crates/ferrite-e2e/src/emulation.rs) | [emulation_options.rs](crates/ferrite-e2e/tests/emulation_options.rs) |
| C06 | Scoped CDP session ownership (M, Chromium). | [cdp_session.rs](crates/ferrite-e2e/src/cdp_session.rs) | [cdp_sessions.rs](crates/ferrite-e2e/tests/cdp_sessions.rs) |
| D01 | CI flaky-test policy and focus protection wiring (M; needs B14). | [runner.rs](crates/ferrite-e2e/src/runner.rs) | [ci_policy.rs](crates/ferrite-e2e/tests/ci_policy.rs) |
| D02 | Assertion polling options (M; needs B12/B13). | [expect.rs](crates/ferrite-e2e/src/expect.rs) | [polling_options.rs](crates/ferrite-e2e/tests/polling_options.rs) |
| D03 | Output retention policies (M; needs B14/B11). | [output_retention.rs](crates/ferrite-e2e/src/output_retention.rs) | [output_retention.rs](crates/ferrite-e2e/tests/output_retention.rs) |
| D04 | Run metadata and slow-test reporting (M; needs B14/B11). | [resolved_config.rs](crates/ferrite-e2e/src/resolved_config.rs) | [run_metadata.rs](crates/ferrite-e2e/tests/run_metadata.rs) |
| D05 | Typed Web Storage enumeration/bulk helpers (S–M, both engines). | [page.rs](crates/ferrite-e2e/src/page.rs) | [web_storage.rs](crates/ferrite-e2e/tests/web_storage.rs) |
| D06 | Chromium WebSocket diagnostic lifecycle (M; capability gated). | [websocket.rs](crates/ferrite-e2e/src/websocket.rs) | [websocket_diagnostics.rs](crates/ferrite-e2e/tests/websocket_diagnostics.rs) |

## Cross-cutting completion gates

- Reconcile Cargo metadata with every integration target and terminal result;
  report failures, filtered cases and native browser absence explicitly.
- Preserve typed control errors and one caller/enclosing deadline. Verify source
  loss, detached requests, cancelled startup, queued pause acknowledgement,
  native page/context absence and retry ownership using the G04 units and native
  regressions identified in the TODO. Failed native cleanup is an error, not
  proof of released resources.
- Compile public examples and exports through all-target Clippy. Validate the
  [example guide](examples/e2e/README.md), [pinned corpus guide](scripts/e2e-conformance/README.md),
  compatibility notes, serde defaults and engine capability table.
- Regenerate the member matrix using the pinned upstream cache; validate local
  evidence links and source anchors, package formatting and the final diff.
- Preserve recorded visual inspection of changed portable HTML reports and
  native PDF/screenshot artifacts. Subsequent layout changes require fresh visual
  inspection; HTML text assertions alone cannot substitute for it.
- Commit verified changes. Check G04 only after the complete requirement audit
  and final-source regression gates pass.

Deferred substantial projects listed in the TODO remain outside this goal.
Completing this backlog does not claim equivalence for all 1,018 upstream members
or add a WebKit backend.
