# Ferrite E2E engine capabilities

Maintained alongside [the member matrix](PLAYWRIGHT-API-MATRIX.md) and
[the implementation TODO](E2E-PARITY-TODO.md). Supported means the described
operation has a native path; it does not imply all Playwright options or semantics.
Partial means the listed limitation must be retained in API/docs/test evidence.

| Operation | Chromium / CDP | Firefox / stock BiDi | Scope or metadata limit |
|---|---|---|---|
| Launch, fresh/persistent contexts, native input | Supported | Supported | Stock browser ownership; no managed installer/channels |
| Context browser owner and connection state | Supported | Supported | Weak context-to-owner access; retrieved/cloned Browser handles share one process, profile, defaults and registry. Context/page state reflects native disconnects; attached Chromium close leaves the remote process running. |
| Strict locators and same-origin frame locators | Supported | Supported | Cross-origin/OOPIF lazy traversal excluded |
| Locator descriptions and operation diagnostics | Supported | Supported | Label is separate from selector; derived selectors clear it. Actions/assertions retain labels in errors, steps and owned traces; automatic source locations remain test definitions. Opaque failures retain their typed cause via E2eError::Diagnostic. |
| Raw/rendered text, list/class/state assertions | Supported | Supported | Rust regex syntax; accessibility remains a DOM approximation |
| Typed synthetic event dispatch | Supported | Supported | JSON initialization; synthetic events are untrusted; live handle arguments excluded |
| Native intersection ratios | Supported | Supported | Owning-document IntersectionObserver and native clipping |
| URL/request/response predicates | Supported | Supported | Legacy snapshots plus typed handle waits; 256-event channel fails explicitly on lag |
| URL wait document readiness | Supported | Supported | Commit, DOMContentLoaded, Load; page-only observed HTTP idle for 500ms, frame idle unsupported |
| Request/response lifecycle and redirects | Supported | Supported | IDs are scoped to a page; optional native metadata must stay optional |
| Native frame/document/dialog-close events | Partial | Partial | Owned frame/page metadata, main DOM/load readiness and subtree removal; Firefox navigation/history/dialog-close subscriptions are probed separately. Frame names and CDP dialog-frame/type fields remain unavailable. Chromium observes its current target session; OOPIF session adoption is deferred, and native detach reason swap denotes leaving that session. |
| Earliest popup diagnostics | Supported | Supported | Transport-ingress source ownership, startup console/errors/requests and native closure, including failed/immediate adoption; 64 history entries/context, 64 strong pending captures/connection and 1,024 events/2 MiB per projection. Truncation and adoption remain separate; native request/frame data is not fabricated. Owned snapshots retain request completion after adoption/closure. |
| Typed request/response metadata and completion | Supported | Supported | Per-hop identity, page/frame ownership, bounded weak redirect history; completion independent of body capture |
| Native headers and request POST text | Partial | Partial | Chromium extra headers require native hop flags; Firefox can fold repeated fields and omit POST text/resource destination; completeness remains optional |
| API/header lookup and route fulfillment pairs | Supported | Supported | Case-insensitive lookup and duplicate-preserving serialized arrays; acknowledged route headers have an explicit source flag when native events omit/fold them. Firefox 156/157 omits first-hop synthetic redirect response/completion events; that observation settles unavailable. |
| Console/error metadata and attempt history | Supported | Supported | Source/timestamp/page identity, bounded tagged native argument previews and owned stack frames retained through attempts/traces. Unknown fields remain None; Firefox uncaught logs provide no separate error name/message. Chrome constructor class is not Error.name; remote argument handles/IDs are excluded. |
| In-memory/path uploads | Supported | Supported | DOM File/DataTransfer injection, 64 MiB total; no chooser/directories |
| Page/context functions and bindings | Supported | Supported | JSON sync/async callbacks; main/same-origin frames; startup preload and named removal; cross-origin/handles excluded |
| Page clocks | Partial | Partial | Document-local virtual-time semantics documented in parity audit |
| Cookies, API cookie sharing, localStorage state | Supported | Supported | IndexedDB/OPFS and complete partition/SameSite semantics excluded |
| Extra headers and offline after launch | Supported | Unsupported | Firefox calls return an explicit unsupported error |
| Browser HTTP challenge credentials | Supported | Unsupported | API Basic auth is separate and available on both |
| Locale/timezone after launch | Supported | Unsupported | Chromium overrides apply to loaded/future documents as documented |
| Media/device/UA/JS/CSP overrides after launch | Supported | Unsupported | Some Firefox settings can be configured at launch; no runtime emulation parity |
| JS/CSS coverage and explicit GC | Supported | Unsupported | Native V8/CSS ranges; narrower coverage options |
| Screenshots and completed download files | Supported | Supported | Backend rendering differs; Firefox download URL/failure metadata is limited |
| PDF export | Partial | Unsupported | Native Chromium Page.printToPDF; options extension tracked separately |
| Native response-body capture | Partial | Unsupported | Chromium capture is capped/truncated; missing is distinct from an empty body |
| Route request control | Partial | Partial | Supported request interception operations; see route regressions |
| Context-linked route fetch and fulfillment options | Supported | Partial | HTTP(S)/base-relative fetch URL, method/header/raw/JSON overrides, cookies/TLS/proxy/auth, redirect/retry budgets and inherited response/file/JSON fulfillment. Chromium lossless request bytes capped at 16 MiB; text fallback is a preview. Firefox original request bodies remain unavailable and require an explicit override. Request-origin CORS preparation uses the new RouteInfo companion. |
| Page/context route removal policies | Supported | Supported | Default/ignore-errors release active requests while callbacks settle; wait retains decisions; explicit Cancel is a Rust extension. Match limits are reserved atomically across pages, including fallback. |
| Native route response-stage rewriting / intercepted URL override | Supported | Unsupported | Firefox rejects unsupported native overrides explicitly; an independent route fetch to another HTTP(S) URL and prepared fulfillment are supported separately. |
| HAR capture/replay | Partial | Partial | Engine body and response rewriting limits carry through |
| WebSocket observation | Supported | Unsupported | No stock BiDi socket-frame stream; routing/mocking excluded |
| Video / frame streams | Supported | Supported | Chromium video requires ffmpeg; Firefox records natively |
| Browser CDP connections / raw sessions | Supported | Unsupported | Scoped session ownership is a separate extension |
| Attempt-owned soft assertions | Supported | Supported | Only assertion mismatches are softened; operational/control errors propagate. Source/step/message data and failure status survive cleanup/retries. Atomic sealing rejects late writes; Rust runtime modifiers remain attempt-local. |
| Resolved project/run/attempt configuration | Supported | Supported | Library/shared TOML/JSON/CLI selection and precedence; whole context overrides, actual owner versions, project artifact/snapshot paths, read-only TestInfo snapshots and serde-defaulted report metadata. Dedicated versions stay absent before launch; no project dependency scheduling. |
| Fixture limits and shared cleanup clocks | Supported | Supported | Separate local setup/teardown limits capped by enclosing budgets; ready release after exhaustion, explicit pending errors and disposal surviving dropped waits. Rust accounting differs from upstream separate fixture budgets. |
| CI flaky policy and focus protection | Supported | Supported | Opt-in aggregate failure preserves actual attempt outcomes; registered focus checked before filters/shards, including skipped descendants. CI forces existing focus protection. JSON/live/HTML aggregate status and explicit JUnit policy markers; upstream filtering/JUnit differences documented. |
| Portable reports, runner/fixtures/retries | Supported | Supported | Tokio workers, cooperative cancellation, current artifact formats |

