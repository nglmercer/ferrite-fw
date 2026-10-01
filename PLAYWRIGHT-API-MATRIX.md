# Playwright API member matrix

Audit date: 2026-09-29. Baseline: [Playwright v1.63.0](https://github.com/microsoft/playwright/releases/tag/v1.63.0). Local target: current working tree of `ferrite-e2e`, after the practical-parity implementation.

Read [PLAYWRIGHT-PARITY.md](PLAYWRIGHT-PARITY.md) for findings, engine limitations, priorities and test evidence.

This inventory covers every JavaScript-applicable method, property and event documented in the pinned upstream browser API, test API, reporter API, Electron API and Android API directories. Language-specific members are excluded; same-name overloads are collapsed within a member kind. Deprecated APIs and experimental APIs remain visible. Inherited members are represented on their declaring class. Arguments/options are reviewed by feature in the companion report, not counted as separate members.

| Status | Meaning |
|---|---|
| Equivalent | A counterpart exists for the basic operation/value; this is not a claim of full class/options/engine parity. |
| Partial | Related exposed operation, manual composition or field exists, with semantic/options/engine differences. |
| Idiomatic | Comparable checks/operations are expressed through Rust language/library facilities; no Playwright-style API object. |
| Missing | No dedicated counterpart found; arbitrary JS evaluation or raw CDP/BiDi calls do not establish feature parity. |

Inventory: **73 classes, 1018 distinct members**. Equivalent: 15; Partial: 645; Idiomatic: 41; Missing: 317. These counts are inventory labels, not a percentage of behavioral compatibility.

## APIRequest

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-apirequest.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `APIRequest.newContext` | method | Partial | `ApiClient.with_options` ([source](crates/ferrite-e2e/src/api.rs#L271)) | Base URL, headers, client TLS/proxy, timeout, manual redirect limits, imported storage state and origin-scoped Basic auth. Legacy preemptive default and explicit-header precedence; Unauthorized selects challenge-only behavior. URL-normalized credential origins differ from raw upstream strings; no full options surface. |

## APIRequestContext

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-apirequestcontext.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `APIRequestContext.delete` | method | Partial | `ApiClient.delete` ([source](crates/ferrite-e2e/src/api.rs#L462)) | HTTP verb available; fetch_with adds payload/header/timeout/status, per-request redirect limits and reset-only retries. Manual redirect methods, replay, hop cookies and cancellation share one budget; client TLS options and legacy preemptive Basic defaults remain narrower. |
| `APIRequestContext.dispose` | method | Partial | `ApiClient.dispose` ([source](crates/ferrite-e2e/src/api.rs#L211)) | Enumerable cookies with domain/path/expiry/HttpOnly/Secure/SameSite state import/export; origin data retained, IndexedDB deferred. Disposal cancels clones; returned Rust response buffers remain independently owned. |
| `APIRequestContext.fetch` | method | Partial | `ApiClient.fetch_with` ([source](crates/ferrite-e2e/src/api.rs#L346)) | Generic method with exclusive JSON/form/multipart/raw payloads, per-request redirect limits, pre-header reset retries, hop cookies and 2xx/3xx status policy. One total deadline/cancellation; replayable owned bodies, client-scoped TLS opt-out, no automatic compression decoding. |
| `APIRequestContext.get` | method | Partial | `ApiClient.get` ([source](crates/ferrite-e2e/src/api.rs#L455)) | HTTP verb available; fetch_with adds payload/header/timeout/status, per-request redirect limits and reset-only retries. Manual redirect methods, replay, hop cookies and cancellation share one budget; client TLS options and legacy preemptive Basic defaults remain narrower. |
| `APIRequestContext.head` | method | Partial | `ApiClient.head` ([source](crates/ferrite-e2e/src/api.rs#L490)) | HTTP verb available; fetch_with adds payload/header/timeout/status, per-request redirect limits and reset-only retries. Manual redirect methods, replay, hop cookies and cancellation share one budget; client TLS options and legacy preemptive Basic defaults remain narrower. |
| `APIRequestContext.patch` | method | Partial | `ApiClient.patch_json` ([source](crates/ferrite-e2e/src/api.rs#L483)) | HTTP verb available; fetch_with adds payload/header/timeout/status, per-request redirect limits and reset-only retries. Manual redirect methods, replay, hop cookies and cancellation share one budget; client TLS options and legacy preemptive Basic defaults remain narrower. |
| `APIRequestContext.post` | method | Partial | `ApiClient.post_json` ([source](crates/ferrite-e2e/src/api.rs#L469)) | HTTP verb available; fetch_with adds payload/header/timeout/status, per-request redirect limits and reset-only retries. Manual redirect methods, replay, hop cookies and cancellation share one budget; client TLS options and legacy preemptive Basic defaults remain narrower. |
| `APIRequestContext.put` | method | Partial | `ApiClient.put_json` ([source](crates/ferrite-e2e/src/api.rs#L476)) | HTTP verb available; fetch_with adds payload/header/timeout/status, per-request redirect limits and reset-only retries. Manual redirect methods, replay, hop cookies and cancellation share one budget; client TLS options and legacy preemptive Basic defaults remain narrower. |
| `APIRequestContext.storageState` | method | Partial | `ApiClient.storage_state` ([source](crates/ferrite-e2e/src/api.rs#L215)) | Enumerable cookies with domain/path/expiry/HttpOnly/Secure/SameSite state import/export; origin data retained, IndexedDB deferred. Disposal cancels clones; returned Rust response buffers remain independently owned. |
| `APIRequestContext.tracing` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## APIResponse

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-apiresponse.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `APIResponse.body` | method | Equivalent | `ApiResponse.bytes` ([source](crates/ferrite-e2e/src/api.rs#L83)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.dispose` | method | Partial | `ApiResponse.dispose` ([source](crates/ferrite-e2e/src/api.rs#L36)) | Final response URL/status text and explicit body-buffer disposal; clones own their buffers. |
| `APIResponse.headers` | method | Partial | `ApiResponse.headers` ([source](crates/ferrite-e2e/src/api.rs#L55)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.headersArray` | method | Partial | `ApiResponse.headers_array` ([source](crates/ferrite-e2e/src/api.rs#L69)) | Duplicate-preserving serialized HttpHeader entries with case-insensitive header_value/header_values companions; native binary route fulfillment and duplicate Set-Cookie/header forwarding verified on both engines. Legacy header() returns the first value. |
| `APIResponse.json` | method | Equivalent | `ApiResponse.json` ([source](crates/ferrite-e2e/src/api.rs#L94)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.ok` | method | Equivalent | `ApiResponse.ok` ([source](crates/ferrite-e2e/src/api.rs#L49)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.securityDetails` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIResponse.serverAddr` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIResponse.status` | method | Equivalent | `ApiResponse.status` ([source](crates/ferrite-e2e/src/api.rs#L43)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.statusText` | method | Partial | `ApiResponse.status_text` ([source](crates/ferrite-e2e/src/api.rs#L31)) | Final response URL/status text and explicit body-buffer disposal; clones own their buffers. |
| `APIResponse.text` | method | Partial | `ApiResponse.text` ([source](crates/ferrite-e2e/src/api.rs#L89)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.timing` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIResponse.url` | method | Partial | `ApiResponse.url` ([source](crates/ferrite-e2e/src/api.rs#L26)) | Final response URL/status text and explicit body-buffer disposal; clones own their buffers. |

## APIResponseAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-apiresponseassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `APIResponseAssertions.not` | property | Idiomatic | `ApiResponse.ok` ([source](crates/ferrite-e2e/src/api.rs#L49)) | Use assert!(response.ok()) or its negation; no dedicated retrying assertion object. |
| `APIResponseAssertions.toBeOK` | method | Idiomatic | `ApiResponse.ok` ([source](crates/ferrite-e2e/src/api.rs#L49)) | Use assert!(response.ok()) or its negation; no dedicated retrying assertion object. |

## Android

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/mobile-api/class-android.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Android.connect` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `Android.devices` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `Android.launchServer` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `Android.setDefaultTimeout` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |

## AndroidDevice

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/mobile-api/class-androiddevice.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `AndroidDevice.close` | event | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.webView` | event | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.close` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.drag` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.fill` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.fling` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.info` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.input` | property | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.installApk` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.launchBrowser` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.longTap` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.model` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.open` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.pinchClose` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.pinchOpen` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.press` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.push` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.screenshot` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.scroll` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.serial` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.setDefaultTimeout` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.shell` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.swipe` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.tap` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.wait` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.waitForEvent` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.webView` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidDevice.webViews` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |

## AndroidInput

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/mobile-api/class-androidinput.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `AndroidInput.drag` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidInput.press` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidInput.swipe` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidInput.tap` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidInput.type` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |

## AndroidSocket

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/mobile-api/class-androidsocket.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `AndroidSocket.close` | event | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidSocket.data` | event | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidSocket.close` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidSocket.write` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |

## AndroidWebView

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/mobile-api/class-androidwebview.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `AndroidWebView.close` | event | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidWebView.page` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidWebView.pid` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `AndroidWebView.pkg` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |

## Browser

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-browser.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Browser.context` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Browser.disconnected` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Browser.browserType` | method | Partial | `Browser.kind` ([source](crates/ferrite-e2e/src/browser.rs#L838)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.close` | method | Partial | `Browser.close` ([source](crates/ferrite-e2e/src/browser.rs#L1369)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.contexts` | method | Partial | `Browser.contexts` ([source](crates/ferrite-e2e/src/browser.rs#L1333)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.isConnected` | method | Partial | `Browser.is_connected` ([source](crates/ferrite-e2e/src/browser.rs#L1185)) | Shared owner shutdown and actual transport reader/writer state. Clones share one process/profile/context registry; closing any handle shuts down all. Remote attachment closes Ferrite without killing the source process. |
| `Browser.newBrowserCDPSession` | method | Partial | `Browser.cdp` ([source](crates/ferrite-e2e/src/browser.rs#L1162)) | Raw shared browser CDP connection; no independently detachable CDPSession. |
| `Browser.newContext` | method | Partial | `Browser.new_context` ([source](crates/ferrite-e2e/src/browser.rs#L1205)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.newPage` | method | Partial | `Browser.new_page` ([source](crates/ferrite-e2e/src/browser.rs#L1352)) | Fresh owning context; closing the page disposes it, including its popups. |
| `Browser.bind` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.removeAllListeners` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.startTracing` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.stopTracing` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.unbind` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.version` | method | Partial | `Browser.version` ([source](crates/ferrite-e2e/src/browser.rs#L1179)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |

## BrowserContext

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-browsercontext.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `BrowserContext.backgroundPage` (deprecated) | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.clock` | property | Partial | `Page.clock_install` ([source](crates/ferrite-e2e/src/page.rs#L3634)) | Page-document-local clock; no context-wide clock object. |
| `BrowserContext.credentials` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.debugger` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.close` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L543)) | ContextEventKind::Closed; context subscriptions forward observations from every current/future page; enum payloads have a narrower live object/options model. |
| `BrowserContext.console` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L543)) | ContextEventKind::Console; context subscriptions forward observations from every current/future page; enum payloads have a narrower live object/options model. |
| `BrowserContext.dialog` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L543)) | ContextEventKind::Dialog forwards the existing page observation with source page ID; payloads and backend metadata remain narrower. |
| `BrowserContext.dialogClosed` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L543)) | ContextEventKind::DialogClosed forwards native page/frame or dialog-close metadata exactly once, with source page identity, including ingress popup observations before adoption. Owned snapshots rather than live upstream objects; optional native fields, bounded popup capture and Firefox subscription limits remain explicit. Chromium OOPIF sessions remain deferred. |
| `BrowserContext.download` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L543)) | ContextEventKind::Download forwards the existing page observation with source page ID; payloads and backend metadata remain narrower. |
| `BrowserContext.frameAttached` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L543)) | ContextEventKind::FrameAttached forwards native page/frame or dialog-close metadata exactly once, with source page identity, including ingress popup observations before adoption. Owned snapshots rather than live upstream objects; optional native fields, bounded popup capture and Firefox subscription limits remain explicit. Chromium OOPIF sessions remain deferred. |
| `BrowserContext.frameDetached` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L543)) | ContextEventKind::FrameDetached forwards native page/frame or dialog-close metadata exactly once, with source page identity, including ingress popup observations before adoption. Owned snapshots rather than live upstream objects; optional native fields, bounded popup capture and Firefox subscription limits remain explicit. Chromium OOPIF sessions remain deferred. |
| `BrowserContext.frameNavigated` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L543)) | ContextEventKind::FrameNavigated forwards native page/frame or dialog-close metadata exactly once, with source page identity, including ingress popup observations before adoption. Owned snapshots rather than live upstream objects; optional native fields, bounded popup capture and Firefox subscription limits remain explicit. Chromium OOPIF sessions remain deferred. |
| `BrowserContext.page` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L543)) | ContextEventKind::Page; context subscriptions forward observations from every current/future page; enum payloads have a narrower live object/options model. |
| `BrowserContext.pageClose` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L543)) | ContextEventKind::PageClose forwards the existing page observation with source page ID; payloads and backend metadata remain narrower. |
| `BrowserContext.pageLoad` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.webError` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.request` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L543)) | ContextEventKind::Request; context subscriptions forward observations from every current/future page; enum payloads have a narrower live object/options model. |
| `BrowserContext.requestFailed` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L543)) | RequestFailed enum events on Chromium/Firefox with request IDs and method/URL; failed requests carry transport error text. Context events include page identity. No rich live Request object graph. |
| `BrowserContext.requestFinished` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L543)) | RequestFinished enum events on Chromium/Firefox with request IDs and method/URL; failed requests carry transport error text. Context events include page identity. No rich live Request object graph. |
| `BrowserContext.response` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L543)) | ContextEventKind::Response; context subscriptions forward observations from every current/future page; enum payloads have a narrower live object/options model. |
| `BrowserContext.serviceWorker` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.addCookies` | method | Partial | `BrowserContext.add_cookies` ([source](crates/ferrite-e2e/src/context.rs#L960)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.addInitScript` | method | Partial | `BrowserContext.add_init_script` ([source](crates/ferrite-e2e/src/context.rs#L1551)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.backgroundPages` (deprecated) | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.browser` | method | Partial | `BrowserContext.browser` ([source](crates/ferrite-e2e/src/context.rs#L488)) | Option<Browser> upgrades the actual weak shared owner for explicit, convenience, default/persistent and attached contexts. Retrieved handles retain the process; None after its last owner drops. No Android/Electron contexts or JavaScript identity semantics. |
| `BrowserContext.clearCookies` | method | Partial | `BrowserContext.clear_cookies_with` ([source](crates/ferrite-e2e/src/context.rs#L939)) | ANDed exact/regex name/domain/path filters on native stores; empty filter clears all. Linked API requests refresh cookies; opaque partition key deletion fails explicitly on Chromium. Rust regex syntax and partition filter fields remain narrower. |
| `BrowserContext.clearPermissions` | method | Partial | `BrowserContext.clear_permissions` ([source](crates/ferrite-e2e/src/context.rs#L1345)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.close` | method | Partial | `BrowserContext.close` ([source](crates/ferrite-e2e/src/context.rs#L1729)) | Once-only background cleanup survives dropped waits and repeated calls await completion. Live disposal errors are aggregated; an already-lost transport permits idempotent local cleanup without native release confirmation. No reason option. |
| `BrowserContext.cookies` | method | Partial | `BrowserContext.cookies` ([source](crates/ferrite-e2e/src/context.rs#L916)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.exposeBinding` | method | Partial | `BrowserContext.expose_binding` ([source](crates/ferrite-e2e/src/callbacks.rs#L560)) | Async JSON binding with owning context/page/native frame identity. Same-origin frame dispatch; native startup preloads and navigation/disposal cleanup. Cross-origin/OOPIF callers and handle arguments deferred. |
| `BrowserContext.exposeFunction` | method | Partial | `BrowserContext.expose_function_async` ([source](crates/ferrite-e2e/src/callbacks.rs#L549)) | Sync/async JSON callbacks in current/future same-origin documents; independent bounded dispatch, native preload ownership, duplicate-name errors and named removal. Rust errors/panics reject JS promises; cross-origin dispatch/handle arguments deferred. |
| `BrowserContext.grantPermissions` | method | Partial | `BrowserContext.grant_permissions` ([source](crates/ferrite-e2e/src/context.rs#L1316)) | No origin argument; Chromium grants broadly, Firefox grants after navigation for the current origin. |
| `BrowserContext.isClosed` | method | Partial | `BrowserContext.is_closed` ([source](crates/ferrite-e2e/src/context.rs#L1648)) | Tracks explicit context disposal, shared browser shutdown, last-owner drop and native transport loss. Enum event waits distinguish disconnect errors from observed native events; no full upstream emitter/reason surface. |
| `BrowserContext.newCDPSession` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.newPage` | method | Partial | `BrowserContext.new_page` ([source](crates/ferrite-e2e/src/context.rs#L643)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.pages` | method | Partial | `BrowserContext.pages` ([source](crates/ferrite-e2e/src/context.rs#L848)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.removeAllListeners` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.request` | property | Partial | `BrowserContext.request` ([source](crates/ferrite-e2e/src/context.rs#L886)) | Context-linked HTTP client sharing cookies and inheriting headers, Basic auth, TLS, proxy and timeout settings at creation. Transport overrides use ApiClientOptions. |
| `BrowserContext.route` | method | Partial | `BrowserContext.route_matching` ([source](crates/ferrite-e2e/src/context.rs#L1042)) | Shared resolved UrlMatcher for rules/handlers/HAR filters, with match limits and identity-based removal; legacy string/globset contracts preserved. Invalid patterns fail before registration, including empty contexts; native response/URL override differences remain. In-flight removal and fuller HAR policies tracked separately. |
| `BrowserContext.routeFromHAR` | method | Partial | `BrowserContext.route_from_har` ([source](crates/ferrite-e2e/src/context.rs#L1277)) | Shared resolved UrlMatcher for rules/handlers/HAR filters, with match limits and identity-based removal; legacy string/globset contracts preserved. Invalid patterns fail before registration, including empty contexts; native response/URL override differences remain. In-flight removal and fuller HAR policies tracked separately. |
| `BrowserContext.routeWebSocket` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.serviceWorkers` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.setDefaultNavigationTimeout` | method | Partial | `BrowserContext.set_default_navigation_timeout` ([source](crates/ferrite-e2e/src/context.rs#L864)) | Shared action/protocol defaults update existing and future pages; zero disables timeout, cancellation is independently supported. |
| `BrowserContext.setDefaultTimeout` | method | Partial | `BrowserContext.set_default_timeout` ([source](crates/ferrite-e2e/src/context.rs#L853)) | Shared action/protocol defaults update existing and future pages; zero disables timeout, cancellation is independently supported. |
| `BrowserContext.setExtraHTTPHeaders` | method | Partial | `BrowserContext.set_extra_http_headers` ([source](crates/ferrite-e2e/src/context.rs#L1461)) | Chromium only; HTTP credentials hold one username/password pair, no origin list. |
| `BrowserContext.setGeolocation` | method | Partial | `BrowserContext.set_geolocation` ([source](crates/ferrite-e2e/src/context.rs#L1336)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.setHTTPCredentials` | method | Partial | `BrowserContext.set_http_credentials` ([source](crates/ferrite-e2e/src/context.rs#L1435)) | Chromium only; HTTP credentials hold one username/password pair, no origin list. |
| `BrowserContext.setOffline` | method | Partial | `BrowserContext.set_offline` ([source](crates/ferrite-e2e/src/context.rs#L1425)) | Chromium only; HTTP credentials hold one username/password pair, no origin list. |
| `BrowserContext.storageState` | method | Partial | `BrowserContext.storage_state` ([source](crates/ferrite-e2e/src/context.rs#L1618)) | Playwright cookies/origins JSON, localStorage from live pages across origins; closed-origin inventory and IndexedDB/OPFS are deferred. |
| `BrowserContext.setStorageState` | method | Partial | `BrowserContext.load_storage_state` ([source](crates/ferrite-e2e/src/context.rs#L1566)) | Playwright multi-origin state and legacy files; cookies restore before navigation and localStorage before app scripts. IndexedDB/OPFS not persisted. |
| `BrowserContext.tracing` | property | Partial | `BrowserContext.start_tracing` ([source](crates/ferrite-e2e/src/context.rs#L1127)) | Custom JSON trace, no Trace Viewer-compatible archive, DOM snapshots or chunks. |
| `BrowserContext.unrouteAll` | method | Partial | `BrowserContext.unroute_all_with` ([source](crates/ferrite-e2e/src/context.rs#L1202)) | Pattern/shared-matcher/all removal with default/wait/ignore-errors and explicit Rust Cancel. Default/ignore-errors release requests while callbacks settle and discard late decisions; wait preserves native decisions within shared deadlines. Independent bounded dispatch, atomic invocation limits including fallback, disposal/retry/disconnect cleanup verified on Chromium/Firefox. Rust first-registration/page priority and no callback-identity removal remain differences. |
| `BrowserContext.unroute` | method | Partial | `BrowserContext.unroute_matching_with` ([source](crates/ferrite-e2e/src/context.rs#L1209)) | Pattern/shared-matcher/all removal with default/wait/ignore-errors and explicit Rust Cancel. Default/ignore-errors release requests while callbacks settle and discard late decisions; wait preserves native decisions within shared deadlines. Independent bounded dispatch, atomic invocation limits including fallback, disposal/retry/disconnect cleanup verified on Chromium/Firefox. Rust first-registration/page priority and no callback-identity removal remain differences. |
| `BrowserContext.waitForEvent` | method | Partial | `BrowserContext.wait_for_event` ([source](crates/ferrite-e2e/src/context.rs#L546)) | Context-wide page/popup, console/error, network, download and close events with source page identity; enum-based filtering, no listener callback API or rich live Request/WebError objects. |

## BrowserServer

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-browserserver.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `BrowserServer.close` | event | Missing | — | No public BrowserServer abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `BrowserServer.close` | method | Missing | — | No public BrowserServer abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `BrowserServer.kill` | method | Missing | — | No public BrowserServer abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `BrowserServer.process` | method | Missing | — | No public BrowserServer abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `BrowserServer.wsEndpoint` | method | Missing | — | No public BrowserServer abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |

## BrowserType

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-browsertype.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `BrowserType.connect` | method | Missing | — | No Playwright-protocol remote connection; Browser.connect is Chromium CDP over a loopback debug port. |
| `BrowserType.connectOverCDP` | method | Partial | `Browser.connect_over_cdp` ([source](crates/ferrite-e2e/src/browser.rs#L787)) | Chromium HTTP or browser WebSocket endpoint; no Playwright remote protocol or headers/options surface. |
| `BrowserType.executablePath` | method | Partial | `find_chromium` ([source](crates/ferrite-e2e/src/browser.rs#L171)) | Stock-browser discovery/launch; launched stdout/stderr are continuously drained with a bounded startup stderr tail. No channels/installer or full Playwright connection options. |
| `BrowserType.launch` | method | Partial | `Browser.launch` ([source](crates/ferrite-e2e/src/browser.rs#L444)) | Stock-browser discovery/launch; launched stdout/stderr are continuously drained with a bounded startup stderr tail. No channels/installer or full Playwright connection options. |
| `BrowserType.launchPersistentContext` | method | Partial | `LaunchOptions.user_data_dir` ([source](crates/ferrite-e2e/src/browser.rs#L140)) | Reusable Chromium/Firefox profile; obtain browser.default_context(). Dedicated contexts remain isolated from persistent storage. |
| `BrowserType.launchServer` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserType.name` | method | Partial | `BrowserKind.name` ([source](crates/ferrite-e2e/src/browser.rs#L48)) | Stock-browser discovery/launch; launched stdout/stderr are continuously drained with a bounded startup stderr tail. No channels/installer or full Playwright connection options. |

## CDPSession

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-cdpsession.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `CDPSession.close` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `CDPSession.event` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `CDPSession.detach` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `CDPSession.send` | method | Partial | `CdpConnection.call` ([source](crates/ferrite-e2e/src/cdp.rs#L182)) | Low-level CDP transport, with caller-managed session IDs; no dedicated CDPSession lifecycle. |

## Clock

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-clock.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Clock.fastForward` | method | Partial | `Page.clock_fast_forward` ([source](crates/ferrite-e2e/src/page.rs#L3702)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.install` | method | Partial | `Page.clock_install_at` ([source](crates/ferrite-e2e/src/page.rs#L3730)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.runFor` | method | Partial | `Page.clock_run_for` ([source](crates/ferrite-e2e/src/page.rs#L3709)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.pauseAt` | method | Partial | `Page.clock_pause_at` ([source](crates/ferrite-e2e/src/page.rs#L3723)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.resume` | method | Partial | `Page.clock_resume` ([source](crates/ferrite-e2e/src/page.rs#L3788)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.setFixedTime` | method | Partial | `Page.clock_set_fixed_time` ([source](crates/ferrite-e2e/src/page.rs#L3666)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.setSystemTime` | method | Partial | `Page.clock_set_system_time` ([source](crates/ferrite-e2e/src/page.rs#L3716)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |

## ConsoleMessage

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-consolemessage.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `ConsoleMessage.args` | method | Partial | `ConsoleMessage.arguments` ([source](crates/ferrite-e2e/src/page.rs#L90)) | Owned native argument previews: tagged JSON/null, special primitives, CDP property previews or BiDi typed values and explicit unavailable/reference/truncation flags. 64 arguments/32 KiB; no live handles or complete object serialization. Retains native bigint spelling where pinned Playwright 1.63 returned an undefined handle. |
| `ConsoleMessage.location` | method | Partial | `ConsoleMessage.location` ([source](crates/ferrite-e2e/src/page.rs#L81)) | Native CDP/BiDi source URL/zero-based position, epoch-ms timestamp and owning page ID when available. Optional unknown metadata; page is an ID rather than a Page object. Bounded tagged native argument previews and structured optional errors retain data through context/popup/attempt/trace history; no live JSHandle argument or worker ownership parity. |
| `ConsoleMessage.page` | method | Partial | `ConsoleMessage.page_id` ([source](crates/ferrite-e2e/src/page.rs#L87)) | Native CDP/BiDi source URL/zero-based position, epoch-ms timestamp and owning page ID when available. Optional unknown metadata; page is an ID rather than a Page object. Bounded tagged native argument previews and structured optional errors retain data through context/popup/attempt/trace history; no live JSHandle argument or worker ownership parity. |
| `ConsoleMessage.text` | method | Partial | `ConsoleMessage.text` ([source](crates/ferrite-e2e/src/page.rs#L79)) | Native CDP/BiDi source URL/zero-based position, epoch-ms timestamp and owning page ID when available. Optional unknown metadata; page is an ID rather than a Page object. Bounded tagged native argument previews and structured optional errors retain data through context/popup/attempt/trace history; no live JSHandle argument or worker ownership parity. |
| `ConsoleMessage.timestamp` | method | Partial | `ConsoleMessage.timestamp_ms` ([source](crates/ferrite-e2e/src/page.rs#L84)) | Native CDP/BiDi source URL/zero-based position, epoch-ms timestamp and owning page ID when available. Optional unknown metadata; page is an ID rather than a Page object. Bounded tagged native argument previews and structured optional errors retain data through context/popup/attempt/trace history; no live JSHandle argument or worker ownership parity. |
| `ConsoleMessage.type` | method | Partial | `ConsoleMessage.kind` ([source](crates/ferrite-e2e/src/page.rs#L77)) | Native CDP/BiDi source URL/zero-based position, epoch-ms timestamp and owning page ID when available. Optional unknown metadata; page is an ID rather than a Page object. Bounded tagged native argument previews and structured optional errors retain data through context/popup/attempt/trace history; no live JSHandle argument or worker ownership parity. |
| `ConsoleMessage.worker` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## Coverage

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-coverage.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Coverage.startCSSCoverage` | method | Partial | `Coverage.start_css_coverage` ([source](crates/ferrite-e2e/src/coverage.rs#L141)) | Chromium only; returns native V8 function/block or CSS rule-use ranges, not flattened Playwright disjoint ranges. Navigation options absent. |
| `Coverage.startJSCoverage` | method | Partial | `Coverage.start_js_coverage` ([source](crates/ferrite-e2e/src/coverage.rs#L78)) | Chromium only; returns native V8 function/block or CSS rule-use ranges, not flattened Playwright disjoint ranges. Navigation options absent. |
| `Coverage.stopCSSCoverage` | method | Partial | `Coverage.stop_css_coverage` ([source](crates/ferrite-e2e/src/coverage.rs#L195)) | Chromium only; returns native V8 function/block or CSS rule-use ranges, not flattened Playwright disjoint ranges. Navigation options absent. |
| `Coverage.stopJSCoverage` | method | Partial | `Coverage.stop_js_coverage` ([source](crates/ferrite-e2e/src/coverage.rs#L101)) | Chromium only; returns native V8 function/block or CSS rule-use ranges, not flattened Playwright disjoint ranges. Navigation options absent. |

## Credentials

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-credentials.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Credentials.install` | method | Missing | — | No public Credentials abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Credentials.create` | method | Missing | — | No public Credentials abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Credentials.delete` | method | Missing | — | No public Credentials abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Credentials.get` | method | Missing | — | No public Credentials abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |

## Debugger

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-debugger.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Debugger.pausedStateChanged` | event | Missing | — | No public Debugger abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Debugger.pausedDetails` | method | Missing | — | No public Debugger abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Debugger.requestPause` | method | Missing | — | No public Debugger abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Debugger.resume` | method | Missing | — | No public Debugger abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Debugger.next` | method | Missing | — | No public Debugger abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Debugger.runTo` | method | Missing | — | No public Debugger abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |

## Dialog

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-dialog.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Dialog.accept` | method | Partial | `DialogDecision.accept_with` ([source](crates/ferrite-e2e/src/page.rs#L1220)) | Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page. |
| `Dialog.defaultValue` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Dialog.dismiss` | method | Partial | `DialogDecision.dismiss` ([source](crates/ferrite-e2e/src/page.rs#L1225)) | Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page. |
| `Dialog.message` | method | Partial | `DialogInfo.message` ([source](crates/ferrite-e2e/src/page.rs#L235)) | Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page. |
| `Dialog.page` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Dialog.type` | method | Partial | `DialogInfo.dialog_type` ([source](crates/ferrite-e2e/src/page.rs#L233)) | Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page. |

## Disposable

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-disposable.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Disposable.dispose` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## Download

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-download.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Download.cancel` | method | Partial | `Page.cancel_downloads` ([source](crates/ferrite-e2e/src/page.rs#L4920)) | Cancels page-tracked downloads on Chromium; no per-Download.cancel method. |
| `Download.createReadStream` | method | Partial | `Download.create_read_stream` ([source](crates/ferrite-e2e/src/page.rs#L1562)) | Tokio AsyncRead/AsyncSeek file for a completed successful download; options bound open only, caller can wrap reads in CancellationToken.run. Active native streams remain unsupported. |
| `Download.delete` | method | Partial | `Download.delete` ([source](crates/ferrite-e2e/src/page.rs#L1618)) | Completed file deletion is idempotent only for NotFound; other filesystem errors propagate. Active downloads are not represented. |
| `Download.failure` | method | Partial | `Download.failure` ([source](crates/ferrite-e2e/src/page.rs#L1514)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.page` | method | Partial | `Download.page_id` ([source](crates/ferrite-e2e/src/page.rs#L1541)) | Owning native page identity without retaining a live Page; None for hand-built completed paths. No upstream live page object. |
| `Download.path` | method | Partial | `Download.path` ([source](crates/ferrite-e2e/src/page.rs#L1506)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.saveAs` | method | Partial | `Download.save_as` ([source](crates/ferrite-e2e/src/page.rs#L1602)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.suggestedFilename` | method | Partial | `Download.suggested_filename` ([source](crates/ferrite-e2e/src/page.rs#L1508)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.url` | method | Partial | `Download.url` ([source](crates/ferrite-e2e/src/page.rs#L1511)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |

## Electron

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/electron-api/class-electron.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Electron.launch` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |

## ElectronApplication

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/electron-api/class-electronapplication.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `ElectronApplication.close` | event | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `ElectronApplication.console` | event | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `ElectronApplication.window` | event | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `ElectronApplication.browserWindow` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `ElectronApplication.close` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `ElectronApplication.context` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `ElectronApplication.evaluate` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `ElectronApplication.evaluateHandle` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `ElectronApplication.firstWindow` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `ElectronApplication.process` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `ElectronApplication.waitForEvent` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |
| `ElectronApplication.windows` | method | Missing | — | Experimental upstream API; Ferrite has no Android/ADB/WebView or Electron backend. |

## ElementHandle

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-elementhandle.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `ElementHandle.boundingBox` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.check` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.click` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.contentFrame` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.dblclick` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.dispatchEvent` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.evalOnSelector` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.evalOnSelectorAll` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.fill` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.focus` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.getAttribute` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.hover` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.innerHTML` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.innerText` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.inputValue` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.isChecked` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.isDisabled` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.isEditable` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.isEnabled` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.isHidden` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.isVisible` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.ownerFrame` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.press` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.querySelector` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.querySelectorAll` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.screenshot` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.scrollIntoViewIfNeeded` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.selectOption` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.selectText` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.setChecked` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.setInputFiles` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.tap` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.textContent` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.type` (deprecated) | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.uncheck` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.waitForElementState` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |
| `ElementHandle.waitForSelector` | method | Missing | — | No ElementHandle abstraction; locator replacements cover many DOM actions but do not reproduce handle identity/lifetime semantics. |

## FileChooser

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-filechooser.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `FileChooser.element` | method | Missing | — | No public FileChooser abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `FileChooser.isMultiple` | method | Missing | — | No public FileChooser abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `FileChooser.page` | method | Missing | — | No public FileChooser abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `FileChooser.setFiles` | method | Missing | — | No public FileChooser abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |

## Fixtures

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-fixtures.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Fixtures.browser` | property | Partial | `Browser` ([source](crates/ferrite-e2e/src/browser.rs#L324)) | Manual Browser/Context/client setup or injected Page; runner injects a fresh context/page for every attempt. |
| `Fixtures.browserName` | property | Partial | `Browser.kind` ([source](crates/ferrite-e2e/src/browser.rs#L838)) | Manual Browser/Context/client setup or injected Page; runner injects a fresh context/page for every attempt. |
| `Fixtures.context` | property | Partial | `TestContext.context` ([source](crates/ferrite-e2e/src/runner.rs#L1246)) | Fresh per-attempt resource, usable as a typed fixture dependency. request is isolated from browser cookies; context.request() shares cookies. |
| `Fixtures.mount` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Fixtures.page` | property | Partial | `TestContext.page` ([source](crates/ferrite-e2e/src/runner.rs#L1244)) | Fresh per-attempt resource, usable as a typed fixture dependency. request is isolated from browser cookies; context.request() shares cookies. |
| `Fixtures.request` | property | Partial | `TestContext.request` ([source](crates/ferrite-e2e/src/runner.rs#L1248)) | Fresh per-attempt resource, usable as a typed fixture dependency. request is isolated from browser cookies; context.request() shares cookies. |

## Frame

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-frame.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Frame.addScriptTag` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.addStyleTag` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.check` | method | Partial | `Locator.check` ([source](crates/ferrite-e2e/src/locator.rs#L2375)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.childFrames` | method | Partial | `Frame.child_frames` ([source](crates/ferrite-e2e/src/page.rs#L639)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.click` | method | Partial | `Locator.click` ([source](crates/ferrite-e2e/src/locator.rs#L1601)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.content` | method | Equivalent | `Frame.content` ([source](crates/ferrite-e2e/src/page.rs#L673)) | Basic document access. |
| `Frame.dblclick` | method | Partial | `Locator.dblclick` ([source](crates/ferrite-e2e/src/locator.rs#L1748)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.dispatchEvent` | method | Partial | `Locator.dispatch_event` ([source](crates/ferrite-e2e/src/locator.rs#L1987)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.dragAndDrop` | method | Partial | `Locator.drag_to` ([source](crates/ferrite-e2e/src/locator.rs#L1893)) | Frame.locator(source).drag_to(target, steps); fewer options and frame-coordinate restrictions. |
| `Frame.evalOnSelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.evalOnSelectorAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.evaluate` | method | Partial | `Frame.evaluate` ([source](crates/ferrite-e2e/src/page.rs#L682)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.evaluateHandle` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.fill` | method | Partial | `Locator.fill` ([source](crates/ferrite-e2e/src/locator.rs#L2151)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.focus` | method | Partial | `Locator.focus` ([source](crates/ferrite-e2e/src/locator.rs#L1824)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.frameElement` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.frameLocator` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.getAttribute` | method | Partial | `Locator.attribute` ([source](crates/ferrite-e2e/src/locator.rs#L2613)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.getByAltText` | method | Partial | `Frame.get_by_alt` ([source](crates/ferrite-e2e/src/page.rs#L740)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByLabel` | method | Partial | `Frame.get_by_label` ([source](crates/ferrite-e2e/src/page.rs#L730)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByPlaceholder` | method | Partial | `Frame.get_by_placeholder` ([source](crates/ferrite-e2e/src/page.rs#L735)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByRole` | method | Partial | `Frame.get_by_role_with` ([source](crates/ferrite-e2e/src/page.rs#L721)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByTestId` | method | Partial | `Frame.get_by_test_id` ([source](crates/ferrite-e2e/src/page.rs#L706)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByText` | method | Partial | `Frame.get_by_text` ([source](crates/ferrite-e2e/src/page.rs#L711)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByTitle` | method | Partial | `Frame.get_by_title` ([source](crates/ferrite-e2e/src/page.rs#L745)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.goto` | method | Partial | `Frame.goto` ([source](crates/ferrite-e2e/src/page.rs#L617)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.hover` | method | Partial | `Locator.hover` ([source](crates/ferrite-e2e/src/locator.rs#L1774)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.innerHTML` | method | Partial | `Locator.inner_html` ([source](crates/ferrite-e2e/src/locator.rs#L2687)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.innerText` | method | Partial | `Locator.inner_text` ([source](crates/ferrite-e2e/src/locator.rs#L1002)) | Frame.locator(selector) followed by the distinct rendered/raw text getter; strict resolution and same-origin frame limitations remain. |
| `Frame.inputValue` | method | Partial | `Locator.input_value` ([source](crates/ferrite-e2e/src/locator.rs#L2561)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isChecked` | method | Partial | `Locator.is_checked` ([source](crates/ferrite-e2e/src/locator.rs#L2878)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isDetached` | method | Partial | `Frame.is_detached` ([source](crates/ferrite-e2e/src/page.rs#L651)) | Asynchronous native tree identity check; explicit page closure is detached, disconnection errors propagate. No replacement retargeting. |
| `Frame.isDisabled` | method | Partial | `Locator.is_disabled` ([source](crates/ferrite-e2e/src/locator.rs#L2861)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isEditable` | method | Partial | `Locator.is_editable` ([source](crates/ferrite-e2e/src/locator.rs#L2895)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isEnabled` | method | Partial | `Locator.is_enabled` ([source](crates/ferrite-e2e/src/locator.rs#L2844)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isHidden` | method | Partial | `Locator.is_hidden` ([source](crates/ferrite-e2e/src/locator.rs#L2824)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isVisible` | method | Partial | `Locator.is_visible` ([source](crates/ferrite-e2e/src/locator.rs#L2807)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.locator` | method | Partial | `Frame.locator` ([source](crates/ferrite-e2e/src/page.rs#L701)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.name` | method | Partial | `Frame.name` ([source](crates/ferrite-e2e/src/page.rs#L606)) | Native lookup name snapshot; Chromium reports names, Firefox metadata is empty. No inferred name. |
| `Frame.page` | method | Partial | `Frame.page` ([source](crates/ferrite-e2e/src/page.rs#L494)) | Frame-scoped counterpart; unit-returning waits, fewer predicate/options modes; frame NetworkIdle remains unsupported. current_url() reads navigation updates. |
| `Frame.parentFrame` | method | Partial | `Frame.parent` ([source](crates/ferrite-e2e/src/page.rs#L626)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.press` | method | Partial | `Locator.press` ([source](crates/ferrite-e2e/src/locator.rs#L2191)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.querySelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.querySelectorAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.selectOption` | method | Partial | `Locator.select_options` ([source](crates/ferrite-e2e/src/locator.rs#L2481)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.setChecked` | method | Partial | `Locator.set_checked` ([source](crates/ferrite-e2e/src/locator.rs#L2988)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.setContent` | method | Partial | `Frame.set_content` ([source](crates/ferrite-e2e/src/page.rs#L499)) | Frame-scoped counterpart; unit-returning waits, fewer predicate/options modes; frame NetworkIdle remains unsupported. current_url() reads navigation updates. |
| `Frame.setInputFiles` | method | Partial | `Locator.set_input_file_payloads` ([source](crates/ferrite-e2e/src/locator.rs#L2352)) | FilePayload filename/MIME/bytes or existing path uploads; empty lists clear, multiple files require a multiple input, 64 MiB total cap. DOM File/DataTransfer injection on both engines; no native chooser/directory upload/options parity. Frame uses Frame.locator. |
| `Frame.tap` | method | Partial | `Locator.tap` ([source](crates/ferrite-e2e/src/locator.rs#L1864)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.textContent` | method | Partial | `Locator.text_content` ([source](crates/ferrite-e2e/src/locator.rs#L979)) | Frame.locator(selector) followed by the distinct rendered/raw text getter; strict resolution and same-origin frame limitations remain. |
| `Frame.title` | method | Equivalent | `Frame.title` ([source](crates/ferrite-e2e/src/page.rs#L664)) | Basic document access. |
| `Frame.type` (deprecated) | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L2231)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.uncheck` | method | Partial | `Locator.uncheck` ([source](crates/ferrite-e2e/src/locator.rs#L2394)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.url` | method | Partial | `Frame.current_url` ([source](crates/ferrite-e2e/src/page.rs#L536)) | current_url() reads live URL; url() retains lookup snapshot. Native realm/tree errors propagate after detach. |
| `Frame.waitForFunction` | method | Partial | `Frame.wait_for_function_value` ([source](crates/ferrite-e2e/src/page.rs#L524)) | JSON arguments, native animation-frame/interval polling and captured JSON results; frame remote handles unsupported. Legacy expression helper returns unit. |
| `Frame.waitForLoadState` | method | Partial | `Frame.wait_for_load_state` ([source](crates/ferrite-e2e/src/page.rs#L544)) | Frame-scoped counterpart; unit-returning waits, fewer predicate/options modes; frame NetworkIdle remains unsupported. current_url() reads navigation updates. |
| `Frame.waitForNavigation` (deprecated) | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.waitForSelector` | method | Partial | `Frame.wait_for_selector` ([source](crates/ferrite-e2e/src/page.rs#L548)) | Frame-scoped counterpart; unit-returning waits, fewer predicate/options modes; frame NetworkIdle remains unsupported. current_url() reads navigation updates. |
| `Frame.waitForTimeout` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.waitForURL` | method | Partial | `Frame.wait_for_url_matching_with_options` ([source](crates/ferrite-e2e/src/page.rs#L582)) | Exact (relative to base URL), full-URL glob/regex or predicate with Commit/DOMContentLoaded/Load readiness in one navigation budget. Page NetworkIdle uses 500ms observed HTTP quiet; frames reject it. Legacy helpers retain substring/URL-only semantics. Zero timeout, cancellation and enclosing budgets supported; URLPattern absent. |

## FrameLocator

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-framelocator.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `FrameLocator.first` (deprecated) | method | Partial | `FrameLocator.first` ([source](crates/ferrite-e2e/src/frame_locator.rs#L44)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.frameLocator` | method | Partial | `FrameLocator.frame_locator` ([source](crates/ferrite-e2e/src/frame_locator.rs#L55)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.getByAltText` | method | Partial | `FrameLocator.get_by_alt` ([source](crates/ferrite-e2e/src/frame_locator.rs#L79)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.getByLabel` | method | Partial | `FrameLocator.get_by_label` ([source](crates/ferrite-e2e/src/frame_locator.rs#L70)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.getByPlaceholder` | method | Partial | `FrameLocator.get_by_placeholder` ([source](crates/ferrite-e2e/src/frame_locator.rs#L76)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.getByRole` | method | Partial | `FrameLocator.get_by_role` ([source](crates/ferrite-e2e/src/frame_locator.rs#L61)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.getByTestId` | method | Partial | `FrameLocator.get_by_test_id` ([source](crates/ferrite-e2e/src/frame_locator.rs#L73)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.getByText` | method | Partial | `FrameLocator.get_by_text` ([source](crates/ferrite-e2e/src/frame_locator.rs#L67)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.getByTitle` | method | Partial | `FrameLocator.get_by_title` ([source](crates/ferrite-e2e/src/frame_locator.rs#L82)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.last` (deprecated) | method | Partial | `FrameLocator.last` ([source](crates/ferrite-e2e/src/frame_locator.rs#L47)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.locator` | method | Partial | `FrameLocator.locator` ([source](crates/ferrite-e2e/src/frame_locator.rs#L58)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.nth` (deprecated) | method | Partial | `FrameLocator.nth` ([source](crates/ferrite-e2e/src/frame_locator.rs#L50)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.owner` | method | Partial | `FrameLocator.owner` ([source](crates/ferrite-e2e/src/frame_locator.rs#L36)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |

## FullConfig

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-fullconfig.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `FullConfig.argv` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.configFile` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.failOnFlakyTests` | property | Partial | `ResolvedRunConfig.fail_on_flaky_tests` ([source](crates/ferrite-e2e/src/resolved_config.rs#L45)) | Effective opt-in aggregate failure policy; original passed/flaky results and attempts retained. CLI/config/Runner wiring and explicit JUnit policy violations are documented in D01. |
| `FullConfig.forbidOnly` | property | Partial | `ResolvedRunConfig.forbid_only` ([source](crates/ferrite-e2e/src/resolved_config.rs#L44)) | Effective config/CLI/CI focus protection; registered test/suite inventory audited before filters/shards, including skipped descendants. Stricter than pinned upstream grep behavior. |
| `FullConfig.fullyParallel` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.globalSetup` | property | Partial | `Runner.global_setup` ([source](crates/ferrite-e2e/src/runner.rs#L2965)) | Comparable runner/configuration knob; effective configuration is a Rust subset and runner/CLI scheduling semantics differ. |
| `FullConfig.globalTeardown` | property | Partial | `Runner.global_teardown` ([source](crates/ferrite-e2e/src/runner.rs#L2976)) | Comparable runner/configuration knob; effective configuration is a Rust subset and runner/CLI scheduling semantics differ. |
| `FullConfig.globalTimeout` | property | Partial | `ResolvedRunConfig.global_timeout_ms` ([source](crates/ferrite-e2e/src/resolved_config.rs#L40)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. |
| `FullConfig.grep` | property | Partial | `ResolvedRunConfig.grep` ([source](crates/ferrite-e2e/src/resolved_config.rs#L60)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. |
| `FullConfig.grepInvert` | property | Partial | `ResolvedRunConfig.grep_invert` ([source](crates/ferrite-e2e/src/resolved_config.rs#L61)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. |
| `FullConfig.maxFailures` | property | Partial | `ResolvedRunConfig.max_failures` ([source](crates/ferrite-e2e/src/resolved_config.rs#L41)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. |
| `FullConfig.metadata` | property | Partial | `ResolvedRunConfig.metadata` ([source](crates/ferrite-e2e/src/resolved_config.rs#L33)) | JSON-safe BTreeMap user metadata, frozen before hooks and exposed through config/project_config and owned WorkerInfo snapshots; project values replace the whole map or inherit the run map. Explicit empty maps survive reports/CLI/bundles; no upstream actualWorkers injection or mutable JS object identity. |
| `FullConfig.preserveOutput` | property | Partial | `ResolvedRunConfig.output_retention` ([source](crates/ferrite-e2e/src/resolved_config.rs#L51)) | Frozen typed owned-output retention policy. Cleanup follows live reporters and bundle export; safe/pruned report links and fail-closed ownership/export checks are documented in D03. |
| `FullConfig.projects` | property | Partial | `ResolvedRunConfig.projects` ([source](crates/ferrite-e2e/src/resolved_config.rs#L64)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. |
| `FullConfig.quiet` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.reporter` | property | Partial | `ResolvedRunConfig.reporter` ([source](crates/ferrite-e2e/src/resolved_config.rs#L42)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. |
| `FullConfig.reportSlowTests` | property | Partial | `ResolvedRunConfig.report_slow_tests` ([source](crates/ferrite-e2e/src/resolved_config.rs#L34)) | Optional SlowTestOptions; strict threshold_ms and bounded max <=1000, zero disables, absent opts preserve legacy reports. Rust summarizes scheduled test/project/repeat result indices, not source files; retry total duration counted once. Upstream defaults on and max zero is unlimited. |
| `FullConfig.rootDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.shard` | property | Partial | `ResolvedRunConfig.shard` ([source](crates/ferrite-e2e/src/resolved_config.rs#L62)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. |
| `FullConfig.tags` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.updateSnapshots` | property | Partial | `ResolvedRunConfig.snapshot_update` ([source](crates/ferrite-e2e/src/resolved_config.rs#L55)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. |
| `FullConfig.updateSourceMethod` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.version` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.webServer` | property | Partial | `E2eConfig.web_server` ([source](crates/ferrite-config/src/lib.rs#L521)) | Comparable runner/configuration knob; effective configuration is a Rust subset and runner/CLI scheduling semantics differ. |
| `FullConfig.workers` | property | Partial | `ResolvedRunConfig.workers` ([source](crates/ferrite-e2e/src/resolved_config.rs#L35)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. |

## FullProject

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-fullproject.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `FullProject.dependencies` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.grep` | property | Partial | `ResolvedProjectConfig.grep` ([source](crates/ferrite-e2e/src/resolved_config.rs#L16)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. Context overrides replace the whole value; launch proxy/TLS inheritance is shared with native creation. |
| `FullProject.grepInvert` | property | Partial | `ResolvedProjectConfig.grep_invert` ([source](crates/ferrite-e2e/src/resolved_config.rs#L17)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. Context overrides replace the whole value; launch proxy/TLS inheritance is shared with native creation. |
| `FullProject.ignoreSnapshots` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.metadata` | property | Partial | `ResolvedProjectConfig.metadata` ([source](crates/ferrite-e2e/src/resolved_config.rs#L10)) | JSON-safe BTreeMap user metadata, frozen before hooks and exposed through config/project_config and owned WorkerInfo snapshots; project values replace the whole map or inherit the run map. Explicit empty maps survive reports/CLI/bundles; no upstream actualWorkers injection or mutable JS object identity. |
| `FullProject.name` | property | Partial | `ResolvedProjectConfig.name` ([source](crates/ferrite-e2e/src/resolved_config.rs#L9)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. Context overrides replace the whole value; launch proxy/TLS inheritance is shared with native creation. |
| `FullProject.snapshotDir` | property | Partial | `ResolvedProjectConfig.snapshot_dir` ([source](crates/ferrite-e2e/src/resolved_config.rs#L22)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. Context overrides replace the whole value; launch proxy/TLS inheritance is shared with native creation. |
| `FullProject.outputDir` | property | Partial | `ResolvedProjectConfig.output_dir` ([source](crates/ferrite-e2e/src/resolved_config.rs#L21)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. Context overrides replace the whole value; launch proxy/TLS inheritance is shared with native creation. |
| `FullProject.repeatEach` | property | Partial | `ResolvedProjectConfig.repeat_each` ([source](crates/ferrite-e2e/src/resolved_config.rs#L20)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. Context overrides replace the whole value; launch proxy/TLS inheritance is shared with native creation. |
| `FullProject.retries` | property | Partial | `ResolvedProjectConfig.retries` ([source](crates/ferrite-e2e/src/resolved_config.rs#L18)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. Context overrides replace the whole value; launch proxy/TLS inheritance is shared with native creation. |
| `FullProject.teardown` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.testDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.testIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.testMatch` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.timeout` | property | Partial | `ResolvedProjectConfig.timeout_ms` ([source](crates/ferrite-e2e/src/resolved_config.rs#L19)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. Context overrides replace the whole value; launch proxy/TLS inheritance is shared with native creation. |
| `FullProject.use` | property | Partial | `ResolvedProjectConfig.context` ([source](crates/ferrite-e2e/src/resolved_config.rs#L24)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. Context overrides replace the whole value; launch proxy/TLS inheritance is shared with native creation. |

## GenericAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-genericassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `GenericAssertions.not` | property | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.resolves` | property | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.rejects` | property | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toBe` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toBeCloseTo` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toBeDefined` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toBeFalsy` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toBeGreaterThan` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toBeGreaterThanOrEqual` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toBeInstanceOf` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toBeLessThan` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toBeLessThanOrEqual` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toBeNaN` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toBeNull` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toBeTruthy` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toBeUndefined` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toContain` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toContainEqual` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toEqual` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toHaveLength` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toHaveProperty` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toMatch` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toMatchObject` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toStrictEqual` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toThrow` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.toThrowError` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.any` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.anything` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.arrayContaining` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.arrayOf` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.closeTo` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.objectContaining` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.stringContaining` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |
| `GenericAssertions.stringMatching` | method | Idiomatic | — | Rust assert!/assert_eq!, Option/Result matching and explicit predicates provide analogous checks; no Playwright matcher/negation/asymmetric-matcher object. |

## JSHandle

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-jshandle.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `JSHandle.asElement` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `JSHandle.dispose` | method | Partial | `JSHandle.dispose` ([source](crates/ferrite-e2e/src/jshandle.rs#L93)) | Remote references exist; expression-string/JSON bridge has fewer value and argument types. |
| `JSHandle.evaluate` | method | Partial | `JSHandle.evaluate` ([source](crates/ferrite-e2e/src/jshandle.rs#L66)) | Remote references exist; expression-string/JSON bridge has fewer value and argument types. |
| `JSHandle.evaluateHandle` | method | Partial | `JSHandle.evaluate_handle` ([source](crates/ferrite-e2e/src/jshandle.rs#L75)) | Retains remote results and enumerable own string-keyed properties; no ElementHandle conversion/JSHandle arguments. |
| `JSHandle.getProperties` | method | Partial | `JSHandle.get_properties` ([source](crates/ferrite-e2e/src/jshandle.rs#L82)) | Retains remote results and enumerable own string-keyed properties; no ElementHandle conversion/JSHandle arguments. |
| `JSHandle.getProperty` | method | Partial | `JSHandle.get_property` ([source](crates/ferrite-e2e/src/jshandle.rs#L58)) | Remote references exist; expression-string/JSON bridge has fewer value and argument types. |
| `JSHandle.jsonValue` | method | Partial | `JSHandle.json_value` ([source](crates/ferrite-e2e/src/jshandle.rs#L49)) | Remote references exist; expression-string/JSON bridge has fewer value and argument types. |

## Keyboard

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-keyboard.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Keyboard.down` | method | Partial | `Page.key_down` ([source](crates/ferrite-e2e/src/page.rs#L3215)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |
| `Keyboard.insertText` | method | Partial | `Page.insert_text` ([source](crates/ferrite-e2e/src/page.rs#L3135)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |
| `Keyboard.press` | method | Partial | `Page.press_key_with` ([source](crates/ferrite-e2e/src/page.rs#L3157)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |
| `Keyboard.type` | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L2231)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |
| `Keyboard.up` | method | Partial | `Page.key_up` ([source](crates/ferrite-e2e/src/page.rs#L3226)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |

## Location

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-location.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Location.file` | property | Partial | `SourceLocation.file` ([source](crates/ferrite-e2e/src/report.rs#L32)) | Rust caller location for user steps; test phase errors point to the test definition, with column zero when unknown. |
| `Location.line` | property | Partial | `SourceLocation.line` ([source](crates/ferrite-e2e/src/report.rs#L33)) | Rust caller location for user steps; test phase errors point to the test definition, with column zero when unknown. |
| `Location.column` | property | Partial | `SourceLocation.column` ([source](crates/ferrite-e2e/src/report.rs#L34)) | Rust caller location for user steps; test phase errors point to the test definition, with column zero when unknown. |

## Locator

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-locator.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Locator.all` | method | Equivalent | `Locator.all` ([source](crates/ferrite-e2e/src/locator.rs#L1348)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.allInnerTexts` | method | Partial | `Locator.all_inner_texts` ([source](crates/ferrite-e2e/src/locator.rs#L959)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.allTextContents` | method | Partial | `Locator.all_text_contents` ([source](crates/ferrite-e2e/src/locator.rs#L939)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.and` | method | Equivalent | `Locator.and_` ([source](crates/ferrite-e2e/src/locator.rs#L1288)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.ariaSnapshot` | method | Partial | `Locator.aria_snapshot_with` ([source](crates/ferrite-e2e/src/locator.rs#L1113)) | Opt-in bounded DOM approximation: role depth, rounded frame-local boxes, optional supported state, explicit traversal/name budgets and deterministic safety markers. Legacy methods retain unbounded output. Native name/layout work is not preemptible; text fragments, full accessible names, AI modes and YAML syntax differ. |
| `Locator.ariaSnapshotJSON` | method | Partial | `Locator.aria_snapshot_json_with` ([source](crates/ferrite-e2e/src/locator.rs#L1080)) | Opt-in bounded DOM approximation: role depth, rounded frame-local boxes, optional supported state, explicit traversal/name budgets and deterministic safety markers. Legacy methods retain unbounded output. Native name/layout work is not preemptible; text fragments, full accessible names, AI modes and YAML syntax differ. |
| `Locator.blur` | method | Partial | `Locator.blur` ([source](crates/ferrite-e2e/src/locator.rs#L1844)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.boundingBox` | method | Partial | `Locator.bounding_box` ([source](crates/ferrite-e2e/src/locator.rs#L2946)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.check` | method | Partial | `Locator.check_with_options` ([source](crates/ferrite-e2e/src/locator.rs#L2414)) | Trusted pointer input with padding-box positions, modifiers, trial readiness and scoped timeout. Trial may scroll but sends no input; action-acquired keys/buttons are released on failure/cancellation. Same-origin offsets and positive axis scaling supported; rotated/perspective frames and cross-origin coordinates unsupported. Other upstream options remain narrower. |
| `Locator.clear` | method | Partial | `Locator.clear` ([source](crates/ferrite-e2e/src/locator.rs#L2269)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.click` | method | Partial | `Locator.click_with_options` ([source](crates/ferrite-e2e/src/locator.rs#L1620)) | Trusted pointer input with padding-box positions, modifiers, trial readiness and scoped timeout. Trial may scroll but sends no input; action-acquired keys/buttons are released on failure/cancellation. Same-origin offsets and positive axis scaling supported; rotated/perspective frames and cross-origin coordinates unsupported. Other upstream options remain narrower. |
| `Locator.count` | method | Equivalent | `Locator.count` ([source](crates/ferrite-e2e/src/locator.rs#L1416)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.dblclick` | method | Partial | `Locator.dblclick` ([source](crates/ferrite-e2e/src/locator.rs#L1748)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.describe` | method | Partial | `Locator.describe` ([source](crates/ferrite-e2e/src/locator.rs#L759)) | Optional label stored separately from DOM selectors; clone/decorators retain it, derived selectors clear it, and content-frame owner retains it. Labels appear once in Rust action/assertion errors, steps and owned traces; the actual pinned Playwright timeout omits the label. |
| `Locator.description` | method | Partial | `Locator.description` ([source](crates/ferrite-e2e/src/locator.rs#L768)) | Returns Option<&str>; empty describe or clear_description removes the label. Actual pinned clone/chaining observations verified on both native engines. |
| `Locator.dispatchEvent` | method | Partial | `Locator.dispatch_event_with` ([source](crates/ferrite-e2e/src/locator.rs#L2020)) | Typed synthetic DOM constructors with JSON event-specific initialization and bubbles/cancelable/composed flags; CustomEvent legacy helper retained. Auto input events follow the pinned Event constructor; InputEvent can be requested explicitly. No live handle arguments. |
| `Locator.dragTo` | method | Partial | `Locator.drag_to_with_options` ([source](crates/ferrite-e2e/src/locator.rs#L1904)) | Trusted pointer input with padding-box positions, modifiers, trial readiness and scoped timeout. Trial may scroll but sends no input; action-acquired keys/buttons are released on failure/cancellation. Same-origin offsets and positive axis scaling supported; rotated/perspective frames and cross-origin coordinates unsupported. Other upstream options remain narrower. |
| `Locator.drop` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.elementHandle` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.elementHandles` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.contentFrame` | method | Partial | `Locator.content_frame` ([source](crates/ferrite-e2e/src/locator.rs#L848)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.evaluate` | method | Partial | `Locator.evaluate_with_arg` ([source](crates/ferrite-e2e/src/locator.rs#L888)) | Function with element and JSON argument; no JSHandle arguments or arbitrary JS result serialization. |
| `Locator.evaluateAll` | method | Partial | `Locator.evaluate_all` ([source](crates/ferrite-e2e/src/locator.rs#L915)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.evaluateHandle` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.fill` | method | Partial | `Locator.fill` ([source](crates/ferrite-e2e/src/locator.rs#L2151)) | Retries visibility/enabled/editability, uses native input setters or contenteditable text plus input/change events; not all native input validation/event semantics. |
| `Locator.filter` | method | Partial | `Locator.filter_with` ([source](crates/ferrite-e2e/src/locator.rs#L1250)) | Relative has/hasNot/text and visibility filters; exact/regex matching through locator builders. Inner locators must share the document. |
| `Locator.first` | method | Partial | `Locator.first` ([source](crates/ferrite-e2e/src/locator.rs#L1192)) | Narrows the resolved set to its first match, including count/all; selector semantics remain narrower. |
| `Locator.focus` | method | Partial | `Locator.focus` ([source](crates/ferrite-e2e/src/locator.rs#L1824)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.frameLocator` | method | Partial | `Locator.frame_locator` ([source](crates/ferrite-e2e/src/locator.rs#L854)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.getAttribute` | method | Partial | `Locator.attribute` ([source](crates/ferrite-e2e/src/locator.rs#L2613)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.getByAltText` | method | Partial | `Locator.get_by_alt` ([source](crates/ferrite-e2e/src/locator.rs#L1405)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByLabel` | method | Partial | `Locator.get_by_label` ([source](crates/ferrite-e2e/src/locator.rs#L1393)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByPlaceholder` | method | Partial | `Locator.get_by_placeholder` ([source](crates/ferrite-e2e/src/locator.rs#L1399)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByRole` | method | Partial | `Locator.get_by_role_with` ([source](crates/ferrite-e2e/src/locator.rs#L1387)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByTestId` | method | Partial | `Locator.get_by_test_id` ([source](crates/ferrite-e2e/src/locator.rs#L1369)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByText` | method | Partial | `Locator.get_by_text` ([source](crates/ferrite-e2e/src/locator.rs#L1375)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByTitle` | method | Partial | `Locator.get_by_title` ([source](crates/ferrite-e2e/src/locator.rs#L1411)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.hideHighlight` | method | Partial | `Locator.hide_highlight` ([source](crates/ferrite-e2e/src/locator.rs#L1025)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.highlight` | method | Partial | `Locator.highlight` ([source](crates/ferrite-e2e/src/locator.rs#L2963)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.hover` | method | Partial | `Locator.hover_with_options` ([source](crates/ferrite-e2e/src/locator.rs#L1785)) | Trusted pointer input with padding-box positions, modifiers, trial readiness and scoped timeout. Trial may scroll but sends no input; action-acquired keys/buttons are released on failure/cancellation. Same-origin offsets and positive axis scaling supported; rotated/perspective frames and cross-origin coordinates unsupported. Other upstream options remain narrower. |
| `Locator.innerHTML` | method | Partial | `Locator.inner_html` ([source](crates/ferrite-e2e/src/locator.rs#L2687)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.innerText` | method | Partial | `Locator.inner_text` ([source](crates/ferrite-e2e/src/locator.rs#L1002)) | Distinct rendered DOM innerText getter; strict resolution, nullable Rust result and same-origin lazy frame limitations remain. |
| `Locator.inputValue` | method | Partial | `Locator.input_value` ([source](crates/ferrite-e2e/src/locator.rs#L2561)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isChecked` | method | Partial | `Locator.is_checked` ([source](crates/ferrite-e2e/src/locator.rs#L2878)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isDisabled` | method | Partial | `Locator.is_disabled` ([source](crates/ferrite-e2e/src/locator.rs#L2861)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isEditable` | method | Partial | `Locator.is_editable` ([source](crates/ferrite-e2e/src/locator.rs#L2895)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isEnabled` | method | Partial | `Locator.is_enabled` ([source](crates/ferrite-e2e/src/locator.rs#L2844)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isHidden` | method | Partial | `Locator.is_hidden` ([source](crates/ferrite-e2e/src/locator.rs#L2824)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isVisible` | method | Partial | `Locator.is_visible` ([source](crates/ferrite-e2e/src/locator.rs#L2807)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.last` | method | Partial | `Locator.last` ([source](crates/ferrite-e2e/src/locator.rs#L1198)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.locator` | method | Partial | `Locator.locator` ([source](crates/ferrite-e2e/src/locator.rs#L1186)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.normalize` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.nth` | method | Equivalent | `Locator.nth` ([source](crates/ferrite-e2e/src/locator.rs#L1204)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.or` | method | Equivalent | `Locator.or_` ([source](crates/ferrite-e2e/src/locator.rs#L1283)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.page` | method | Partial | `Locator.page` ([source](crates/ferrite-e2e/src/locator.rs#L859)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.press` | method | Partial | `Locator.press_with` ([source](crates/ferrite-e2e/src/locator.rs#L2211)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.pressSequentially` | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L2231)) | Trusted per-character key input with optional delay; fewer keyboard layout/modifier options. |
| `Locator.screenshot` | method | Partial | `Locator.screenshot_with` ([source](crates/ferrite-e2e/src/locator.rs#L2122)) | Element-defined document capture with masks/colors/styles, scale/background/quality and shared timeout. Scrolls into view, preserves metrics and owns restoration. clip/full_page rejected; Firefox transparency unavailable and Css uses raster normalization; same-origin frame/open-root limits remain. |
| `Locator.scrollIntoViewIfNeeded` | method | Partial | `Locator.scroll_into_view` ([source](crates/ferrite-e2e/src/locator.rs#L1967)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.selectOption` | method | Partial | `Locator.select_options` ([source](crates/ferrite-e2e/src/locator.rs#L2481)) | Value/label/index matching exists; returns unit rather than selected values, with fewer options. |
| `Locator.selectText` | method | Partial | `Locator.select_text` ([source](crates/ferrite-e2e/src/locator.rs#L2079)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.setChecked` | method | Partial | `Locator.set_checked_with_options` ([source](crates/ferrite-e2e/src/locator.rs#L2430)) | Trusted pointer input with padding-box positions, modifiers, trial readiness and scoped timeout. Trial may scroll but sends no input; action-acquired keys/buttons are released on failure/cancellation. Same-origin offsets and positive axis scaling supported; rotated/perspective frames and cross-origin coordinates unsupported. Other upstream options remain narrower. |
| `Locator.setInputFiles` | method | Partial | `Locator.set_input_file_payloads` ([source](crates/ferrite-e2e/src/locator.rs#L2352)) | FilePayload filename/MIME/bytes or existing path uploads; empty lists clear, multiple files require a multiple input, 64 MiB total cap. DOM File/DataTransfer injection on both engines; no native chooser/directory upload/options parity. Frame uses Frame.locator. |
| `Locator.tap` | method | Partial | `Locator.tap` ([source](crates/ferrite-e2e/src/locator.rs#L1864)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.textContent` | method | Partial | `Locator.text_content` ([source](crates/ferrite-e2e/src/locator.rs#L979)) | Untrimmed nullable DOM textContent with strict single-target resolution; no ElementHandle or full options surface. |
| `Locator.toString` | method | Idiomatic | `Locator` ([source](crates/ferrite-e2e/src/locator.rs#L720)) | Rust Display/to_string returns the label or raw selector; selector() always exposes the raw selector. No Playwright locator expression representation. |
| `Locator.type` (deprecated) | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L2231)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.uncheck` | method | Partial | `Locator.uncheck_with_options` ([source](crates/ferrite-e2e/src/locator.rs#L2422)) | Trusted pointer input with padding-box positions, modifiers, trial readiness and scoped timeout. Trial may scroll but sends no input; action-acquired keys/buttons are released on failure/cancellation. Same-origin offsets and positive axis scaling supported; rotated/perspective frames and cross-origin coordinates unsupported. Other upstream options remain narrower. |
| `Locator.visible` | method | Partial | `Locator.visible` ([source](crates/ferrite-e2e/src/locator.rs#L864)) | Lazy visibility filter reapplied when resolving; uses the shared DOM visibility approximation. |
| `Locator.waitFor` | method | Partial | `Locator.wait_for_state` ([source](crates/ferrite-e2e/src/locator.rs#L1490)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.waitForFunction` | method | Partial | `Locator.wait_for_function` ([source](crates/ferrite-e2e/src/locator.rs#L1126)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |

## LocatorAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-locatorassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `LocatorAssertions.not` | property | Partial | `LocatorExpect.not` ([source](crates/ferrite-e2e/src/expect.rs#L931)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeAttached` | method | Partial | `LocatorExpect.attached` ([source](crates/ferrite-e2e/src/expect.rs#L1388)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeChecked` | method | Partial | `LocatorExpect.checked_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L423)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toBeDisabled` | method | Partial | `LocatorExpect.disabled` ([source](crates/ferrite-e2e/src/expect.rs#L1247)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeEditable` | method | Partial | `LocatorExpect.editable` ([source](crates/ferrite-e2e/src/expect.rs#L1280)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeEmpty` | method | Partial | `LocatorExpect.empty` ([source](crates/ferrite-e2e/src/expect.rs#L1316)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeEnabled` | method | Partial | `LocatorExpect.enabled` ([source](crates/ferrite-e2e/src/expect.rs#L1214)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeFocused` | method | Partial | `LocatorExpect.focused` ([source](crates/ferrite-e2e/src/expect.rs#L1355)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeHidden` | method | Partial | `LocatorExpect.hidden` ([source](crates/ferrite-e2e/src/expect.rs#L973)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeInViewport` | method | Partial | `LocatorExpect.in_viewport_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L468)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toBeVisible` | method | Partial | `LocatorExpect.visible` ([source](crates/ferrite-e2e/src/expect.rs#L937)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toContainClass` | method | Partial | `LocatorExpect.contains_class_tokens` ([source](crates/ferrite-e2e/src/assertion_options.rs#L370)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toContainText` | method | Partial | `LocatorExpect.contains_texts_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L286)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toHaveAccessibleDescription` | method | Partial | `LocatorExpect.accessible_description_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L534)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toHaveAccessibleErrorMessage` | method | Partial | `LocatorExpect.accessible_error_message_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L550)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toHaveAccessibleName` | method | Partial | `LocatorExpect.accessible_name_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L521)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toHaveAttribute` | method | Partial | `LocatorExpect.attribute` ([source](crates/ferrite-e2e/src/expect.rs#L1421)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveClass` | method | Partial | `LocatorExpect.class_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L299)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toHaveCount` | method | Partial | `LocatorExpect.count` ([source](crates/ferrite-e2e/src/expect.rs#L1127)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveCSS` | method | Partial | `LocatorExpect.css` ([source](crates/ferrite-e2e/src/expect.rs#L1517)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveId` | method | Partial | `LocatorExpect.id` ([source](crates/ferrite-e2e/src/expect.rs#L1506)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveJSProperty` | method | Partial | `LocatorExpect.js_property` ([source](crates/ferrite-e2e/src/expect.rs#L1558)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveRole` | method | Partial | `LocatorExpect.role` ([source](crates/ferrite-e2e/src/expect.rs#L850)) | Retrying dedicated matcher; ARIA snapshots use exact text and a DOM approximation, not full YAML matching. |
| `LocatorAssertions.toHaveScreenshot` | method | Partial | `LocatorExpect.screenshot_with` ([source](crates/ferrite-e2e/src/expect.rs#L1705)) | Successive stable PNG captures within one assertion window, real downloadable-font readiness, native animation finish/cancel-resume and B09 capture options and typed operation errors. Missing/all/changed generation requires stability; negation needs a valid baseline. Runner/project/explicit path templates; attempt/step-owned expected/actual/diff copies, last-pair stability diagnostics, final-only completed outer-poll images and portable bundles. Comparison/read/Css/diff/staging jobs have two active slots and cooperative cancellation, 64-million-pixel decoding and 512-MiB encoded-input limits. Frozen expected bytes, staged foreground baseline replacement, non-overwriting Missing installation and preserved symlink aliases. Queued memory and opaque OS/codec phases are not hard bounded. Final failure diagnostics share one additional five-second budget under enclosing cancellation/deadlines, with staged non-overwriting foreground attachment publication and a 1024-name collision cap. Per-channel comparison, visual diff, update/default semantics and retained single-mismatch probe semantics differ. |
| `LocatorAssertions.toHaveText` | method | Partial | `LocatorExpect.text_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L204)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toHaveValue` | method | Partial | `LocatorExpect.value` ([source](crates/ferrite-e2e/src/expect.rs#L1088)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveValues` | method | Partial | `LocatorExpect.values_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L346)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toMatchAriaSnapshot` | method | Partial | `LocatorExpect.aria_snapshot_with` ([source](crates/ferrite-e2e/src/expect.rs#L890)) | Retrying exact Ferrite text with bounded capture options; DOM approximation and truncation markers, no upstream YAML patterns or AI modes. |

## Logger

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-logger.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Logger.isEnabled` | method | Missing | — | No public Logger abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Logger.log` | method | Missing | — | No public Logger abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |

## Mouse

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-mouse.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Mouse.click` | method | Partial | `Page.mouse_click_with` ([source](crates/ferrite-e2e/src/page.rs#L3108)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.dblclick` | method | Partial | `Page.mouse_click` ([source](crates/ferrite-e2e/src/page.rs#L3097)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.down` | method | Partial | `Page.mouse_down` ([source](crates/ferrite-e2e/src/page.rs#L3171)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.move` | method | Partial | `Page.mouse_move` ([source](crates/ferrite-e2e/src/page.rs#L3124)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.up` | method | Partial | `Page.mouse_up` ([source](crates/ferrite-e2e/src/page.rs#L3182)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.wheel` | method | Partial | `Page.mouse_wheel` ([source](crates/ferrite-e2e/src/page.rs#L3204)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |

## Page

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-page.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Page.clock` | property | Partial | `Page.clock_install` ([source](crates/ferrite-e2e/src/page.rs#L3634)) | Fake clock is scoped to the current document and resets on navigation; semantic differences below. |
| `Page.close` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1982)) | PageEvent.Closed via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.console` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1982)) | PageEvent.Console via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.crash` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.dialog` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1982)) | PageEvent.Dialog via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.dialogClosed` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1982)) | PageEventKind::DialogClosed; native frame/page identity, parent/URL/document metadata when supplied, same-document history events, child-first detach and main-document readiness. Owned metadata snapshots rather than live Frame/Dialog objects; optional Firefox subscriptions and frame-name absence remain explicit. Chromium current target only: OOPIF adoption is deferred and swap denotes session departure. |
| `Page.DOMContentLoaded` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1982)) | PageEventKind::DomContentLoaded; native frame/page identity, parent/URL/document metadata when supplied, same-document history events, child-first detach and main-document readiness. Owned metadata snapshots rather than live Frame/Dialog objects; optional Firefox subscriptions and frame-name absence remain explicit. Chromium current target only: OOPIF adoption is deferred and swap denotes session departure. |
| `Page.download` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1982)) | PageEvent.Download via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.fileChooser` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.frameAttached` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1982)) | PageEventKind::FrameAttached; native frame/page identity, parent/URL/document metadata when supplied, same-document history events, child-first detach and main-document readiness. Owned metadata snapshots rather than live Frame/Dialog objects; optional Firefox subscriptions and frame-name absence remain explicit. Chromium current target only: OOPIF adoption is deferred and swap denotes session departure. |
| `Page.frameDetached` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1982)) | PageEventKind::FrameDetached; native frame/page identity, parent/URL/document metadata when supplied, same-document history events, child-first detach and main-document readiness. Owned metadata snapshots rather than live Frame/Dialog objects; optional Firefox subscriptions and frame-name absence remain explicit. Chromium current target only: OOPIF adoption is deferred and swap denotes session departure. |
| `Page.frameNavigated` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1982)) | PageEventKind::FrameNavigated; native frame/page identity, parent/URL/document metadata when supplied, same-document history events, child-first detach and main-document readiness. Owned metadata snapshots rather than live Frame/Dialog objects; optional Firefox subscriptions and frame-name absence remain explicit. Chromium current target only: OOPIF adoption is deferred and swap denotes session departure. |
| `Page.load` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1982)) | PageEventKind::Load; native frame/page identity, parent/URL/document metadata when supplied, same-document history events, child-first detach and main-document readiness. Owned metadata snapshots rather than live Frame/Dialog objects; optional Firefox subscriptions and frame-name absence remain explicit. Chromium current target only: OOPIF adoption is deferred and swap denotes session departure. |
| `Page.pageError` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.popup` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1982)) | PageEvent.Popup via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.request` | event | Partial | `Page.subscribe_network` ([source](crates/ferrite-e2e/src/page.rs#L4540)) | NetworkEvent Request/Response/Finished/Failed carries live per-hop handles; legacy Page.subscribe keeps existing payloads. 256-event channel, explicit lag errors and lifecycle/disconnection cancellation; context forwarding retains its existing enum snapshots. |
| `Page.requestFailed` | event | Partial | `Page.subscribe_network` ([source](crates/ferrite-e2e/src/page.rs#L4540)) | NetworkEvent Request/Response/Finished/Failed carries live per-hop handles; legacy Page.subscribe keeps existing payloads. 256-event channel, explicit lag errors and lifecycle/disconnection cancellation; context forwarding retains its existing enum snapshots. |
| `Page.requestFinished` | event | Partial | `Page.subscribe_network` ([source](crates/ferrite-e2e/src/page.rs#L4540)) | NetworkEvent Request/Response/Finished/Failed carries live per-hop handles; legacy Page.subscribe keeps existing payloads. 256-event channel, explicit lag errors and lifecycle/disconnection cancellation; context forwarding retains its existing enum snapshots. |
| `Page.response` | event | Partial | `Page.subscribe_network` ([source](crates/ferrite-e2e/src/page.rs#L4540)) | NetworkEvent Request/Response/Finished/Failed carries live per-hop handles; legacy Page.subscribe keeps existing payloads. 256-event channel, explicit lag errors and lifecycle/disconnection cancellation; context forwarding retains its existing enum snapshots. |
| `Page.webSocket` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1982)) | PageEvent.WebSocket via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.worker` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.addInitScript` | method | Partial | `Page.add_init_script` ([source](crates/ferrite-e2e/src/page.rs#L4711)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.addScriptTag` | method | Partial | `Page.add_script_tag_url` ([source](crates/ferrite-e2e/src/page.rs#L4718)) | URL/content helpers return unit; no ElementHandle return, file/type options; other helper: Page.add_script_tag_content |
| `Page.addStyleTag` | method | Partial | `Page.add_style_tag_url` ([source](crates/ferrite-e2e/src/page.rs#L4752)) | URL/content helpers return unit; no ElementHandle return, file/type options; other helper: Page.add_style_tag_content |
| `Page.bringToFront` | method | Partial | `Page.bring_to_front` ([source](crates/ferrite-e2e/src/page.rs#L2511)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.cancelPickLocator` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.check` | method | Partial | `Locator.check` ([source](crates/ferrite-e2e/src/locator.rs#L2375)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.click` | method | Partial | `Locator.click` ([source](crates/ferrite-e2e/src/locator.rs#L1601)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.close` | method | Partial | `Page.close` ([source](crates/ferrite-e2e/src/page.rs#L5464)) | Closes the target and its owning convenience context through once-only background disposal; dropped waits do not abandon it and repeated calls await its result. No runBeforeUnload/reason options. |
| `Page.content` | method | Equivalent | `Page.content` ([source](crates/ferrite-e2e/src/page.rs#L2480)) | Basic document access. |
| `Page.context` | method | Partial | `Page.context` ([source](crates/ferrite-e2e/src/page.rs#L1929)) | Owning context while registered; returns Option and becomes None after context disposal. |
| `Page.coverage` | property | Partial | `Page.coverage` ([source](crates/ferrite-e2e/src/page.rs#L1899)) | Dedicated API exists; coverage is Chromium-only, lazy frame locators are same-origin, exceptions have ConsoleMessage shape. |
| `Page.dblclick` | method | Partial | `Locator.dblclick` ([source](crates/ferrite-e2e/src/locator.rs#L1748)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.dispatchEvent` | method | Partial | `Locator.dispatch_event` ([source](crates/ferrite-e2e/src/locator.rs#L1987)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.dragAndDrop` | method | Partial | `Locator.drag_to` ([source](crates/ferrite-e2e/src/locator.rs#L1893)) | Page.locator(source).drag_to(target, steps); fewer options and frame-coordinate restrictions. |
| `Page.emulateMedia` | method | Partial | `Page.emulate_media` ([source](crates/ferrite-e2e/src/page.rs#L4149)) | Chromium only; color scheme/reduced motion only, no full media/forcedColors/contrast surface. |
| `Page.evalOnSelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.evalOnSelectorAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.evaluate` | method | Partial | `Page.evaluate_with_arg` ([source](crates/ferrite-e2e/src/page.rs#L2518)) | JSON-serializable arguments and results; no JSHandle argument or arbitrary JS value serialization. |
| `Page.evaluateHandle` | method | Partial | `Page.evaluate_handle` ([source](crates/ferrite-e2e/src/page.rs#L2583)) | Remote JSHandle supported; no ElementHandle conversion or separate evaluation argument. |
| `Page.exposeBinding` | method | Partial | `Page.expose_binding` ([source](crates/ferrite-e2e/src/callbacks.rs#L229)) | Async JSON binding with owning context/page/native frame identity. Same-origin frame dispatch; native startup preloads and navigation/disposal cleanup. Cross-origin/OOPIF callers and handle arguments deferred. |
| `Page.exposeFunction` | method | Partial | `Page.expose_function_async` ([source](crates/ferrite-e2e/src/callbacks.rs#L217)) | Sync/async JSON callbacks in current/future same-origin documents; independent bounded dispatch, native preload ownership, duplicate-name errors and named removal. Rust errors/panics reject JS promises; cross-origin dispatch/handle arguments deferred. |
| `Page.fill` | method | Partial | `Locator.fill` ([source](crates/ferrite-e2e/src/locator.rs#L2151)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.focus` | method | Partial | `Locator.focus` ([source](crates/ferrite-e2e/src/locator.rs#L1824)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.frame` | method | Partial | `Page.frame_by_url_matching` ([source](crates/ferrite-e2e/src/page.rs#L4820)) | Exact/contains/glob/regex or URL predicate snapshot lookup, plus existing name/substring helpers; relative matchers resolve base URL. Native Firefox frame names remain empty. |
| `Page.frameLocator` | method | Partial | `Page.frame_locator` ([source](crates/ferrite-e2e/src/page.rs#L1887)) | Dedicated API exists; coverage is Chromium-only, lazy frame locators are same-origin, exceptions have ConsoleMessage shape. |
| `Page.frames` | method | Partial | `Page.document_frames` ([source](crates/ferrite-e2e/src/page.rs#L4788)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.getAttribute` | method | Partial | `Locator.attribute` ([source](crates/ferrite-e2e/src/locator.rs#L2613)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.getByAltText` | method | Partial | `Page.get_by_alt` ([source](crates/ferrite-e2e/src/page.rs#L2897)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByLabel` | method | Partial | `Page.get_by_label` ([source](crates/ferrite-e2e/src/page.rs#L2885)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByPlaceholder` | method | Partial | `Page.get_by_placeholder` ([source](crates/ferrite-e2e/src/page.rs#L2891)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByRole` | method | Partial | `Page.get_by_role_with` ([source](crates/ferrite-e2e/src/page.rs#L2873)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByTestId` | method | Partial | `Page.get_by_test_id` ([source](crates/ferrite-e2e/src/page.rs#L2855)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByText` | method | Partial | `Page.get_by_text` ([source](crates/ferrite-e2e/src/page.rs#L2861)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByTitle` | method | Partial | `Page.get_by_title` ([source](crates/ferrite-e2e/src/page.rs#L2903)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.goBack` | method | Partial | `Page.go_back` ([source](crates/ferrite-e2e/src/page.rs#L2446)) | Returns unit, not a navigation Response; fewer navigation options. Relative URLs resolve through the configured base URL. |
| `Page.goForward` | method | Partial | `Page.go_forward` ([source](crates/ferrite-e2e/src/page.rs#L2456)) | Returns unit, not a navigation Response; fewer navigation options. Relative URLs resolve through the configured base URL. |
| `Page.requestGC` | method | Partial | `Page.request_gc` ([source](crates/ferrite-e2e/src/page.rs#L3090)) | Chromium only; Firefox returns an unsupported error. |
| `Page.goto` | method | Partial | `Page.goto_with_options` ([source](crates/ferrite-e2e/src/page.rs#L2351)) | Returns unit, not a navigation Response; fewer navigation options. Relative URLs resolve through the configured base URL. |
| `Page.hideHighlight` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.hover` | method | Partial | `Locator.hover` ([source](crates/ferrite-e2e/src/locator.rs#L1774)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.innerHTML` | method | Partial | `Locator.inner_html` ([source](crates/ferrite-e2e/src/locator.rs#L2687)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.innerText` | method | Partial | `Locator.inner_text` ([source](crates/ferrite-e2e/src/locator.rs#L1002)) | Page.locator(selector) followed by the distinct rendered/raw text getter; strict resolution and same-origin frame limitations remain. |
| `Page.inputValue` | method | Partial | `Locator.input_value` ([source](crates/ferrite-e2e/src/locator.rs#L2561)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isChecked` | method | Partial | `Locator.is_checked` ([source](crates/ferrite-e2e/src/locator.rs#L2878)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isClosed` | method | Partial | `Page.is_closed` ([source](crates/ferrite-e2e/src/page.rs#L2113)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.isDisabled` | method | Partial | `Locator.is_disabled` ([source](crates/ferrite-e2e/src/locator.rs#L2861)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isEditable` | method | Partial | `Locator.is_editable` ([source](crates/ferrite-e2e/src/locator.rs#L2895)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isEnabled` | method | Partial | `Locator.is_enabled` ([source](crates/ferrite-e2e/src/locator.rs#L2844)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isHidden` | method | Partial | `Locator.is_hidden` ([source](crates/ferrite-e2e/src/locator.rs#L2824)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isVisible` | method | Partial | `Locator.is_visible` ([source](crates/ferrite-e2e/src/locator.rs#L2807)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.keyboard` | property | Partial | `Page.press_key` ([source](crates/ferrite-e2e/src/page.rs#L3146)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.clearConsoleMessages` | method | Partial | `Page.clear_console_messages` ([source](crates/ferrite-e2e/src/page.rs#L1947)) | Dedicated API exists; coverage is Chromium-only, lazy frame locators are same-origin, exceptions have ConsoleMessage shape. |
| `Page.clearPageErrors` | method | Partial | `Page.clear_page_errors` ([source](crates/ferrite-e2e/src/page.rs#L1964)) | Dedicated API exists; coverage is Chromium-only, lazy frame locators are same-origin, exceptions have ConsoleMessage shape. |
| `Page.localStorage` | property | Partial | `Page.local_storage_get` ([source](crates/ferrite-e2e/src/page.rs#L3416)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.sessionStorage` | property | Partial | `Page.session_storage_get` ([source](crates/ferrite-e2e/src/page.rs#L3453)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.consoleMessages` | method | Partial | `Page.console_messages` ([source](crates/ferrite-e2e/src/page.rs#L2298)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.pageErrors` | method | Partial | `Page.page_errors` ([source](crates/ferrite-e2e/src/page.rs#L1956)) | Dedicated API exists; coverage is Chromium-only, lazy frame locators are same-origin, exceptions have ConsoleMessage shape. |
| `Page.locator` | method | Partial | `Page.locator` ([source](crates/ferrite-e2e/src/page.rs#L2849)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.mainFrame` | method | Partial | `Page.main_frame` ([source](crates/ferrite-e2e/src/page.rs#L4810)) | Dedicated asynchronous native root lookup; closed/disconnected pages fail, no fabricated root. Selector-free OOPIF traversal remains deferred. |
| `Page.mouse` | property | Partial | `Page.mouse_click` ([source](crates/ferrite-e2e/src/page.rs#L3097)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.opener` | method | Partial | `Page.opener` ([source](crates/ferrite-e2e/src/page.rs#L2160)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.pause` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.pdf` | method | Partial | `Page.pdf` ([source](crates/ferrite-e2e/src/page.rs#L3607)) | PDF export exists; no PDF options builder. Engine behavior differs. |
| `Page.pickLocator` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.press` | method | Partial | `Locator.press` ([source](crates/ferrite-e2e/src/locator.rs#L2191)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.querySelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.querySelectorAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.requests` | method | Partial | `Page.network_requests` ([source](crates/ferrite-e2e/src/page.rs#L4528)) | Typed metadata observed since page creation, capped at 4,096 hops/16 MiB; legacy Page.requests remains capture-based RecordedRequest snapshots. Eviction/truncation is explicit and bodies are separate. |
| `Page.addLocatorHandler` | method | Partial | `Page.add_locator_handler_with` ([source](crates/ferrite-e2e/src/page.rs#L2932)) | Visibility-based overlay handlers run before actions and state/custom assertions; no full dismissal/noWaitAfter semantics. |
| `Page.removeAllListeners` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.removeLocatorHandler` | method | Partial | `Page.remove_locator_handler` ([source](crates/ferrite-e2e/src/page.rs#L2954)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.reload` | method | Partial | `Page.reload` ([source](crates/ferrite-e2e/src/page.rs#L2438)) | Returns unit, not a navigation Response; fewer navigation options. Relative URLs resolve through the configured base URL. |
| `Page.request` | property | Partial | `Page.request` ([source](crates/ferrite-e2e/src/page.rs#L1940)) | HTTP client sharing owning-context cookies and inheriting its transport defaults; cancellation follows context disposal. |
| `Page.route` | method | Partial | `Page.route_matching` ([source](crates/ferrite-e2e/src/page.rs#L4210)) | Shared resolved UrlMatcher for rules/handlers/HAR filters, with match limits and identity-based removal; legacy string/globset contracts preserved. Invalid patterns fail before registration, including empty contexts; native response/URL override differences remain. In-flight removal and fuller HAR policies tracked separately. |
| `Page.routeFromHAR` | method | Partial | `Page.route_from_har` ([source](crates/ferrite-e2e/src/page.rs#L4396)) | Shared resolved UrlMatcher for rules/handlers/HAR filters, with match limits and identity-based removal; legacy string/globset contracts preserved. Invalid patterns fail before registration, including empty contexts; native response/URL override differences remain. In-flight removal and fuller HAR policies tracked separately. |
| `Page.routeWebSocket` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.screencast` | property | Partial | `Page.frames` ([source](crates/ferrite-e2e/src/page.rs#L5417)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.screenshot` | method | Partial | `Page.screenshot` ([source](crates/ferrite-e2e/src/page.rs#L3573)) | Validated viewport/document clips, full-page masks/colors and temporary styles, Device/Css output and shared timeout. Chromium transparent PNG and retained metrics/background; Firefox Css raster normalization and explicit transparency unavailability. Owned restoration/cancellation and visible cleanup errors; native finite finish/infinite cancel-resume, zero-rate preservation and late CSS listeners. Application-realm helpers, formats and root traversal remain narrower. |
| `Page.selectOption` | method | Partial | `Locator.select_options` ([source](crates/ferrite-e2e/src/locator.rs#L2481)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.setChecked` | method | Partial | `Locator.set_checked` ([source](crates/ferrite-e2e/src/locator.rs#L2988)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.setContent` | method | Partial | `Page.set_content` ([source](crates/ferrite-e2e/src/page.rs#L2490)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.setDefaultNavigationTimeout` | method | Partial | `Page.set_navigation_timeout` ([source](crates/ferrite-e2e/src/page.rs#L2230)) | Navigation default distinct from locator timeout. |
| `Page.setDefaultTimeout` | method | Partial | `Page.set_timeout` ([source](crates/ferrite-e2e/src/page.rs#L2221)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.setExtraHTTPHeaders` | method | Partial | `Page.set_extra_http_headers` ([source](crates/ferrite-e2e/src/page.rs#L4079)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.setInputFiles` | method | Partial | `Page.set_input_file_payloads` ([source](crates/ferrite-e2e/src/page.rs#L4681)) | FilePayload filename/MIME/bytes or existing path uploads; empty lists clear, multiple files require a multiple input, 64 MiB total cap. DOM File/DataTransfer injection on both engines; no native chooser/directory upload/options parity. Frame uses Frame.locator. |
| `Page.setViewportSize` | method | Partial | `Page.set_viewport` ([source](crates/ferrite-e2e/src/page.rs#L3614)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.ariaSnapshot` | method | Partial | `Page.aria_snapshot_with` ([source](crates/ferrite-e2e/src/page.rs#L5004)) | Opt-in bounded DOM approximation: role depth, rounded frame-local boxes, optional supported state, explicit traversal/name budgets and deterministic safety markers. Legacy methods retain unbounded output. Native name/layout work is not preemptible; text fragments, full accessible names, AI modes and YAML syntax differ. |
| `Page.ariaSnapshotJSON` | method | Partial | `Page.aria_snapshot_json_with` ([source](crates/ferrite-e2e/src/page.rs#L4983)) | Opt-in bounded DOM approximation: role depth, rounded frame-local boxes, optional supported state, explicit traversal/name budgets and deterministic safety markers. Legacy methods retain unbounded output. Native name/layout work is not preemptible; text fragments, full accessible names, AI modes and YAML syntax differ. |
| `Page.tap` | method | Partial | `Locator.tap` ([source](crates/ferrite-e2e/src/locator.rs#L1864)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.textContent` | method | Partial | `Locator.text_content` ([source](crates/ferrite-e2e/src/locator.rs#L979)) | Page.locator(selector) followed by the distinct rendered/raw text getter; strict resolution and same-origin frame limitations remain. |
| `Page.title` | method | Equivalent | `Page.title` ([source](crates/ferrite-e2e/src/page.rs#L2466)) | Basic document access. |
| `Page.touchscreen` | property | Partial | `Page.touchscreen_tap` ([source](crates/ferrite-e2e/src/page.rs#L3237)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.type` (deprecated) | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L2231)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.uncheck` | method | Partial | `Locator.uncheck` ([source](crates/ferrite-e2e/src/locator.rs#L2394)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.unrouteAll` | method | Partial | `Page.unroute_all_with` ([source](crates/ferrite-e2e/src/page.rs#L4319)) | Pattern/shared-matcher/all removal with default/wait/ignore-errors and explicit Rust Cancel. Default/ignore-errors release requests while callbacks settle and discard late decisions; wait preserves native decisions within shared deadlines. Independent bounded dispatch, atomic invocation limits including fallback, disposal/retry/disconnect cleanup verified on Chromium/Firefox. Rust first-registration/page priority and no callback-identity removal remain differences. |
| `Page.unroute` | method | Partial | `Page.unroute_matching_with` ([source](crates/ferrite-e2e/src/page.rs#L4327)) | Pattern/shared-matcher/all removal with default/wait/ignore-errors and explicit Rust Cancel. Default/ignore-errors release requests while callbacks settle and discard late decisions; wait preserves native decisions within shared deadlines. Independent bounded dispatch, atomic invocation limits including fallback, disposal/retry/disconnect cleanup verified on Chromium/Firefox. Rust first-registration/page priority and no callback-identity removal remain differences. |
| `Page.url` | method | Equivalent | `Page.url` ([source](crates/ferrite-e2e/src/page.rs#L2473)) | Basic document access. |
| `Page.video` | method | Partial | `Page.start_video` ([source](crates/ferrite-e2e/src/page.rs#L5344)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.viewportSize` | method | Partial | `Page.viewport_size` ([source](crates/ferrite-e2e/src/page.rs#L3075)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.waitForEvent` | method | Partial | `Page.wait_for_event` ([source](crates/ferrite-e2e/src/page.rs#L1991)) | Enum-based page observations, including native frame/document readiness and dialog closure; shared deadlines/cancellation/disposal/transport loss. New Firefox events are capability-probed, missing navigation/closure support fails explicitly. Closure payloads are native snapshots rather than live Dialog objects; callback listener parity remains narrower. |
| `Page.waitForFunction` | method | Partial | `Page.wait_for_function_handle` ([source](crates/ferrite-e2e/src/page.rs#L2659)) | JSON argument, native animation-frame/interval polling and retained truthy result; JSON helper supports frames. Frame remote handles and arbitrary argument serialization remain unsupported. Legacy expression helper returns unit. |
| `Page.waitForLoadState` | method | Partial | `Page.wait_for_load_state` ([source](crates/ferrite-e2e/src/page.rs#L2830)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.waitForNavigation` (deprecated) | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.waitForRequest` | method | Partial | `Page.wait_for_request_handle` ([source](crates/ferrite-e2e/src/page.rs#L4553)) | Shared exact/base URL/glob/regex matchers and typed predicate companions, returning live per-hop Request/Response at start/headers. Existing snapshot and async predicate waits remain available. Cancellation/lag/disposal handled; no body capture or URLPattern implied. |
| `Page.waitForResponse` | method | Partial | `Page.wait_for_response_handle` ([source](crates/ferrite-e2e/src/page.rs#L4598)) | Shared exact/base URL/glob/regex matchers and typed predicate companions, returning live per-hop Request/Response at start/headers. Existing snapshot and async predicate waits remain available. Cancellation/lag/disposal handled; no body capture or URLPattern implied. |
| `Page.waitForSelector` | method | Partial | `Page.wait_for_selector_with` ([source](crates/ferrite-e2e/src/page.rs#L2131)) | Waits for requested state and returns Locator, not ElementHandle. |
| `Page.waitForTimeout` | method | Partial | `Page.wait_for_timeout` ([source](crates/ferrite-e2e/src/page.rs#L2683)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.waitForURL` | method | Partial | `Page.wait_for_url_matching_with_options` ([source](crates/ferrite-e2e/src/page.rs#L2734)) | Exact (relative to base URL), full-URL glob/regex or predicate with Commit/DOMContentLoaded/Load readiness in one navigation budget. Page NetworkIdle uses 500ms observed HTTP quiet; frames reject it. Legacy helpers retain substring/URL-only semantics. Zero timeout, cancellation and enclosing budgets supported; URLPattern absent. |
| `Page.workers` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## PageAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-pageassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `PageAssertions.not` | property | Partial | `PageExpect.not` ([source](crates/ferrite-e2e/src/expect.rs#L395)) | Retrying page assertion; exact/regex title and URL helpers, with narrower options and different screenshot/ARIA algorithms. |
| `PageAssertions.toMatchAriaSnapshot` | method | Partial | `PageExpect.aria_snapshot_with` ([source](crates/ferrite-e2e/src/expect.rs#L658)) | Retrying exact Ferrite text with bounded capture options; DOM approximation and truncation markers, no upstream YAML patterns or AI modes. |
| `PageAssertions.toHaveScreenshot` | method | Partial | `PageExpect.screenshot_with` ([source](crates/ferrite-e2e/src/expect.rs#L620)) | Successive stable PNG captures within one assertion window, real downloadable-font readiness, native animation finish/cancel-resume and B09 capture options and typed operation errors. Missing/all/changed generation requires stability; negation needs a valid baseline. Runner/project/explicit path templates; attempt/step-owned expected/actual/diff copies, last-pair stability diagnostics, final-only completed outer-poll images and portable bundles. Comparison/read/Css/diff/staging jobs have two active slots and cooperative cancellation, 64-million-pixel decoding and 512-MiB encoded-input limits. Frozen expected bytes, staged foreground baseline replacement, non-overwriting Missing installation and preserved symlink aliases. Queued memory and opaque OS/codec phases are not hard bounded. Final failure diagnostics share one additional five-second budget under enclosing cancellation/deadlines, with staged non-overwriting foreground attachment publication and a 1024-name collision cap. Per-channel comparison, visual diff, update/default semantics and retained single-mismatch probe semantics differ. |
| `PageAssertions.toHaveTitle` | method | Partial | `PageExpect.title` ([source](crates/ferrite-e2e/src/expect.rs#L484)) | Retrying page assertion; exact/regex title and URL helpers, with narrower options and different screenshot/ARIA algorithms. |
| `PageAssertions.toHaveURL` | method | Partial | `PageExpect.url_matching` ([source](crates/ferrite-e2e/src/expect.rs#L424)) | Shared exact/base URL, glob, Rust regex or url_where predicate; retrying negation and cancellation. Legacy assertion helpers retain string contracts; explicit globs are a Rust extension, and URLPattern/case option parity remains absent. |

## Playwright

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-playwright.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Playwright.chromium` | property | Partial | `Browser.launch` ([source](crates/ferrite-e2e/src/browser.rs#L444)) | Select BrowserKind with LaunchOptions; no BrowserType object or bundled browser installer. |
| `Playwright.devices` | property | Partial | `DeviceDescriptor` ([source](crates/ferrite-e2e/src/page.rs#L785)) | Seven metrics presets; no full device catalog or device user-agent metadata. |
| `Playwright.errors` | property | Idiomatic | `E2eError` ([source](crates/ferrite-e2e/src/error.rs#L8)) | Rust error enum, with different variants and diagnostics. |
| `Playwright.firefox` | property | Partial | `Browser.launch` ([source](crates/ferrite-e2e/src/browser.rs#L444)) | Select BrowserKind with LaunchOptions; no BrowserType object or bundled browser installer. |
| `Playwright.request` | property | Partial | `ApiClient` ([source](crates/ferrite-e2e/src/api.rs#L172)) | Standalone or context-linked HTTP client; smaller APIRequest option surface. |
| `Playwright.selectors` | property | Partial | `set_test_id_attribute` ([source](crates/ferrite-e2e/src/locator.rs#L21)) | Only test-id configuration; no custom selector registration. |
| `Playwright.webkit` | property | Missing | — | No WebKit backend; BrowserKind contains Chromium and Firefox only. |

## PlaywrightAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-playwrightassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `PlaywrightAssertions.expectAPIResponse` | method | Idiomatic | `ApiResponse.ok` ([source](crates/ferrite-e2e/src/api.rs#L49)) | Use native Rust assertions over the standalone response. |
| `PlaywrightAssertions.expectGeneric` | method | Idiomatic | — | Use native Rust assertions and explicit pattern/container checks; no Playwright expect matcher library. |
| `PlaywrightAssertions.expectLocator` | method | Partial | `Locator.expect` ([source](crates/ferrite-e2e/src/expect.rs#L1882)) | Rust assertion builders with attempt-owned TestInfo.soft_asserts / AttemptSoftAsserts.run contextual steps and automatic mismatch failure. Operational/control errors propagate; no custom matcher registry or full expect configuration. |
| `PlaywrightAssertions.expectPage` | method | Partial | `Page.expect` ([source](crates/ferrite-e2e/src/expect.rs#L1864)) | Rust assertion builders with attempt-owned TestInfo.soft_asserts / AttemptSoftAsserts.run contextual steps and automatic mismatch failure. Operational/control errors propagate; no custom matcher registry or full expect configuration. |

## Reporter

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-reporter.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Reporter.onBegin` | method | Partial | `Reporter.on_begin` ([source](crates/ferrite-e2e/src/report.rs#L292)) | Live synchronous thread-safe callbacks; attempt identity includes retry/project/repetition/worker, errors and interrupted user steps reported. No Suite/TestCase graph, asynchronous end/status override or worker stdout capture. |
| `Reporter.onEnd` | method | Partial | `Reporter.on_end` ([source](crates/ferrite-e2e/src/report.rs#L315)) | Live synchronous callbacks retain attempt-specific steps, console and optional bounded data-only network summaries. Aggregate reports add inline search/status/project filters and pagination with portable artifact links. No Suite/TestCase graph, asynchronous end/status override, worker stdout or Trace Viewer archive. |
| `Reporter.onError` | method | Partial | `Reporter.on_error` ([source](crates/ferrite-e2e/src/report.rs#L311)) | Live synchronous thread-safe callbacks; attempt identity includes retry/project/repetition/worker, errors and interrupted user steps reported. No Suite/TestCase graph, asynchronous end/status override or worker stdout capture. |
| `Reporter.onExit` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Reporter.onStdErr` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Reporter.onStdOut` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Reporter.onStepBegin` | method | Partial | `Reporter.on_step_begin` ([source](crates/ferrite-e2e/src/report.rs#L307)) | Live synchronous thread-safe callbacks; attempt identity includes retry/project/repetition/worker, errors and interrupted user steps reported. No Suite/TestCase graph, asynchronous end/status override or worker stdout capture. |
| `Reporter.onStepEnd` | method | Partial | `Reporter.on_step_end` ([source](crates/ferrite-e2e/src/report.rs#L308)) | Live synchronous thread-safe callbacks; attempt identity includes retry/project/repetition/worker, errors and interrupted user steps reported. No Suite/TestCase graph, asynchronous end/status override or worker stdout capture. |
| `Reporter.onTestBegin` | method | Partial | `Reporter.on_test_begin` ([source](crates/ferrite-e2e/src/report.rs#L304)) | Live synchronous thread-safe callbacks; attempt identity includes retry/project/repetition/worker, errors and interrupted user steps reported. No Suite/TestCase graph, asynchronous end/status override or worker stdout capture. |
| `Reporter.onTestEnd` | method | Partial | `Reporter.on_test_end` ([source](crates/ferrite-e2e/src/report.rs#L306)) | Live synchronous callbacks retain attempt-specific steps, console and optional bounded data-only network summaries. Aggregate reports add inline search/status/project filters and pagination with portable artifact links. No Suite/TestCase graph, asynchronous end/status override, worker stdout or Trace Viewer archive. |
| `Reporter.printsToStdio` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Reporter.preprocess` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |

## Request

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-request.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Request.allHeaders` | method | Partial | `Request.headers_array` ([source](crates/ferrite-e2e/src/network.rs#L846)) | Duplicate-preserving pairs and case-insensitive lookup; sync live getters rather than upstream await-for-all map APIs. Native extra events correlate by hop, with explicit completeness/truncation. Successfully acknowledged route-supplied pairs have an explicit source flag when events omit/fold them; same-URL hop identity preserved. Generic Firefox comma folding is not reversed. |
| `Request.failure` | method | Partial | `Request.failure` ([source](crates/ferrite-e2e/src/network.rs#L913)) | Live per-redirect-hop observation; native metadata remains optional, Firefox POST text/resource destination may be absent, and frame lookup is async/current-tree based. Redirect handles use bounded weak history; snapshot keeps IDs/truncation flags. JSON/form parsing is dedicated; no worker/OOPIF object graph. |
| `Request.frame` | method | Partial | `Request.frame` ([source](crates/ferrite-e2e/src/network.rs#L835)) | Live per-redirect-hop observation; native metadata remains optional, Firefox POST text/resource destination may be absent, and frame lookup is async/current-tree based. Redirect handles use bounded weak history; snapshot keeps IDs/truncation flags. JSON/form parsing is dedicated; no worker/OOPIF object graph. |
| `Request.headers` | method | Partial | `Request.headers_array` ([source](crates/ferrite-e2e/src/network.rs#L846)) | Duplicate-preserving pairs and case-insensitive lookup; sync live getters rather than upstream await-for-all map APIs. Native extra events correlate by hop, with explicit completeness/truncation. Successfully acknowledged route-supplied pairs have an explicit source flag when events omit/fold them; same-URL hop identity preserved. Generic Firefox comma folding is not reversed. |
| `Request.headersArray` | method | Partial | `Request.headers_array` ([source](crates/ferrite-e2e/src/network.rs#L846)) | Duplicate-preserving pairs and case-insensitive lookup; sync live getters rather than upstream await-for-all map APIs. Native extra events correlate by hop, with explicit completeness/truncation. Successfully acknowledged route-supplied pairs have an explicit source flag when events omit/fold them; same-URL hop identity preserved. Generic Firefox comma folding is not reversed. |
| `Request.headerValue` | method | Partial | `Request.header_value` ([source](crates/ferrite-e2e/src/network.rs#L869)) | Duplicate-preserving pairs and case-insensitive lookup; sync live getters rather than upstream await-for-all map APIs. Native extra events correlate by hop, with explicit completeness/truncation. Successfully acknowledged route-supplied pairs have an explicit source flag when events omit/fold them; same-URL hop identity preserved. Generic Firefox comma folding is not reversed. |
| `Request.isNavigationRequest` | method | Partial | `Request.is_navigation_request` ([source](crates/ferrite-e2e/src/network.rs#L829)) | Live per-redirect-hop observation; native metadata remains optional, Firefox POST text/resource destination may be absent, and frame lookup is async/current-tree based. Redirect handles use bounded weak history; snapshot keeps IDs/truncation flags. JSON/form parsing is dedicated; no worker/OOPIF object graph. |
| `Request.method` | method | Partial | `Request.method` ([source](crates/ferrite-e2e/src/network.rs#L811)) | Live per-redirect-hop observation; native metadata remains optional, Firefox POST text/resource destination may be absent, and frame lookup is async/current-tree based. Redirect handles use bounded weak history; snapshot keeps IDs/truncation flags. JSON/form parsing is dedicated; no worker/OOPIF object graph. |
| `Request.postData` | method | Partial | `Request.post_data` ([source](crates/ferrite-e2e/src/network.rs#L881)) | Live per-redirect-hop observation; native metadata remains optional, Firefox POST text/resource destination may be absent, and frame lookup is async/current-tree based. Redirect handles use bounded weak history; snapshot keeps IDs/truncation flags. JSON/form parsing is dedicated; no worker/OOPIF object graph. |
| `Request.postDataBuffer` | method | Partial | `RecordedRequest.post_data` ([source](crates/ferrite-e2e/src/page.rs#L181)) | Text capture only; no lossless binary request body API. |
| `Request.postDataJSON` | method | Partial | `Request.post_data_json` ([source](crates/ferrite-e2e/src/network.rs#L890)) | Live per-redirect-hop observation; native metadata remains optional, Firefox POST text/resource destination may be absent, and frame lookup is async/current-tree based. Redirect handles use bounded weak history; snapshot keeps IDs/truncation flags. JSON/form parsing is dedicated; no worker/OOPIF object graph. |
| `Request.redirectedFrom` | method | Partial | `Request.redirected_from` ([source](crates/ferrite-e2e/src/network.rs#L922)) | Live per-redirect-hop observation; native metadata remains optional, Firefox POST text/resource destination may be absent, and frame lookup is async/current-tree based. Redirect handles use bounded weak history; snapshot keeps IDs/truncation flags. JSON/form parsing is dedicated; no worker/OOPIF object graph. |
| `Request.redirectedTo` | method | Partial | `Request.redirected_to` ([source](crates/ferrite-e2e/src/network.rs#L929)) | Live per-redirect-hop observation; native metadata remains optional, Firefox POST text/resource destination may be absent, and frame lookup is async/current-tree based. Redirect handles use bounded weak history; snapshot keeps IDs/truncation flags. JSON/form parsing is dedicated; no worker/OOPIF object graph. |
| `Request.resourceType` | method | Partial | `Request.resource_type` ([source](crates/ferrite-e2e/src/network.rs#L826)) | Live per-redirect-hop observation; native metadata remains optional, Firefox POST text/resource destination may be absent, and frame lookup is async/current-tree based. Redirect handles use bounded weak history; snapshot keeps IDs/truncation flags. JSON/form parsing is dedicated; no worker/OOPIF object graph. |
| `Request.response` | method | Partial | `Request.response` ([source](crates/ferrite-e2e/src/network.rs#L939)) | Returns an existing optional Response after headers; does not await future headers. Failed pre-header requests never fabricate a response. Use Page.wait_for_response_handle for a header wait. |
| `Request.existingResponse` | method | Partial | `Request.response` ([source](crates/ferrite-e2e/src/network.rs#L939)) | Existing optional Response after native headers; no full worker/OOPIF graph. |
| `Request.serviceWorker` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.sizes` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.timing` | method | Partial | `RecordedRequest.duration_ms` ([source](crates/ferrite-e2e/src/page.rs#L184)) | Total elapsed time only; no DNS/connect/TLS/response timing breakdown. |
| `Request.url` | method | Partial | `Request.url` ([source](crates/ferrite-e2e/src/network.rs#L814)) | Live per-redirect-hop observation; native metadata remains optional, Firefox POST text/resource destination may be absent, and frame lookup is async/current-tree based. Redirect handles use bounded weak history; snapshot keeps IDs/truncation flags. JSON/form parsing is dedicated; no worker/OOPIF object graph. |

## Response

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-response.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Response.allHeaders` | method | Partial | `Response.headers_array` ([source](crates/ferrite-e2e/src/network.rs#L995)) | Duplicate-preserving pairs and case-insensitive lookup; sync live getters rather than upstream await-for-all map APIs. Native extra events correlate by hop, with explicit completeness/truncation. Successfully acknowledged route-supplied pairs have an explicit source flag when events omit/fold them; same-URL hop identity preserved. Generic Firefox comma folding is not reversed. |
| `Response.body` | method | Partial | `RecordedRequest.body` ([source](crates/ferrite-e2e/src/page.rs#L205)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.finished` | method | Partial | `Response.finished_with_options` ([source](crates/ferrite-e2e/src/network.rs#L1036)) | Native completion independent of body capture, including Firefox; also settles associated fulfillment acknowledgements. HTTP errors finish successfully; transport failures return E2eError::Network. Shared defaults, zero/enclosing deadlines, cancellation, disposal and disconnect bounded; already terminal metadata survives page close. |
| `Response.frame` | method | Partial | `Response.frame` ([source](crates/ferrite-e2e/src/network.rs#L970)) | Typed native response metadata with owning request/page; headers may be natively folded, frame is async optional lookup, and fields remain bounded. Response bodies stay separate in legacy capture; no worker/security/full timing graph. |
| `Response.fromServiceWorker` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.headers` | method | Partial | `Response.headers_array` ([source](crates/ferrite-e2e/src/network.rs#L995)) | Duplicate-preserving pairs and case-insensitive lookup; sync live getters rather than upstream await-for-all map APIs. Native extra events correlate by hop, with explicit completeness/truncation. Successfully acknowledged route-supplied pairs have an explicit source flag when events omit/fold them; same-URL hop identity preserved. Generic Firefox comma folding is not reversed. |
| `Response.headersArray` | method | Partial | `Response.headers_array` ([source](crates/ferrite-e2e/src/network.rs#L995)) | Duplicate-preserving pairs and case-insensitive lookup; sync live getters rather than upstream await-for-all map APIs. Native extra events correlate by hop, with explicit completeness/truncation. Successfully acknowledged route-supplied pairs have an explicit source flag when events omit/fold them; same-URL hop identity preserved. Generic Firefox comma folding is not reversed. |
| `Response.headerValue` | method | Partial | `Response.header_value` ([source](crates/ferrite-e2e/src/network.rs#L1018)) | Duplicate-preserving pairs and case-insensitive lookup; sync live getters rather than upstream await-for-all map APIs. Native extra events correlate by hop, with explicit completeness/truncation. Successfully acknowledged route-supplied pairs have an explicit source flag when events omit/fold them; same-URL hop identity preserved. Generic Firefox comma folding is not reversed. |
| `Response.headerValues` | method | Partial | `Response.header_values` ([source](crates/ferrite-e2e/src/network.rs#L1006)) | Typed native response metadata with owning request/page; headers may be natively folded, frame is async optional lookup, and fields remain bounded. Response bodies stay separate in legacy capture; no worker/security/full timing graph. |
| `Response.httpVersion` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.json` | method | Partial | `RecordedRequest.body_json` ([source](crates/ferrite-e2e/src/page.rs#L222)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.ok` | method | Partial | `Response.ok` ([source](crates/ferrite-e2e/src/network.rs#L992)) | Typed native response metadata with owning request/page; headers may be natively folded, frame is async optional lookup, and fields remain bounded. Response bodies stay separate in legacy capture; no worker/security/full timing graph. |
| `Response.request` | method | Partial | `Response.request` ([source](crates/ferrite-e2e/src/network.rs#L961)) | Typed native response metadata with owning request/page; headers may be natively folded, frame is async optional lookup, and fields remain bounded. Response bodies stay separate in legacy capture; no worker/security/full timing graph. |
| `Response.securityDetails` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.serverAddr` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.status` | method | Partial | `Response.status` ([source](crates/ferrite-e2e/src/network.rs#L973)) | Typed native response metadata with owning request/page; headers may be natively folded, frame is async optional lookup, and fields remain bounded. Response bodies stay separate in legacy capture; no worker/security/full timing graph. |
| `Response.statusText` | method | Partial | `Response.status_text` ([source](crates/ferrite-e2e/src/network.rs#L982)) | Typed native response metadata with owning request/page; headers may be natively folded, frame is async optional lookup, and fields remain bounded. Response bodies stay separate in legacy capture; no worker/security/full timing graph. |
| `Response.text` | method | Partial | `RecordedRequest.body_text` ([source](crates/ferrite-e2e/src/page.rs#L214)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.url` | method | Partial | `Response.url` ([source](crates/ferrite-e2e/src/network.rs#L964)) | Typed native response metadata with owning request/page; headers may be natively folded, frame is async optional lookup, and fields remain bounded. Response bodies stay separate in legacy capture; no worker/security/full timing graph. |

## Route

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-route.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Route.abort` | method | Partial | `RouteAction.abort_with` ([source](crates/ferrite-e2e/src/page.rs#L1237)) | Rule/action model with returned decisions; native intercepted-request URL rewriting and response-stage modification are Chromium-only. Out-of-band HTTP fetch and prepared fulfillment work on both engines. |
| `Route.continue` | method | Partial | `RouteRule.continue_with` ([source](crates/ferrite-e2e/src/page.rs#L1398)) | Rule/action model with returned decisions; native intercepted-request URL rewriting and response-stage modification are Chromium-only. Out-of-band HTTP fetch and prepared fulfillment work on both engines. |
| `Route.fallback` | method | Partial | `RouteAction.fallback` ([source](crates/ferrite-e2e/src/page.rs#L1274)) | Rule/action model with returned decisions; native intercepted-request URL rewriting and response-stage modification are Chromium-only. Out-of-band HTTP fetch and prepared fulfillment work on both engines. |
| `Route.fetch` | method | Partial | `RouteInfo.fetch_with` ([source](crates/ferrite-e2e/src/route_options.rs#L170)) | Context-linked cookies/TLS/proxy/auth, live page deadlines, caller cancellation, method/header/raw/JSON and HTTP(S) URL overrides, redirect limits and reset-only retries. Relative URLs use the context base URL. Firefox original body bytes are unavailable and require an override; Chromium lossless binary capture is bounded. No automatic compression decoding or arbitrary streams. |
| `Route.fulfill` | method | Partial | `RouteInfo.fulfill_with` ([source](crates/ferrite-e2e/src/route_options.rs#L268)) | Owned API response inheritance, status/header/binary/JSON/file overrides, duplicate headers and request-origin CORS preparation. Pinned content-type/file/body/content-length precedence; final status validation, regular-file paths and explicit Rust preparation deadline/cancellation. Returned RouteAction rather than live-route method; legacy static helpers keep their contracts. |
| `Route.request` | method | Partial | `RouteInfo` ([source](crates/ferrite-e2e/src/page.rs#L1126)) | Rule/action model with returned decisions; native intercepted-request URL rewriting and response-stage modification are Chromium-only. Out-of-band HTTP fetch and prepared fulfillment work on both engines. |

## Screencast

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-screencast.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Screencast.start` | method | Partial | `Page.start_video` ([source](crates/ferrite-e2e/src/page.rs#L5344)) | Video recording/live frames available; fewer formats/options, no screencast overlay/action system. |
| `Screencast.stop` | method | Partial | `Page.stop_video` ([source](crates/ferrite-e2e/src/page.rs#L5385)) | Explicit output path; Chromium assembles frames with ffmpeg, Firefox records natively. |
| `Screencast.showOverlay` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Screencast.showChapter` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Screencast.showActions` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Screencast.showOverlays` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Screencast.hideActions` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Screencast.hideOverlays` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## Selectors

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-selectors.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Selectors.register` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Selectors.setTestIdAttribute` | method | Equivalent | `set_test_id_attribute` ([source](crates/ferrite-e2e/src/locator.rs#L21)) | Process-global attribute override. |

## SnapshotAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-snapshotassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `SnapshotAssertions.toMatchSnapshot` | method | Partial | `assert_snapshot_text` ([source](crates/ferrite-e2e/src/snapshot.rs#L459)) | Text and PNG helpers with missing/changed/all/none update modes and explicit path templates/context, including TestInfo.snapshot_options. Matching changed baselines retained; PNG validation and bounded pixels. No source-update modes, arbitrary binary snapshots or full upstream naming/kind overloads. |

## Suite

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-suite.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Suite.allTests` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Suite.entries` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Suite.location` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Suite.parent` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Suite.project` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Suite.suites` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Suite.tests` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Suite.title` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Suite.titlePath` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Suite.type` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |

## Test

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-test.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Test.(call)` | method | Partial | `test` ([source](crates/ferrite-e2e/src/runner.rs#L567)) | Rust test closure; dynamic details/locks/options differ. |
| `Test.afterAll` | method | Partial | `Suite.after_all_with_context` ([source](crates/ferrite-e2e/src/runner.rs#L643)) | ContextHook explicitly declares lazy fixture roots; WorkerHook permits only worker roots. Nested lifecycle ordering; no process workers or callback parameter inference. |
| `Test.afterEach` | method | Partial | `Suite.after_each_with_context` ([source](crates/ferrite-e2e/src/runner.rs#L708)) | ContextHook explicitly declares lazy fixture roots; WorkerHook permits only worker roots. Nested lifecycle ordering; no process workers or callback parameter inference. |
| `Test.beforeAll` | method | Partial | `Suite.before_all_with_context` ([source](crates/ferrite-e2e/src/runner.rs#L638)) | ContextHook explicitly declares lazy fixture roots; WorkerHook permits only worker roots. Nested lifecycle ordering; no process workers or callback parameter inference. |
| `Test.beforeEach` | method | Partial | `Suite.before_each_with_context` ([source](crates/ferrite-e2e/src/runner.rs#L703)) | ContextHook explicitly declares lazy fixture roots; WorkerHook permits only worker roots. Nested lifecycle ordering; no process workers or callback parameter inference. |
| `Test.describe` | method | Partial | `Suite.tests` ([source](crates/ferrite-e2e/src/runner.rs#L743)) | Nested identity, scoped hooks, inherited timeout/retry/context/tags; no serial/fully-parallel suite scheduling configuration. |
| `Test.describe.configure` | method | Partial | `Suite.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L655)) | Suite timeout/retry/context settings inherited by descendants; no serial/fully-parallel execution mode configuration. |
| `Test.describe.fixme` | method | Partial | `Suite.fixme` ([source](crates/ferrite-e2e/src/runner.rs#L675)) | Applies focus/skip/fixme to descendants; Rust builders, no JavaScript describe callbacks. |
| `Test.describe.only` | method | Partial | `Suite.only` ([source](crates/ferrite-e2e/src/runner.rs#L680)) | Applies focus/skip/fixme to descendants; Rust builders, no JavaScript describe callbacks. |
| `Test.describe.parallel` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.parallel.only` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.serial` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.serial.only` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.skip` | method | Partial | `Suite.skip` ([source](crates/ferrite-e2e/src/runner.rs#L670)) | Applies focus/skip/fixme to descendants; Rust builders, no JavaScript describe callbacks. |
| `Test.expect` | property | Partial | `Locator.expect` ([source](crates/ferrite-e2e/src/expect.rs#L1882)) | Rust builders, expect_poll/expect_to_pass and options companions with validated intervals, messages, shared budgets, cancellation and scoped soft retries. Rust defaults/error/step semantics differ; no matcher registry/configure API. |
| `Test.extend` | method | Partial | `Runner.fixture_definition` ([source](crates/ferrite-e2e/src/runner.rs#L3143)) | Typed lazy built-in/user dependencies, test/worker scopes and reverse teardown. Separate Fixture.setup_timeout/teardown_timeout limits are capped by enclosing clocks; cleanup shares one scope budget, unlike upstream separate fixture accounting. No named overrides or callback parameter inference. |
| `Test.abort` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.fail` | method | Partial | `TestInfo.fail` ([source](crates/ferrite-e2e/src/runner.rs#L1054)) | Static Test builders plus runtime TestInfo controls; use Rust conditionals and skip(reason)? for immediate closure exit. No JavaScript overload inference. |
| `Test.fail.only` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.fixme` | method | Partial | `Test.fixme` ([source](crates/ferrite-e2e/src/runner.rs#L522)) | Static test builder setting; no runtime conditional/context metadata overloads. |
| `Test.info` | method | Partial | `TestContext.info` ([source](crates/ferrite-e2e/src/runner.rs#L1250)) | Provided through test_with_context; fewer live metadata fields and mutators. |
| `Test.only` | method | Partial | `Test.only` ([source](crates/ferrite-e2e/src/runner.rs#L502)) | Static test builder setting; no runtime conditional/context metadata overloads. |
| `Test.setTimeout` | method | Partial | `TestInfo.set_timeout` ([source](crates/ferrite-e2e/src/runner.rs#L1073)) | Static Test builders plus runtime TestInfo controls; use Rust conditionals and skip(reason)? for immediate closure exit. No JavaScript overload inference. |
| `Test.skip` | method | Partial | `TestInfo.skip` ([source](crates/ferrite-e2e/src/runner.rs#L1043)) | Static Test builders plus runtime TestInfo controls; use Rust conditionals and skip(reason)? for immediate closure exit. No JavaScript overload inference. |
| `Test.slow` | method | Partial | `TestInfo.slow` ([source](crates/ferrite-e2e/src/runner.rs#L1062)) | Static Test builders plus runtime TestInfo controls; use Rust conditionals and skip(reason)? for immediate closure exit. No JavaScript overload inference. |
| `Test.step` | method | Partial | `Page.step_with` ([source](crates/ferrite-e2e/src/page.rs#L3280)) | Nested controlled steps with local timeout, skip, annotations and title paths through Page.step_with; legacy step_result remains available. Automatic navigation/locator/assertion/hook/fixture scopes; no boxing or subtitle/params options. |
| `Test.step.skip` | method | Partial | `Page.step_with` ([source](crates/ferrite-e2e/src/page.rs#L3280)) | StepOptions.skip records a skipped user step without constructing its closure; StepOutcome carries the reason. Rust option rather than a separate JS method. |
| `Test.use` | method | Partial | `Suite.context_options` ([source](crates/ferrite-e2e/src/runner.rs#L633)) | Nested suite/test context inheritance; no general named fixture option overrides. |

## TestCase

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-testcase.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestCase.annotations` | property | Partial | `Test.annotations` ([source](crates/ferrite-e2e/src/runner.rs#L450)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.expectedStatus` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestCase.id` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestCase.location` | property | Partial | `Test.file` ([source](crates/ferrite-e2e/src/runner.rs#L452)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.ok` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestCase.outcome` | method | Partial | `TestResult.flaky` ([source](crates/ferrite-e2e/src/report.rs#L801)) | Recovered successful retries flagged flaky; aggregate TestStatus separates passed/failed/skipped/expected-failed. Rust fields, not the upstream outcome() enum. |
| `TestCase.parent` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestCase.repeatEachIndex` | property | Partial | `TestResult.repeat_each_index` ([source](crates/ferrite-e2e/src/report.rs#L827)) | Result metadata only; no TestCase/Suite reporter tree. |
| `TestCase.results` | property | Partial | `TestResult.attempt_results` ([source](crates/ferrite-e2e/src/report.rs#L798)) | Full attempt history retained under the aggregate test result; no upstream TestCase/Suite graph. |
| `TestCase.retries` | property | Partial | `Test.retries` ([source](crates/ferrite-e2e/src/runner.rs#L552)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.tags` | property | Partial | `Test.tags` ([source](crates/ferrite-e2e/src/runner.rs#L448)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.timeout` | property | Partial | `Test.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L559)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.title` | property | Partial | `Test.name` ([source](crates/ferrite-e2e/src/runner.rs#L442)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.titlePath` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestCase.type` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |

## TestConfig

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-testconfig.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestConfig.build` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.captureGitInfo` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.expect` | property | Partial | `E2eConfig.expect_timeout_ms` ([source](crates/ferrite-config/src/lib.rs#L482)) | Consumed by CLI environment bridge and Runner; per-page/expect overrides supported. |
| `TestConfig.failOnFlakyTests` | property | Partial | `Runner.fail_on_flaky_tests` ([source](crates/ferrite-e2e/src/runner.rs#L3061)) | Opt-in aggregate failure preserves passed/flaky results and raw attempt statuses; CLI/config/effective snapshots wired. JUnit emits explicit policy violations, unlike upstream zero-failure XML. |
| `TestConfig.forbidOnly` | property | Partial | `Runner.forbid_only` ([source](crates/ferrite-e2e/src/runner.rs#L3053)) | Shared config/CLI and CI protection audit registered test/suite focus before filters/shards, including skipped/fixme/expected-failure descendants. Stricter than pinned upstream grep behavior. |
| `TestConfig.fullyParallel` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.globalSetup` | property | Partial | `Runner.global_setup` ([source](crates/ferrite-e2e/src/runner.rs#L2965)) | Comparable runner/configuration knob; effective configuration is a Rust subset and runner/CLI scheduling semantics differ. |
| `TestConfig.globalTeardown` | property | Partial | `Runner.global_teardown` ([source](crates/ferrite-e2e/src/runner.rs#L2976)) | Comparable runner/configuration knob; effective configuration is a Rust subset and runner/CLI scheduling semantics differ. |
| `TestConfig.globalTimeout` | property | Partial | `E2eConfig.global_timeout_ms` ([source](crates/ferrite-config/src/lib.rs#L471)) | Consumed by Runner and CLI; global cancellation with bounded teardown and final unexpected-failure scheduling limit. Active workers finish on maxFailures; no process-worker orchestration. |
| `TestConfig.grep` | property | Partial | `E2eConfig.grep` ([source](crates/ferrite-config/src/lib.rs#L501)) | Shared TOML/JSON configuration, complete CLI child bridge and Runner builders. Legacy environment overrides are read by from_env; explicit builder values win at run resolution. Name/tag substring filters and Rust scheduling differ from upstream. |
| `TestConfig.grepInvert` | property | Partial | `E2eConfig.grep_invert` ([source](crates/ferrite-config/src/lib.rs#L502)) | Shared TOML/JSON configuration, complete CLI child bridge and Runner builders. Legacy environment overrides are read by from_env; explicit builder values win at run resolution. Name/tag substring filters and Rust scheduling differ from upstream. |
| `TestConfig.ignoreSnapshots` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.maxFailures` | property | Partial | `E2eConfig.max_failures` ([source](crates/ferrite-config/src/lib.rs#L473)) | Consumed by Runner and CLI; global cancellation with bounded teardown and final unexpected-failure scheduling limit. Active workers finish on maxFailures; no process-worker orchestration. |
| `TestConfig.metadata` | property | Partial | `E2eConfig.metadata` ([source](crates/ferrite-config/src/lib.rs#L446)) | JSON-safe BTreeMap user metadata, frozen before hooks and exposed through config/project_config and owned WorkerInfo snapshots; project values replace the whole map or inherit the run map. Explicit empty maps survive reports/CLI/bundles; no upstream actualWorkers injection or mutable JS object identity. |
| `TestConfig.name` | property | Partial | `Runner.run_name` ([source](crates/ferrite-e2e/src/runner.rs#L2617)) | Optional Rust run name retained through frozen configuration and reports; not upstream test-directory naming or a mutable resolved JS configuration. |
| `TestConfig.outputDir` | property | Partial | `E2eConfig.output_dir` ([source](crates/ferrite-config/src/lib.rs#L490)) | Shared TOML/JSON configuration, complete CLI child bridge and Runner builders. Legacy environment overrides are read by from_env; explicit builder values win at run resolution. Name/tag substring filters and Rust scheduling differ from upstream. |
| `TestConfig.snapshotDir` | property | Partial | `E2eConfig.snapshot_dir` ([source](crates/ferrite-config/src/lib.rs#L494)) | Shared TOML/JSON configuration, complete CLI child bridge and Runner builders. Legacy environment overrides are read by from_env; explicit builder values win at run resolution. Name/tag substring filters and Rust scheduling differ from upstream. |
| `TestConfig.snapshotPathTemplate` | property | Partial | `Runner.snapshot_path_template` ([source](crates/ferrite-e2e/src/runner.rs#L2387)) | Validated explicit token templates, optional single-character prefixes and frozen runner root; project and assertion overrides win. Adds browserName; Rust lowercase slugs, canonical png/snap extensions, nested string names and legacy paths differ. Unknown tokens fail; no anonymous/ARIA/snapshotSuffix overloads or JS configDir/testDir discovery semantics. Shared E2eConfig/CLI/env bridge included. |
| `TestConfig.preserveOutput` | property | Partial | `Runner.output_retention` ([source](crates/ferrite-e2e/src/runner.rs#L2611)) | Validated always/never/failures-only config/env/CLI bridge, compatible always default. Final per-attempt expectation classification preserves failed retries/interruption/cleanup failures. Verified owned-directory cleanup protects baselines/caller sources; Rust prunes removed source links and preserves portable copies rather than retaining upstream stale attachment paths. |
| `TestConfig.projects` | property | Partial | `E2eConfig.projects` ([source](crates/ferrite-config/src/lib.rs#L506)) | Shared TOML/JSON configuration, complete CLI child bridge and Runner builders. Legacy environment overrides are read by from_env; explicit builder values win at run resolution. Name/tag substring filters and Rust scheduling differ from upstream. |
| `TestConfig.quiet` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.repeatEach` | property | Partial | `E2eConfig.repeat_each` ([source](crates/ferrite-config/src/lib.rs#L498)) | Shared TOML/JSON configuration, complete CLI child bridge and Runner builders. Legacy environment overrides are read by from_env; explicit builder values win at run resolution. Name/tag substring filters and Rust scheduling differ from upstream. |
| `TestConfig.reporter` | property | Partial | `E2eConfig.reporter` ([source](crates/ferrite-config/src/lib.rs#L488)) | Comparable runner/configuration knob; effective configuration is a Rust subset and runner/CLI scheduling semantics differ. |
| `TestConfig.reportSlowTests` | property | Partial | `Runner.report_slow_tests` ([source](crates/ferrite-e2e/src/runner.rs#L2628)) | Optional SlowTestOptions; strict threshold_ms and bounded max <=1000, zero disables, absent opts preserve legacy reports. Rust summarizes scheduled test/project/repeat result indices, not source files; retry total duration counted once. Upstream defaults on and max zero is unlimited. |
| `TestConfig.respectGitIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.retries` | property | Partial | `Runner.retries` ([source](crates/ferrite-e2e/src/runner.rs#L2654)) | Comparable runner/configuration knob; effective configuration is a Rust subset and runner/CLI scheduling semantics differ. |
| `TestConfig.retryStrategy` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.shard` | property | Partial | `E2eConfig.shard` ([source](crates/ferrite-config/src/lib.rs#L504)) | Shared TOML/JSON configuration, complete CLI child bridge and Runner builders. Legacy environment overrides are read by from_env; explicit builder values win at run resolution. Name/tag substring filters and Rust scheduling differ from upstream. |
| `TestConfig.tag` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.testDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.testIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.testMatch` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.timeout` | property | Partial | `Runner.test_timeout` ([source](crates/ferrite-e2e/src/runner.rs#L2661)) | Comparable runner/configuration knob; effective configuration is a Rust subset and runner/CLI scheduling semantics differ. |
| `TestConfig.tsconfig` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.updateSnapshots` | property | Partial | `E2eConfig.update_snapshots` ([source](crates/ferrite-config/src/lib.rs#L517)) | Comparable runner/configuration knob; effective configuration is a Rust subset and runner/CLI scheduling semantics differ. |
| `TestConfig.updateSourceMethod` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.use` | property | Partial | `ContextOptions` ([source](crates/ferrite-e2e/src/context.rs#L33)) | Comparable runner/configuration knob; effective configuration is a Rust subset and runner/CLI scheduling semantics differ. |
| `TestConfig.webServer` | property | Partial | `E2eConfig.web_server` ([source](crates/ferrite-config/src/lib.rs#L521)) | Comparable runner/configuration knob; effective configuration is a Rust subset and runner/CLI scheduling semantics differ. |
| `TestConfig.workers` | property | Partial | `Runner.workers` ([source](crates/ferrite-e2e/src/runner.rs#L2647)) | Comparable runner/configuration knob; effective configuration is a Rust subset and runner/CLI scheduling semantics differ. |

## TestError

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-testerror.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestError.cause` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestError.message` | property | Partial | `TestError.message` ([source](crates/ferrite-e2e/src/report.rs#L49)) | Structured Rust diagnostics carry message, code, phase and optional source; no JavaScript stack/cause/snippet object. |
| `TestError.stack` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestError.value` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestError.location` | property | Partial | `TestError.location` ([source](crates/ferrite-e2e/src/report.rs#L52)) | Optional Rust source location; exact user-step call site, test definition for runner phase errors, not a JS throw-site location. |
| `TestError.snippet` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## TestInfo

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-testinfo.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestInfo.annotations` | property | Partial | `TestInfo.annotations` ([source](crates/ferrite-e2e/src/runner.rs#L1087)) | Shared runtime control affects the active setup/body future and final annotations; skip uses Result propagation, timeouts include elapsed time, cleanup retains independent budgets. No full TestInfo object parity. |
| `TestInfo.attachments` | property | Partial | `TestInfo.attachments` ([source](crates/ferrite-e2e/src/runner.rs#L1165)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.attach` | method | Partial | `TestInfo.attach` ([source](crates/ferrite-e2e/src/runner.rs#L1144)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.column` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.config` | property | Partial | `TestInfo.config` ([source](crates/ferrite-e2e/src/runner.rs#L901)) | Owned effective run/project/attempt snapshots, selected projects, absolute output/snapshot paths, actual supplied browser identity and native dedicated versions after startup. Rust name/tag substring filters, Tokio workers and a smaller configuration surface; no JS fixture/config object compatibility. |
| `TestInfo.duration` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.error` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.errors` | property | Partial | `TestInfo.errors` ([source](crates/ferrite-e2e/src/runner.rs#L931)) | Live getters shared across metadata clones; raw outcome published before afterEach and updated after cleanup failures. status is None until a body outcome or soft mismatch; soft failures publish immediately with source/step/message metadata and survive successful bodies/cleanup. Attempts seal against late writes; no JS stack/cause/snippet serialization. |
| `TestInfo.expectedStatus` | property | Partial | `TestInfo.expected_status` ([source](crates/ferrite-e2e/src/runner.rs#L920)) | Live getters shared across metadata clones; raw outcome published before afterEach and updated after cleanup failures. status is None until a body outcome or soft mismatch; soft failures publish immediately with source/step/message metadata and survive successful bodies/cleanup. Attempts seal against late writes; no JS stack/cause/snippet serialization. |
| `TestInfo.fail` | method | Partial | `TestInfo.fail` ([source](crates/ferrite-e2e/src/runner.rs#L1054)) | Shared runtime control affects the active setup/body future and final annotations; skip uses Result propagation, timeouts include elapsed time, cleanup retains independent budgets. No full TestInfo object parity. |
| `TestInfo.file` | property | Partial | `TestInfo.file` ([source](crates/ferrite-e2e/src/runner.rs#L775)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.fixme` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.fn` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.tags` | property | Partial | `TestInfo.tags` ([source](crates/ferrite-e2e/src/runner.rs#L779)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.testId` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.line` | property | Partial | `TestInfo.line` ([source](crates/ferrite-e2e/src/runner.rs#L777)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.snapshotDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.outputDir` | property | Partial | `TestInfo.output_dir` ([source](crates/ferrite-e2e/src/runner.rs#L789)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.outputPath` | method | Partial | `TestInfo.output_path` ([source](crates/ferrite-e2e/src/runner.rs#L1091)) | Attempt-specific artifact path; parent traversal and absolute paths rejected. No snapshot-path templates. |
| `TestInfo.parallelIndex` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.project` | property | Partial | `TestInfo.project_config` ([source](crates/ferrite-e2e/src/runner.rs#L905)) | Read-only selected project defaults through project_config, with actual attempt context/retry/repetition and runtime timeout through settings(). Existing public project remains an optional name. No full upstream FullProject object or dependency graph. |
| `TestInfo.repeatEachIndex` | property | Partial | `TestInfo.repeat_each_index` ([source](crates/ferrite-e2e/src/runner.rs#L785)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.retry` | property | Partial | `TestInfo.retry` ([source](crates/ferrite-e2e/src/runner.rs#L781)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.setTimeout` | method | Partial | `TestInfo.set_timeout` ([source](crates/ferrite-e2e/src/runner.rs#L1073)) | Shared runtime control affects the active setup/body future and final annotations; skip uses Result propagation, timeouts include elapsed time, cleanup retains independent budgets. No full TestInfo object parity. |
| `TestInfo.skip` | method | Partial | `TestInfo.skip` ([source](crates/ferrite-e2e/src/runner.rs#L1043)) | Shared runtime control affects the active setup/body future and final annotations; skip uses Result propagation, timeouts include elapsed time, cleanup retains independent budgets. No full TestInfo object parity. |
| `TestInfo.slow` | method | Partial | `TestInfo.slow` ([source](crates/ferrite-e2e/src/runner.rs#L1062)) | Shared runtime control affects the active setup/body future and final annotations; skip uses Result propagation, timeouts include elapsed time, cleanup retains independent budgets. No full TestInfo object parity. |
| `TestInfo.snapshotPath` | method | Partial | `TestInfo.snapshot_path` ([source](crates/ferrite-e2e/src/runner.rs#L1131)) | Validated explicit token templates, optional single-character prefixes and frozen runner root; project and assertion overrides win. Adds browserName; Rust lowercase slugs, canonical png/snap extensions, nested string names and legacy paths differ. Unknown tokens fail; no anonymous/ARIA/snapshotSuffix overloads or JS configDir/testDir discovery semantics. Typed SnapshotKind and no filesystem creation; retries share identity. |
| `TestInfo.snapshotSuffix` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.status` | property | Partial | `TestInfo.status` ([source](crates/ferrite-e2e/src/runner.rs#L916)) | Live getters shared across metadata clones; raw outcome published before afterEach and updated after cleanup failures. status is None until a body outcome or soft mismatch; soft failures publish immediately with source/step/message metadata and survive successful bodies/cleanup. Attempts seal against late writes; no JS stack/cause/snippet serialization. |
| `TestInfo.timeout` | property | Partial | `TestInfo.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L787)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.title` | property | Partial | `TestInfo.title` ([source](crates/ferrite-e2e/src/runner.rs#L773)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.titlePath` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.workerIndex` | property | Partial | `TestInfo.worker_index` ([source](crates/ferrite-e2e/src/runner.rs#L783)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |

## TestInfoError

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-testinfoerror.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestInfoError.cause` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfoError.message` | property | Partial | `TestError.message` ([source](crates/ferrite-e2e/src/report.rs#L49)) | Structured Rust diagnostics carry message, code, phase and optional source; no JavaScript stack/cause/snippet object. |
| `TestInfoError.stack` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfoError.errorContext` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfoError.value` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## TestOptions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-testoptions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestOptions.acceptDownloads` | property | Partial | `ContextOptions.accept_downloads` ([source](crates/ferrite-e2e/src/context.rs#L208)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.baseURL` | property | Partial | `E2eConfig.base_url` ([source](crates/ferrite-config/src/lib.rs#L467)) | Runner/launch config field; no automatic named-fixture/project options equivalence. |
| `TestOptions.browserName` | property | Partial | `Project.browser` ([source](crates/ferrite-e2e/src/runner.rs#L1727)) | Projects select Chromium/Firefox independently; no WebKit backend. |
| `TestOptions.actionTimeout` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.bypassCSP` | property | Partial | `ContextOptions.bypass_csp` ([source](crates/ferrite-e2e/src/context.rs#L201)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.channel` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.clientCertificates` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.colorScheme` | property | Partial | `Page.emulate_media` ([source](crates/ferrite-e2e/src/page.rs#L4149)) | Chromium page-level manual emulation; no context/test option binding. |
| `TestOptions.connectOptions` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.contextOptions` | property | Partial | `Project.context_options` ([source](crates/ferrite-e2e/src/runner.rs#L1740)) | Isolated context per attempt, runner defaults and per-project overrides; no full named-fixture test.use model. |
| `TestOptions.contrast` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.deviceScaleFactor` | property | Partial | `ContextOptions.device_scale_factor` ([source](crates/ferrite-e2e/src/context.rs#L173)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.extraHTTPHeaders` | property | Partial | `ContextOptions.extra_http_headers` ([source](crates/ferrite-e2e/src/context.rs#L166)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.forcedColors` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.geolocation` | property | Partial | `ContextOptions.geolocation` ([source](crates/ferrite-e2e/src/context.rs#L138)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.hasTouch` | property | Partial | `ContextOptions.has_touch` ([source](crates/ferrite-e2e/src/context.rs#L187)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.headless` | property | Partial | `E2eConfig.headless` ([source](crates/ferrite-config/src/lib.rs#L452)) | Runner/launch config field; no automatic named-fixture/project options equivalence. |
| `TestOptions.httpCredentials` | property | Partial | `ContextOptions.http_credentials` ([source](crates/ferrite-e2e/src/context.rs#L159)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.ignoreHTTPSErrors` | property | Partial | `ContextOptions.ignore_https_errors` ([source](crates/ferrite-e2e/src/context.rs#L43)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.isMobile` | property | Partial | `ContextOptions.is_mobile` ([source](crates/ferrite-e2e/src/context.rs#L180)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.javaScriptEnabled` | property | Partial | `ContextOptions.java_script_enabled` ([source](crates/ferrite-e2e/src/context.rs#L194)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.launchOptions` | property | Partial | `Project.launch_options` ([source](crates/ferrite-e2e/src/runner.rs#L1733)) | Dedicated project launch settings; persistent profiles supported, no managed channels. |
| `TestOptions.locale` | property | Partial | `ContextOptions.locale` ([source](crates/ferrite-e2e/src/context.rs#L124)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.navigationTimeout` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.offline` | property | Partial | `ContextOptions.offline` ([source](crates/ferrite-e2e/src/context.rs#L152)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.permissions` | property | Partial | `ContextOptions.permissions` ([source](crates/ferrite-e2e/src/context.rs#L145)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.proxy` | property | Partial | `ContextOptions.proxy_server` ([source](crates/ferrite-e2e/src/context.rs#L41)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.reducedMotion` | property | Partial | `Page.emulate_media` ([source](crates/ferrite-e2e/src/page.rs#L4149)) | Chromium page-level manual emulation; no context/test option binding. |
| `TestOptions.reuseContext` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.screenshot` | property | Partial | `E2eConfig.screenshot` ([source](crates/ferrite-config/src/lib.rs#L509)) | Runner/launch config field; no automatic named-fixture/project options equivalence. |
| `TestOptions.storageState` | property | Partial | `ContextOptions.storage_state` ([source](crates/ferrite-e2e/src/context.rs#L222)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.testIdAttribute` | property | Partial | `set_test_id_attribute` ([source](crates/ferrite-e2e/src/locator.rs#L21)) | Process-global setter, not project/test-specific fixture option. |
| `TestOptions.timezoneId` | property | Partial | `ContextOptions.timezone_id` ([source](crates/ferrite-e2e/src/context.rs#L131)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.trace` | property | Partial | `BrowserContext.start_tracing` ([source](crates/ferrite-e2e/src/context.rs#L1127)) | Manual custom JSON traces plus runner JSON; no trace mode policy or Trace Viewer compatibility. |
| `TestOptions.userAgent` | property | Partial | `ContextOptions.user_agent` ([source](crates/ferrite-e2e/src/context.rs#L117)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.video` | property | Partial | `E2eConfig.video` ([source](crates/ferrite-config/src/lib.rs#L511)) | Runner/launch config field; no automatic named-fixture/project options equivalence. |
| `TestOptions.viewport` | property | Partial | `ContextOptions.viewport` ([source](crates/ferrite-e2e/src/context.rs#L110)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.serviceWorkers` | property | Partial | `ContextOptions.service_workers` ([source](crates/ferrite-e2e/src/context.rs#L229)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |

## TestProject

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-testproject.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestProject.dependencies` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.expect` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.fullyParallel` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.grep` | property | Partial | `Project.grep` ([source](crates/ferrite-e2e/src/runner.rs#L1757)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `TestProject.grepInvert` | property | Partial | `Project.grep_invert` ([source](crates/ferrite-e2e/src/runner.rs#L1762)) | Library Project plus shared E2eProjectConfig fields and CLI configuration bridge; project overrides precede global values. Repetition zero normalizes to one, paths are absolute at run resolution; dependency scheduling remains absent. |
| `TestProject.ignoreSnapshots` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.metadata` | property | Partial | `Project.metadata` ([source](crates/ferrite-e2e/src/runner.rs#L1722)) | JSON-safe BTreeMap user metadata, frozen before hooks and exposed through config/project_config and owned WorkerInfo snapshots; project values replace the whole map or inherit the run map. Explicit empty maps survive reports/CLI/bundles; no upstream actualWorkers injection or mutable JS object identity. |
| `TestProject.name` | property | Partial | `Project.name` ([source](crates/ferrite-e2e/src/runner.rs#L1706)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `TestProject.snapshotDir` | property | Partial | `Project.snapshot_dir` ([source](crates/ferrite-e2e/src/runner.rs#L1774)) | Library Project plus shared E2eProjectConfig fields and CLI configuration bridge; project overrides precede global values. Repetition zero normalizes to one, paths are absolute at run resolution; dependency scheduling remains absent. |
| `TestProject.snapshotPathTemplate` | property | Partial | `Project.snapshot_path_template` ([source](crates/ferrite-e2e/src/runner.rs#L1779)) | Validated explicit token templates, optional single-character prefixes and frozen runner root; project and assertion overrides win. Adds browserName; Rust lowercase slugs, canonical png/snap extensions, nested string names and legacy paths differ. Unknown tokens fail; no anonymous/ARIA/snapshotSuffix overloads or JS configDir/testDir discovery semantics. |
| `TestProject.outputDir` | property | Partial | `Project.output_dir` ([source](crates/ferrite-e2e/src/runner.rs#L1770)) | Library Project plus shared E2eProjectConfig fields and CLI configuration bridge; project overrides precede global values. Repetition zero normalizes to one, paths are absolute at run resolution; dependency scheduling remains absent. |
| `TestProject.repeatEach` | property | Partial | `Project.repeat_each` ([source](crates/ferrite-e2e/src/runner.rs#L1766)) | Library Project plus shared E2eProjectConfig fields and CLI configuration bridge; project overrides precede global values. Repetition zero normalizes to one, paths are absolute at run resolution; dependency scheduling remains absent. |
| `TestProject.respectGitIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.retries` | property | Partial | `Project.retries` ([source](crates/ferrite-e2e/src/runner.rs#L1786)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `TestProject.teardown` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.testDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.testIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.testMatch` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.timeout` | property | Partial | `Project.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L1793)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `TestProject.use` | property | Partial | `Project.context_options` ([source](crates/ferrite-e2e/src/runner.rs#L1740)) | Project-specific context settings; optional browser/launch overrides, suite/test context inheritance, no named fixture overrides or project dependencies. |
| `TestProject.workers` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## TestResult

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-testresult.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestResult.attachments` | property | Partial | `AttemptResult.attachments` ([source](crates/ferrite-e2e/src/report.rs#L279)) | Each attempt is retained in TestResult.attempt_results and JSON/HTML; raw status includes timeout/interruption, errors include phase/code and steps include user/action/assertion/hook/fixture trees; soft_assertions retains contextual mismatch source and step paths with serde defaults. Different Rust schema, no stdout/stderr capture. |
| `TestResult.annotations` | property | Partial | `AttemptResult.annotations` ([source](crates/ferrite-e2e/src/report.rs#L277)) | Each attempt is retained in TestResult.attempt_results and JSON/HTML; raw status includes timeout/interruption, errors include phase/code and steps include user/action/assertion/hook/fixture trees; soft_assertions retains contextual mismatch source and step paths with serde defaults. Different Rust schema, no stdout/stderr capture. |
| `TestResult.duration` | property | Partial | `AttemptResult.duration_ms` ([source](crates/ferrite-e2e/src/report.rs#L275)) | Each attempt is retained in TestResult.attempt_results and JSON/HTML; raw status includes timeout/interruption, errors include phase/code and steps include user/action/assertion/hook/fixture trees; soft_assertions retains contextual mismatch source and step paths with serde defaults. Different Rust schema, no stdout/stderr capture. |
| `TestResult.error` | property | Partial | `TestResult.error` ([source](crates/ferrite-e2e/src/report.rs#L812)) | Aggregate result fields; attempts aggregates retries, not one TestResult per attempt; errors are strings. Live callbacks additionally receive per-attempt results; the final report aggregates retries. |
| `TestResult.errors` | property | Partial | `AttemptResult.errors` ([source](crates/ferrite-e2e/src/report.rs#L276)) | Each attempt is retained in TestResult.attempt_results and JSON/HTML; raw status includes timeout/interruption, errors include phase/code and steps include user/action/assertion/hook/fixture trees; soft_assertions retains contextual mismatch source and step paths with serde defaults. Different Rust schema, no stdout/stderr capture. |
| `TestResult.retry` | property | Partial | `AttemptInfo.retry` ([source](crates/ferrite-e2e/src/report.rs#L15)) | Per-attempt identity on AttemptResult.info; logical Tokio workers, not process-worker IDs. |
| `TestResult.startTime` | property | Partial | `AttemptResult.start_time_ms` ([source](crates/ferrite-e2e/src/report.rs#L274)) | Each attempt is retained in TestResult.attempt_results and JSON/HTML; raw status includes timeout/interruption, errors include phase/code and steps include user/action/assertion/hook/fixture trees; soft_assertions retains contextual mismatch source and step paths with serde defaults. Different Rust schema, no stdout/stderr capture. |
| `TestResult.status` | property | Partial | `AttemptResult.status` ([source](crates/ferrite-e2e/src/report.rs#L270)) | Each attempt is retained in TestResult.attempt_results and JSON/HTML; raw status includes timeout/interruption, errors include phase/code and steps include user/action/assertion/hook/fixture trees; soft_assertions retains contextual mismatch source and step paths with serde defaults. Different Rust schema, no stdout/stderr capture. |
| `TestResult.stderr` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestResult.stdout` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestResult.steps` | property | Partial | `AttemptResult.steps` ([source](crates/ferrite-e2e/src/report.rs#L278)) | Each attempt is retained in TestResult.attempt_results and JSON/HTML; raw status includes timeout/interruption, errors include phase/code and steps include user/action/assertion/hook/fixture trees; soft_assertions retains contextual mismatch source and step paths with serde defaults. Different Rust schema, no stdout/stderr capture. |
| `TestResult.workerIndex` | property | Partial | `AttemptInfo.worker_index` ([source](crates/ferrite-e2e/src/report.rs#L13)) | Per-attempt identity on AttemptResult.info; logical Tokio workers, not process-worker IDs. |
| `TestResult.parallelIndex` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## TestRun

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-testrun.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestRun.exclude` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestRun.fail` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestRun.fixme` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestRun.skip` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestRun.skipSharding` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |

## TestStep

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-teststep.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestStep.category` | property | Partial | `StepInfo.category` ([source](crates/ferrite-e2e/src/report.rs#L225)) | Persisted annotations and full title path; categories user/action/assertion/hook/fixture differ from the upstream category vocabulary and not all Page/protocol operations are wrapped. |
| `TestStep.duration` | property | Partial | `StepInfo.duration_ms` ([source](crates/ferrite-e2e/src/report.rs#L238)) | Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data. |
| `TestStep.location` | property | Partial | `StepInfo.location` ([source](crates/ferrite-e2e/src/report.rs#L235)) | Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data. |
| `TestStep.error` | property | Partial | `StepInfo.error` ([source](crates/ferrite-e2e/src/report.rs#L240)) | Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data. |
| `TestStep.parent` | property | Partial | `StepInfo.parent_id` ([source](crates/ferrite-e2e/src/report.rs#L233)) | Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data. |
| `TestStep.params` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestStep.startTime` | property | Partial | `StepInfo.start_time_ms` ([source](crates/ferrite-e2e/src/report.rs#L237)) | Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data. |
| `TestStep.steps` | property | Partial | `StepInfo.steps` ([source](crates/ferrite-e2e/src/report.rs#L241)) | Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data. |
| `TestStep.annotations` | property | Partial | `StepInfo.annotations` ([source](crates/ferrite-e2e/src/report.rs#L229)) | Persisted annotations and full title path; categories user/action/assertion/hook/fixture differ from the upstream category vocabulary and not all Page/protocol operations are wrapped. |
| `TestStep.attachments` | property | Partial | `StepInfo.attachments` ([source](crates/ferrite-e2e/src/report.rs#L242)) | Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data. |
| `TestStep.title` | property | Partial | `StepInfo.title` ([source](crates/ferrite-e2e/src/report.rs#L234)) | Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data. |
| `TestStep.subtitle` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestStep.titlePath` | method | Partial | `StepInfo.title_path` ([source](crates/ferrite-e2e/src/report.rs#L246)) | Persisted annotations and full title path; categories user/action/assertion/hook/fixture differ from the upstream category vocabulary and not all Page/protocol operations are wrapped. |

## TestStepInfo

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-teststepinfo.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestStepInfo.annotations` | property | Partial | `StepContext.annotations` ([source](crates/ferrite-e2e/src/report.rs#L197)) | Live getters on the context passed to Page.step_with; annotate adds source-aware metadata, title paths include file/test/ancestor steps. Completed records reject late annotations. |
| `TestStepInfo.attach` | method | Partial | `TestInfo.attach` ([source](crates/ferrite-e2e/src/runner.rs#L1144)) | Attachments inside an awaited user step associate with that step and its attempt. Page.step_with passes a live StepContext; attachments still use TestInfo.attach and associate with the active scope. Detached Tokio tasks do not inherit parent scope. |
| `TestStepInfo.skip` | method | Partial | `StepContext.skip` ([source](crates/ferrite-e2e/src/report.rs#L155)) | Use step.skip(reason)? to abort only this controlled step. Shared clones cancel pending children; returns StepOutcome::Skipped, distinct from whole-test skip. Conditional skipping uses a Rust if statement. |
| `TestStepInfo.titlePath` | property | Partial | `StepContext.title_path` ([source](crates/ferrite-e2e/src/report.rs#L208)) | Live getters on the context passed to Page.step_with; annotate adds source-aware metadata, title paths include file/test/ancestor steps. Completed records reject late annotations. |

## TimeoutError

No own JS-applicable member headings. The class/error type is not exposed as a Ferrite class; use `E2eError`.

## Touchscreen

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-touchscreen.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Touchscreen.tap` | method | Partial | `Page.touchscreen_tap` ([source](crates/ferrite-e2e/src/page.rs#L3237)) | Coordinate tap available; no separate Touchscreen object. |

## Tracing

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-tracing.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Tracing.start` | method | Partial | `BrowserContext.start_tracing` ([source](crates/ferrite-e2e/src/context.rs#L1127)) | Custom JSON actions/logs/requests; optional screenshots at Page.step only, no DOM/ARIA/source snapshots. |
| `Tracing.startChunk` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Tracing.startHar` | method | Partial | `Page.start_request_capture` ([source](crates/ferrite-e2e/src/page.rs#L4501)) | Page capture + save_har_with; no Tracing.startHar API or full browser/API-request tracing. |
| `Tracing.group` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Tracing.groupEnd` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Tracing.stop` | method | Partial | `BrowserContext.stop_tracing` ([source](crates/ferrite-e2e/src/context.rs#L1140)) | Writes JSON, not a Trace Viewer-compatible zip archive. |
| `Tracing.stopChunk` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Tracing.stopHar` | method | Partial | `Page.save_har_with` ([source](crates/ferrite-e2e/src/page.rs#L4658)) | HAR exporter; no Tracing.stopHar interface, update/rewrite mode or full timing/body coverage. |

## Video

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-video.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Video.delete` | method | Idiomatic | — | Caller deletes artifact through Rust filesystem operations; no Video object. |
| `Video.path` | method | Partial | `TestResult.video` ([source](crates/ferrite-e2e/src/report.rs#L821)) | Runner records an optional artifact path, not Page.video()/Video object. |
| `Video.saveAs` | method | Partial | `Page.stop_video` ([source](crates/ferrite-e2e/src/page.rs#L5385)) | Stop recording to a path; no independently awaitable Video handle. |

## WebError

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-weberror.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `WebError.page` | method | Partial | `ConsoleMessage.page_id` ([source](crates/ferrite-e2e/src/page.rs#L87)) | Native owning page identity retained in context error events and histories, without a live Page/worker object. Unknown fields remain optional. |
| `WebError.error` | method | Partial | `ConsoleMessage.error` ([source](crates/ferrite-e2e/src/page.rs#L93)) | Optional owned PageErrorInfo with native own name/message/stack, constructor class and supplied stack frames. Firefox exposes only text/frames; no complete live JavaScript Error/cause/property object. |
| `WebError.location` | method | Partial | `ConsoleMessage.location` ([source](crates/ferrite-e2e/src/page.rs#L81)) | Optional native source URL and zero-based line/column on context exception observations; absent source metadata stays unavailable rather than producing an always-present location object. |

## WebSocket

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-websocket.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `WebSocket.close` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1982)) | PageEvent::WebSocket direction Closed; Chromium-only observation without a WebSocket object. |
| `WebSocket.frameReceived` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1982)) | PageEvent::WebSocket direction Received; Chromium-only observation without a WebSocket object. |
| `WebSocket.frameSent` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1982)) | PageEvent::WebSocket direction Sent; Chromium-only observation without a WebSocket object. |
| `WebSocket.socketError` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `WebSocket.isClosed` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `WebSocket.url` | method | Partial | `WebSocketEvent.url` ([source](crates/ferrite-e2e/src/page.rs#L287)) | Captured socket URL, Chromium only. |
| `WebSocket.waitForEvent` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## WebSocketRoute

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-websocketroute.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `WebSocketRoute.close` | method | Missing | — | No public WebSocketRoute abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `WebSocketRoute.connectToServer` | method | Missing | — | No public WebSocketRoute abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `WebSocketRoute.onClose` | method | Missing | — | No public WebSocketRoute abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `WebSocketRoute.onMessage` | method | Missing | — | No public WebSocketRoute abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `WebSocketRoute.send` | method | Missing | — | No public WebSocketRoute abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `WebSocketRoute.protocols` | method | Missing | — | No public WebSocketRoute abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `WebSocketRoute.url` | method | Missing | — | No public WebSocketRoute abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |

## WebStorage

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-webstorage.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `WebStorage.items` | method | Partial | `Page.local_storage_items` ([source](crates/ferrite-e2e/src/page.rs#L3491)) | Page helper methods for local and session storage; no WebStorage object. |
| `WebStorage.getItem` | method | Partial | `Page.local_storage_get` ([source](crates/ferrite-e2e/src/page.rs#L3416)) | Page helper methods for local and session storage; no WebStorage object. |
| `WebStorage.setItem` | method | Partial | `Page.local_storage_set` ([source](crates/ferrite-e2e/src/page.rs#L3423)) | Page helper methods for local and session storage; no WebStorage object. |
| `WebStorage.removeItem` | method | Partial | `Page.local_storage_remove` ([source](crates/ferrite-e2e/src/page.rs#L3430)) | Page helper methods for local and session storage; no WebStorage object. |
| `WebStorage.clear` | method | Partial | `Page.local_storage_clear` ([source](crates/ferrite-e2e/src/page.rs#L3443)) | Page helper methods for local and session storage; no WebStorage object. |

## Worker

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-worker.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Worker.close` | event | Missing | — | No public Worker abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Worker.console` | event | Missing | — | No public Worker abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Worker.evaluate` | method | Missing | — | No public Worker abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Worker.evaluateHandle` | method | Missing | — | No public Worker abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Worker.url` | method | Missing | — | No public Worker abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Worker.waitForEvent` | method | Missing | — | No public Worker abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |

## WorkerInfo

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-workerinfo.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `WorkerInfo.config` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `WorkerInfo.parallelIndex` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `WorkerInfo.project` | property | Partial | `WorkerInfo.project` ([source](crates/ferrite-e2e/src/runner.rs#L307)) | Optional project name only, not a resolved FullProject object. |
| `WorkerInfo.workerIndex` | property | Partial | `WorkerInfo.worker_index` ([source](crates/ferrite-e2e/src/runner.rs#L306)) | Logical Tokio worker index, not a process identity. |
