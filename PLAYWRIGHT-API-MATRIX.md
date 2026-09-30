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

Inventory: **73 classes, 1018 distinct members**. Equivalent: 15; Partial: 589; Idiomatic: 42; Missing: 372. These counts are inventory labels, not a percentage of behavioral compatibility.

## APIRequest

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-apirequest.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `APIRequest.newContext` | method | Partial | `ApiClient.with_options` ([source](crates/ferrite-e2e/src/api.rs#L237)) | Base URL, headers, TLS, proxy, timeout, redirects, Basic auth and imported storage state; no full Playwright options. |

## APIRequestContext

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-apirequestcontext.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `APIRequestContext.delete` | method | Partial | `ApiClient.delete` ([source](crates/ferrite-e2e/src/api.rs#L401)) | HTTP verb available; use fetch_with for query/body/header/timeout/retry/status options. Client has cookies/auth/proxy/redirect settings; API state import/export and shared disposal/cancellation exist; narrower retry and redirect options. |
| `APIRequestContext.dispose` | method | Partial | `ApiClient.dispose` ([source](crates/ferrite-e2e/src/api.rs#L177)) | Enumerable cookies with domain/path/expiry/HttpOnly/Secure/SameSite state import/export; origin data retained, IndexedDB deferred. Disposal cancels clones; returned Rust response buffers remain independently owned. |
| `APIRequestContext.fetch` | method | Partial | `ApiClient.fetch_with` ([source](crates/ferrite-e2e/src/api.rs#L291)) | Generic HTTP method; headers/query/JSON/form/multipart/raw payloads, timeout, status checks and connection retries. Zero timeout disables the request budget, including body reads; cancellation/disposal supported, narrower redirect/retry options. |
| `APIRequestContext.get` | method | Partial | `ApiClient.get` ([source](crates/ferrite-e2e/src/api.rs#L394)) | HTTP verb available; use fetch_with for query/body/header/timeout/retry/status options. Client has cookies/auth/proxy/redirect settings; API state import/export and shared disposal/cancellation exist; narrower retry and redirect options. |
| `APIRequestContext.head` | method | Partial | `ApiClient.head` ([source](crates/ferrite-e2e/src/api.rs#L429)) | HTTP verb available; use fetch_with for query/body/header/timeout/retry/status options. Client has cookies/auth/proxy/redirect settings; API state import/export and shared disposal/cancellation exist; narrower retry and redirect options. |
| `APIRequestContext.patch` | method | Partial | `ApiClient.patch_json` ([source](crates/ferrite-e2e/src/api.rs#L422)) | HTTP verb available; use fetch_with for query/body/header/timeout/retry/status options. Client has cookies/auth/proxy/redirect settings; API state import/export and shared disposal/cancellation exist; narrower retry and redirect options. |
| `APIRequestContext.post` | method | Partial | `ApiClient.post_json` ([source](crates/ferrite-e2e/src/api.rs#L408)) | HTTP verb available; use fetch_with for query/body/header/timeout/retry/status options. Client has cookies/auth/proxy/redirect settings; API state import/export and shared disposal/cancellation exist; narrower retry and redirect options. |
| `APIRequestContext.put` | method | Partial | `ApiClient.put_json` ([source](crates/ferrite-e2e/src/api.rs#L415)) | HTTP verb available; use fetch_with for query/body/header/timeout/retry/status options. Client has cookies/auth/proxy/redirect settings; API state import/export and shared disposal/cancellation exist; narrower retry and redirect options. |
| `APIRequestContext.storageState` | method | Partial | `ApiClient.storage_state` ([source](crates/ferrite-e2e/src/api.rs#L181)) | Enumerable cookies with domain/path/expiry/HttpOnly/Secure/SameSite state import/export; origin data retained, IndexedDB deferred. Disposal cancels clones; returned Rust response buffers remain independently owned. |
| `APIRequestContext.tracing` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## APIResponse

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-apiresponse.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `APIResponse.body` | method | Equivalent | `ApiResponse.bytes` ([source](crates/ferrite-e2e/src/api.rs#L70)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.dispose` | method | Partial | `ApiResponse.dispose` ([source](crates/ferrite-e2e/src/api.rs#L36)) | Final response URL/status text and explicit body-buffer disposal; clones own their buffers. |
| `APIResponse.headers` | method | Partial | `ApiResponse.headers` ([source](crates/ferrite-e2e/src/api.rs#L55)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.headersArray` | method | Partial | `ApiResponse.headers` ([source](crates/ferrite-e2e/src/api.rs#L55)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.json` | method | Equivalent | `ApiResponse.json` ([source](crates/ferrite-e2e/src/api.rs#L81)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.ok` | method | Equivalent | `ApiResponse.ok` ([source](crates/ferrite-e2e/src/api.rs#L49)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.securityDetails` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIResponse.serverAddr` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIResponse.status` | method | Equivalent | `ApiResponse.status` ([source](crates/ferrite-e2e/src/api.rs#L43)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.statusText` | method | Partial | `ApiResponse.status_text` ([source](crates/ferrite-e2e/src/api.rs#L31)) | Final response URL/status text and explicit body-buffer disposal; clones own their buffers. |
| `APIResponse.text` | method | Partial | `ApiResponse.text` ([source](crates/ferrite-e2e/src/api.rs#L76)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
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
| `Browser.browserType` | method | Partial | `Browser.kind` ([source](crates/ferrite-e2e/src/browser.rs#L721)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.close` | method | Partial | `Browser.close` ([source](crates/ferrite-e2e/src/browser.rs#L1096)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.contexts` | method | Partial | `Browser.contexts` ([source](crates/ferrite-e2e/src/browser.rs#L1062)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.isConnected` | method | Partial | `Browser.is_connected` ([source](crates/ferrite-e2e/src/browser.rs#L953)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.newBrowserCDPSession` | method | Partial | `Browser.cdp` ([source](crates/ferrite-e2e/src/browser.rs#L930)) | Raw shared browser CDP connection; no independently detachable CDPSession. |
| `Browser.newContext` | method | Partial | `Browser.new_context` ([source](crates/ferrite-e2e/src/browser.rs#L961)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.newPage` | method | Partial | `Browser.new_page` ([source](crates/ferrite-e2e/src/browser.rs#L1080)) | Fresh owning context; closing the page disposes it, including its popups. |
| `Browser.bind` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.removeAllListeners` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.startTracing` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.stopTracing` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.unbind` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.version` | method | Partial | `Browser.version` ([source](crates/ferrite-e2e/src/browser.rs#L947)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |

## BrowserContext

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-browsercontext.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `BrowserContext.backgroundPage` (deprecated) | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.clock` | property | Partial | `Page.clock_install` ([source](crates/ferrite-e2e/src/page.rs#L3525)) | Page-document-local clock; no context-wide clock object. |
| `BrowserContext.credentials` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.debugger` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.close` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L474)) | ContextEventKind::Closed; context subscriptions forward observations from every current/future page; enum payloads have a narrower live object/options model. |
| `BrowserContext.console` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L474)) | ContextEventKind::Console; context subscriptions forward observations from every current/future page; enum payloads have a narrower live object/options model. |
| `BrowserContext.dialog` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L474)) | ContextEventKind::Dialog forwards the existing page observation with source page ID; payloads and backend metadata remain narrower. |
| `BrowserContext.dialogClosed` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.download` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L474)) | ContextEventKind::Download forwards the existing page observation with source page ID; payloads and backend metadata remain narrower. |
| `BrowserContext.frameAttached` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.frameDetached` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.frameNavigated` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.page` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L474)) | ContextEventKind::Page; context subscriptions forward observations from every current/future page; enum payloads have a narrower live object/options model. |
| `BrowserContext.pageClose` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L474)) | ContextEventKind::PageClose forwards the existing page observation with source page ID; payloads and backend metadata remain narrower. |
| `BrowserContext.pageLoad` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.webError` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.request` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L474)) | ContextEventKind::Request; context subscriptions forward observations from every current/future page; enum payloads have a narrower live object/options model. |
| `BrowserContext.requestFailed` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L474)) | RequestFailed enum events on Chromium/Firefox with request IDs and method/URL; failed requests carry transport error text. Context events include page identity. No rich live Request object graph. |
| `BrowserContext.requestFinished` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L474)) | RequestFinished enum events on Chromium/Firefox with request IDs and method/URL; failed requests carry transport error text. Context events include page identity. No rich live Request object graph. |
| `BrowserContext.response` | event | Partial | `BrowserContext.subscribe` ([source](crates/ferrite-e2e/src/context.rs#L474)) | ContextEventKind::Response; context subscriptions forward observations from every current/future page; enum payloads have a narrower live object/options model. |
| `BrowserContext.serviceWorker` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.addCookies` | method | Partial | `BrowserContext.add_cookies` ([source](crates/ferrite-e2e/src/context.rs#L849)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.addInitScript` | method | Partial | `BrowserContext.add_init_script` ([source](crates/ferrite-e2e/src/context.rs#L1416)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.backgroundPages` (deprecated) | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.browser` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.clearCookies` | method | Partial | `BrowserContext.clear_cookies_with` ([source](crates/ferrite-e2e/src/context.rs#L828)) | ANDed exact/regex name/domain/path filters on native stores; empty filter clears all. Linked API requests refresh cookies; opaque partition key deletion fails explicitly on Chromium. Rust regex syntax and partition filter fields remain narrower. |
| `BrowserContext.clearPermissions` | method | Partial | `BrowserContext.clear_permissions` ([source](crates/ferrite-e2e/src/context.rs#L1210)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.close` | method | Partial | `BrowserContext.close` ([source](crates/ferrite-e2e/src/context.rs#L1582)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.cookies` | method | Partial | `BrowserContext.cookies` ([source](crates/ferrite-e2e/src/context.rs#L805)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.exposeBinding` | method | Partial | `BrowserContext.expose_binding` ([source](crates/ferrite-e2e/src/callbacks.rs#L560)) | Async JSON binding with owning context/page/native frame identity. Same-origin frame dispatch; native startup preloads and navigation/disposal cleanup. Cross-origin/OOPIF callers and handle arguments deferred. |
| `BrowserContext.exposeFunction` | method | Partial | `BrowserContext.expose_function_async` ([source](crates/ferrite-e2e/src/callbacks.rs#L549)) | Sync/async JSON callbacks in current/future same-origin documents; independent bounded dispatch, native preload ownership, duplicate-name errors and named removal. Rust errors/panics reject JS promises; cross-origin dispatch/handle arguments deferred. |
| `BrowserContext.grantPermissions` | method | Partial | `BrowserContext.grant_permissions` ([source](crates/ferrite-e2e/src/context.rs#L1181)) | No origin argument; Chromium grants broadly, Firefox grants after navigation for the current origin. |
| `BrowserContext.isClosed` | method | Partial | `BrowserContext.is_closed` ([source](crates/ferrite-e2e/src/context.rs#L1510)) | Tracks explicit context disposal; no full remote-disconnection lifecycle semantics. |
| `BrowserContext.newCDPSession` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.newPage` | method | Partial | `BrowserContext.new_page` ([source](crates/ferrite-e2e/src/context.rs#L543)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.pages` | method | Partial | `BrowserContext.pages` ([source](crates/ferrite-e2e/src/context.rs#L737)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.removeAllListeners` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.request` | property | Partial | `BrowserContext.request` ([source](crates/ferrite-e2e/src/context.rs#L775)) | Context-linked HTTP client sharing cookies and inheriting headers, Basic auth, TLS, proxy and timeout settings at creation. Transport overrides use ApiClientOptions. |
| `BrowserContext.route` | method | Partial | `BrowserContext.route_matching` ([source](crates/ferrite-e2e/src/context.rs#L931)) | Shared resolved UrlMatcher for rules/handlers/HAR filters, with match limits and identity-based removal; legacy string/globset contracts preserved. Invalid patterns fail before registration, including empty contexts; native response/URL override differences remain. In-flight removal and fuller HAR policies tracked separately. |
| `BrowserContext.routeFromHAR` | method | Partial | `BrowserContext.route_from_har` ([source](crates/ferrite-e2e/src/context.rs#L1142)) | Shared resolved UrlMatcher for rules/handlers/HAR filters, with match limits and identity-based removal; legacy string/globset contracts preserved. Invalid patterns fail before registration, including empty contexts; native response/URL override differences remain. In-flight removal and fuller HAR policies tracked separately. |
| `BrowserContext.routeWebSocket` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.serviceWorkers` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.setDefaultNavigationTimeout` | method | Partial | `BrowserContext.set_default_navigation_timeout` ([source](crates/ferrite-e2e/src/context.rs#L753)) | Shared action/protocol defaults update existing and future pages; zero disables timeout, cancellation is independently supported. |
| `BrowserContext.setDefaultTimeout` | method | Partial | `BrowserContext.set_default_timeout` ([source](crates/ferrite-e2e/src/context.rs#L742)) | Shared action/protocol defaults update existing and future pages; zero disables timeout, cancellation is independently supported. |
| `BrowserContext.setExtraHTTPHeaders` | method | Partial | `BrowserContext.set_extra_http_headers` ([source](crates/ferrite-e2e/src/context.rs#L1326)) | Chromium only; HTTP credentials hold one username/password pair, no origin list. |
| `BrowserContext.setGeolocation` | method | Partial | `BrowserContext.set_geolocation` ([source](crates/ferrite-e2e/src/context.rs#L1201)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.setHTTPCredentials` | method | Partial | `BrowserContext.set_http_credentials` ([source](crates/ferrite-e2e/src/context.rs#L1300)) | Chromium only; HTTP credentials hold one username/password pair, no origin list. |
| `BrowserContext.setOffline` | method | Partial | `BrowserContext.set_offline` ([source](crates/ferrite-e2e/src/context.rs#L1290)) | Chromium only; HTTP credentials hold one username/password pair, no origin list. |
| `BrowserContext.storageState` | method | Partial | `BrowserContext.storage_state` ([source](crates/ferrite-e2e/src/context.rs#L1483)) | Playwright cookies/origins JSON, localStorage from live pages across origins; closed-origin inventory and IndexedDB/OPFS are deferred. |
| `BrowserContext.setStorageState` | method | Partial | `BrowserContext.load_storage_state` ([source](crates/ferrite-e2e/src/context.rs#L1431)) | Playwright multi-origin state and legacy files; cookies restore before navigation and localStorage before app scripts. IndexedDB/OPFS not persisted. |
| `BrowserContext.tracing` | property | Partial | `BrowserContext.start_tracing` ([source](crates/ferrite-e2e/src/context.rs#L1016)) | Custom JSON trace, no Trace Viewer-compatible archive, DOM snapshots or chunks. |
| `BrowserContext.unrouteAll` | method | Partial | `BrowserContext.unroute_all` ([source](crates/ferrite-e2e/src/context.rs#L1096)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.unroute` | method | Partial | `BrowserContext.unroute_matching` ([source](crates/ferrite-e2e/src/context.rs#L1113)) | Shared resolved UrlMatcher for rules/handlers/HAR filters, with match limits and identity-based removal; legacy string/globset contracts preserved. Invalid patterns fail before registration, including empty contexts; native response/URL override differences remain. In-flight removal and fuller HAR policies tracked separately. |
| `BrowserContext.waitForEvent` | method | Partial | `BrowserContext.wait_for_event` ([source](crates/ferrite-e2e/src/context.rs#L477)) | Context-wide page/popup, console/error, network, download and close events with source page identity; enum-based filtering, no listener callback API or rich live Request/WebError objects. |

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
| `BrowserType.connectOverCDP` | method | Partial | `Browser.connect_over_cdp` ([source](crates/ferrite-e2e/src/browser.rs#L675)) | Chromium HTTP or browser WebSocket endpoint; no Playwright remote protocol or headers/options surface. |
| `BrowserType.executablePath` | method | Partial | `find_chromium` ([source](crates/ferrite-e2e/src/browser.rs#L169)) | Stock-browser discovery/launch; no channels/installer or full Playwright connection options. |
| `BrowserType.launch` | method | Partial | `Browser.launch` ([source](crates/ferrite-e2e/src/browser.rs#L367)) | Stock-browser discovery/launch; no channels/installer or full Playwright connection options. |
| `BrowserType.launchPersistentContext` | method | Partial | `LaunchOptions.user_data_dir` ([source](crates/ferrite-e2e/src/browser.rs#L138)) | Reusable Chromium/Firefox profile; obtain browser.default_context(). Dedicated contexts remain isolated from persistent storage. |
| `BrowserType.launchServer` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserType.name` | method | Partial | `BrowserKind.name` ([source](crates/ferrite-e2e/src/browser.rs#L47)) | Stock-browser discovery/launch; no channels/installer or full Playwright connection options. |

## CDPSession

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-cdpsession.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `CDPSession.close` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `CDPSession.event` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `CDPSession.detach` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `CDPSession.send` | method | Partial | `CdpConnection.call` ([source](crates/ferrite-e2e/src/cdp.rs#L154)) | Low-level CDP transport, with caller-managed session IDs; no dedicated CDPSession lifecycle. |

## Clock

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-clock.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Clock.fastForward` | method | Partial | `Page.clock_fast_forward` ([source](crates/ferrite-e2e/src/page.rs#L3593)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.install` | method | Partial | `Page.clock_install_at` ([source](crates/ferrite-e2e/src/page.rs#L3621)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.runFor` | method | Partial | `Page.clock_run_for` ([source](crates/ferrite-e2e/src/page.rs#L3600)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.pauseAt` | method | Partial | `Page.clock_pause_at` ([source](crates/ferrite-e2e/src/page.rs#L3614)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.resume` | method | Partial | `Page.clock_resume` ([source](crates/ferrite-e2e/src/page.rs#L3679)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.setFixedTime` | method | Partial | `Page.clock_set_fixed_time` ([source](crates/ferrite-e2e/src/page.rs#L3557)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.setSystemTime` | method | Partial | `Page.clock_set_system_time` ([source](crates/ferrite-e2e/src/page.rs#L3607)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |

## ConsoleMessage

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-consolemessage.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `ConsoleMessage.args` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `ConsoleMessage.location` | method | Partial | `ConsoleMessage.location` ([source](crates/ferrite-e2e/src/page.rs#L82)) | Native CDP/BiDi source URL/zero-based position, epoch-ms timestamp and owning page ID when available. Optional unknown metadata; page is an ID rather than a Page object. String previews, not JSHandle argument or worker ownership parity. Context history and attempt JSON/HTML/trace retain closed-page messages. |
| `ConsoleMessage.page` | method | Partial | `ConsoleMessage.page_id` ([source](crates/ferrite-e2e/src/page.rs#L88)) | Native CDP/BiDi source URL/zero-based position, epoch-ms timestamp and owning page ID when available. Optional unknown metadata; page is an ID rather than a Page object. String previews, not JSHandle argument or worker ownership parity. Context history and attempt JSON/HTML/trace retain closed-page messages. |
| `ConsoleMessage.text` | method | Partial | `ConsoleMessage.text` ([source](crates/ferrite-e2e/src/page.rs#L80)) | Native CDP/BiDi source URL/zero-based position, epoch-ms timestamp and owning page ID when available. Optional unknown metadata; page is an ID rather than a Page object. String previews, not JSHandle argument or worker ownership parity. Context history and attempt JSON/HTML/trace retain closed-page messages. |
| `ConsoleMessage.timestamp` | method | Partial | `ConsoleMessage.timestamp_ms` ([source](crates/ferrite-e2e/src/page.rs#L85)) | Native CDP/BiDi source URL/zero-based position, epoch-ms timestamp and owning page ID when available. Optional unknown metadata; page is an ID rather than a Page object. String previews, not JSHandle argument or worker ownership parity. Context history and attempt JSON/HTML/trace retain closed-page messages. |
| `ConsoleMessage.type` | method | Partial | `ConsoleMessage.kind` ([source](crates/ferrite-e2e/src/page.rs#L78)) | Native CDP/BiDi source URL/zero-based position, epoch-ms timestamp and owning page ID when available. Optional unknown metadata; page is an ID rather than a Page object. String previews, not JSHandle argument or worker ownership parity. Context history and attempt JSON/HTML/trace retain closed-page messages. |
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
| `Dialog.accept` | method | Partial | `DialogDecision.accept_with` ([source](crates/ferrite-e2e/src/page.rs#L1191)) | Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page. |
| `Dialog.defaultValue` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Dialog.dismiss` | method | Partial | `DialogDecision.dismiss` ([source](crates/ferrite-e2e/src/page.rs#L1196)) | Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page. |
| `Dialog.message` | method | Partial | `DialogInfo.message` ([source](crates/ferrite-e2e/src/page.rs#L230)) | Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page. |
| `Dialog.page` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Dialog.type` | method | Partial | `DialogInfo.dialog_type` ([source](crates/ferrite-e2e/src/page.rs#L228)) | Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page. |

## Disposable

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-disposable.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Disposable.dispose` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## Download

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-download.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Download.cancel` | method | Partial | `Page.cancel_downloads` ([source](crates/ferrite-e2e/src/page.rs#L4646)) | Cancels page-tracked downloads on Chromium; no per-Download.cancel method. |
| `Download.createReadStream` | method | Partial | `Download.create_read_stream` ([source](crates/ferrite-e2e/src/page.rs#L1528)) | Tokio AsyncRead/AsyncSeek file for a completed successful download; options bound open only, caller can wrap reads in CancellationToken.run. Active native streams remain unsupported. |
| `Download.delete` | method | Partial | `Download.delete` ([source](crates/ferrite-e2e/src/page.rs#L1584)) | Completed file deletion is idempotent only for NotFound; other filesystem errors propagate. Active downloads are not represented. |
| `Download.failure` | method | Partial | `Download.failure` ([source](crates/ferrite-e2e/src/page.rs#L1480)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.page` | method | Partial | `Download.page_id` ([source](crates/ferrite-e2e/src/page.rs#L1507)) | Owning native page identity without retaining a live Page; None for hand-built completed paths. No upstream live page object. |
| `Download.path` | method | Partial | `Download.path` ([source](crates/ferrite-e2e/src/page.rs#L1472)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.saveAs` | method | Partial | `Download.save_as` ([source](crates/ferrite-e2e/src/page.rs#L1568)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.suggestedFilename` | method | Partial | `Download.suggested_filename` ([source](crates/ferrite-e2e/src/page.rs#L1474)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.url` | method | Partial | `Download.url` ([source](crates/ferrite-e2e/src/page.rs#L1477)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |

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
| `Fixtures.browser` | property | Partial | `Browser` ([source](crates/ferrite-e2e/src/browser.rs#L320)) | Manual Browser/Context/client setup or injected Page; runner injects a fresh context/page for every attempt. |
| `Fixtures.browserName` | property | Partial | `Browser.kind` ([source](crates/ferrite-e2e/src/browser.rs#L721)) | Manual Browser/Context/client setup or injected Page; runner injects a fresh context/page for every attempt. |
| `Fixtures.context` | property | Partial | `TestContext.context` ([source](crates/ferrite-e2e/src/runner.rs#L815)) | Fresh per-attempt resource, usable as a typed fixture dependency. request is isolated from browser cookies; context.request() shares cookies. |
| `Fixtures.mount` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Fixtures.page` | property | Partial | `TestContext.page` ([source](crates/ferrite-e2e/src/runner.rs#L813)) | Fresh per-attempt resource, usable as a typed fixture dependency. request is isolated from browser cookies; context.request() shares cookies. |
| `Fixtures.request` | property | Partial | `TestContext.request` ([source](crates/ferrite-e2e/src/runner.rs#L817)) | Fresh per-attempt resource, usable as a typed fixture dependency. request is isolated from browser cookies; context.request() shares cookies. |

## Frame

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-frame.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Frame.addScriptTag` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.addStyleTag` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.check` | method | Partial | `Locator.check` ([source](crates/ferrite-e2e/src/locator.rs#L2206)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.childFrames` | method | Partial | `Frame.child_frames` ([source](crates/ferrite-e2e/src/page.rs#L610)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.click` | method | Partial | `Locator.click` ([source](crates/ferrite-e2e/src/locator.rs#L1465)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.content` | method | Equivalent | `Frame.content` ([source](crates/ferrite-e2e/src/page.rs#L644)) | Basic document access. |
| `Frame.dblclick` | method | Partial | `Locator.dblclick` ([source](crates/ferrite-e2e/src/locator.rs#L1605)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.dispatchEvent` | method | Partial | `Locator.dispatch_event` ([source](crates/ferrite-e2e/src/locator.rs#L1820)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.dragAndDrop` | method | Partial | `Locator.drag_to` ([source](crates/ferrite-e2e/src/locator.rs#L1740)) | Frame.locator(source).drag_to(target, steps); fewer options and frame-coordinate restrictions. |
| `Frame.evalOnSelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.evalOnSelectorAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.evaluate` | method | Partial | `Frame.evaluate` ([source](crates/ferrite-e2e/src/page.rs#L653)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.evaluateHandle` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.fill` | method | Partial | `Locator.fill` ([source](crates/ferrite-e2e/src/locator.rs#L1966)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.focus` | method | Partial | `Locator.focus` ([source](crates/ferrite-e2e/src/locator.rs#L1668)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.frameElement` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.frameLocator` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.getAttribute` | method | Partial | `Locator.attribute` ([source](crates/ferrite-e2e/src/locator.rs#L2435)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.getByAltText` | method | Partial | `Frame.get_by_alt` ([source](crates/ferrite-e2e/src/page.rs#L711)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByLabel` | method | Partial | `Frame.get_by_label` ([source](crates/ferrite-e2e/src/page.rs#L701)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByPlaceholder` | method | Partial | `Frame.get_by_placeholder` ([source](crates/ferrite-e2e/src/page.rs#L706)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByRole` | method | Partial | `Frame.get_by_role_with` ([source](crates/ferrite-e2e/src/page.rs#L692)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByTestId` | method | Partial | `Frame.get_by_test_id` ([source](crates/ferrite-e2e/src/page.rs#L677)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByText` | method | Partial | `Frame.get_by_text` ([source](crates/ferrite-e2e/src/page.rs#L682)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByTitle` | method | Partial | `Frame.get_by_title` ([source](crates/ferrite-e2e/src/page.rs#L716)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.goto` | method | Partial | `Frame.goto` ([source](crates/ferrite-e2e/src/page.rs#L588)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.hover` | method | Partial | `Locator.hover` ([source](crates/ferrite-e2e/src/locator.rs#L1632)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.innerHTML` | method | Partial | `Locator.inner_html` ([source](crates/ferrite-e2e/src/locator.rs#L2513)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.innerText` | method | Partial | `Locator.inner_text` ([source](crates/ferrite-e2e/src/locator.rs#L916)) | Frame.locator(selector) followed by the distinct rendered/raw text getter; strict resolution and same-origin frame limitations remain. |
| `Frame.inputValue` | method | Partial | `Locator.input_value` ([source](crates/ferrite-e2e/src/locator.rs#L2383)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isChecked` | method | Partial | `Locator.is_checked` ([source](crates/ferrite-e2e/src/locator.rs#L2713)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isDetached` | method | Partial | `Frame.is_detached` ([source](crates/ferrite-e2e/src/page.rs#L622)) | Asynchronous native tree identity check; explicit page closure is detached, disconnection errors propagate. No replacement retargeting. |
| `Frame.isDisabled` | method | Partial | `Locator.is_disabled` ([source](crates/ferrite-e2e/src/locator.rs#L2695)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isEditable` | method | Partial | `Locator.is_editable` ([source](crates/ferrite-e2e/src/locator.rs#L2731)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isEnabled` | method | Partial | `Locator.is_enabled` ([source](crates/ferrite-e2e/src/locator.rs#L2677)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isHidden` | method | Partial | `Locator.is_hidden` ([source](crates/ferrite-e2e/src/locator.rs#L2656)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isVisible` | method | Partial | `Locator.is_visible` ([source](crates/ferrite-e2e/src/locator.rs#L2638)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.locator` | method | Partial | `Frame.locator` ([source](crates/ferrite-e2e/src/page.rs#L672)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.name` | method | Partial | `Frame.name` ([source](crates/ferrite-e2e/src/page.rs#L577)) | Native lookup name snapshot; Chromium reports names, Firefox metadata is empty. No inferred name. |
| `Frame.page` | method | Partial | `Frame.page` ([source](crates/ferrite-e2e/src/page.rs#L465)) | Frame-scoped counterpart; unit-returning waits, fewer predicate/options modes; frame NetworkIdle remains unsupported. current_url() reads navigation updates. |
| `Frame.parentFrame` | method | Partial | `Frame.parent` ([source](crates/ferrite-e2e/src/page.rs#L597)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.press` | method | Partial | `Locator.press` ([source](crates/ferrite-e2e/src/locator.rs#L2008)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.querySelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.querySelectorAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.selectOption` | method | Partial | `Locator.select_options` ([source](crates/ferrite-e2e/src/locator.rs#L2298)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.setChecked` | method | Partial | `Locator.set_checked` ([source](crates/ferrite-e2e/src/locator.rs#L2829)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.setContent` | method | Partial | `Frame.set_content` ([source](crates/ferrite-e2e/src/page.rs#L470)) | Frame-scoped counterpart; unit-returning waits, fewer predicate/options modes; frame NetworkIdle remains unsupported. current_url() reads navigation updates. |
| `Frame.setInputFiles` | method | Partial | `Locator.set_input_file_payloads` ([source](crates/ferrite-e2e/src/locator.rs#L2182)) | FilePayload filename/MIME/bytes or existing path uploads; empty lists clear, multiple files require a multiple input, 64 MiB total cap. DOM File/DataTransfer injection on both engines; no native chooser/directory upload/options parity. Frame uses Frame.locator. |
| `Frame.tap` | method | Partial | `Locator.tap` ([source](crates/ferrite-e2e/src/locator.rs#L1710)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.textContent` | method | Partial | `Locator.text_content` ([source](crates/ferrite-e2e/src/locator.rs#L892)) | Frame.locator(selector) followed by the distinct rendered/raw text getter; strict resolution and same-origin frame limitations remain. |
| `Frame.title` | method | Equivalent | `Frame.title` ([source](crates/ferrite-e2e/src/page.rs#L635)) | Basic document access. |
| `Frame.type` (deprecated) | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L2050)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.uncheck` | method | Partial | `Locator.uncheck` ([source](crates/ferrite-e2e/src/locator.rs#L2226)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.url` | method | Partial | `Frame.current_url` ([source](crates/ferrite-e2e/src/page.rs#L507)) | current_url() reads live URL; url() retains lookup snapshot. Native realm/tree errors propagate after detach. |
| `Frame.waitForFunction` | method | Partial | `Frame.wait_for_function_value` ([source](crates/ferrite-e2e/src/page.rs#L495)) | JSON arguments, native animation-frame/interval polling and captured JSON results; frame remote handles unsupported. Legacy expression helper returns unit. |
| `Frame.waitForLoadState` | method | Partial | `Frame.wait_for_load_state` ([source](crates/ferrite-e2e/src/page.rs#L515)) | Frame-scoped counterpart; unit-returning waits, fewer predicate/options modes; frame NetworkIdle remains unsupported. current_url() reads navigation updates. |
| `Frame.waitForNavigation` (deprecated) | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.waitForSelector` | method | Partial | `Frame.wait_for_selector` ([source](crates/ferrite-e2e/src/page.rs#L519)) | Frame-scoped counterpart; unit-returning waits, fewer predicate/options modes; frame NetworkIdle remains unsupported. current_url() reads navigation updates. |
| `Frame.waitForTimeout` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.waitForURL` | method | Partial | `Frame.wait_for_url_matching_with_options` ([source](crates/ferrite-e2e/src/page.rs#L553)) | Exact (relative to base URL), full-URL glob/regex or predicate with Commit/DOMContentLoaded/Load readiness in one navigation budget. Page NetworkIdle uses 500ms observed HTTP quiet; frames reject it. Legacy helpers retain substring/URL-only semantics. Zero timeout, cancellation and enclosing budgets supported; URLPattern absent. |

## FrameLocator

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-framelocator.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `FrameLocator.first` (deprecated) | method | Partial | `FrameLocator.first` ([source](crates/ferrite-e2e/src/frame_locator.rs#L30)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.frameLocator` | method | Partial | `FrameLocator.frame_locator` ([source](crates/ferrite-e2e/src/frame_locator.rs#L41)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.getByAltText` | method | Partial | `FrameLocator.get_by_alt` ([source](crates/ferrite-e2e/src/frame_locator.rs#L65)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.getByLabel` | method | Partial | `FrameLocator.get_by_label` ([source](crates/ferrite-e2e/src/frame_locator.rs#L56)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.getByPlaceholder` | method | Partial | `FrameLocator.get_by_placeholder` ([source](crates/ferrite-e2e/src/frame_locator.rs#L62)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.getByRole` | method | Partial | `FrameLocator.get_by_role` ([source](crates/ferrite-e2e/src/frame_locator.rs#L47)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.getByTestId` | method | Partial | `FrameLocator.get_by_test_id` ([source](crates/ferrite-e2e/src/frame_locator.rs#L59)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.getByText` | method | Partial | `FrameLocator.get_by_text` ([source](crates/ferrite-e2e/src/frame_locator.rs#L53)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.getByTitle` | method | Partial | `FrameLocator.get_by_title` ([source](crates/ferrite-e2e/src/frame_locator.rs#L68)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.last` (deprecated) | method | Partial | `FrameLocator.last` ([source](crates/ferrite-e2e/src/frame_locator.rs#L33)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.locator` | method | Partial | `FrameLocator.locator` ([source](crates/ferrite-e2e/src/frame_locator.rs#L44)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.nth` (deprecated) | method | Partial | `FrameLocator.nth` ([source](crates/ferrite-e2e/src/frame_locator.rs#L36)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |
| `FrameLocator.owner` | method | Partial | `FrameLocator.owner` ([source](crates/ferrite-e2e/src/frame_locator.rs#L26)) | Lazy nested/replacement frame selection through same-origin DOM evaluation; cross-origin/OOPIF and selector-free cross-frame traversal deferred. |

## FullConfig

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-fullconfig.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `FullConfig.argv` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.configFile` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.failOnFlakyTests` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.forbidOnly` | property | Partial | `Runner.forbid_only` ([source](crates/ferrite-e2e/src/runner.rs#L1884)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.fullyParallel` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.globalSetup` | property | Partial | `Runner.global_setup` ([source](crates/ferrite-e2e/src/runner.rs#L1797)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.globalTeardown` | property | Partial | `Runner.global_teardown` ([source](crates/ferrite-e2e/src/runner.rs#L1808)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.globalTimeout` | property | Partial | `E2eConfig.global_timeout_ms` ([source](crates/ferrite-config/src/lib.rs#L418)) | Consumed by Runner and CLI; global cancellation with bounded teardown and final unexpected-failure scheduling limit. Active workers finish on maxFailures; no process-worker orchestration. |
| `FullConfig.grep` | property | Partial | `Runner.grep` ([source](crates/ferrite-e2e/src/runner.rs#L1720)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.grepInvert` | property | Partial | `Runner.grep_invert` ([source](crates/ferrite-e2e/src/runner.rs#L1729)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.maxFailures` | property | Partial | `E2eConfig.max_failures` ([source](crates/ferrite-config/src/lib.rs#L420)) | Consumed by Runner and CLI; global cancellation with bounded teardown and final unexpected-failure scheduling limit. Active workers finish on maxFailures; no process-worker orchestration. |
| `FullConfig.metadata` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.preserveOutput` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.projects` | property | Partial | `Runner.project` ([source](crates/ferrite-e2e/src/runner.rs#L1868)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.quiet` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.reporter` | property | Partial | `E2eConfig.reporter` ([source](crates/ferrite-config/src/lib.rs#L430)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.reportSlowTests` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.rootDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.shard` | property | Partial | `Runner.shard` ([source](crates/ferrite-e2e/src/runner.rs#L1826)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.tags` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.updateSnapshots` | property | Partial | `E2eConfig.update_snapshots` ([source](crates/ferrite-config/src/lib.rs#L442)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.updateSourceMethod` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.version` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.webServer` | property | Partial | `E2eConfig.web_server` ([source](crates/ferrite-config/src/lib.rs#L446)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.workers` | property | Partial | `Runner.workers` ([source](crates/ferrite-e2e/src/runner.rs#L1625)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |

## FullProject

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-fullproject.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `FullProject.dependencies` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.grep` | property | Partial | `Project.grep` ([source](crates/ferrite-e2e/src/runner.rs#L1271)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `FullProject.grepInvert` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.ignoreSnapshots` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.metadata` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.name` | property | Partial | `Project.name` ([source](crates/ferrite-e2e/src/runner.rs#L1231)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `FullProject.snapshotDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.outputDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.repeatEach` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.retries` | property | Partial | `Project.retries` ([source](crates/ferrite-e2e/src/runner.rs#L1278)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `FullProject.teardown` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.testDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.testIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.testMatch` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.timeout` | property | Partial | `Project.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L1285)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `FullProject.use` | property | Partial | `Project.context_options` ([source](crates/ferrite-e2e/src/runner.rs#L1255)) | Project-specific context settings; optional browser/launch overrides, suite/test context inheritance, no named fixture overrides or project dependencies. |

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
| `Keyboard.down` | method | Partial | `Page.key_down` ([source](crates/ferrite-e2e/src/page.rs#L3121)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |
| `Keyboard.insertText` | method | Partial | `Page.insert_text` ([source](crates/ferrite-e2e/src/page.rs#L3041)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |
| `Keyboard.press` | method | Partial | `Page.press_key_with` ([source](crates/ferrite-e2e/src/page.rs#L3063)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |
| `Keyboard.type` | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L2050)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |
| `Keyboard.up` | method | Partial | `Page.key_up` ([source](crates/ferrite-e2e/src/page.rs#L3132)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |

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
| `Locator.all` | method | Equivalent | `Locator.all` ([source](crates/ferrite-e2e/src/locator.rs#L1206)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.allInnerTexts` | method | Partial | `Locator.all_inner_texts` ([source](crates/ferrite-e2e/src/locator.rs#L871)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.allTextContents` | method | Partial | `Locator.all_text_contents` ([source](crates/ferrite-e2e/src/locator.rs#L850)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.and` | method | Equivalent | `Locator.and_` ([source](crates/ferrite-e2e/src/locator.rs#L1155)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.ariaSnapshot` | method | Partial | `Locator.aria_snapshot` ([source](crates/ferrite-e2e/src/locator.rs#L973)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.ariaSnapshotJSON` | method | Partial | `Locator.aria_snapshot_json` ([source](crates/ferrite-e2e/src/locator.rs#L952)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.blur` | method | Partial | `Locator.blur` ([source](crates/ferrite-e2e/src/locator.rs#L1689)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.boundingBox` | method | Partial | `Locator.bounding_box` ([source](crates/ferrite-e2e/src/locator.rs#L2785)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.check` | method | Partial | `Locator.check_with_options` ([source](crates/ferrite-e2e/src/locator.rs#L2247)) | Trusted pointer input with padding-box positions, modifiers, trial readiness and scoped timeout. Trial may scroll but sends no input; action-acquired keys/buttons are released on failure/cancellation. Same-origin offsets and positive axis scaling supported; rotated/perspective frames and cross-origin coordinates unsupported. Other upstream options remain narrower. |
| `Locator.clear` | method | Partial | `Locator.clear` ([source](crates/ferrite-e2e/src/locator.rs#L2089)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.click` | method | Partial | `Locator.click_with_options` ([source](crates/ferrite-e2e/src/locator.rs#L1485)) | Trusted pointer input with padding-box positions, modifiers, trial readiness and scoped timeout. Trial may scroll but sends no input; action-acquired keys/buttons are released on failure/cancellation. Same-origin offsets and positive axis scaling supported; rotated/perspective frames and cross-origin coordinates unsupported. Other upstream options remain narrower. |
| `Locator.count` | method | Equivalent | `Locator.count` ([source](crates/ferrite-e2e/src/locator.rs#L1275)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.dblclick` | method | Partial | `Locator.dblclick` ([source](crates/ferrite-e2e/src/locator.rs#L1605)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.describe` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.description` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.dispatchEvent` | method | Partial | `Locator.dispatch_event_with` ([source](crates/ferrite-e2e/src/locator.rs#L1854)) | Typed synthetic DOM constructors with JSON event-specific initialization and bubbles/cancelable/composed flags; CustomEvent legacy helper retained. Auto input events follow the pinned Event constructor; InputEvent can be requested explicitly. No live handle arguments. |
| `Locator.dragTo` | method | Partial | `Locator.drag_to_with_options` ([source](crates/ferrite-e2e/src/locator.rs#L1744)) | Trusted pointer input with padding-box positions, modifiers, trial readiness and scoped timeout. Trial may scroll but sends no input; action-acquired keys/buttons are released on failure/cancellation. Same-origin offsets and positive axis scaling supported; rotated/perspective frames and cross-origin coordinates unsupported. Other upstream options remain narrower. |
| `Locator.drop` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.elementHandle` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.elementHandles` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.contentFrame` | method | Partial | `Locator.content_frame` ([source](crates/ferrite-e2e/src/locator.rs#L758)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.evaluate` | method | Partial | `Locator.evaluate_with_arg` ([source](crates/ferrite-e2e/src/locator.rs#L794)) | Function with element and JSON argument; no JSHandle arguments or arbitrary JS result serialization. |
| `Locator.evaluateAll` | method | Partial | `Locator.evaluate_all` ([source](crates/ferrite-e2e/src/locator.rs#L822)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.evaluateHandle` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.fill` | method | Partial | `Locator.fill` ([source](crates/ferrite-e2e/src/locator.rs#L1966)) | Retries visibility/enabled/editability, uses native input setters or contenteditable text plus input/change events; not all native input validation/event semantics. |
| `Locator.filter` | method | Partial | `Locator.filter_with` ([source](crates/ferrite-e2e/src/locator.rs#L1118)) | Relative has/hasNot/text and visibility filters; exact/regex matching through locator builders. Inner locators must share the document. |
| `Locator.first` | method | Partial | `Locator.first` ([source](crates/ferrite-e2e/src/locator.rs#L1063)) | Narrows the resolved set to its first match, including count/all; selector semantics remain narrower. |
| `Locator.focus` | method | Partial | `Locator.focus` ([source](crates/ferrite-e2e/src/locator.rs#L1668)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.frameLocator` | method | Partial | `Locator.frame_locator` ([source](crates/ferrite-e2e/src/locator.rs#L763)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.getAttribute` | method | Partial | `Locator.attribute` ([source](crates/ferrite-e2e/src/locator.rs#L2435)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.getByAltText` | method | Partial | `Locator.get_by_alt` ([source](crates/ferrite-e2e/src/locator.rs#L1264)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByLabel` | method | Partial | `Locator.get_by_label` ([source](crates/ferrite-e2e/src/locator.rs#L1252)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByPlaceholder` | method | Partial | `Locator.get_by_placeholder` ([source](crates/ferrite-e2e/src/locator.rs#L1258)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByRole` | method | Partial | `Locator.get_by_role_with` ([source](crates/ferrite-e2e/src/locator.rs#L1246)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByTestId` | method | Partial | `Locator.get_by_test_id` ([source](crates/ferrite-e2e/src/locator.rs#L1228)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByText` | method | Partial | `Locator.get_by_text` ([source](crates/ferrite-e2e/src/locator.rs#L1234)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByTitle` | method | Partial | `Locator.get_by_title` ([source](crates/ferrite-e2e/src/locator.rs#L1270)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.hideHighlight` | method | Partial | `Locator.hide_highlight` ([source](crates/ferrite-e2e/src/locator.rs#L940)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.highlight` | method | Partial | `Locator.highlight` ([source](crates/ferrite-e2e/src/locator.rs#L2803)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.hover` | method | Partial | `Locator.hover_with_options` ([source](crates/ferrite-e2e/src/locator.rs#L1636)) | Trusted pointer input with padding-box positions, modifiers, trial readiness and scoped timeout. Trial may scroll but sends no input; action-acquired keys/buttons are released on failure/cancellation. Same-origin offsets and positive axis scaling supported; rotated/perspective frames and cross-origin coordinates unsupported. Other upstream options remain narrower. |
| `Locator.innerHTML` | method | Partial | `Locator.inner_html` ([source](crates/ferrite-e2e/src/locator.rs#L2513)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.innerText` | method | Partial | `Locator.inner_text` ([source](crates/ferrite-e2e/src/locator.rs#L916)) | Distinct rendered DOM innerText getter; strict resolution, nullable Rust result and same-origin lazy frame limitations remain. |
| `Locator.inputValue` | method | Partial | `Locator.input_value` ([source](crates/ferrite-e2e/src/locator.rs#L2383)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isChecked` | method | Partial | `Locator.is_checked` ([source](crates/ferrite-e2e/src/locator.rs#L2713)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isDisabled` | method | Partial | `Locator.is_disabled` ([source](crates/ferrite-e2e/src/locator.rs#L2695)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isEditable` | method | Partial | `Locator.is_editable` ([source](crates/ferrite-e2e/src/locator.rs#L2731)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isEnabled` | method | Partial | `Locator.is_enabled` ([source](crates/ferrite-e2e/src/locator.rs#L2677)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isHidden` | method | Partial | `Locator.is_hidden` ([source](crates/ferrite-e2e/src/locator.rs#L2656)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isVisible` | method | Partial | `Locator.is_visible` ([source](crates/ferrite-e2e/src/locator.rs#L2638)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.last` | method | Partial | `Locator.last` ([source](crates/ferrite-e2e/src/locator.rs#L1069)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.locator` | method | Partial | `Locator.locator` ([source](crates/ferrite-e2e/src/locator.rs#L1057)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.normalize` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.nth` | method | Equivalent | `Locator.nth` ([source](crates/ferrite-e2e/src/locator.rs#L1075)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.or` | method | Equivalent | `Locator.or_` ([source](crates/ferrite-e2e/src/locator.rs#L1150)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.page` | method | Partial | `Locator.page` ([source](crates/ferrite-e2e/src/locator.rs#L768)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.press` | method | Partial | `Locator.press_with` ([source](crates/ferrite-e2e/src/locator.rs#L2029)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.pressSequentially` | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L2050)) | Trusted per-character key input with optional delay; fewer keyboard layout/modifier options. |
| `Locator.screenshot` | method | Partial | `Locator.screenshot` ([source](crates/ferrite-e2e/src/locator.rs#L1936)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.scrollIntoViewIfNeeded` | method | Partial | `Locator.scroll_into_view` ([source](crates/ferrite-e2e/src/locator.rs#L1799)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.selectOption` | method | Partial | `Locator.select_options` ([source](crates/ferrite-e2e/src/locator.rs#L2298)) | Value/label/index matching exists; returns unit rather than selected values, with fewer options. |
| `Locator.selectText` | method | Partial | `Locator.select_text` ([source](crates/ferrite-e2e/src/locator.rs#L1899)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.setChecked` | method | Partial | `Locator.set_checked_with_options` ([source](crates/ferrite-e2e/src/locator.rs#L2253)) | Trusted pointer input with padding-box positions, modifiers, trial readiness and scoped timeout. Trial may scroll but sends no input; action-acquired keys/buttons are released on failure/cancellation. Same-origin offsets and positive axis scaling supported; rotated/perspective frames and cross-origin coordinates unsupported. Other upstream options remain narrower. |
| `Locator.setInputFiles` | method | Partial | `Locator.set_input_file_payloads` ([source](crates/ferrite-e2e/src/locator.rs#L2182)) | FilePayload filename/MIME/bytes or existing path uploads; empty lists clear, multiple files require a multiple input, 64 MiB total cap. DOM File/DataTransfer injection on both engines; no native chooser/directory upload/options parity. Frame uses Frame.locator. |
| `Locator.tap` | method | Partial | `Locator.tap` ([source](crates/ferrite-e2e/src/locator.rs#L1710)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.textContent` | method | Partial | `Locator.text_content` ([source](crates/ferrite-e2e/src/locator.rs#L892)) | Untrimmed nullable DOM textContent with strict single-target resolution; no ElementHandle or full options surface. |
| `Locator.toString` | method | Idiomatic | `Locator.selector` ([source](crates/ferrite-e2e/src/locator.rs#L1040)) | Selector string available; no Playwright locator expression representation. |
| `Locator.type` (deprecated) | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L2050)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.uncheck` | method | Partial | `Locator.uncheck_with_options` ([source](crates/ferrite-e2e/src/locator.rs#L2250)) | Trusted pointer input with padding-box positions, modifiers, trial readiness and scoped timeout. Trial may scroll but sends no input; action-acquired keys/buttons are released on failure/cancellation. Same-origin offsets and positive axis scaling supported; rotated/perspective frames and cross-origin coordinates unsupported. Other upstream options remain narrower. |
| `Locator.visible` | method | Partial | `Locator.visible` ([source](crates/ferrite-e2e/src/locator.rs#L773)) | Lazy visibility filter reapplied when resolving; uses the shared DOM visibility approximation. |
| `Locator.waitFor` | method | Partial | `Locator.wait_for_state` ([source](crates/ferrite-e2e/src/locator.rs#L1352)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.waitForFunction` | method | Partial | `Locator.wait_for_function` ([source](crates/ferrite-e2e/src/locator.rs#L997)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |

## LocatorAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-locatorassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `LocatorAssertions.not` | property | Partial | `LocatorExpect.not` ([source](crates/ferrite-e2e/src/expect.rs#L761)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeAttached` | method | Partial | `LocatorExpect.attached` ([source](crates/ferrite-e2e/src/expect.rs#L1231)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeChecked` | method | Partial | `LocatorExpect.checked_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L358)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toBeDisabled` | method | Partial | `LocatorExpect.disabled` ([source](crates/ferrite-e2e/src/expect.rs#L1086)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeEditable` | method | Partial | `LocatorExpect.editable` ([source](crates/ferrite-e2e/src/expect.rs#L1120)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeEmpty` | method | Partial | `LocatorExpect.empty` ([source](crates/ferrite-e2e/src/expect.rs#L1157)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeEnabled` | method | Partial | `LocatorExpect.enabled` ([source](crates/ferrite-e2e/src/expect.rs#L1052)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeFocused` | method | Partial | `LocatorExpect.focused` ([source](crates/ferrite-e2e/src/expect.rs#L1197)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeHidden` | method | Partial | `LocatorExpect.hidden` ([source](crates/ferrite-e2e/src/expect.rs#L804)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeInViewport` | method | Partial | `LocatorExpect.in_viewport_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L389)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toBeVisible` | method | Partial | `LocatorExpect.visible` ([source](crates/ferrite-e2e/src/expect.rs#L767)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toContainClass` | method | Partial | `LocatorExpect.contains_class_tokens` ([source](crates/ferrite-e2e/src/assertion_options.rs#L324)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toContainText` | method | Partial | `LocatorExpect.contains_texts_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L269)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toHaveAccessibleDescription` | method | Partial | `LocatorExpect.accessible_description_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L441)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toHaveAccessibleErrorMessage` | method | Partial | `LocatorExpect.accessible_error_message_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L448)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toHaveAccessibleName` | method | Partial | `LocatorExpect.accessible_name_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L434)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toHaveAttribute` | method | Partial | `LocatorExpect.attribute` ([source](crates/ferrite-e2e/src/expect.rs#L1265)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveClass` | method | Partial | `LocatorExpect.class_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L276)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toHaveCount` | method | Partial | `LocatorExpect.count` ([source](crates/ferrite-e2e/src/expect.rs#L962)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveCSS` | method | Partial | `LocatorExpect.css` ([source](crates/ferrite-e2e/src/expect.rs#L1364)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveId` | method | Partial | `LocatorExpect.id` ([source](crates/ferrite-e2e/src/expect.rs#L1352)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveJSProperty` | method | Partial | `LocatorExpect.js_property` ([source](crates/ferrite-e2e/src/expect.rs#L1406)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveRole` | method | Partial | `LocatorExpect.role` ([source](crates/ferrite-e2e/src/expect.rs#L698)) | Retrying dedicated matcher; ARIA snapshots use exact text and a DOM approximation, not full YAML matching. |
| `LocatorAssertions.toHaveScreenshot` | method | Partial | `LocatorExpect.screenshot_with` ([source](crates/ferrite-e2e/src/expect.rs#L1551)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveText` | method | Partial | `LocatorExpect.text_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L205)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toHaveValue` | method | Partial | `LocatorExpect.value` ([source](crates/ferrite-e2e/src/expect.rs#L922)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveValues` | method | Partial | `LocatorExpect.values_with` ([source](crates/ferrite-e2e/src/assertion_options.rs#L307)) | Dedicated options API: raw Rust regex vs normalized string text, rendered-text/case options, mixed lists and ordered text subsets, exact class order vs token containment, checkbox indeterminate and native viewport ratios. Accessible computation remains a DOM approximation; some upstream overload/options remain absent. |
| `LocatorAssertions.toMatchAriaSnapshot` | method | Partial | `LocatorExpect.aria_snapshot` ([source](crates/ferrite-e2e/src/expect.rs#L735)) | Retrying dedicated matcher; ARIA snapshots use exact text and a DOM approximation, not full YAML matching. |

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
| `Mouse.click` | method | Partial | `Page.mouse_click_with` ([source](crates/ferrite-e2e/src/page.rs#L3014)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.dblclick` | method | Partial | `Page.mouse_click` ([source](crates/ferrite-e2e/src/page.rs#L3003)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.down` | method | Partial | `Page.mouse_down` ([source](crates/ferrite-e2e/src/page.rs#L3077)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.move` | method | Partial | `Page.mouse_move` ([source](crates/ferrite-e2e/src/page.rs#L3030)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.up` | method | Partial | `Page.mouse_up` ([source](crates/ferrite-e2e/src/page.rs#L3088)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.wheel` | method | Partial | `Page.mouse_wheel` ([source](crates/ferrite-e2e/src/page.rs#L3110)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |

## Page

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-page.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Page.clock` | property | Partial | `Page.clock_install` ([source](crates/ferrite-e2e/src/page.rs#L3525)) | Fake clock is scoped to the current document and resets on navigation; semantic differences below. |
| `Page.close` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1932)) | PageEvent.Closed via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.console` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1932)) | PageEvent.Console via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.crash` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.dialog` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1932)) | PageEvent.Dialog via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.dialogClosed` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.DOMContentLoaded` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.download` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1932)) | PageEvent.Download via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.fileChooser` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.frameAttached` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.frameDetached` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.frameNavigated` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.load` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.pageError` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.popup` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1932)) | PageEvent.Popup via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.request` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1932)) | PageEvent.Request via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.requestFailed` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1932)) | RequestFailed enum events on Chromium/Firefox with request IDs and method/URL; failed requests carry transport error text. Context events include page identity. No rich live Request object graph. |
| `Page.requestFinished` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1932)) | RequestFinished enum events on Chromium/Firefox with request IDs and method/URL; failed requests carry transport error text. Context events include page identity. No rich live Request object graph. |
| `Page.response` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1932)) | PageEvent.Response via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.webSocket` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1932)) | PageEvent.WebSocket via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.worker` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.addInitScript` | method | Partial | `Page.add_init_script` ([source](crates/ferrite-e2e/src/page.rs#L4437)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.addScriptTag` | method | Partial | `Page.add_script_tag_url` ([source](crates/ferrite-e2e/src/page.rs#L4444)) | URL/content helpers return unit; no ElementHandle return, file/type options; other helper: Page.add_script_tag_content |
| `Page.addStyleTag` | method | Partial | `Page.add_style_tag_url` ([source](crates/ferrite-e2e/src/page.rs#L4478)) | URL/content helpers return unit; no ElementHandle return, file/type options; other helper: Page.add_style_tag_content |
| `Page.bringToFront` | method | Partial | `Page.bring_to_front` ([source](crates/ferrite-e2e/src/page.rs#L2417)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.cancelPickLocator` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.check` | method | Partial | `Locator.check` ([source](crates/ferrite-e2e/src/locator.rs#L2206)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.click` | method | Partial | `Locator.click` ([source](crates/ferrite-e2e/src/locator.rs#L1465)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.close` | method | Partial | `Page.close` ([source](crates/ferrite-e2e/src/page.rs#L5147)) | Closes the target and its owning convenience context; no runBeforeUnload/reason options. |
| `Page.content` | method | Equivalent | `Page.content` ([source](crates/ferrite-e2e/src/page.rs#L2386)) | Basic document access. |
| `Page.context` | method | Partial | `Page.context` ([source](crates/ferrite-e2e/src/page.rs#L1879)) | Owning context while registered; returns Option and becomes None after context disposal. |
| `Page.coverage` | property | Partial | `Page.coverage` ([source](crates/ferrite-e2e/src/page.rs#L1849)) | Dedicated API exists; coverage is Chromium-only, lazy frame locators are same-origin, exceptions have ConsoleMessage shape. |
| `Page.dblclick` | method | Partial | `Locator.dblclick` ([source](crates/ferrite-e2e/src/locator.rs#L1605)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.dispatchEvent` | method | Partial | `Locator.dispatch_event` ([source](crates/ferrite-e2e/src/locator.rs#L1820)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.dragAndDrop` | method | Partial | `Locator.drag_to` ([source](crates/ferrite-e2e/src/locator.rs#L1740)) | Page.locator(source).drag_to(target, steps); fewer options and frame-coordinate restrictions. |
| `Page.emulateMedia` | method | Partial | `Page.emulate_media` ([source](crates/ferrite-e2e/src/page.rs#L4043)) | Chromium only; color scheme/reduced motion only, no full media/forcedColors/contrast surface. |
| `Page.evalOnSelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.evalOnSelectorAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.evaluate` | method | Partial | `Page.evaluate_with_arg` ([source](crates/ferrite-e2e/src/page.rs#L2424)) | JSON-serializable arguments and results; no JSHandle argument or arbitrary JS value serialization. |
| `Page.evaluateHandle` | method | Partial | `Page.evaluate_handle` ([source](crates/ferrite-e2e/src/page.rs#L2489)) | Remote JSHandle supported; no ElementHandle conversion or separate evaluation argument. |
| `Page.exposeBinding` | method | Partial | `Page.expose_binding` ([source](crates/ferrite-e2e/src/callbacks.rs#L229)) | Async JSON binding with owning context/page/native frame identity. Same-origin frame dispatch; native startup preloads and navigation/disposal cleanup. Cross-origin/OOPIF callers and handle arguments deferred. |
| `Page.exposeFunction` | method | Partial | `Page.expose_function_async` ([source](crates/ferrite-e2e/src/callbacks.rs#L217)) | Sync/async JSON callbacks in current/future same-origin documents; independent bounded dispatch, native preload ownership, duplicate-name errors and named removal. Rust errors/panics reject JS promises; cross-origin dispatch/handle arguments deferred. |
| `Page.fill` | method | Partial | `Locator.fill` ([source](crates/ferrite-e2e/src/locator.rs#L1966)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.focus` | method | Partial | `Locator.focus` ([source](crates/ferrite-e2e/src/locator.rs#L1668)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.frame` | method | Partial | `Page.frame_by_url_matching` ([source](crates/ferrite-e2e/src/page.rs#L4546)) | Exact/contains/glob/regex or URL predicate snapshot lookup, plus existing name/substring helpers; relative matchers resolve base URL. Native Firefox frame names remain empty. |
| `Page.frameLocator` | method | Partial | `Page.frame_locator` ([source](crates/ferrite-e2e/src/page.rs#L1837)) | Dedicated API exists; coverage is Chromium-only, lazy frame locators are same-origin, exceptions have ConsoleMessage shape. |
| `Page.frames` | method | Partial | `Page.document_frames` ([source](crates/ferrite-e2e/src/page.rs#L4514)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.getAttribute` | method | Partial | `Locator.attribute` ([source](crates/ferrite-e2e/src/locator.rs#L2435)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.getByAltText` | method | Partial | `Page.get_by_alt` ([source](crates/ferrite-e2e/src/page.rs#L2803)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByLabel` | method | Partial | `Page.get_by_label` ([source](crates/ferrite-e2e/src/page.rs#L2791)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByPlaceholder` | method | Partial | `Page.get_by_placeholder` ([source](crates/ferrite-e2e/src/page.rs#L2797)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByRole` | method | Partial | `Page.get_by_role_with` ([source](crates/ferrite-e2e/src/page.rs#L2779)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByTestId` | method | Partial | `Page.get_by_test_id` ([source](crates/ferrite-e2e/src/page.rs#L2761)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByText` | method | Partial | `Page.get_by_text` ([source](crates/ferrite-e2e/src/page.rs#L2767)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByTitle` | method | Partial | `Page.get_by_title` ([source](crates/ferrite-e2e/src/page.rs#L2809)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.goBack` | method | Partial | `Page.go_back` ([source](crates/ferrite-e2e/src/page.rs#L2352)) | Returns unit, not a navigation Response; fewer navigation options. Relative URLs resolve through the configured base URL. |
| `Page.goForward` | method | Partial | `Page.go_forward` ([source](crates/ferrite-e2e/src/page.rs#L2362)) | Returns unit, not a navigation Response; fewer navigation options. Relative URLs resolve through the configured base URL. |
| `Page.requestGC` | method | Partial | `Page.request_gc` ([source](crates/ferrite-e2e/src/page.rs#L2996)) | Chromium only; Firefox returns an unsupported error. |
| `Page.goto` | method | Partial | `Page.goto_with_options` ([source](crates/ferrite-e2e/src/page.rs#L2257)) | Returns unit, not a navigation Response; fewer navigation options. Relative URLs resolve through the configured base URL. |
| `Page.hideHighlight` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.hover` | method | Partial | `Locator.hover` ([source](crates/ferrite-e2e/src/locator.rs#L1632)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.innerHTML` | method | Partial | `Locator.inner_html` ([source](crates/ferrite-e2e/src/locator.rs#L2513)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.innerText` | method | Partial | `Locator.inner_text` ([source](crates/ferrite-e2e/src/locator.rs#L916)) | Page.locator(selector) followed by the distinct rendered/raw text getter; strict resolution and same-origin frame limitations remain. |
| `Page.inputValue` | method | Partial | `Locator.input_value` ([source](crates/ferrite-e2e/src/locator.rs#L2383)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isChecked` | method | Partial | `Locator.is_checked` ([source](crates/ferrite-e2e/src/locator.rs#L2713)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isClosed` | method | Partial | `Page.is_closed` ([source](crates/ferrite-e2e/src/page.rs#L2037)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.isDisabled` | method | Partial | `Locator.is_disabled` ([source](crates/ferrite-e2e/src/locator.rs#L2695)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isEditable` | method | Partial | `Locator.is_editable` ([source](crates/ferrite-e2e/src/locator.rs#L2731)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isEnabled` | method | Partial | `Locator.is_enabled` ([source](crates/ferrite-e2e/src/locator.rs#L2677)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isHidden` | method | Partial | `Locator.is_hidden` ([source](crates/ferrite-e2e/src/locator.rs#L2656)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isVisible` | method | Partial | `Locator.is_visible` ([source](crates/ferrite-e2e/src/locator.rs#L2638)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.keyboard` | property | Partial | `Page.press_key` ([source](crates/ferrite-e2e/src/page.rs#L3052)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.clearConsoleMessages` | method | Partial | `Page.clear_console_messages` ([source](crates/ferrite-e2e/src/page.rs#L1897)) | Dedicated API exists; coverage is Chromium-only, lazy frame locators are same-origin, exceptions have ConsoleMessage shape. |
| `Page.clearPageErrors` | method | Partial | `Page.clear_page_errors` ([source](crates/ferrite-e2e/src/page.rs#L1914)) | Dedicated API exists; coverage is Chromium-only, lazy frame locators are same-origin, exceptions have ConsoleMessage shape. |
| `Page.localStorage` | property | Partial | `Page.local_storage_get` ([source](crates/ferrite-e2e/src/page.rs#L3312)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.sessionStorage` | property | Partial | `Page.session_storage_get` ([source](crates/ferrite-e2e/src/page.rs#L3349)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.consoleMessages` | method | Partial | `Page.console_messages` ([source](crates/ferrite-e2e/src/page.rs#L2204)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.pageErrors` | method | Partial | `Page.page_errors` ([source](crates/ferrite-e2e/src/page.rs#L1906)) | Dedicated API exists; coverage is Chromium-only, lazy frame locators are same-origin, exceptions have ConsoleMessage shape. |
| `Page.locator` | method | Partial | `Page.locator` ([source](crates/ferrite-e2e/src/page.rs#L2755)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.mainFrame` | method | Partial | `Page.main_frame` ([source](crates/ferrite-e2e/src/page.rs#L4536)) | Dedicated asynchronous native root lookup; closed/disconnected pages fail, no fabricated root. Selector-free OOPIF traversal remains deferred. |
| `Page.mouse` | property | Partial | `Page.mouse_click` ([source](crates/ferrite-e2e/src/page.rs#L3003)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.opener` | method | Partial | `Page.opener` ([source](crates/ferrite-e2e/src/page.rs#L2082)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.pause` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.pdf` | method | Partial | `Page.pdf` ([source](crates/ferrite-e2e/src/page.rs#L3498)) | PDF export exists; no PDF options builder. Engine behavior differs. |
| `Page.pickLocator` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.press` | method | Partial | `Locator.press` ([source](crates/ferrite-e2e/src/locator.rs#L2008)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.querySelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.querySelectorAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.requests` | method | Partial | `Page.requests` ([source](crates/ferrite-e2e/src/page.rs#L4365)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.addLocatorHandler` | method | Partial | `Page.add_locator_handler_with` ([source](crates/ferrite-e2e/src/page.rs#L2838)) | Visibility-based overlay handlers run before actions and state/custom assertions; no full dismissal/noWaitAfter semantics. |
| `Page.removeAllListeners` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.removeLocatorHandler` | method | Partial | `Page.remove_locator_handler` ([source](crates/ferrite-e2e/src/page.rs#L2860)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.reload` | method | Partial | `Page.reload` ([source](crates/ferrite-e2e/src/page.rs#L2344)) | Returns unit, not a navigation Response; fewer navigation options. Relative URLs resolve through the configured base URL. |
| `Page.request` | property | Partial | `Page.request` ([source](crates/ferrite-e2e/src/page.rs#L1890)) | HTTP client sharing owning-context cookies and inheriting its transport defaults; cancellation follows context disposal. |
| `Page.route` | method | Partial | `Page.route_matching` ([source](crates/ferrite-e2e/src/page.rs#L4104)) | Shared resolved UrlMatcher for rules/handlers/HAR filters, with match limits and identity-based removal; legacy string/globset contracts preserved. Invalid patterns fail before registration, including empty contexts; native response/URL override differences remain. In-flight removal and fuller HAR policies tracked separately. |
| `Page.routeFromHAR` | method | Partial | `Page.route_from_har` ([source](crates/ferrite-e2e/src/page.rs#L4258)) | Shared resolved UrlMatcher for rules/handlers/HAR filters, with match limits and identity-based removal; legacy string/globset contracts preserved. Invalid patterns fail before registration, including empty contexts; native response/URL override differences remain. In-flight removal and fuller HAR policies tracked separately. |
| `Page.routeWebSocket` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.screencast` | property | Partial | `Page.frames` ([source](crates/ferrite-e2e/src/page.rs#L5111)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.screenshot` | method | Partial | `Page.screenshot` ([source](crates/ferrite-e2e/src/page.rs#L3405)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.selectOption` | method | Partial | `Locator.select_options` ([source](crates/ferrite-e2e/src/locator.rs#L2298)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.setChecked` | method | Partial | `Locator.set_checked` ([source](crates/ferrite-e2e/src/locator.rs#L2829)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.setContent` | method | Partial | `Page.set_content` ([source](crates/ferrite-e2e/src/page.rs#L2396)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.setDefaultNavigationTimeout` | method | Partial | `Page.set_navigation_timeout` ([source](crates/ferrite-e2e/src/page.rs#L2152)) | Navigation default distinct from locator timeout. |
| `Page.setDefaultTimeout` | method | Partial | `Page.set_timeout` ([source](crates/ferrite-e2e/src/page.rs#L2143)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.setExtraHTTPHeaders` | method | Partial | `Page.set_extra_http_headers` ([source](crates/ferrite-e2e/src/page.rs#L3973)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.setInputFiles` | method | Partial | `Page.set_input_file_payloads` ([source](crates/ferrite-e2e/src/page.rs#L4407)) | FilePayload filename/MIME/bytes or existing path uploads; empty lists clear, multiple files require a multiple input, 64 MiB total cap. DOM File/DataTransfer injection on both engines; no native chooser/directory upload/options parity. Frame uses Frame.locator. |
| `Page.setViewportSize` | method | Partial | `Page.set_viewport` ([source](crates/ferrite-e2e/src/page.rs#L3505)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.ariaSnapshot` | method | Partial | `Page.aria_snapshot` ([source](crates/ferrite-e2e/src/page.rs#L4696)) | Structured role/name/state DOM approximation, including open shadow roots; no full ARIA/YAML matching, mode/depth/boxes options. |
| `Page.ariaSnapshotJSON` | method | Partial | `Page.aria_snapshot_json` ([source](crates/ferrite-e2e/src/page.rs#L4686)) | Nested role/name/state DOM tree without name/node truncation; not the complete accessibility algorithm. |
| `Page.tap` | method | Partial | `Locator.tap` ([source](crates/ferrite-e2e/src/locator.rs#L1710)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.textContent` | method | Partial | `Locator.text_content` ([source](crates/ferrite-e2e/src/locator.rs#L892)) | Page.locator(selector) followed by the distinct rendered/raw text getter; strict resolution and same-origin frame limitations remain. |
| `Page.title` | method | Equivalent | `Page.title` ([source](crates/ferrite-e2e/src/page.rs#L2372)) | Basic document access. |
| `Page.touchscreen` | property | Partial | `Page.touchscreen_tap` ([source](crates/ferrite-e2e/src/page.rs#L3143)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.type` (deprecated) | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L2050)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.uncheck` | method | Partial | `Locator.uncheck` ([source](crates/ferrite-e2e/src/locator.rs#L2226)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.unrouteAll` | method | Partial | `Page.unroute_all` ([source](crates/ferrite-e2e/src/page.rs#L4209)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.unroute` | method | Partial | `Page.unroute_matching` ([source](crates/ferrite-e2e/src/page.rs#L4229)) | Shared resolved UrlMatcher for rules/handlers/HAR filters, with match limits and identity-based removal; legacy string/globset contracts preserved. Invalid patterns fail before registration, including empty contexts; native response/URL override differences remain. In-flight removal and fuller HAR policies tracked separately. |
| `Page.url` | method | Equivalent | `Page.url` ([source](crates/ferrite-e2e/src/page.rs#L2379)) | Basic document access. |
| `Page.video` | method | Partial | `Page.start_video` ([source](crates/ferrite-e2e/src/page.rs#L5038)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.viewportSize` | method | Partial | `Page.viewport_size` ([source](crates/ferrite-e2e/src/page.rs#L2981)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.waitForEvent` | method | Partial | `Page.wait_for_event` ([source](crates/ferrite-e2e/src/page.rs#L1941)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.waitForFunction` | method | Partial | `Page.wait_for_function_handle` ([source](crates/ferrite-e2e/src/page.rs#L2565)) | JSON argument, native animation-frame/interval polling and retained truthy result; JSON helper supports frames. Frame remote handles and arbitrary argument serialization remain unsupported. Legacy expression helper returns unit. |
| `Page.waitForLoadState` | method | Partial | `Page.wait_for_load_state` ([source](crates/ferrite-e2e/src/page.rs#L2736)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.waitForNavigation` (deprecated) | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.waitForRequest` | method | Partial | `Page.wait_for_request_async` ([source](crates/ferrite-e2e/src/page.rs#L4855)) | Exact/glob/regex or sync/async predicates over RecordedRequest fields; request waits resolve at start, response waits at headers including already in-flight requests. Returns a metadata snapshot rather than rich live Request/Response/body objects. Legacy strings retain substring semantics; lag fails explicitly. |
| `Page.waitForResponse` | method | Partial | `Page.wait_for_response_async` ([source](crates/ferrite-e2e/src/page.rs#L4899)) | Exact/glob/regex or sync/async predicates over RecordedRequest fields; request waits resolve at start, response waits at headers including already in-flight requests. Returns a metadata snapshot rather than rich live Request/Response/body objects. Legacy strings retain substring semantics; lag fails explicitly. |
| `Page.waitForSelector` | method | Partial | `Page.wait_for_selector_with` ([source](crates/ferrite-e2e/src/page.rs#L2053)) | Waits for requested state and returns Locator, not ElementHandle. |
| `Page.waitForTimeout` | method | Partial | `Page.wait_for_timeout` ([source](crates/ferrite-e2e/src/page.rs#L2589)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.waitForURL` | method | Partial | `Page.wait_for_url_matching_with_options` ([source](crates/ferrite-e2e/src/page.rs#L2640)) | Exact (relative to base URL), full-URL glob/regex or predicate with Commit/DOMContentLoaded/Load readiness in one navigation budget. Page NetworkIdle uses 500ms observed HTTP quiet; frames reject it. Legacy helpers retain substring/URL-only semantics. Zero timeout, cancellation and enclosing budgets supported; URLPattern absent. |
| `Page.workers` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## PageAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-pageassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `PageAssertions.not` | property | Partial | `PageExpect.not` ([source](crates/ferrite-e2e/src/expect.rs#L235)) | Retrying page assertion; exact/regex title and URL helpers, with narrower options and different screenshot/ARIA algorithms. |
| `PageAssertions.toMatchAriaSnapshot` | method | Partial | `PageExpect.aria_snapshot` ([source](crates/ferrite-e2e/src/expect.rs#L516)) | Retrying page assertion; exact/regex title and URL helpers, with narrower options and different screenshot/ARIA algorithms. |
| `PageAssertions.toHaveScreenshot` | method | Partial | `PageExpect.screenshot_with` ([source](crates/ferrite-e2e/src/expect.rs#L457)) | Retrying page assertion; exact/regex title and URL helpers, with narrower options and different screenshot/ARIA algorithms. |
| `PageAssertions.toHaveTitle` | method | Partial | `PageExpect.title` ([source](crates/ferrite-e2e/src/expect.rs#L324)) | Retrying page assertion; exact/regex title and URL helpers, with narrower options and different screenshot/ARIA algorithms. |
| `PageAssertions.toHaveURL` | method | Partial | `PageExpect.url_matching` ([source](crates/ferrite-e2e/src/expect.rs#L264)) | Shared exact/base URL, glob, Rust regex or url_where predicate; retrying negation and cancellation. Legacy assertion helpers retain string contracts; explicit globs are a Rust extension, and URLPattern/case option parity remains absent. |

## Playwright

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-playwright.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Playwright.chromium` | property | Partial | `Browser.launch` ([source](crates/ferrite-e2e/src/browser.rs#L367)) | Select BrowserKind with LaunchOptions; no BrowserType object or bundled browser installer. |
| `Playwright.devices` | property | Partial | `DeviceDescriptor` ([source](crates/ferrite-e2e/src/page.rs#L745)) | Seven metrics presets; no full device catalog or device user-agent metadata. |
| `Playwright.errors` | property | Idiomatic | `E2eError` ([source](crates/ferrite-e2e/src/error.rs#L8)) | Rust error enum, with different variants and diagnostics. |
| `Playwright.firefox` | property | Partial | `Browser.launch` ([source](crates/ferrite-e2e/src/browser.rs#L367)) | Select BrowserKind with LaunchOptions; no BrowserType object or bundled browser installer. |
| `Playwright.request` | property | Partial | `ApiClient` ([source](crates/ferrite-e2e/src/api.rs#L141)) | Standalone or context-linked HTTP client; smaller APIRequest option surface. |
| `Playwright.selectors` | property | Partial | `set_test_id_attribute` ([source](crates/ferrite-e2e/src/locator.rs#L21)) | Only test-id configuration; no custom selector registration. |
| `Playwright.webkit` | property | Missing | — | No WebKit backend; BrowserKind contains Chromium and Firefox only. |

## PlaywrightAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-playwrightassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `PlaywrightAssertions.expectAPIResponse` | method | Idiomatic | `ApiResponse.ok` ([source](crates/ferrite-e2e/src/api.rs#L49)) | Use native Rust assertions over the standalone response. |
| `PlaywrightAssertions.expectGeneric` | method | Idiomatic | — | Use native Rust assertions and explicit pattern/container checks; no Playwright expect matcher library. |
| `PlaywrightAssertions.expectLocator` | method | Partial | `Locator.expect` ([source](crates/ferrite-e2e/src/expect.rs#L1636)) | Rust assertion builders; fewer matcher/expect configuration capabilities. |
| `PlaywrightAssertions.expectPage` | method | Partial | `Page.expect` ([source](crates/ferrite-e2e/src/expect.rs#L1618)) | Rust assertion builders; fewer matcher/expect configuration capabilities. |

## Reporter

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-reporter.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Reporter.onBegin` | method | Partial | `Reporter.on_begin` ([source](crates/ferrite-e2e/src/report.rs#L266)) | Live synchronous thread-safe callbacks; attempt identity includes retry/project/repetition/worker, errors and interrupted user steps reported. No Suite/TestCase graph, asynchronous end/status override or worker stdout capture. |
| `Reporter.onEnd` | method | Partial | `Reporter.on_end` ([source](crates/ferrite-e2e/src/report.rs#L275)) | Live synchronous thread-safe callbacks; attempt identity includes retry/project/repetition/worker, errors and interrupted user steps reported. No Suite/TestCase graph, asynchronous end/status override or worker stdout capture. |
| `Reporter.onError` | method | Partial | `Reporter.on_error` ([source](crates/ferrite-e2e/src/report.rs#L274)) | Live synchronous thread-safe callbacks; attempt identity includes retry/project/repetition/worker, errors and interrupted user steps reported. No Suite/TestCase graph, asynchronous end/status override or worker stdout capture. |
| `Reporter.onExit` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Reporter.onStdErr` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Reporter.onStdOut` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Reporter.onStepBegin` | method | Partial | `Reporter.on_step_begin` ([source](crates/ferrite-e2e/src/report.rs#L270)) | Live synchronous thread-safe callbacks; attempt identity includes retry/project/repetition/worker, errors and interrupted user steps reported. No Suite/TestCase graph, asynchronous end/status override or worker stdout capture. |
| `Reporter.onStepEnd` | method | Partial | `Reporter.on_step_end` ([source](crates/ferrite-e2e/src/report.rs#L271)) | Live synchronous thread-safe callbacks; attempt identity includes retry/project/repetition/worker, errors and interrupted user steps reported. No Suite/TestCase graph, asynchronous end/status override or worker stdout capture. |
| `Reporter.onTestBegin` | method | Partial | `Reporter.on_test_begin` ([source](crates/ferrite-e2e/src/report.rs#L267)) | Live synchronous thread-safe callbacks; attempt identity includes retry/project/repetition/worker, errors and interrupted user steps reported. No Suite/TestCase graph, asynchronous end/status override or worker stdout capture. |
| `Reporter.onTestEnd` | method | Partial | `Reporter.on_test_end` ([source](crates/ferrite-e2e/src/report.rs#L269)) | Live synchronous thread-safe callbacks; attempt identity includes retry/project/repetition/worker, errors and interrupted user steps reported. No Suite/TestCase graph, asynchronous end/status override or worker stdout capture. |
| `Reporter.printsToStdio` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `Reporter.preprocess` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |

## Request

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-request.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Request.allHeaders` | method | Partial | `RecordedRequest.headers` ([source](crates/ferrite-e2e/src/page.rs#L173)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |
| `Request.failure` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.frame` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.headers` | method | Partial | `RecordedRequest.headers` ([source](crates/ferrite-e2e/src/page.rs#L173)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |
| `Request.headersArray` | method | Partial | `RecordedRequest.headers` ([source](crates/ferrite-e2e/src/page.rs#L173)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |
| `Request.headerValue` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.isNavigationRequest` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.method` | method | Partial | `RecordedRequest.method` ([source](crates/ferrite-e2e/src/page.rs#L166)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |
| `Request.postData` | method | Partial | `RecordedRequest.post_data` ([source](crates/ferrite-e2e/src/page.rs#L176)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |
| `Request.postDataBuffer` | method | Partial | `RecordedRequest.post_data` ([source](crates/ferrite-e2e/src/page.rs#L176)) | Text capture only; no lossless binary request body API. |
| `Request.postDataJSON` | method | Partial | `RecordedRequest.post_data` ([source](crates/ferrite-e2e/src/page.rs#L176)) | Caller parses the captured request text with serde_json; body_json() is RESPONSE JSON, not request postDataJSON. |
| `Request.redirectedFrom` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.redirectedTo` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.resourceType` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.response` | method | Partial | `RecordedRequest.status` ([source](crates/ferrite-e2e/src/page.rs#L170)) | Captured response data are merged into the record; no separate live Response or request lifecycle object. |
| `Request.existingResponse` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.serviceWorker` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.sizes` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.timing` | method | Partial | `RecordedRequest.duration_ms` ([source](crates/ferrite-e2e/src/page.rs#L179)) | Total elapsed time only; no DNS/connect/TLS/response timing breakdown. |
| `Request.url` | method | Partial | `RecordedRequest.url` ([source](crates/ferrite-e2e/src/page.rs#L168)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |

## Response

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-response.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Response.allHeaders` | method | Partial | `RecordedRequest.response_headers` ([source](crates/ferrite-e2e/src/page.rs#L188)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.body` | method | Partial | `RecordedRequest.body` ([source](crates/ferrite-e2e/src/page.rs#L200)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.finished` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.frame` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.fromServiceWorker` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.headers` | method | Partial | `RecordedRequest.response_headers` ([source](crates/ferrite-e2e/src/page.rs#L188)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.headersArray` | method | Partial | `RecordedRequest.response_headers` ([source](crates/ferrite-e2e/src/page.rs#L188)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.headerValue` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.headerValues` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.httpVersion` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.json` | method | Partial | `RecordedRequest.body_json` ([source](crates/ferrite-e2e/src/page.rs#L217)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.ok` | method | Idiomatic | `RecordedRequest.status` ([source](crates/ferrite-e2e/src/page.rs#L170)) | Check (200..300).contains(&record.status); no dedicated Response.ok() method. |
| `Response.request` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.securityDetails` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.serverAddr` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.status` | method | Partial | `RecordedRequest.status` ([source](crates/ferrite-e2e/src/page.rs#L170)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.statusText` | method | Partial | `RecordedRequest.status_text` ([source](crates/ferrite-e2e/src/page.rs#L182)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.text` | method | Partial | `RecordedRequest.body_text` ([source](crates/ferrite-e2e/src/page.rs#L209)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.url` | method | Partial | `RecordedRequest.url` ([source](crates/ferrite-e2e/src/page.rs#L168)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |

## Route

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-route.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Route.abort` | method | Partial | `RouteAction.abort_with` ([source](crates/ferrite-e2e/src/page.rs#L1208)) | Rule/action model, with glob patterns and fewer options; response modification and URL override are Chromium-only. |
| `Route.continue` | method | Partial | `RouteRule.continue_with` ([source](crates/ferrite-e2e/src/page.rs#L1367)) | Rule/action model, with glob patterns and fewer options; response modification and URL override are Chromium-only. |
| `Route.fallback` | method | Partial | `RouteAction.fallback` ([source](crates/ferrite-e2e/src/page.rs#L1245)) | Rule/action model, with glob patterns and fewer options; response modification and URL override are Chromium-only. |
| `Route.fetch` | method | Partial | `RouteInfo.fetch` ([source](crates/ferrite-e2e/src/page.rs#L1100)) | Standalone reqwest client, not a shared browser cookie context; no full redirect/retry/options surface. |
| `Route.fulfill` | method | Partial | `RouteAction.fulfill_full` ([source](crates/ferrite-e2e/src/page.rs#L1224)) | Rule/action model, with glob patterns and fewer options; response modification and URL override are Chromium-only. |
| `Route.request` | method | Partial | `RouteInfo` ([source](crates/ferrite-e2e/src/page.rs#L1086)) | Rule/action model, with glob patterns and fewer options; response modification and URL override are Chromium-only. |

## Screencast

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-screencast.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Screencast.start` | method | Partial | `Page.start_video` ([source](crates/ferrite-e2e/src/page.rs#L5038)) | Video recording/live frames available; fewer formats/options, no screencast overlay/action system. |
| `Screencast.stop` | method | Partial | `Page.stop_video` ([source](crates/ferrite-e2e/src/page.rs#L5079)) | Explicit output path; Chromium assembles frames with ffmpeg, Firefox records natively. |
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
| `SnapshotAssertions.toMatchSnapshot` | method | Partial | `assert_snapshot_text` ([source](crates/ferrite-e2e/src/snapshot.rs#L243)) | Text and PNG helper functions; no Playwright snapshot-path template/source update/changed mode or arbitrary binary snapshots. |

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
| `Test.(call)` | method | Partial | `test` ([source](crates/ferrite-e2e/src/runner.rs#L365)) | Rust test closure; dynamic details/locks/options differ. |
| `Test.afterAll` | method | Partial | `Suite.after_all_with_context` ([source](crates/ferrite-e2e/src/runner.rs#L439)) | ContextHook explicitly declares lazy fixture roots; WorkerHook permits only worker roots. Nested lifecycle ordering; no process workers or callback parameter inference. |
| `Test.afterEach` | method | Partial | `Suite.after_each_with_context` ([source](crates/ferrite-e2e/src/runner.rs#L501)) | ContextHook explicitly declares lazy fixture roots; WorkerHook permits only worker roots. Nested lifecycle ordering; no process workers or callback parameter inference. |
| `Test.beforeAll` | method | Partial | `Suite.before_all_with_context` ([source](crates/ferrite-e2e/src/runner.rs#L434)) | ContextHook explicitly declares lazy fixture roots; WorkerHook permits only worker roots. Nested lifecycle ordering; no process workers or callback parameter inference. |
| `Test.beforeEach` | method | Partial | `Suite.before_each_with_context` ([source](crates/ferrite-e2e/src/runner.rs#L496)) | ContextHook explicitly declares lazy fixture roots; WorkerHook permits only worker roots. Nested lifecycle ordering; no process workers or callback parameter inference. |
| `Test.describe` | method | Partial | `Suite.tests` ([source](crates/ferrite-e2e/src/runner.rs#L536)) | Nested identity, scoped hooks, inherited timeout/retry/context/tags; no serial/fully-parallel suite scheduling configuration. |
| `Test.describe.configure` | method | Partial | `Suite.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L451)) | Suite timeout/retry/context settings inherited by descendants; no serial/fully-parallel execution mode configuration. |
| `Test.describe.fixme` | method | Partial | `Suite.fixme` ([source](crates/ferrite-e2e/src/runner.rs#L471)) | Applies focus/skip/fixme to descendants; Rust builders, no JavaScript describe callbacks. |
| `Test.describe.only` | method | Partial | `Suite.only` ([source](crates/ferrite-e2e/src/runner.rs#L476)) | Applies focus/skip/fixme to descendants; Rust builders, no JavaScript describe callbacks. |
| `Test.describe.parallel` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.parallel.only` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.serial` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.serial.only` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.skip` | method | Partial | `Suite.skip` ([source](crates/ferrite-e2e/src/runner.rs#L466)) | Applies focus/skip/fixme to descendants; Rust builders, no JavaScript describe callbacks. |
| `Test.expect` | property | Partial | `Locator.expect` ([source](crates/ferrite-e2e/src/expect.rs#L1636)) | Rust builders, expect_poll, expect_to_pass and SoftAsserts; no generic matcher registry/configure API. |
| `Test.extend` | method | Partial | `Runner.fixture_definition` ([source](crates/ferrite-e2e/src/runner.rs#L1962)) | Typed lazy dependencies including built-in page/context/request/TestInfo and browser/WorkerInfo, test/worker scopes and reverse teardown. No named overrides or callback parameter inference. |
| `Test.abort` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.fail` | method | Partial | `TestInfo.fail` ([source](crates/ferrite-e2e/src/runner.rs#L670)) | Static Test builders plus runtime TestInfo controls; use Rust conditionals and skip(reason)? for immediate closure exit. No JavaScript overload inference. |
| `Test.fail.only` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.fixme` | method | Partial | `Test.fixme` ([source](crates/ferrite-e2e/src/runner.rs#L322)) | Static test builder setting; no runtime conditional/context metadata overloads. |
| `Test.info` | method | Partial | `TestContext.info` ([source](crates/ferrite-e2e/src/runner.rs#L819)) | Provided through test_with_context; fewer live metadata fields and mutators. |
| `Test.only` | method | Partial | `Test.only` ([source](crates/ferrite-e2e/src/runner.rs#L315)) | Static test builder setting; no runtime conditional/context metadata overloads. |
| `Test.setTimeout` | method | Partial | `TestInfo.set_timeout` ([source](crates/ferrite-e2e/src/runner.rs#L689)) | Static Test builders plus runtime TestInfo controls; use Rust conditionals and skip(reason)? for immediate closure exit. No JavaScript overload inference. |
| `Test.skip` | method | Partial | `TestInfo.skip` ([source](crates/ferrite-e2e/src/runner.rs#L659)) | Static Test builders plus runtime TestInfo controls; use Rust conditionals and skip(reason)? for immediate closure exit. No JavaScript overload inference. |
| `Test.slow` | method | Partial | `TestInfo.slow` ([source](crates/ferrite-e2e/src/runner.rs#L678)) | Static Test builders plus runtime TestInfo controls; use Rust conditionals and skip(reason)? for immediate closure exit. No JavaScript overload inference. |
| `Test.step` | method | Partial | `Page.step_with` ([source](crates/ferrite-e2e/src/page.rs#L3186)) | Nested controlled steps with local timeout, skip, annotations and title paths through Page.step_with; legacy step_result remains available. Automatic navigation/locator/assertion/hook/fixture scopes; no boxing or subtitle/params options. |
| `Test.step.skip` | method | Partial | `Page.step_with` ([source](crates/ferrite-e2e/src/page.rs#L3186)) | StepOptions.skip records a skipped user step without constructing its closure; StepOutcome carries the reason. Rust option rather than a separate JS method. |
| `Test.use` | method | Partial | `Suite.context_options` ([source](crates/ferrite-e2e/src/runner.rs#L429)) | Nested suite/test context inheritance; no general named fixture option overrides. |

## TestCase

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-testcase.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestCase.annotations` | property | Partial | `Test.annotations` ([source](crates/ferrite-e2e/src/runner.rs#L267)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.expectedStatus` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestCase.id` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestCase.location` | property | Partial | `Test.file` ([source](crates/ferrite-e2e/src/runner.rs#L269)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.ok` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestCase.outcome` | method | Partial | `TestResult.flaky` ([source](crates/ferrite-e2e/src/report.rs#L715)) | Recovered successful retries flagged flaky; aggregate TestStatus separates passed/failed/skipped/expected-failed. Rust fields, not the upstream outcome() enum. |
| `TestCase.parent` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestCase.repeatEachIndex` | property | Partial | `TestResult.repeat_each_index` ([source](crates/ferrite-e2e/src/report.rs#L741)) | Result metadata only; no TestCase/Suite reporter tree. |
| `TestCase.results` | property | Partial | `TestResult.attempt_results` ([source](crates/ferrite-e2e/src/report.rs#L712)) | Full attempt history retained under the aggregate test result; no upstream TestCase/Suite graph. |
| `TestCase.retries` | property | Partial | `Test.retries` ([source](crates/ferrite-e2e/src/runner.rs#L350)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.tags` | property | Partial | `Test.tags` ([source](crates/ferrite-e2e/src/runner.rs#L265)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.timeout` | property | Partial | `Test.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L357)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.title` | property | Partial | `Test.name` ([source](crates/ferrite-e2e/src/runner.rs#L259)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.titlePath` | method | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestCase.type` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |

## TestConfig

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-testconfig.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestConfig.build` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.captureGitInfo` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.expect` | property | Partial | `E2eConfig.expect_timeout_ms` ([source](crates/ferrite-config/src/lib.rs#L424)) | Consumed by CLI environment bridge and Runner; per-page/expect overrides supported. |
| `TestConfig.failOnFlakyTests` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.forbidOnly` | property | Partial | `Runner.forbid_only` ([source](crates/ferrite-e2e/src/runner.rs#L1884)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.fullyParallel` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.globalSetup` | property | Partial | `Runner.global_setup` ([source](crates/ferrite-e2e/src/runner.rs#L1797)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.globalTeardown` | property | Partial | `Runner.global_teardown` ([source](crates/ferrite-e2e/src/runner.rs#L1808)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.globalTimeout` | property | Partial | `E2eConfig.global_timeout_ms` ([source](crates/ferrite-config/src/lib.rs#L418)) | Consumed by Runner and CLI; global cancellation with bounded teardown and final unexpected-failure scheduling limit. Active workers finish on maxFailures; no process-worker orchestration. |
| `TestConfig.grep` | property | Partial | `Runner.grep` ([source](crates/ferrite-e2e/src/runner.rs#L1720)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.grepInvert` | property | Partial | `Runner.grep_invert` ([source](crates/ferrite-e2e/src/runner.rs#L1729)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.ignoreSnapshots` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.maxFailures` | property | Partial | `E2eConfig.max_failures` ([source](crates/ferrite-config/src/lib.rs#L420)) | Consumed by Runner and CLI; global cancellation with bounded teardown and final unexpected-failure scheduling limit. Active workers finish on maxFailures; no process-worker orchestration. |
| `TestConfig.metadata` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.name` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.outputDir` | property | Partial | `E2eConfig.output_dir` ([source](crates/ferrite-config/src/lib.rs#L432)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.snapshotDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.snapshotPathTemplate` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.preserveOutput` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.projects` | property | Partial | `Runner.project` ([source](crates/ferrite-e2e/src/runner.rs#L1868)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.quiet` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.repeatEach` | property | Partial | `Runner.repeat_each` ([source](crates/ferrite-e2e/src/runner.rs#L1875)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.reporter` | property | Partial | `E2eConfig.reporter` ([source](crates/ferrite-config/src/lib.rs#L430)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.reportSlowTests` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.respectGitIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.retries` | property | Partial | `Runner.retries` ([source](crates/ferrite-e2e/src/runner.rs#L1632)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.retryStrategy` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.shard` | property | Partial | `Runner.shard` ([source](crates/ferrite-e2e/src/runner.rs#L1826)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.tag` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.testDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.testIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.testMatch` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.timeout` | property | Partial | `Runner.test_timeout` ([source](crates/ferrite-e2e/src/runner.rs#L1639)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.tsconfig` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.updateSnapshots` | property | Partial | `E2eConfig.update_snapshots` ([source](crates/ferrite-config/src/lib.rs#L442)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.updateSourceMethod` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.use` | property | Partial | `ContextOptions` ([source](crates/ferrite-e2e/src/context.rs#L31)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.webServer` | property | Partial | `E2eConfig.web_server` ([source](crates/ferrite-config/src/lib.rs#L446)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.workers` | property | Partial | `Runner.workers` ([source](crates/ferrite-e2e/src/runner.rs#L1625)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |

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
| `TestInfo.annotations` | property | Partial | `TestInfo.annotations` ([source](crates/ferrite-e2e/src/runner.rs#L703)) | Shared runtime control affects the active setup/body future and final annotations; skip uses Result propagation, timeouts include elapsed time, cleanup retains independent budgets. No full TestInfo object parity. |
| `TestInfo.attachments` | property | Partial | `TestInfo.attachments` ([source](crates/ferrite-e2e/src/runner.rs#L762)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.attach` | method | Partial | `TestInfo.attach` ([source](crates/ferrite-e2e/src/runner.rs#L731)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.column` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.config` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.duration` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.error` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.errors` | property | Partial | `TestInfo.errors` ([source](crates/ferrite-e2e/src/runner.rs#L611)) | Live getters shared across metadata clones; raw outcome published before afterEach and updated after cleanup failures. status returns None during setup/body; errors have phase/code/message/location, without JS stack/cause/snippet serialization. |
| `TestInfo.expectedStatus` | property | Partial | `TestInfo.expected_status` ([source](crates/ferrite-e2e/src/runner.rs#L600)) | Live getters shared across metadata clones; raw outcome published before afterEach and updated after cleanup failures. status returns None during setup/body; errors have phase/code/message/location, without JS stack/cause/snippet serialization. |
| `TestInfo.fail` | method | Partial | `TestInfo.fail` ([source](crates/ferrite-e2e/src/runner.rs#L670)) | Shared runtime control affects the active setup/body future and final annotations; skip uses Result propagation, timeouts include elapsed time, cleanup retains independent budgets. No full TestInfo object parity. |
| `TestInfo.file` | property | Partial | `TestInfo.file` ([source](crates/ferrite-e2e/src/runner.rs#L568)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.fixme` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.fn` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.tags` | property | Partial | `TestInfo.tags` ([source](crates/ferrite-e2e/src/runner.rs#L572)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.testId` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.line` | property | Partial | `TestInfo.line` ([source](crates/ferrite-e2e/src/runner.rs#L570)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.snapshotDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.outputDir` | property | Partial | `TestInfo.output_dir` ([source](crates/ferrite-e2e/src/runner.rs#L582)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.outputPath` | method | Partial | `TestInfo.output_path` ([source](crates/ferrite-e2e/src/runner.rs#L707)) | Attempt-specific artifact path; parent traversal and absolute paths rejected. No snapshot-path templates. |
| `TestInfo.parallelIndex` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.project` | property | Partial | `TestInfo.project` ([source](crates/ferrite-e2e/src/runner.rs#L584)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.repeatEachIndex` | property | Partial | `TestInfo.repeat_each_index` ([source](crates/ferrite-e2e/src/runner.rs#L578)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.retry` | property | Partial | `TestInfo.retry` ([source](crates/ferrite-e2e/src/runner.rs#L574)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.setTimeout` | method | Partial | `TestInfo.set_timeout` ([source](crates/ferrite-e2e/src/runner.rs#L689)) | Shared runtime control affects the active setup/body future and final annotations; skip uses Result propagation, timeouts include elapsed time, cleanup retains independent budgets. No full TestInfo object parity. |
| `TestInfo.skip` | method | Partial | `TestInfo.skip` ([source](crates/ferrite-e2e/src/runner.rs#L659)) | Shared runtime control affects the active setup/body future and final annotations; skip uses Result propagation, timeouts include elapsed time, cleanup retains independent budgets. No full TestInfo object parity. |
| `TestInfo.slow` | method | Partial | `TestInfo.slow` ([source](crates/ferrite-e2e/src/runner.rs#L678)) | Shared runtime control affects the active setup/body future and final annotations; skip uses Result propagation, timeouts include elapsed time, cleanup retains independent budgets. No full TestInfo object parity. |
| `TestInfo.snapshotPath` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.snapshotSuffix` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.status` | property | Partial | `TestInfo.status` ([source](crates/ferrite-e2e/src/runner.rs#L596)) | Live getters shared across metadata clones; raw outcome published before afterEach and updated after cleanup failures. status returns None during setup/body; errors have phase/code/message/location, without JS stack/cause/snippet serialization. |
| `TestInfo.timeout` | property | Partial | `TestInfo.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L580)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.title` | property | Partial | `TestInfo.title` ([source](crates/ferrite-e2e/src/runner.rs#L566)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.titlePath` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.workerIndex` | property | Partial | `TestInfo.worker_index` ([source](crates/ferrite-e2e/src/runner.rs#L576)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |

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
| `TestOptions.acceptDownloads` | property | Partial | `ContextOptions.accept_downloads` ([source](crates/ferrite-e2e/src/context.rs#L206)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.baseURL` | property | Partial | `E2eConfig.base_url` ([source](crates/ferrite-config/src/lib.rs#L414)) | Runner/launch config field; no automatic named-fixture/project options equivalence. |
| `TestOptions.browserName` | property | Partial | `Project.browser` ([source](crates/ferrite-e2e/src/runner.rs#L1242)) | Projects select Chromium/Firefox independently; no WebKit backend. |
| `TestOptions.actionTimeout` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.bypassCSP` | property | Partial | `ContextOptions.bypass_csp` ([source](crates/ferrite-e2e/src/context.rs#L199)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.channel` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.clientCertificates` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.colorScheme` | property | Partial | `Page.emulate_media` ([source](crates/ferrite-e2e/src/page.rs#L4043)) | Chromium page-level manual emulation; no context/test option binding. |
| `TestOptions.connectOptions` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.contextOptions` | property | Partial | `Project.context_options` ([source](crates/ferrite-e2e/src/runner.rs#L1255)) | Isolated context per attempt, runner defaults and per-project overrides; no full named-fixture test.use model. |
| `TestOptions.contrast` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.deviceScaleFactor` | property | Partial | `ContextOptions.device_scale_factor` ([source](crates/ferrite-e2e/src/context.rs#L171)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.extraHTTPHeaders` | property | Partial | `ContextOptions.extra_http_headers` ([source](crates/ferrite-e2e/src/context.rs#L164)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.forcedColors` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.geolocation` | property | Partial | `ContextOptions.geolocation` ([source](crates/ferrite-e2e/src/context.rs#L136)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.hasTouch` | property | Partial | `ContextOptions.has_touch` ([source](crates/ferrite-e2e/src/context.rs#L185)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.headless` | property | Partial | `E2eConfig.headless` ([source](crates/ferrite-config/src/lib.rs#L399)) | Runner/launch config field; no automatic named-fixture/project options equivalence. |
| `TestOptions.httpCredentials` | property | Partial | `ContextOptions.http_credentials` ([source](crates/ferrite-e2e/src/context.rs#L157)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.ignoreHTTPSErrors` | property | Partial | `ContextOptions.ignore_https_errors` ([source](crates/ferrite-e2e/src/context.rs#L41)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.isMobile` | property | Partial | `ContextOptions.is_mobile` ([source](crates/ferrite-e2e/src/context.rs#L178)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.javaScriptEnabled` | property | Partial | `ContextOptions.java_script_enabled` ([source](crates/ferrite-e2e/src/context.rs#L192)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.launchOptions` | property | Partial | `Project.launch_options` ([source](crates/ferrite-e2e/src/runner.rs#L1248)) | Dedicated project launch settings; persistent profiles supported, no managed channels. |
| `TestOptions.locale` | property | Partial | `ContextOptions.locale` ([source](crates/ferrite-e2e/src/context.rs#L122)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.navigationTimeout` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.offline` | property | Partial | `ContextOptions.offline` ([source](crates/ferrite-e2e/src/context.rs#L150)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.permissions` | property | Partial | `ContextOptions.permissions` ([source](crates/ferrite-e2e/src/context.rs#L143)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.proxy` | property | Partial | `ContextOptions.proxy_server` ([source](crates/ferrite-e2e/src/context.rs#L39)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.reducedMotion` | property | Partial | `Page.emulate_media` ([source](crates/ferrite-e2e/src/page.rs#L4043)) | Chromium page-level manual emulation; no context/test option binding. |
| `TestOptions.reuseContext` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.screenshot` | property | Partial | `E2eConfig.screenshot` ([source](crates/ferrite-config/src/lib.rs#L434)) | Runner/launch config field; no automatic named-fixture/project options equivalence. |
| `TestOptions.storageState` | property | Partial | `ContextOptions.storage_state` ([source](crates/ferrite-e2e/src/context.rs#L220)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.testIdAttribute` | property | Partial | `set_test_id_attribute` ([source](crates/ferrite-e2e/src/locator.rs#L21)) | Process-global setter, not project/test-specific fixture option. |
| `TestOptions.timezoneId` | property | Partial | `ContextOptions.timezone_id` ([source](crates/ferrite-e2e/src/context.rs#L129)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.trace` | property | Partial | `BrowserContext.start_tracing` ([source](crates/ferrite-e2e/src/context.rs#L1016)) | Manual custom JSON traces plus runner JSON; no trace mode policy or Trace Viewer compatibility. |
| `TestOptions.userAgent` | property | Partial | `ContextOptions.user_agent` ([source](crates/ferrite-e2e/src/context.rs#L115)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.video` | property | Partial | `E2eConfig.video` ([source](crates/ferrite-config/src/lib.rs#L436)) | Runner/launch config field; no automatic named-fixture/project options equivalence. |
| `TestOptions.viewport` | property | Partial | `ContextOptions.viewport` ([source](crates/ferrite-e2e/src/context.rs#L108)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.serviceWorkers` | property | Partial | `ContextOptions.service_workers` ([source](crates/ferrite-e2e/src/context.rs#L227)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |

## TestProject

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-testproject.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestProject.dependencies` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.expect` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.fullyParallel` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.grep` | property | Partial | `Project.grep` ([source](crates/ferrite-e2e/src/runner.rs#L1271)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `TestProject.grepInvert` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.ignoreSnapshots` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.metadata` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.name` | property | Partial | `Project.name` ([source](crates/ferrite-e2e/src/runner.rs#L1231)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `TestProject.snapshotDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.snapshotPathTemplate` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.outputDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.repeatEach` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.respectGitIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.retries` | property | Partial | `Project.retries` ([source](crates/ferrite-e2e/src/runner.rs#L1278)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `TestProject.teardown` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.testDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.testIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.testMatch` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.timeout` | property | Partial | `Project.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L1285)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `TestProject.use` | property | Partial | `Project.context_options` ([source](crates/ferrite-e2e/src/runner.rs#L1255)) | Project-specific context settings; optional browser/launch overrides, suite/test context inheritance, no named fixture overrides or project dependencies. |
| `TestProject.workers` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## TestResult

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-testresult.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestResult.attachments` | property | Partial | `AttemptResult.attachments` ([source](crates/ferrite-e2e/src/report.rs#L254)) | Each attempt is retained in TestResult.attempt_results and JSON/HTML; raw status includes timeout/interruption, errors include phase/code and steps include user/action/assertion/hook/fixture trees. Different Rust schema, no stdout/stderr capture. |
| `TestResult.annotations` | property | Partial | `AttemptResult.annotations` ([source](crates/ferrite-e2e/src/report.rs#L252)) | Each attempt is retained in TestResult.attempt_results and JSON/HTML; raw status includes timeout/interruption, errors include phase/code and steps include user/action/assertion/hook/fixture trees. Different Rust schema, no stdout/stderr capture. |
| `TestResult.duration` | property | Partial | `AttemptResult.duration_ms` ([source](crates/ferrite-e2e/src/report.rs#L250)) | Each attempt is retained in TestResult.attempt_results and JSON/HTML; raw status includes timeout/interruption, errors include phase/code and steps include user/action/assertion/hook/fixture trees. Different Rust schema, no stdout/stderr capture. |
| `TestResult.error` | property | Partial | `TestResult.error` ([source](crates/ferrite-e2e/src/report.rs#L726)) | Aggregate result fields; attempts aggregates retries, not one TestResult per attempt; errors are strings. Live callbacks additionally receive per-attempt results; the final report aggregates retries. |
| `TestResult.errors` | property | Partial | `AttemptResult.errors` ([source](crates/ferrite-e2e/src/report.rs#L251)) | Each attempt is retained in TestResult.attempt_results and JSON/HTML; raw status includes timeout/interruption, errors include phase/code and steps include user/action/assertion/hook/fixture trees. Different Rust schema, no stdout/stderr capture. |
| `TestResult.retry` | property | Partial | `AttemptInfo.retry` ([source](crates/ferrite-e2e/src/report.rs#L15)) | Per-attempt identity on AttemptResult.info; logical Tokio workers, not process-worker IDs. |
| `TestResult.startTime` | property | Partial | `AttemptResult.start_time_ms` ([source](crates/ferrite-e2e/src/report.rs#L249)) | Each attempt is retained in TestResult.attempt_results and JSON/HTML; raw status includes timeout/interruption, errors include phase/code and steps include user/action/assertion/hook/fixture trees. Different Rust schema, no stdout/stderr capture. |
| `TestResult.status` | property | Partial | `AttemptResult.status` ([source](crates/ferrite-e2e/src/report.rs#L245)) | Each attempt is retained in TestResult.attempt_results and JSON/HTML; raw status includes timeout/interruption, errors include phase/code and steps include user/action/assertion/hook/fixture trees. Different Rust schema, no stdout/stderr capture. |
| `TestResult.stderr` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestResult.stdout` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestResult.steps` | property | Partial | `AttemptResult.steps` ([source](crates/ferrite-e2e/src/report.rs#L253)) | Each attempt is retained in TestResult.attempt_results and JSON/HTML; raw status includes timeout/interruption, errors include phase/code and steps include user/action/assertion/hook/fixture trees. Different Rust schema, no stdout/stderr capture. |
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
| `TestStep.category` | property | Partial | `StepInfo.category` ([source](crates/ferrite-e2e/src/report.rs#L212)) | Persisted annotations and full title path; categories user/action/assertion/hook/fixture differ from the upstream category vocabulary and not all Page/protocol operations are wrapped. |
| `TestStep.duration` | property | Partial | `StepInfo.duration_ms` ([source](crates/ferrite-e2e/src/report.rs#L225)) | Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data. |
| `TestStep.location` | property | Partial | `StepInfo.location` ([source](crates/ferrite-e2e/src/report.rs#L222)) | Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data. |
| `TestStep.error` | property | Partial | `StepInfo.error` ([source](crates/ferrite-e2e/src/report.rs#L227)) | Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data. |
| `TestStep.parent` | property | Partial | `StepInfo.parent_id` ([source](crates/ferrite-e2e/src/report.rs#L220)) | Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data. |
| `TestStep.params` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestStep.startTime` | property | Partial | `StepInfo.start_time_ms` ([source](crates/ferrite-e2e/src/report.rs#L224)) | Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data. |
| `TestStep.steps` | property | Partial | `StepInfo.steps` ([source](crates/ferrite-e2e/src/report.rs#L228)) | Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data. |
| `TestStep.annotations` | property | Partial | `StepInfo.annotations` ([source](crates/ferrite-e2e/src/report.rs#L216)) | Persisted annotations and full title path; categories user/action/assertion/hook/fixture differ from the upstream category vocabulary and not all Page/protocol operations are wrapped. |
| `TestStep.attachments` | property | Partial | `StepInfo.attachments` ([source](crates/ferrite-e2e/src/report.rs#L229)) | Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data. |
| `TestStep.title` | property | Partial | `StepInfo.title` ([source](crates/ferrite-e2e/src/report.rs#L221)) | Persisted user/action/assertion/hook/fixture trees on attempts and live callbacks; run-wide lifecycle scopes appear on TestReport.run_steps. Parent is an ID; exact Rust sources only for explicit user steps, automatic sources use enclosing test definition. No JS stack data. |
| `TestStep.subtitle` | property | Missing | — | No dedicated counterpart for this member. Ferrite has live Reporter callbacks with AttemptInfo/StepInfo and aggregate TestReport, but no complete upstream object graph or all plugin controls. |
| `TestStep.titlePath` | method | Partial | `StepInfo.title_path` ([source](crates/ferrite-e2e/src/report.rs#L233)) | Persisted annotations and full title path; categories user/action/assertion/hook/fixture differ from the upstream category vocabulary and not all Page/protocol operations are wrapped. |

## TestStepInfo

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-teststepinfo.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestStepInfo.annotations` | property | Partial | `StepContext.annotations` ([source](crates/ferrite-e2e/src/report.rs#L184)) | Live getters on the context passed to Page.step_with; annotate adds source-aware metadata, title paths include file/test/ancestor steps. Completed records reject late annotations. |
| `TestStepInfo.attach` | method | Partial | `TestInfo.attach` ([source](crates/ferrite-e2e/src/runner.rs#L731)) | Attachments inside an awaited user step associate with that step and its attempt. Page.step_with passes a live StepContext; attachments still use TestInfo.attach and associate with the active scope. Detached Tokio tasks do not inherit parent scope. |
| `TestStepInfo.skip` | method | Partial | `StepContext.skip` ([source](crates/ferrite-e2e/src/report.rs#L142)) | Use step.skip(reason)? to abort only this controlled step. Shared clones cancel pending children; returns StepOutcome::Skipped, distinct from whole-test skip. Conditional skipping uses a Rust if statement. |
| `TestStepInfo.titlePath` | property | Partial | `StepContext.title_path` ([source](crates/ferrite-e2e/src/report.rs#L195)) | Live getters on the context passed to Page.step_with; annotate adds source-aware metadata, title paths include file/test/ancestor steps. Completed records reject late annotations. |

## TimeoutError

No own JS-applicable member headings. The class/error type is not exposed as a Ferrite class; use `E2eError`.

## Touchscreen

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-touchscreen.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Touchscreen.tap` | method | Partial | `Page.touchscreen_tap` ([source](crates/ferrite-e2e/src/page.rs#L3143)) | Coordinate tap available; no separate Touchscreen object. |

## Tracing

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-tracing.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Tracing.start` | method | Partial | `BrowserContext.start_tracing` ([source](crates/ferrite-e2e/src/context.rs#L1016)) | Custom JSON actions/logs/requests; optional screenshots at Page.step only, no DOM/ARIA/source snapshots. |
| `Tracing.startChunk` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Tracing.startHar` | method | Partial | `Page.start_request_capture` ([source](crates/ferrite-e2e/src/page.rs#L4344)) | Page capture + save_har_with; no Tracing.startHar API or full browser/API-request tracing. |
| `Tracing.group` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Tracing.groupEnd` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Tracing.stop` | method | Partial | `BrowserContext.stop_tracing` ([source](crates/ferrite-e2e/src/context.rs#L1029)) | Writes JSON, not a Trace Viewer-compatible zip archive. |
| `Tracing.stopChunk` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Tracing.stopHar` | method | Partial | `Page.save_har_with` ([source](crates/ferrite-e2e/src/page.rs#L4384)) | HAR exporter; no Tracing.stopHar interface, update/rewrite mode or full timing/body coverage. |

## Video

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-video.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Video.delete` | method | Idiomatic | — | Caller deletes artifact through Rust filesystem operations; no Video object. |
| `Video.path` | method | Partial | `TestResult.video` ([source](crates/ferrite-e2e/src/report.rs#L735)) | Runner records an optional artifact path, not Page.video()/Video object. |
| `Video.saveAs` | method | Partial | `Page.stop_video` ([source](crates/ferrite-e2e/src/page.rs#L5079)) | Stop recording to a path; no independently awaitable Video handle. |

## WebError

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-weberror.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `WebError.page` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `WebError.error` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `WebError.location` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## WebSocket

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-websocket.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `WebSocket.close` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1932)) | PageEvent::WebSocket direction Closed; Chromium-only observation without a WebSocket object. |
| `WebSocket.frameReceived` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1932)) | PageEvent::WebSocket direction Received; Chromium-only observation without a WebSocket object. |
| `WebSocket.frameSent` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1932)) | PageEvent::WebSocket direction Sent; Chromium-only observation without a WebSocket object. |
| `WebSocket.socketError` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `WebSocket.isClosed` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `WebSocket.url` | method | Partial | `WebSocketEvent.url` ([source](crates/ferrite-e2e/src/page.rs#L276)) | Captured socket URL, Chromium only. |
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
| `WebStorage.items` | method | Missing | — | No dedicated storage enumeration API; general evaluate can be used as a workaround. |
| `WebStorage.getItem` | method | Partial | `Page.local_storage_get` ([source](crates/ferrite-e2e/src/page.rs#L3312)) | Page helper methods for local and session storage; no WebStorage object. |
| `WebStorage.setItem` | method | Partial | `Page.local_storage_set` ([source](crates/ferrite-e2e/src/page.rs#L3319)) | Page helper methods for local and session storage; no WebStorage object. |
| `WebStorage.removeItem` | method | Partial | `Page.local_storage_remove` ([source](crates/ferrite-e2e/src/page.rs#L3326)) | Page helper methods for local and session storage; no WebStorage object. |
| `WebStorage.clear` | method | Partial | `Page.local_storage_clear` ([source](crates/ferrite-e2e/src/page.rs#L3339)) | Page helper methods for local and session storage; no WebStorage object. |

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
| `WorkerInfo.project` | property | Partial | `WorkerInfo.project` ([source](crates/ferrite-e2e/src/runner.rs#L127)) | Optional project name only, not a resolved FullProject object. |
| `WorkerInfo.workerIndex` | property | Partial | `WorkerInfo.worker_index` ([source](crates/ferrite-e2e/src/runner.rs#L126)) | Logical Tokio worker index, not a process identity. |