## Evidence and validation gates

[ci_policy.rs](crates/ferrite-e2e/tests/ci_policy.rs) verifies flaky policy,
live/serialized reports, retry/repetition/project scheduling, registered focus,
global/user interruption and isolated child exits. Fourteen pinned runner cases
record intentional upstream differences; twelve actual CLI cases ran across
full Chrome 153/Firefox 157 with parsed XML and child exit checks. The current
combined inventory is 386 E2E plus 28 CLI/config checks, with strict Clippy and
package formatting. Native HTML previews were inspected on both engines.

[fixture_budgets.rs](crates/ferrite-e2e/tests/fixture_budgets.rs) verifies six
native groups on full Chrome/Firefox, including actual removed target and user
context IDs after dropped close waits. Six lifecycle/budget unit groups cover
shared/intersected clocks, disposal error replay and owner release, suite/worker
retirement, final run hooks and default/zero/larger fixture caps. Accounting
differences are recorded by eight actual pinned runner observations. The combined
verified B13 checkpoint inventory was 377 E2E plus 27 CLI/config checks, with strict Clippy and
package formatting. An already-lost transport permits idempotent local close;
it does not establish release of a remote native context.


[core_conformance.rs](crates/ferrite-e2e/tests/core_conformance.rs) compares
shared native behavior with the [pinned reference corpus](scripts/e2e-conformance/README.md)
on Chromium and Firefox, and separately probes native media, locale/timezone,
headers, GC and JS coverage with explicit Firefox errors. It reports installed
browser identity and version. Existing
[browser regressions](crates/ferrite-e2e/tests/browser.rs) cover the remaining
context/emulation/routing/coverage/download/screenshot/video restrictions;
[network and context lifecycle](crates/ferrite-e2e/tests/scopes_and_network.rs)
and [wait/upload/console](crates/ferrite-e2e/tests/waits_uploads_and_console.rs)
groups cover metadata and lifecycle semantics.

[browser_ownership.rs](crates/ferrite-e2e/tests/browser_ownership.rs) compares two
pinned Chromium observations on both engines and verifies real owner operations,
shared defaults, failed storage setup cleanup, final-owner release, persistent
profiles, canceled/concurrent shutdown and actual transport loss. Linux checks
the launched process and temporary profile are released. Remote Chromium owner
close leaves its source process running; weak contexts do not retain a browser.

[effective_configuration.rs](crates/ferrite-e2e/tests/effective_configuration.rs)
compares four pinned runner observations and verifies five native groups on both
engines. Actual selected settings, whole-context/suite/test precedence, runtime
timeout changes, retry isolation, dedicated owner identity/release, project
artifact/snapshot directories, old JSON, early setup and portable reports are
covered. A separate child process verifies legacy environment values are fixed
before hooks without changing the parent's environment. All 29 integration
targets and strict E2E/CLI/configuration gates passed for this phase.

[soft_assertions.rs](crates/ferrite-e2e/tests/soft_assertions.rs) compares five
actual pinned runner cases and verifies contextual sources/steps, retained weak
handles, setup/cleanup/fixtures, operational cancellation/timeouts and failed
setup dependency release on both engines. Old report JSON defaults remain valid;
soft mismatch text and paths are escaped in portable reports.

[locator_descriptions.rs](crates/ferrite-e2e/tests/locator_descriptions.rs) compares
three actual pinned description/resolution/error cases on both engines. It also
checks native early validation, typed JSON causes, cancellation and local values/
callbacks, retry/live/source metadata, owned traces, JSON and escaped reports.
The pinned timeout omits the user label; richer Rust errors deliberately differ.

[lifecycle_events.rs](crates/ferrite-e2e/tests/lifecycle_events.rs) compares four
pinned frame/readiness/dialog cases and verifies stable native identity,
child-first subtree detach, replacement/history events, repeated set-content
readiness, optional metadata and single context forwarding. Zero/live-default/
caller/enclosing deadlines, disposal, retry and disconnect waits run on both
engines. A protocol fixture separately verifies that interrupted page setup
releases its listener while the shared browser transport remains open.

[typed network metadata](crates/ferrite-e2e/tests/network_metadata.rs) verifies
concurrent requests, child frames, redirects, JSON/form request text, duplicate
cookies, headers-before-completion, HTTP/transport errors, disposal, disconnect,
zero/caller/enclosing deadlines and runner retries on both engines. Missing
Firefox fields are checked as absent rather than inferred.

[header_forwarding.rs](crates/ferrite-e2e/tests/header_forwarding.rs) verifies
actual API transport, serialization, binary fulfillment, separate cookies with
Expires commas, browser cookie storage and per-hop same-URL synthetic redirects.
Generic native comma folding stays intact; known route-supplied pairs are
retained separately from native completeness signals.

[route_lifecycle.rs](crates/ferrite-e2e/tests/route_lifecycle.rs) checks active-call
removal, independent dispatch, registration churn, default/wait/ignore-errors/
cancel, errors/panics, shared finite hit limits, context/future-page routing,
budgets, retries, disposal and transport loss. Its pinned Chromium reference
records actual Playwright release behavior; native regressions also run on
Firefox. Empty interception is released after pending native stages and calls
settle; Firefox pauses queued before removal are explicitly continued.

[route_options.rs](crates/ferrite-e2e/tests/route_options.rs) compares the pinned
fetch/fulfill reference with native binary responses, inferred/explicit headers,
duplicate cookies and cross-origin CORS results. It also verifies unavailable
Firefox request bytes, context TLS/proxy/credentials, live budgets, cancellation,
disposal, transport loss and runner retries. Firefox's browser certificate
acceptance still needs the launch setting; the context-linked HTTP client uses
its own inherited TLS setting. Neither engine gains automatic compressed-body
decoding through these helpers.

Run the portable gate with both executable paths supplied:

```bash
FERRITE_CHROMIUM_PATH=/path/to/chromium \
FERRITE_FIREFOX_PATH=/path/to/firefox \
TMPDIR=/path/to/writable-temp bash scripts/e2e-conformance/validate.sh
```

The gate fails before testing if either executable is unavailable, prints native
versions, and requires successful launches of both engines in shared regression
groups. Unset require-both mode is still available for local single-engine work;
such a run does not prove two-engine compatibility. Adding an engine-specific
API requires a supported native regression and an explicit unsupported-engine
regression, rather than a silent skip or a successful no-op.
