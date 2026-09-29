# Playwright API member matrix

Audit date: 2026-09-29. Baseline: [Playwright v1.63.0](https://github.com/microsoft/playwright/releases/tag/v1.63.0). Local target: current working tree of `ferrite-e2e`, including pre-existing local edits.

Read [PLAYWRIGHT-PARITY.md](PLAYWRIGHT-PARITY.md) for findings, engine limitations, priorities and test evidence.

This inventory covers every JavaScript-applicable method, property and event documented in the pinned upstream browser API, test API, reporter API, Electron API and Android API directories. Language-specific members are excluded; same-name overloads are collapsed within a member kind. Deprecated APIs and experimental APIs remain visible. Inherited members are represented on their declaring class. Arguments/options are reviewed by feature in the companion report, not counted as separate members.

| Status | Meaning |
|---|---|
| Equivalent | A counterpart exists for the basic operation/value; this is not a claim of full class/options/engine parity. |
| Partial | Related exposed operation, manual composition or field exists, with semantic/options/engine differences. |
| Idiomatic | Comparable checks/operations are expressed through Rust language/library facilities; no Playwright-style API object. |
| Missing | No dedicated counterpart found; arbitrary JS evaluation or raw CDP/BiDi calls do not establish feature parity. |

Inventory: **73 classes, 1018 distinct members**. Equivalent: 15; Partial: 458; Idiomatic: 42; Missing: 503. These counts are inventory labels, not a percentage of behavioral compatibility.

## APIRequest

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-apirequest.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `APIRequest.newContext` | method | Partial | `ApiClient.new` ([source](crates/ferrite-e2e/src/api.rs#L73)) | Standalone client constructor; no options/configurable cookie storage. |

## APIRequestContext

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-apirequestcontext.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `APIRequestContext.delete` | method | Partial | `ApiClient.delete` ([source](crates/ferrite-e2e/src/api.rs#L106)) | HTTP verb available; raw request()/post_form() alternatives; no multipart, cookie jar, storageState, per-call timeout/auth/proxy/retry/redirect policy. |
| `APIRequestContext.dispose` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIRequestContext.fetch` | method | Partial | `ApiClient.fetch` ([source](crates/ferrite-e2e/src/api.rs#L172)) | HTTP verb available; raw request()/post_form() alternatives; no multipart, cookie jar, storageState, per-call timeout/auth/proxy/retry/redirect policy. |
| `APIRequestContext.get` | method | Partial | `ApiClient.get` ([source](crates/ferrite-e2e/src/api.rs#L99)) | HTTP verb available; raw request()/post_form() alternatives; no multipart, cookie jar, storageState, per-call timeout/auth/proxy/retry/redirect policy. |
| `APIRequestContext.head` | method | Partial | `ApiClient.head` ([source](crates/ferrite-e2e/src/api.rs#L134)) | HTTP verb available; raw request()/post_form() alternatives; no multipart, cookie jar, storageState, per-call timeout/auth/proxy/retry/redirect policy. |
| `APIRequestContext.patch` | method | Partial | `ApiClient.patch_json` ([source](crates/ferrite-e2e/src/api.rs#L127)) | HTTP verb available; raw request()/post_form() alternatives; no multipart, cookie jar, storageState, per-call timeout/auth/proxy/retry/redirect policy. |
| `APIRequestContext.post` | method | Partial | `ApiClient.post_json` ([source](crates/ferrite-e2e/src/api.rs#L113)) | HTTP verb available; raw request()/post_form() alternatives; no multipart, cookie jar, storageState, per-call timeout/auth/proxy/retry/redirect policy. |
| `APIRequestContext.put` | method | Partial | `ApiClient.put_json` ([source](crates/ferrite-e2e/src/api.rs#L120)) | HTTP verb available; raw request()/post_form() alternatives; no multipart, cookie jar, storageState, per-call timeout/auth/proxy/retry/redirect policy. |
| `APIRequestContext.storageState` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIRequestContext.tracing` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## APIResponse

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-apiresponse.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `APIResponse.body` | method | Equivalent | `ApiResponse.bytes` ([source](crates/ferrite-e2e/src/api.rs#L46)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.dispose` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIResponse.headers` | method | Partial | `ApiResponse.headers` ([source](crates/ferrite-e2e/src/api.rs#L31)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.headersArray` | method | Partial | `ApiResponse.headers` ([source](crates/ferrite-e2e/src/api.rs#L31)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.json` | method | Equivalent | `ApiResponse.json` ([source](crates/ferrite-e2e/src/api.rs#L57)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.ok` | method | Equivalent | `ApiResponse.ok` ([source](crates/ferrite-e2e/src/api.rs#L25)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.securityDetails` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIResponse.serverAddr` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIResponse.status` | method | Equivalent | `ApiResponse.status` ([source](crates/ferrite-e2e/src/api.rs#L19)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.statusText` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIResponse.text` | method | Partial | `ApiResponse.text` ([source](crates/ferrite-e2e/src/api.rs#L52)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.timing` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIResponse.url` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## APIResponseAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-apiresponseassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `APIResponseAssertions.not` | property | Idiomatic | `ApiResponse.ok` ([source](crates/ferrite-e2e/src/api.rs#L25)) | Use assert!(response.ok()) or its negation; no dedicated retrying assertion object. |
| `APIResponseAssertions.toBeOK` | method | Idiomatic | `ApiResponse.ok` ([source](crates/ferrite-e2e/src/api.rs#L25)) | Use assert!(response.ok()) or its negation; no dedicated retrying assertion object. |

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
| `Browser.browserType` | method | Partial | `Browser.kind` ([source](crates/ferrite-e2e/src/browser.rs#L647)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.close` | method | Partial | `Browser.close` ([source](crates/ferrite-e2e/src/browser.rs#L990)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.contexts` | method | Partial | `Browser.contexts` ([source](crates/ferrite-e2e/src/browser.rs#L967)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.isConnected` | method | Partial | `Browser.is_connected` ([source](crates/ferrite-e2e/src/browser.rs#L864)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.newBrowserCDPSession` | method | Partial | `Browser.cdp` ([source](crates/ferrite-e2e/src/browser.rs#L841)) | Raw shared browser CDP connection; no independently detachable CDPSession. |
| `Browser.newContext` | method | Partial | `Browser.new_context` ([source](crates/ferrite-e2e/src/browser.rs#L872)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.newPage` | method | Partial | `Browser.new_page` ([source](crates/ferrite-e2e/src/browser.rs#L984)) | Uses the shared default context; Playwright creates a new context for this convenience API. |
| `Browser.bind` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.removeAllListeners` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.startTracing` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.stopTracing` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.unbind` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.version` | method | Partial | `Browser.version` ([source](crates/ferrite-e2e/src/browser.rs#L858)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |

## BrowserContext

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-browsercontext.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `BrowserContext.backgroundPage` (deprecated) | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.clock` | property | Partial | `Page.clock_install` ([source](crates/ferrite-e2e/src/page.rs#L2490)) | Page-document-local clock; no context-wide clock object. |
| `BrowserContext.credentials` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.debugger` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.close` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.console` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.dialog` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.dialogClosed` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.download` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.frameAttached` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.frameDetached` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.frameNavigated` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.page` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.pageClose` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.pageLoad` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.webError` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.request` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.requestFailed` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.requestFinished` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.response` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.serviceWorker` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.addCookies` | method | Partial | `BrowserContext.add_cookies` ([source](crates/ferrite-e2e/src/context.rs#L560)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.addInitScript` | method | Partial | `BrowserContext.add_init_script` ([source](crates/ferrite-e2e/src/context.rs#L1037)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.backgroundPages` (deprecated) | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.browser` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.clearCookies` | method | Partial | `BrowserContext.clear_cookies` ([source](crates/ferrite-e2e/src/context.rs#L552)) | Clears all cookies; no name/domain/path filters. |
| `BrowserContext.clearPermissions` | method | Partial | `BrowserContext.clear_permissions` ([source](crates/ferrite-e2e/src/context.rs#L841)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.close` | method | Partial | `BrowserContext.close` ([source](crates/ferrite-e2e/src/context.rs#L1084)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.cookies` | method | Partial | `BrowserContext.cookies` ([source](crates/ferrite-e2e/src/context.rs#L544)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.exposeBinding` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.exposeFunction` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.grantPermissions` | method | Partial | `BrowserContext.grant_permissions` ([source](crates/ferrite-e2e/src/context.rs#L812)) | No origin argument; Chromium grants broadly, Firefox grants after navigation for the current origin. |
| `BrowserContext.isClosed` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.newCDPSession` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.newPage` | method | Partial | `BrowserContext.new_page` ([source](crates/ferrite-e2e/src/context.rs#L387)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.pages` | method | Partial | `BrowserContext.pages` ([source](crates/ferrite-e2e/src/context.rs#L539)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.removeAllListeners` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.request` | property | Partial | `ApiClient` ([source](crates/ferrite-e2e/src/api.rs#L64)) | Available separately; context.request with shared browser cookies is absent. |
| `BrowserContext.route` | method | Partial | `BrowserContext.route_with_handler` ([source](crates/ferrite-e2e/src/context.rs#L600)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.routeFromHAR` | method | Partial | `BrowserContext.route_from_har` ([source](crates/ferrite-e2e/src/context.rs#L767)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.routeWebSocket` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.serviceWorkers` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.setDefaultNavigationTimeout` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.setDefaultTimeout` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.setExtraHTTPHeaders` | method | Partial | `BrowserContext.set_extra_http_headers` ([source](crates/ferrite-e2e/src/context.rs#L947)) | Chromium only; HTTP credentials hold one username/password pair, no origin list. |
| `BrowserContext.setGeolocation` | method | Partial | `BrowserContext.set_geolocation` ([source](crates/ferrite-e2e/src/context.rs#L832)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.setHTTPCredentials` | method | Partial | `BrowserContext.set_http_credentials` ([source](crates/ferrite-e2e/src/context.rs#L921)) | Chromium only; HTTP credentials hold one username/password pair, no origin list. |
| `BrowserContext.setOffline` | method | Partial | `BrowserContext.set_offline` ([source](crates/ferrite-e2e/src/context.rs#L911)) | Chromium only; HTTP credentials hold one username/password pair, no origin list. |
| `BrowserContext.storageState` | method | Partial | `BrowserContext.storage_state` ([source](crates/ferrite-e2e/src/context.rs#L1073)) | Captures only the first open page and one origin; no Playwright origins[] format, IndexedDB or OPFS persistence. |
| `BrowserContext.setStorageState` | method | Partial | `BrowserContext.load_storage_state` ([source](crates/ferrite-e2e/src/context.rs#L1052)) | Loads Ferrite single-origin JSON; no native Playwright storage-state file compatibility. |
| `BrowserContext.tracing` | property | Partial | `BrowserContext.start_tracing` ([source](crates/ferrite-e2e/src/context.rs#L666)) | Custom JSON trace, no Trace Viewer-compatible archive, DOM snapshots or chunks. |
| `BrowserContext.unrouteAll` | method | Partial | `BrowserContext.unroute_all` ([source](crates/ferrite-e2e/src/context.rs#L746)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.unroute` | method | Partial | `BrowserContext.unroute` ([source](crates/ferrite-e2e/src/context.rs#L720)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.waitForEvent` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

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
| `BrowserType.connectOverCDP` | method | Partial | `Browser.connect` ([source](crates/ferrite-e2e/src/browser.rs#L613)) | Stock-browser discovery/launch; connect accepts a loopback Chromium debug port only; no channels/installer or full Playwright connection options. |
| `BrowserType.executablePath` | method | Partial | `find_chromium` ([source](crates/ferrite-e2e/src/browser.rs#L161)) | Stock-browser discovery/launch; connect accepts a loopback Chromium debug port only; no channels/installer or full Playwright connection options. |
| `BrowserType.launch` | method | Partial | `Browser.launch` ([source](crates/ferrite-e2e/src/browser.rs#L335)) | Stock-browser discovery/launch; connect accepts a loopback Chromium debug port only; no channels/installer or full Playwright connection options. |
| `BrowserType.launchPersistentContext` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserType.launchServer` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserType.name` | method | Partial | `BrowserKind.name` ([source](crates/ferrite-e2e/src/browser.rs#L50)) | Stock-browser discovery/launch; connect accepts a loopback Chromium debug port only; no channels/installer or full Playwright connection options. |

## CDPSession

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-cdpsession.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `CDPSession.close` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `CDPSession.event` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `CDPSession.detach` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `CDPSession.send` | method | Partial | `CdpConnection.call` ([source](crates/ferrite-e2e/src/cdp.rs#L140)) | Low-level CDP transport, with caller-managed session IDs; no dedicated CDPSession lifecycle. |

## Clock

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-clock.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Clock.fastForward` | method | Partial | `Page.clock_fast_forward` ([source](crates/ferrite-e2e/src/page.rs#L2535)) | Alias for advance: fires every due interval, unlike Playwright at-most-once fastForward. |
| `Clock.install` | method | Partial | `Page.clock_install` ([source](crates/ferrite-e2e/src/page.rs#L2490)) | Manual document-local fake timers only; see clock semantic differences in feature audit. |
| `Clock.runFor` | method | Partial | `Page.clock_run_for` ([source](crates/ferrite-e2e/src/page.rs#L2540)) | Manual document-local fake timers only; see clock semantic differences in feature audit. |
| `Clock.pauseAt` | method | Partial | `Page.clock_pause` ([source](crates/ferrite-e2e/src/page.rs#L2560)) | No pause-at-time argument or skip-to-time semantics; toggles paused flag only. |
| `Clock.resume` | method | Partial | `Page.clock_resume` ([source](crates/ferrite-e2e/src/page.rs#L2573)) | Only toggles a flag; it does not restart wall-time timer progression. |
| `Clock.setFixedTime` | method | Partial | `Page.clock_set_fixed_time` ([source](crates/ferrite-e2e/src/page.rs#L2513)) | Mutates the same timer clock; subsequent advances move Date too, rather than fixing Date independently. |
| `Clock.setSystemTime` | method | Partial | `Page.clock_set_system_time` ([source](crates/ferrite-e2e/src/page.rs#L2546)) | Alias for set_fixed_time; changes the timer clock as well as Date. |

## ConsoleMessage

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-consolemessage.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `ConsoleMessage.args` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `ConsoleMessage.location` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `ConsoleMessage.page` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `ConsoleMessage.text` | method | Partial | `ConsoleMessage.text` ([source](crates/ferrite-e2e/src/page.rs#L72)) | String previews only; no JSHandle arguments, source location, timestamp or page/worker ownership. |
| `ConsoleMessage.timestamp` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `ConsoleMessage.type` | method | Partial | `ConsoleMessage.kind` ([source](crates/ferrite-e2e/src/page.rs#L70)) | String previews only; no JSHandle arguments, source location, timestamp or page/worker ownership. |
| `ConsoleMessage.worker` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## Coverage

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-coverage.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Coverage.startCSSCoverage` | method | Missing | — | No public Coverage abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Coverage.startJSCoverage` | method | Missing | — | No public Coverage abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Coverage.stopCSSCoverage` | method | Missing | — | No public Coverage abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |
| `Coverage.stopJSCoverage` | method | Missing | — | No public Coverage abstraction or matching feature API; raw protocol calls are not counted as an equivalent. |

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
| `Dialog.accept` | method | Partial | `DialogDecision.accept_with` ([source](crates/ferrite-e2e/src/page.rs#L934)) | Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page. |
| `Dialog.defaultValue` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Dialog.dismiss` | method | Partial | `DialogDecision.dismiss` ([source](crates/ferrite-e2e/src/page.rs#L939)) | Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page. |
| `Dialog.message` | method | Partial | `DialogInfo.message` ([source](crates/ferrite-e2e/src/page.rs#L175)) | Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page. |
| `Dialog.page` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Dialog.type` | method | Partial | `DialogInfo.dialog_type` ([source](crates/ferrite-e2e/src/page.rs#L173)) | Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page. |

## Disposable

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-disposable.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Disposable.dispose` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## Download

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-download.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Download.cancel` | method | Partial | `Page.cancel_downloads` ([source](crates/ferrite-e2e/src/page.rs#L3345)) | Cancels page-tracked downloads on Chromium; no per-Download.cancel method. |
| `Download.createReadStream` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Download.delete` | method | Partial | `Download.delete` ([source](crates/ferrite-e2e/src/page.rs#L1249)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.failure` | method | Partial | `Download.failure` ([source](crates/ferrite-e2e/src/page.rs#L1211)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.page` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Download.path` | method | Partial | `Download.path` ([source](crates/ferrite-e2e/src/page.rs#L1203)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.saveAs` | method | Partial | `Download.save_as` ([source](crates/ferrite-e2e/src/page.rs#L1234)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.suggestedFilename` | method | Partial | `Download.suggested_filename` ([source](crates/ferrite-e2e/src/page.rs#L1205)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.url` | method | Partial | `Download.url` ([source](crates/ferrite-e2e/src/page.rs#L1208)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |

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
| `Fixtures.browser` | property | Partial | `Browser` ([source](crates/ferrite-e2e/src/browser.rs#L312)) | Manual Browser/Context/client setup or injected Page; runner uses shared default context rather than fresh context per attempt. |
| `Fixtures.browserName` | property | Partial | `Browser.kind` ([source](crates/ferrite-e2e/src/browser.rs#L647)) | Manual Browser/Context/client setup or injected Page; runner uses shared default context rather than fresh context per attempt. |
| `Fixtures.context` | property | Partial | `BrowserContext` ([source](crates/ferrite-e2e/src/context.rs#L320)) | Manual Browser/Context/client setup or injected Page; runner uses shared default context rather than fresh context per attempt. |
| `Fixtures.mount` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Fixtures.page` | property | Partial | `test` ([source](crates/ferrite-e2e/src/runner.rs#L137)) | Manual Browser/Context/client setup or injected Page; runner uses shared default context rather than fresh context per attempt. |
| `Fixtures.request` | property | Partial | `ApiClient` ([source](crates/ferrite-e2e/src/api.rs#L64)) | Manual Browser/Context/client setup or injected Page; runner uses shared default context rather than fresh context per attempt. |

## Frame

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-frame.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Frame.addScriptTag` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.addStyleTag` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.check` | method | Partial | `Locator.check` ([source](crates/ferrite-e2e/src/locator.rs#L1369)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.childFrames` | method | Partial | `Frame.child_frames` ([source](crates/ferrite-e2e/src/page.rs#L397)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.click` | method | Partial | `Locator.click` ([source](crates/ferrite-e2e/src/locator.rs#L1102)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.content` | method | Equivalent | `Frame.content` ([source](crates/ferrite-e2e/src/page.rs#L427)) | Basic document access. |
| `Frame.dblclick` | method | Partial | `Locator.dblclick` ([source](crates/ferrite-e2e/src/locator.rs#L1138)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.dispatchEvent` | method | Partial | `Locator.dispatch_event` ([source](crates/ferrite-e2e/src/locator.rs#L1221)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.dragAndDrop` | method | Partial | `Locator.drag_to` ([source](crates/ferrite-e2e/src/locator.rs#L1188)) | Frame.locator(source).drag_to(target, steps); fewer options and frame-coordinate restrictions. |
| `Frame.evalOnSelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.evalOnSelectorAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.evaluate` | method | Partial | `Frame.evaluate` ([source](crates/ferrite-e2e/src/page.rs#L436)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.evaluateHandle` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.fill` | method | Partial | `Locator.fill` ([source](crates/ferrite-e2e/src/locator.rs#L1271)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.focus` | method | Partial | `Locator.focus` ([source](crates/ferrite-e2e/src/locator.rs#L1162)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.frameElement` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.frameLocator` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.getAttribute` | method | Partial | `Locator.attribute` ([source](crates/ferrite-e2e/src/locator.rs#L1449)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.getByAltText` | method | Partial | `Frame.get_by_alt` ([source](crates/ferrite-e2e/src/page.rs#L494)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Frame.getByLabel` | method | Partial | `Frame.get_by_label` ([source](crates/ferrite-e2e/src/page.rs#L484)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Frame.getByPlaceholder` | method | Partial | `Frame.get_by_placeholder` ([source](crates/ferrite-e2e/src/page.rs#L489)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Frame.getByRole` | method | Partial | `Frame.get_by_role_with` ([source](crates/ferrite-e2e/src/page.rs#L475)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Frame.getByTestId` | method | Partial | `Frame.get_by_test_id` ([source](crates/ferrite-e2e/src/page.rs#L460)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Frame.getByText` | method | Partial | `Frame.get_by_text` ([source](crates/ferrite-e2e/src/page.rs#L465)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Frame.getByTitle` | method | Partial | `Frame.get_by_title` ([source](crates/ferrite-e2e/src/page.rs#L499)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Frame.goto` | method | Partial | `Frame.goto` ([source](crates/ferrite-e2e/src/page.rs#L379)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.hover` | method | Partial | `Locator.hover` ([source](crates/ferrite-e2e/src/locator.rs#L1149)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.innerHTML` | method | Partial | `Locator.inner_html` ([source](crates/ferrite-e2e/src/locator.rs#L1481)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.innerText` | method | Partial | `Locator.text` ([source](crates/ferrite-e2e/src/locator.rs#L1421)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.inputValue` | method | Partial | `Locator.input_value` ([source](crates/ferrite-e2e/src/locator.rs#L1426)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isChecked` | method | Partial | `Locator.is_checked` ([source](crates/ferrite-e2e/src/locator.rs#L1551)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isDetached` | method | Partial | `Frame.is_detached` ([source](crates/ferrite-e2e/src/page.rs#L408)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.isDisabled` | method | Partial | `Locator.is_disabled` ([source](crates/ferrite-e2e/src/locator.rs#L1546)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isEditable` | method | Partial | `Locator.is_editable` ([source](crates/ferrite-e2e/src/locator.rs#L1556)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isEnabled` | method | Partial | `Locator.is_enabled` ([source](crates/ferrite-e2e/src/locator.rs#L1541)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isHidden` | method | Partial | `Locator.is_hidden` ([source](crates/ferrite-e2e/src/locator.rs#L1535)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isVisible` | method | Partial | `Locator.is_visible` ([source](crates/ferrite-e2e/src/locator.rs#L1530)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.locator` | method | Partial | `Frame.locator` ([source](crates/ferrite-e2e/src/page.rs#L455)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Frame.name` | method | Partial | `Frame.name` ([source](crates/ferrite-e2e/src/page.rs#L368)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.page` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.parentFrame` | method | Partial | `Frame.parent` ([source](crates/ferrite-e2e/src/page.rs#L384)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.press` | method | Partial | `Locator.press` ([source](crates/ferrite-e2e/src/locator.rs#L1293)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.querySelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.querySelectorAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.selectOption` | method | Partial | `Locator.select_options` ([source](crates/ferrite-e2e/src/locator.rs#L1394)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.setChecked` | method | Partial | `Locator.set_checked` ([source](crates/ferrite-e2e/src/locator.rs#L1593)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.setContent` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.setInputFiles` | method | Partial | `Locator.set_input_files` ([source](crates/ferrite-e2e/src/locator.rs#L1337)) | Use Frame.locator(selector); path uploads only and fewer options. |
| `Frame.tap` | method | Partial | `Locator.tap` ([source](crates/ferrite-e2e/src/locator.rs#L1174)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.textContent` | method | Partial | `Locator.text` ([source](crates/ferrite-e2e/src/locator.rs#L1421)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.title` | method | Equivalent | `Frame.title` ([source](crates/ferrite-e2e/src/page.rs#L418)) | Basic document access. |
| `Frame.type` (deprecated) | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L1305)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.uncheck` | method | Partial | `Locator.uncheck` ([source](crates/ferrite-e2e/src/locator.rs#L1377)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.url` | method | Partial | `Frame.url` ([source](crates/ferrite-e2e/src/page.rs#L374)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.waitForFunction` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.waitForLoadState` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.waitForNavigation` (deprecated) | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.waitForSelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.waitForTimeout` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.waitForURL` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## FrameLocator

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-framelocator.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `FrameLocator.first` (deprecated) | method | Missing | — | No lazy FrameLocator/cross-frame selector API; use document_frames()/frame_by_name()/frame_by_url() and Frame.locator manually. |
| `FrameLocator.frameLocator` | method | Missing | — | No lazy FrameLocator/cross-frame selector API; use document_frames()/frame_by_name()/frame_by_url() and Frame.locator manually. |
| `FrameLocator.getByAltText` | method | Missing | — | No lazy FrameLocator/cross-frame selector API; use document_frames()/frame_by_name()/frame_by_url() and Frame.locator manually. |
| `FrameLocator.getByLabel` | method | Missing | — | No lazy FrameLocator/cross-frame selector API; use document_frames()/frame_by_name()/frame_by_url() and Frame.locator manually. |
| `FrameLocator.getByPlaceholder` | method | Missing | — | No lazy FrameLocator/cross-frame selector API; use document_frames()/frame_by_name()/frame_by_url() and Frame.locator manually. |
| `FrameLocator.getByRole` | method | Missing | — | No lazy FrameLocator/cross-frame selector API; use document_frames()/frame_by_name()/frame_by_url() and Frame.locator manually. |
| `FrameLocator.getByTestId` | method | Missing | — | No lazy FrameLocator/cross-frame selector API; use document_frames()/frame_by_name()/frame_by_url() and Frame.locator manually. |
| `FrameLocator.getByText` | method | Missing | — | No lazy FrameLocator/cross-frame selector API; use document_frames()/frame_by_name()/frame_by_url() and Frame.locator manually. |
| `FrameLocator.getByTitle` | method | Missing | — | No lazy FrameLocator/cross-frame selector API; use document_frames()/frame_by_name()/frame_by_url() and Frame.locator manually. |
| `FrameLocator.last` (deprecated) | method | Missing | — | No lazy FrameLocator/cross-frame selector API; use document_frames()/frame_by_name()/frame_by_url() and Frame.locator manually. |
| `FrameLocator.locator` | method | Missing | — | No lazy FrameLocator/cross-frame selector API; use document_frames()/frame_by_name()/frame_by_url() and Frame.locator manually. |
| `FrameLocator.nth` (deprecated) | method | Missing | — | No lazy FrameLocator/cross-frame selector API; use document_frames()/frame_by_name()/frame_by_url() and Frame.locator manually. |
| `FrameLocator.owner` | method | Missing | — | No lazy FrameLocator/cross-frame selector API; use document_frames()/frame_by_name()/frame_by_url() and Frame.locator manually. |

## FullConfig

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-fullconfig.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `FullConfig.argv` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.configFile` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.failOnFlakyTests` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.forbidOnly` | property | Partial | `Runner.forbid_only` ([source](crates/ferrite-e2e/src/runner.rs#L858)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.fullyParallel` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.globalSetup` | property | Partial | `Runner.global_setup` ([source](crates/ferrite-e2e/src/runner.rs#L771)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.globalTeardown` | property | Partial | `Runner.global_teardown` ([source](crates/ferrite-e2e/src/runner.rs#L782)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.globalTimeout` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.grep` | property | Partial | `Runner.grep` ([source](crates/ferrite-e2e/src/runner.rs#L704)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.grepInvert` | property | Partial | `Runner.grep_invert` ([source](crates/ferrite-e2e/src/runner.rs#L713)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.maxFailures` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.metadata` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.preserveOutput` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.projects` | property | Partial | `Runner.project` ([source](crates/ferrite-e2e/src/runner.rs#L842)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.quiet` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.reporter` | property | Partial | `E2eConfig.reporter` ([source](crates/ferrite-config/src/lib.rs#L424)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.reportSlowTests` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.rootDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.shard` | property | Partial | `Runner.shard` ([source](crates/ferrite-e2e/src/runner.rs#L800)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.tags` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.updateSnapshots` | property | Partial | `E2eConfig.update_snapshots` ([source](crates/ferrite-config/src/lib.rs#L436)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.updateSourceMethod` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.version` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.webServer` | property | Partial | `E2eConfig.web_server` ([source](crates/ferrite-config/src/lib.rs#L440)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.workers` | property | Partial | `Runner.workers` ([source](crates/ferrite-e2e/src/runner.rs#L672)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |

## FullProject

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-fullproject.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `FullProject.dependencies` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.grep` | property | Partial | `Project.grep` ([source](crates/ferrite-e2e/src/runner.rs#L347)) | Project name/filter/retry/timeout override only; all projects run against the same Browser supplied to Runner.run. |
| `FullProject.grepInvert` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.ignoreSnapshots` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.metadata` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.name` | property | Partial | `Project.name` ([source](crates/ferrite-e2e/src/runner.rs#L326)) | Project name/filter/retry/timeout override only; all projects run against the same Browser supplied to Runner.run. |
| `FullProject.snapshotDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.outputDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.repeatEach` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.retries` | property | Partial | `Project.retries` ([source](crates/ferrite-e2e/src/runner.rs#L354)) | Project name/filter/retry/timeout override only; all projects run against the same Browser supplied to Runner.run. |
| `FullProject.teardown` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.testDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.testIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.testMatch` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.timeout` | property | Partial | `Project.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L361)) | Project name/filter/retry/timeout override only; all projects run against the same Browser supplied to Runner.run. |
| `FullProject.use` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

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
| `JSHandle.dispose` | method | Partial | `JSHandle.dispose` ([source](crates/ferrite-e2e/src/jshandle.rs#L76)) | Remote references exist; expression-string/JSON bridge has fewer value and argument types. |
| `JSHandle.evaluate` | method | Partial | `JSHandle.evaluate` ([source](crates/ferrite-e2e/src/jshandle.rs#L66)) | Remote references exist; expression-string/JSON bridge has fewer value and argument types. |
| `JSHandle.evaluateHandle` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `JSHandle.getProperties` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `JSHandle.getProperty` | method | Partial | `JSHandle.get_property` ([source](crates/ferrite-e2e/src/jshandle.rs#L58)) | Remote references exist; expression-string/JSON bridge has fewer value and argument types. |
| `JSHandle.jsonValue` | method | Partial | `JSHandle.json_value` ([source](crates/ferrite-e2e/src/jshandle.rs#L49)) | Remote references exist; expression-string/JSON bridge has fewer value and argument types. |

## Keyboard

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-keyboard.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Keyboard.down` | method | Partial | `Page.key_down` ([source](crates/ferrite-e2e/src/page.rs#L2249)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |
| `Keyboard.insertText` | method | Partial | `Page.insert_text` ([source](crates/ferrite-e2e/src/page.rs#L2196)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |
| `Keyboard.press` | method | Partial | `Page.press_key_with` ([source](crates/ferrite-e2e/src/page.rs#L2210)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |
| `Keyboard.type` | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L1305)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |
| `Keyboard.up` | method | Partial | `Page.key_up` ([source](crates/ferrite-e2e/src/page.rs#L2256)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |

## Location

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-location.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Location.file` | property | Partial | `TestInfo.file` ([source](crates/ferrite-e2e/src/runner.rs#L190)) | Rust track_caller metadata; no column. |
| `Location.line` | property | Partial | `TestInfo.line` ([source](crates/ferrite-e2e/src/runner.rs#L192)) | Rust track_caller metadata; no column. |
| `Location.column` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## Locator

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-locator.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Locator.all` | method | Equivalent | `Locator.all` ([source](crates/ferrite-e2e/src/locator.rs#L974)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.allInnerTexts` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.allTextContents` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.and` | method | Equivalent | `Locator.and_` ([source](crates/ferrite-e2e/src/locator.rs#L926)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.ariaSnapshot` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.ariaSnapshotJSON` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.blur` | method | Partial | `Locator.blur` ([source](crates/ferrite-e2e/src/locator.rs#L1168)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.boundingBox` | method | Partial | `Locator.bounding_box` ([source](crates/ferrite-e2e/src/locator.rs#L1571)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.check` | method | Partial | `Locator.check` ([source](crates/ferrite-e2e/src/locator.rs#L1369)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.clear` | method | Partial | `Locator.clear` ([source](crates/ferrite-e2e/src/locator.rs#L1329)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.click` | method | Partial | `Locator.click_with_options` ([source](crates/ferrite-e2e/src/locator.rs#L1107)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.count` | method | Equivalent | `Locator.count` ([source](crates/ferrite-e2e/src/locator.rs#L1028)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.dblclick` | method | Partial | `Locator.dblclick` ([source](crates/ferrite-e2e/src/locator.rs#L1138)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.describe` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.description` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.dispatchEvent` | method | Partial | `Locator.dispatch_event` ([source](crates/ferrite-e2e/src/locator.rs#L1221)) | Always creates a bubbling CustomEvent with detail; not Playwright eventInit/type-specific event behavior. |
| `Locator.dragTo` | method | Partial | `Locator.drag_to` ([source](crates/ferrite-e2e/src/locator.rs#L1188)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.drop` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.elementHandle` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.elementHandles` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.contentFrame` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.evaluate` | method | Partial | `Locator.evaluate` ([source](crates/ferrite-e2e/src/locator.rs#L1604)) | Function string over the first element, JSON result; no explicit arg/JSHandle support. |
| `Locator.evaluateAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.evaluateHandle` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.fill` | method | Partial | `Locator.fill` ([source](crates/ferrite-e2e/src/locator.rs#L1271)) | Immediate DOM value assignment plus input/change events; no contenteditable support or full auto-wait/editability checks. |
| `Locator.filter` | method | Partial | `Locator.filter_with` ([source](crates/ferrite-e2e/src/locator.rs#L889)) | has/hasNot/text predicates present; no regex or visibility option; containment checks resolve inner locators document-wide. |
| `Locator.first` | method | Partial | `Locator.first` ([source](crates/ferrite-e2e/src/locator.rs#L834)) | Pick::First does not narrow the resolved set; count()/all() can still return every match. Actions already use the first match. |
| `Locator.focus` | method | Partial | `Locator.focus` ([source](crates/ferrite-e2e/src/locator.rs#L1162)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.frameLocator` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.getAttribute` | method | Partial | `Locator.attribute` ([source](crates/ferrite-e2e/src/locator.rs#L1449)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.getByAltText` | method | Partial | `Locator.get_by_alt` ([source](crates/ferrite-e2e/src/locator.rs#L1017)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Locator.getByLabel` | method | Partial | `Locator.get_by_label` ([source](crates/ferrite-e2e/src/locator.rs#L1005)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Locator.getByPlaceholder` | method | Partial | `Locator.get_by_placeholder` ([source](crates/ferrite-e2e/src/locator.rs#L1011)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Locator.getByRole` | method | Partial | `Locator.get_by_role_with` ([source](crates/ferrite-e2e/src/locator.rs#L999)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Locator.getByTestId` | method | Partial | `Locator.get_by_test_id` ([source](crates/ferrite-e2e/src/locator.rs#L981)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Locator.getByText` | method | Partial | `Locator.get_by_text` ([source](crates/ferrite-e2e/src/locator.rs#L987)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Locator.getByTitle` | method | Partial | `Locator.get_by_title` ([source](crates/ferrite-e2e/src/locator.rs#L1023)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Locator.hideHighlight` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.highlight` | method | Partial | `Locator.highlight` ([source](crates/ferrite-e2e/src/locator.rs#L1582)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.hover` | method | Partial | `Locator.hover` ([source](crates/ferrite-e2e/src/locator.rs#L1149)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.innerHTML` | method | Partial | `Locator.inner_html` ([source](crates/ferrite-e2e/src/locator.rs#L1481)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.innerText` | method | Partial | `Locator.text` ([source](crates/ferrite-e2e/src/locator.rs#L1421)) | Reads trimmed textContent, not rendered innerText. |
| `Locator.inputValue` | method | Partial | `Locator.input_value` ([source](crates/ferrite-e2e/src/locator.rs#L1426)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.isChecked` | method | Partial | `Locator.is_checked` ([source](crates/ferrite-e2e/src/locator.rs#L1551)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.isDisabled` | method | Partial | `Locator.is_disabled` ([source](crates/ferrite-e2e/src/locator.rs#L1546)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.isEditable` | method | Partial | `Locator.is_editable` ([source](crates/ferrite-e2e/src/locator.rs#L1556)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.isEnabled` | method | Partial | `Locator.is_enabled` ([source](crates/ferrite-e2e/src/locator.rs#L1541)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.isHidden` | method | Partial | `Locator.is_hidden` ([source](crates/ferrite-e2e/src/locator.rs#L1535)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.isVisible` | method | Partial | `Locator.is_visible` ([source](crates/ferrite-e2e/src/locator.rs#L1530)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.last` | method | Partial | `Locator.last` ([source](crates/ferrite-e2e/src/locator.rs#L840)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.locator` | method | Partial | `Locator.locator` ([source](crates/ferrite-e2e/src/locator.rs#L828)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Locator.normalize` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.nth` | method | Equivalent | `Locator.nth` ([source](crates/ferrite-e2e/src/locator.rs#L846)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.or` | method | Equivalent | `Locator.or_` ([source](crates/ferrite-e2e/src/locator.rs#L921)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.page` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.press` | method | Partial | `Locator.press_with` ([source](crates/ferrite-e2e/src/locator.rs#L1299)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.pressSequentially` | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L1305)) | Delayed helper sends keys; default press_sequentially instead inserts the whole text and synthesizes events. |
| `Locator.screenshot` | method | Partial | `Locator.screenshot` ([source](crates/ferrite-e2e/src/locator.rs#L1258)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.scrollIntoViewIfNeeded` | method | Partial | `Locator.scroll_into_view` ([source](crates/ferrite-e2e/src/locator.rs#L1215)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.selectOption` | method | Partial | `Locator.select_options` ([source](crates/ferrite-e2e/src/locator.rs#L1394)) | Value/label/index matching exists; returns unit rather than selected values, with fewer options. |
| `Locator.selectText` | method | Partial | `Locator.select_text` ([source](crates/ferrite-e2e/src/locator.rs#L1236)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.setChecked` | method | Partial | `Locator.set_checked` ([source](crates/ferrite-e2e/src/locator.rs#L1593)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.setInputFiles` | method | Partial | `Locator.set_input_files` ([source](crates/ferrite-e2e/src/locator.rs#L1337)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.tap` | method | Partial | `Locator.tap` ([source](crates/ferrite-e2e/src/locator.rs#L1174)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.textContent` | method | Partial | `Locator.text` ([source](crates/ferrite-e2e/src/locator.rs#L1421)) | Returns trimmed text as String; differs from nullable, untrimmed textContent. |
| `Locator.toString` | method | Idiomatic | `Locator.selector` ([source](crates/ferrite-e2e/src/locator.rs#L812)) | Selector string available; no Playwright locator expression representation. |
| `Locator.type` (deprecated) | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L1305)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.uncheck` | method | Partial | `Locator.uncheck` ([source](crates/ferrite-e2e/src/locator.rs#L1377)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.visible` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.waitFor` | method | Partial | `Locator.wait_for_state` ([source](crates/ferrite-e2e/src/locator.rs#L1043)) | Basic locator operation exposed; strictness is opt-in, actionability and options differ. |
| `Locator.waitForFunction` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## LocatorAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-locatorassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `LocatorAssertions.not` | property | Partial | `LocatorExpect.not` ([source](crates/ferrite-e2e/src/expect.rs#L389)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeAttached` | method | Partial | `LocatorExpect.attached` ([source](crates/ferrite-e2e/src/expect.rs#L740)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeChecked` | method | Partial | `LocatorExpect.checked` ([source](crates/ferrite-e2e/src/expect.rs#L570)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeDisabled` | method | Partial | `LocatorExpect.disabled` ([source](crates/ferrite-e2e/src/expect.rs#L633)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeEditable` | method | Partial | `LocatorExpect.editable` ([source](crates/ferrite-e2e/src/expect.rs#L658)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeEmpty` | method | Partial | `LocatorExpect.empty` ([source](crates/ferrite-e2e/src/expect.rs#L686)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeEnabled` | method | Partial | `LocatorExpect.enabled` ([source](crates/ferrite-e2e/src/expect.rs#L608)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeFocused` | method | Partial | `LocatorExpect.focused` ([source](crates/ferrite-e2e/src/expect.rs#L715)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeHidden` | method | Partial | `LocatorExpect.hidden` ([source](crates/ferrite-e2e/src/expect.rs#L423)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeInViewport` | method | Partial | `LocatorExpect.in_viewport` ([source](crates/ferrite-e2e/src/expect.rs#L905)) | Checks rectangle overlap only; no IntersectionObserver ratio option. |
| `LocatorAssertions.toBeVisible` | method | Partial | `LocatorExpect.visible` ([source](crates/ferrite-e2e/src/expect.rs#L395)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toContainClass` | method | Partial | `LocatorExpect.contains_class` ([source](crates/ferrite-e2e/src/expect.rs#L798)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toContainText` | method | Partial | `LocatorExpect.contains_text` ([source](crates/ferrite-e2e/src/expect.rs#L479)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveAccessibleDescription` | method | Partial | `LocatorExpect.accessible_description` ([source](crates/ferrite-e2e/src/expect.rs#L961)) | Approximate DOM computation; no full accessibility algorithm or regex matching. |
| `LocatorAssertions.toHaveAccessibleErrorMessage` | method | Missing | — | No dedicated matcher; use available state/getters and expect_poll for custom checks. |
| `LocatorAssertions.toHaveAccessibleName` | method | Partial | `LocatorExpect.accessible_name` ([source](crates/ferrite-e2e/src/expect.rs#L930)) | Approximate DOM computation; no full accessibility algorithm or regex matching. |
| `LocatorAssertions.toHaveAttribute` | method | Partial | `LocatorExpect.attribute` ([source](crates/ferrite-e2e/src/expect.rs#L765)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveClass` | method | Missing | — | No dedicated matcher; use available state/getters and expect_poll for custom checks. |
| `LocatorAssertions.toHaveCount` | method | Partial | `LocatorExpect.count` ([source](crates/ferrite-e2e/src/expect.rs#L541)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveCSS` | method | Partial | `LocatorExpect.css` ([source](crates/ferrite-e2e/src/expect.rs#L839)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveId` | method | Partial | `LocatorExpect.id` ([source](crates/ferrite-e2e/src/expect.rs#L834)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveJSProperty` | method | Partial | `LocatorExpect.js_property` ([source](crates/ferrite-e2e/src/expect.rs#L872)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveRole` | method | Missing | — | No dedicated matcher; use available state/getters and expect_poll for custom checks. |
| `LocatorAssertions.toHaveScreenshot` | method | Partial | `LocatorExpect.screenshot_with` ([source](crates/ferrite-e2e/src/expect.rs#L1001)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveText` | method | Partial | `LocatorExpect.text` ([source](crates/ferrite-e2e/src/expect.rs#L448)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveValue` | method | Partial | `LocatorExpect.value` ([source](crates/ferrite-e2e/src/expect.rs#L510)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveValues` | method | Missing | — | No dedicated matcher; use available state/getters and expect_poll for custom checks. |
| `LocatorAssertions.toMatchAriaSnapshot` | method | Missing | — | No dedicated matcher; use available state/getters and expect_poll for custom checks. |

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
| `Mouse.click` | method | Partial | `Page.mouse_click_with` ([source](crates/ferrite-e2e/src/page.rs#L2177)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.dblclick` | method | Partial | `Page.mouse_click` ([source](crates/ferrite-e2e/src/page.rs#L2170)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.down` | method | Partial | `Page.mouse_down` ([source](crates/ferrite-e2e/src/page.rs#L2221)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.move` | method | Partial | `Page.mouse_move` ([source](crates/ferrite-e2e/src/page.rs#L2189)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.up` | method | Partial | `Page.mouse_up` ([source](crates/ferrite-e2e/src/page.rs#L2228)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.wheel` | method | Partial | `Page.mouse_wheel` ([source](crates/ferrite-e2e/src/page.rs#L2242)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |

## Page

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-page.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Page.clock` | property | Partial | `Page.clock_install` ([source](crates/ferrite-e2e/src/page.rs#L2490)) | Fake clock is scoped to the current document and resets on navigation; semantic differences below. |
| `Page.close` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1491)) | PageEvent.Closed via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.console` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1491)) | PageEvent.Console via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.crash` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.dialog` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1491)) | PageEvent.Dialog via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.dialogClosed` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.DOMContentLoaded` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.download` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1491)) | PageEvent.Download via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.fileChooser` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.frameAttached` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.frameDetached` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.frameNavigated` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.load` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.pageError` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.popup` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1491)) | PageEvent.Popup via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.request` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1491)) | PageEvent.Request via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.requestFailed` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.requestFinished` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.response` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1491)) | PageEvent.Response via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.webSocket` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1491)) | PageEvent.WebSocket via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.worker` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.addInitScript` | method | Partial | `Page.add_init_script` ([source](crates/ferrite-e2e/src/page.rs#L3062)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.addScriptTag` | method | Partial | `Page.add_script_tag_url` ([source](crates/ferrite-e2e/src/page.rs#L3067)) | URL/content helpers return unit; no ElementHandle return, file/type options; other helper: Page.add_script_tag_content |
| `Page.addStyleTag` | method | Partial | `Page.add_style_tag_url` ([source](crates/ferrite-e2e/src/page.rs#L3093)) | URL/content helpers return unit; no ElementHandle return, file/type options; other helper: Page.add_style_tag_content |
| `Page.bringToFront` | method | Partial | `Page.bring_to_front` ([source](crates/ferrite-e2e/src/page.rs#L1877)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.cancelPickLocator` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.check` | method | Partial | `Locator.check` ([source](crates/ferrite-e2e/src/locator.rs#L1369)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.click` | method | Partial | `Locator.click` ([source](crates/ferrite-e2e/src/locator.rs#L1102)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.close` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.content` | method | Equivalent | `Page.content` ([source](crates/ferrite-e2e/src/page.rs#L1866)) | Basic document access. |
| `Page.context` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.coverage` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.dblclick` | method | Partial | `Locator.dblclick` ([source](crates/ferrite-e2e/src/locator.rs#L1138)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.dispatchEvent` | method | Partial | `Locator.dispatch_event` ([source](crates/ferrite-e2e/src/locator.rs#L1221)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.dragAndDrop` | method | Partial | `Locator.drag_to` ([source](crates/ferrite-e2e/src/locator.rs#L1188)) | Page.locator(source).drag_to(target, steps); fewer options and frame-coordinate restrictions. |
| `Page.emulateMedia` | method | Partial | `Page.emulate_media` ([source](crates/ferrite-e2e/src/page.rs#L2773)) | Chromium only; color scheme/reduced motion only, no full media/forcedColors/contrast surface. |
| `Page.evalOnSelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.evalOnSelectorAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.evaluate` | method | Partial | `Page.evaluate` ([source](crates/ferrite-e2e/src/page.rs#L1882)) | Expression string plus JSON deserialization; no separate argument/JSHandle/function bridge or arbitrary JS value serialization. |
| `Page.evaluateHandle` | method | Partial | `Page.evaluate_handle` ([source](crates/ferrite-e2e/src/page.rs#L1903)) | Remote JSHandle supported; no ElementHandle conversion or separate evaluation argument. |
| `Page.exposeBinding` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.exposeFunction` | method | Partial | `Page.expose_function` ([source](crates/ferrite-e2e/src/page.rs#L3160)) | JSON callback installed in the current document only; must be re-exposed after navigation, unlike Playwright; no binding/source metadata. |
| `Page.fill` | method | Partial | `Locator.fill` ([source](crates/ferrite-e2e/src/locator.rs#L1271)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.focus` | method | Partial | `Locator.focus` ([source](crates/ferrite-e2e/src/locator.rs#L1162)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.frame` | method | Partial | `Page.frame_by_name` ([source](crates/ferrite-e2e/src/page.rs#L3138)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.frameLocator` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.frames` | method | Partial | `Page.document_frames` ([source](crates/ferrite-e2e/src/page.rs#L3121)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.getAttribute` | method | Partial | `Locator.attribute` ([source](crates/ferrite-e2e/src/locator.rs#L1449)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.getByAltText` | method | Partial | `Page.get_by_alt` ([source](crates/ferrite-e2e/src/page.rs#L2018)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Page.getByLabel` | method | Partial | `Page.get_by_label` ([source](crates/ferrite-e2e/src/page.rs#L2006)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Page.getByPlaceholder` | method | Partial | `Page.get_by_placeholder` ([source](crates/ferrite-e2e/src/page.rs#L2012)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Page.getByRole` | method | Partial | `Page.get_by_role_with` ([source](crates/ferrite-e2e/src/page.rs#L1994)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Page.getByTestId` | method | Partial | `Page.get_by_test_id` ([source](crates/ferrite-e2e/src/page.rs#L1976)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Page.getByText` | method | Partial | `Page.get_by_text` ([source](crates/ferrite-e2e/src/page.rs#L1982)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Page.getByTitle` | method | Partial | `Page.get_by_title` ([source](crates/ferrite-e2e/src/page.rs#L2024)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Page.goBack` | method | Partial | `Page.go_back` ([source](crates/ferrite-e2e/src/page.rs#L1846)) | Returns unit, not a navigation Response; options and relative-URL resolution differ. |
| `Page.goForward` | method | Partial | `Page.go_forward` ([source](crates/ferrite-e2e/src/page.rs#L1851)) | Returns unit, not a navigation Response; options and relative-URL resolution differ. |
| `Page.requestGC` | method | Partial | `Page.request_gc` ([source](crates/ferrite-e2e/src/page.rs#L2165)) | Chromium only; Firefox returns an unsupported error. |
| `Page.goto` | method | Partial | `Page.goto_with_options` ([source](crates/ferrite-e2e/src/page.rs#L1740)) | Returns unit, not a navigation Response; options and relative-URL resolution differ. |
| `Page.hideHighlight` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.hover` | method | Partial | `Locator.hover` ([source](crates/ferrite-e2e/src/locator.rs#L1149)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.innerHTML` | method | Partial | `Locator.inner_html` ([source](crates/ferrite-e2e/src/locator.rs#L1481)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.innerText` | method | Partial | `Locator.text` ([source](crates/ferrite-e2e/src/locator.rs#L1421)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.inputValue` | method | Partial | `Locator.input_value` ([source](crates/ferrite-e2e/src/locator.rs#L1426)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isChecked` | method | Partial | `Locator.is_checked` ([source](crates/ferrite-e2e/src/locator.rs#L1551)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isClosed` | method | Partial | `Page.is_closed` ([source](crates/ferrite-e2e/src/page.rs#L1569)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.isDisabled` | method | Partial | `Locator.is_disabled` ([source](crates/ferrite-e2e/src/locator.rs#L1546)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isEditable` | method | Partial | `Locator.is_editable` ([source](crates/ferrite-e2e/src/locator.rs#L1556)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isEnabled` | method | Partial | `Locator.is_enabled` ([source](crates/ferrite-e2e/src/locator.rs#L1541)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isHidden` | method | Partial | `Locator.is_hidden` ([source](crates/ferrite-e2e/src/locator.rs#L1535)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isVisible` | method | Partial | `Locator.is_visible` ([source](crates/ferrite-e2e/src/locator.rs#L1530)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.keyboard` | property | Partial | `Page.press_key` ([source](crates/ferrite-e2e/src/page.rs#L2203)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.clearConsoleMessages` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.clearPageErrors` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.localStorage` | property | Partial | `Page.local_storage_get` ([source](crates/ferrite-e2e/src/page.rs#L2318)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.sessionStorage` | property | Partial | `Page.session_storage_get` ([source](crates/ferrite-e2e/src/page.rs#L2343)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.consoleMessages` | method | Partial | `Page.console_messages` ([source](crates/ferrite-e2e/src/page.rs#L1689)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.pageErrors` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.locator` | method | Partial | `Page.locator` ([source](crates/ferrite-e2e/src/page.rs#L1970)) | String selectors only; no regex/exact matching options, shadow-root traversal, or full accessible-name algorithm. Single-target operations are not strict by default. |
| `Page.mainFrame` | method | Partial | `Page.document_frames` ([source](crates/ferrite-e2e/src/page.rs#L3121)) | Enumerate frames and choose the root manually; no dedicated main_frame API. |
| `Page.mouse` | property | Partial | `Page.mouse_click` ([source](crates/ferrite-e2e/src/page.rs#L2170)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.opener` | method | Partial | `Page.opener` ([source](crates/ferrite-e2e/src/page.rs#L1602)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.pause` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.pdf` | method | Partial | `Page.pdf` ([source](crates/ferrite-e2e/src/page.rs#L2471)) | PDF export exists; no PDF options builder. Engine behavior differs. |
| `Page.pickLocator` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.press` | method | Partial | `Locator.press` ([source](crates/ferrite-e2e/src/locator.rs#L1293)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.querySelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.querySelectorAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.requests` | method | Partial | `Page.requests` ([source](crates/ferrite-e2e/src/page.rs#L3005)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.addLocatorHandler` | method | Partial | `Page.add_locator_handler_with` ([source](crates/ferrite-e2e/src/page.rs#L2049)) | Triggered by element count before DOM actions, not visibility before action/assertion retries; lacks noWaitAfter and dismissal waiting. |
| `Page.removeAllListeners` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.removeLocatorHandler` | method | Partial | `Page.remove_locator_handler` ([source](crates/ferrite-e2e/src/page.rs#L2071)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.reload` | method | Partial | `Page.reload` ([source](crates/ferrite-e2e/src/page.rs#L1841)) | Returns unit, not a navigation Response; options and relative-URL resolution differ. |
| `Page.request` | property | Partial | `ApiClient` ([source](crates/ferrite-e2e/src/api.rs#L64)) | Standalone client only; no page.request with shared browser cookies. |
| `Page.route` | method | Partial | `Page.route_with_handler` ([source](crates/ferrite-e2e/src/page.rs#L2798)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.routeFromHAR` | method | Partial | `Page.route_from_har` ([source](crates/ferrite-e2e/src/page.rs#L2901)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.routeWebSocket` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.screencast` | property | Partial | `Page.frames` ([source](crates/ferrite-e2e/src/page.rs#L3693)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.screenshot` | method | Partial | `Page.screenshot` ([source](crates/ferrite-e2e/src/page.rs#L2387)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.selectOption` | method | Partial | `Locator.select_options` ([source](crates/ferrite-e2e/src/locator.rs#L1394)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.setChecked` | method | Partial | `Locator.set_checked` ([source](crates/ferrite-e2e/src/locator.rs#L1593)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.setContent` | method | Partial | `Page.set_content` ([source](crates/ferrite-e2e/src/page.rs#L1872)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.setDefaultNavigationTimeout` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.setDefaultTimeout` | method | Partial | `Page.set_timeout` ([source](crates/ferrite-e2e/src/page.rs#L1651)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.setExtraHTTPHeaders` | method | Partial | `Page.set_extra_http_headers` ([source](crates/ferrite-e2e/src/page.rs#L2750)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.setInputFiles` | method | Partial | `Page.set_input_files` ([source](crates/ferrite-e2e/src/page.rs#L3036)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.setViewportSize` | method | Partial | `Page.set_viewport` ([source](crates/ferrite-e2e/src/page.rs#L2476)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.ariaSnapshot` | method | Partial | `Page.aria_snapshot` ([source](crates/ferrite-e2e/src/page.rs#L3402)) | Approximate flat DOM list, limited to 200 elements and 80-character names; no full ARIA tree, mode/depth/boxes options. |
| `Page.ariaSnapshotJSON` | method | Partial | `Page.aria_snapshot_json` ([source](crates/ferrite-e2e/src/page.rs#L3377)) | Approximate flat DOM list, limited to 200 elements and 80-character names; no full ARIA tree, mode/depth/boxes options. |
| `Page.tap` | method | Partial | `Locator.tap` ([source](crates/ferrite-e2e/src/locator.rs#L1174)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.textContent` | method | Partial | `Locator.text` ([source](crates/ferrite-e2e/src/locator.rs#L1421)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.title` | method | Equivalent | `Page.title` ([source](crates/ferrite-e2e/src/page.rs#L1856)) | Basic document access. |
| `Page.touchscreen` | property | Partial | `Page.touchscreen_tap` ([source](crates/ferrite-e2e/src/page.rs#L2263)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.type` (deprecated) | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L1305)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.uncheck` | method | Partial | `Locator.uncheck` ([source](crates/ferrite-e2e/src/locator.rs#L1377)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.unrouteAll` | method | Partial | `Page.unroute_all` ([source](crates/ferrite-e2e/src/page.rs#L2880)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.unroute` | method | Partial | `Page.unroute` ([source](crates/ferrite-e2e/src/page.rs#L2854)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.url` | method | Equivalent | `Page.url` ([source](crates/ferrite-e2e/src/page.rs#L1861)) | Basic document access. |
| `Page.video` | method | Partial | `Page.start_video` ([source](crates/ferrite-e2e/src/page.rs#L3624)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.viewportSize` | method | Partial | `Page.viewport_size` ([source](crates/ferrite-e2e/src/page.rs#L2154)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.waitForEvent` | method | Partial | `Page.wait_for_event` ([source](crates/ferrite-e2e/src/page.rs#L1500)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.waitForFunction` | method | Partial | `Page.wait_for_function` ([source](crates/ferrite-e2e/src/page.rs#L1918)) | Polling expression to completion returns unit; no JSHandle, argument or configurable polling mode. |
| `Page.waitForLoadState` | method | Partial | `Page.wait_for_load_state` ([source](crates/ferrite-e2e/src/page.rs#L1963)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.waitForNavigation` (deprecated) | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.waitForRequest` | method | Partial | `Page.wait_for_request` ([source](crates/ferrite-e2e/src/page.rs#L3497)) | URL substring match; returns RecordedRequest rather than rich Request/Response; no predicate overload. |
| `Page.waitForResponse` | method | Partial | `Page.wait_for_response` ([source](crates/ferrite-e2e/src/page.rs#L3528)) | URL substring match; returns RecordedRequest rather than rich Request/Response; no predicate overload. |
| `Page.waitForSelector` | method | Partial | `Page.wait_for_selector_with` ([source](crates/ferrite-e2e/src/page.rs#L1581)) | Waits for requested state and returns Locator, not ElementHandle. |
| `Page.waitForTimeout` | method | Partial | `Page.wait_for_timeout` ([source](crates/ferrite-e2e/src/page.rs#L1938)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.waitForURL` | method | Partial | `Page.wait_for_url` ([source](crates/ferrite-e2e/src/page.rs#L1944)) | Substring match only; no exact/glob/regex/predicate or waitUntil option. |
| `Page.workers` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## PageAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-pageassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `PageAssertions.not` | property | Partial | `PageExpect.not` ([source](crates/ferrite-e2e/src/expect.rs#L194)) | Retrying page assertion; string-only matching and different screenshot/ARIA algorithms. |
| `PageAssertions.toMatchAriaSnapshot` | method | Partial | `PageExpect.aria_snapshot` ([source](crates/ferrite-e2e/src/expect.rs#L336)) | Retrying page assertion; string-only matching and different screenshot/ARIA algorithms. |
| `PageAssertions.toHaveScreenshot` | method | Partial | `PageExpect.screenshot_with` ([source](crates/ferrite-e2e/src/expect.rs#L290)) | Retrying page assertion; string-only matching and different screenshot/ARIA algorithms. |
| `PageAssertions.toHaveTitle` | method | Partial | `PageExpect.title` ([source](crates/ferrite-e2e/src/expect.rs#L200)) | Retrying page assertion; string-only matching and different screenshot/ARIA algorithms. |
| `PageAssertions.toHaveURL` | method | Partial | `PageExpect.url_contains` ([source](crates/ferrite-e2e/src/expect.rs#L254)) | Substring-only check, not Playwright exact string/regex/predicate semantics. |

## Playwright

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-playwright.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Playwright.chromium` | property | Partial | `Browser.launch` ([source](crates/ferrite-e2e/src/browser.rs#L335)) | Select BrowserKind with LaunchOptions; no BrowserType object or bundled browser installer. |
| `Playwright.devices` | property | Partial | `DeviceDescriptor` ([source](crates/ferrite-e2e/src/page.rs#L528)) | Seven metrics presets; no full device catalog or device user-agent metadata. |
| `Playwright.errors` | property | Idiomatic | `E2eError` ([source](crates/ferrite-e2e/src/error.rs#L8)) | Rust error enum, with different variants and diagnostics. |
| `Playwright.firefox` | property | Partial | `Browser.launch` ([source](crates/ferrite-e2e/src/browser.rs#L335)) | Select BrowserKind with LaunchOptions; no BrowserType object or bundled browser installer. |
| `Playwright.request` | property | Partial | `ApiClient` ([source](crates/ferrite-e2e/src/api.rs#L64)) | Standalone client; no shared browser cookie jar or full APIRequest options. |
| `Playwright.selectors` | property | Partial | `set_test_id_attribute` ([source](crates/ferrite-e2e/src/locator.rs#L22)) | Only test-id configuration; no custom selector registration. |
| `Playwright.webkit` | property | Missing | — | No WebKit backend; BrowserKind contains Chromium and Firefox only. |

## PlaywrightAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-playwrightassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `PlaywrightAssertions.expectAPIResponse` | method | Idiomatic | `ApiResponse.ok` ([source](crates/ferrite-e2e/src/api.rs#L25)) | Use native Rust assertions over the standalone response. |
| `PlaywrightAssertions.expectGeneric` | method | Idiomatic | — | Use native Rust assertions and explicit pattern/container checks; no Playwright expect matcher library. |
| `PlaywrightAssertions.expectLocator` | method | Partial | `Locator.expect` ([source](crates/ferrite-e2e/src/expect.rs#L1072)) | Rust assertion builders; fewer matcher/expect configuration capabilities. |
| `PlaywrightAssertions.expectPage` | method | Partial | `Page.expect` ([source](crates/ferrite-e2e/src/expect.rs#L1054)) | Rust assertion builders; fewer matcher/expect configuration capabilities. |

## Reporter

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-reporter.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Reporter.onBegin` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Reporter.onEnd` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Reporter.onError` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Reporter.onExit` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Reporter.onStdErr` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Reporter.onStdOut` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Reporter.onStepBegin` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Reporter.onStepEnd` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Reporter.onTestBegin` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Reporter.onTestEnd` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Reporter.printsToStdio` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Reporter.preprocess` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |

## Request

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-request.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Request.allHeaders` | method | Partial | `RecordedRequest.headers` ([source](crates/ferrite-e2e/src/page.rs#L118)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |
| `Request.failure` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.frame` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.headers` | method | Partial | `RecordedRequest.headers` ([source](crates/ferrite-e2e/src/page.rs#L118)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |
| `Request.headersArray` | method | Partial | `RecordedRequest.headers` ([source](crates/ferrite-e2e/src/page.rs#L118)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |
| `Request.headerValue` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.isNavigationRequest` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.method` | method | Partial | `RecordedRequest.method` ([source](crates/ferrite-e2e/src/page.rs#L111)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |
| `Request.postData` | method | Partial | `RecordedRequest.post_data` ([source](crates/ferrite-e2e/src/page.rs#L121)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |
| `Request.postDataBuffer` | method | Partial | `RecordedRequest.post_data` ([source](crates/ferrite-e2e/src/page.rs#L121)) | Text capture only; no lossless binary request body API. |
| `Request.postDataJSON` | method | Partial | `RecordedRequest.post_data` ([source](crates/ferrite-e2e/src/page.rs#L121)) | Caller parses the captured request text with serde_json; body_json() is RESPONSE JSON, not request postDataJSON. |
| `Request.redirectedFrom` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.redirectedTo` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.resourceType` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.response` | method | Partial | `RecordedRequest.status` ([source](crates/ferrite-e2e/src/page.rs#L115)) | Captured response data are merged into the record; no separate live Response or request lifecycle object. |
| `Request.existingResponse` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.serviceWorker` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.sizes` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.timing` | method | Partial | `RecordedRequest.duration_ms` ([source](crates/ferrite-e2e/src/page.rs#L124)) | Total elapsed time only; no DNS/connect/TLS/response timing breakdown. |
| `Request.url` | method | Partial | `RecordedRequest.url` ([source](crates/ferrite-e2e/src/page.rs#L113)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |

## Response

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-response.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Response.allHeaders` | method | Partial | `RecordedRequest.response_headers` ([source](crates/ferrite-e2e/src/page.rs#L133)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.body` | method | Partial | `RecordedRequest.body` ([source](crates/ferrite-e2e/src/page.rs#L145)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.finished` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.frame` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.fromServiceWorker` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.headers` | method | Partial | `RecordedRequest.response_headers` ([source](crates/ferrite-e2e/src/page.rs#L133)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.headersArray` | method | Partial | `RecordedRequest.response_headers` ([source](crates/ferrite-e2e/src/page.rs#L133)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.headerValue` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.headerValues` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.httpVersion` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.json` | method | Partial | `RecordedRequest.body_json` ([source](crates/ferrite-e2e/src/page.rs#L162)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.ok` | method | Idiomatic | `RecordedRequest.status` ([source](crates/ferrite-e2e/src/page.rs#L115)) | Check (200..300).contains(&record.status); no dedicated Response.ok() method. |
| `Response.request` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.securityDetails` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.serverAddr` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.status` | method | Partial | `RecordedRequest.status` ([source](crates/ferrite-e2e/src/page.rs#L115)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.statusText` | method | Partial | `RecordedRequest.status_text` ([source](crates/ferrite-e2e/src/page.rs#L127)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.text` | method | Partial | `RecordedRequest.body_text` ([source](crates/ferrite-e2e/src/page.rs#L154)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.url` | method | Partial | `RecordedRequest.url` ([source](crates/ferrite-e2e/src/page.rs#L113)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |

## Route

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-route.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Route.abort` | method | Partial | `RouteAction.abort_with` ([source](crates/ferrite-e2e/src/page.rs#L951)) | Rule/action model, with glob patterns and fewer options; response modification and URL override are Chromium-only. |
| `Route.continue` | method | Partial | `RouteRule.continue_with` ([source](crates/ferrite-e2e/src/page.rs#L1101)) | Rule/action model, with glob patterns and fewer options; response modification and URL override are Chromium-only. |
| `Route.fallback` | method | Partial | `RouteAction.fallback` ([source](crates/ferrite-e2e/src/page.rs#L988)) | Rule/action model, with glob patterns and fewer options; response modification and URL override are Chromium-only. |
| `Route.fetch` | method | Partial | `RouteInfo.fetch` ([source](crates/ferrite-e2e/src/page.rs#L851)) | Standalone reqwest client, not a shared browser cookie context; no full redirect/retry/options surface. |
| `Route.fulfill` | method | Partial | `RouteAction.fulfill_full` ([source](crates/ferrite-e2e/src/page.rs#L967)) | Rule/action model, with glob patterns and fewer options; response modification and URL override are Chromium-only. |
| `Route.request` | method | Partial | `RouteInfo` ([source](crates/ferrite-e2e/src/page.rs#L837)) | Rule/action model, with glob patterns and fewer options; response modification and URL override are Chromium-only. |

## Screencast

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-screencast.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Screencast.start` | method | Partial | `Page.start_video` ([source](crates/ferrite-e2e/src/page.rs#L3624)) | Video recording/live frames available; fewer formats/options, no screencast overlay/action system. |
| `Screencast.stop` | method | Partial | `Page.stop_video` ([source](crates/ferrite-e2e/src/page.rs#L3661)) | Explicit output path; Chromium assembles frames with ffmpeg, Firefox records natively. |
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
| `Selectors.setTestIdAttribute` | method | Equivalent | `set_test_id_attribute` ([source](crates/ferrite-e2e/src/locator.rs#L22)) | Process-global attribute override. |

## SnapshotAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-snapshotassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `SnapshotAssertions.toMatchSnapshot` | method | Partial | `assert_snapshot_text` ([source](crates/ferrite-e2e/src/snapshot.rs#L243)) | Text and PNG helper functions; no Playwright snapshot-path template/source update/changed mode or arbitrary binary snapshots. |

## Suite

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-suite.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Suite.allTests` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Suite.entries` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Suite.location` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Suite.parent` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Suite.project` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Suite.suites` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Suite.tests` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Suite.title` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Suite.titlePath` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `Suite.type` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |

## Test

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-test.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Test.(call)` | method | Partial | `test` ([source](crates/ferrite-e2e/src/runner.rs#L137)) | Rust test closure; dynamic details/locks/options differ. |
| `Test.afterAll` | method | Partial | `Runner.after_all` ([source](crates/ferrite-e2e/src/runner.rs#L760)) | Global runner hooks; not scoped by describe suite or isolated worker lifecycle. |
| `Test.afterEach` | method | Partial | `Runner.after_each` ([source](crates/ferrite-e2e/src/runner.rs#L732)) | Global runner hooks; not scoped by describe suite or isolated worker lifecycle. |
| `Test.beforeAll` | method | Partial | `Runner.before_all` ([source](crates/ferrite-e2e/src/runner.rs#L747)) | Global runner hooks; not scoped by describe suite or isolated worker lifecycle. |
| `Test.beforeEach` | method | Partial | `Runner.before_each` ([source](crates/ferrite-e2e/src/runner.rs#L720)) | Global runner hooks; not scoped by describe suite or isolated worker lifecycle. |
| `Test.describe` | method | Partial | `describe` ([source](crates/ferrite-e2e/src/runner.rs#L174)) | Prefixes names only; no nested suite execution/hook scope or scheduling model. |
| `Test.describe.configure` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.fixme` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.only` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.parallel` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.parallel.only` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.serial` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.serial.only` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.skip` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.expect` | property | Partial | `Locator.expect` ([source](crates/ferrite-e2e/src/expect.rs#L1072)) | Rust builders, expect_poll and SoftAsserts; no generic expect.extend/configure/toPass library. |
| `Test.extend` | method | Partial | `Runner.fixture_with_teardown` ([source](crates/ferrite-e2e/src/runner.rs#L894)) | Typed per-attempt fixtures; no dependency graph, worker scope, auto fixtures, named overrides or lazy fixture resolution. |
| `Test.abort` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.fail` | method | Partial | `Test.fail` ([source](crates/ferrite-e2e/src/runner.rs#L101)) | Static test builder setting; no runtime conditional/context metadata overloads. |
| `Test.fail.only` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.fixme` | method | Partial | `Test.fixme` ([source](crates/ferrite-e2e/src/runner.rs#L94)) | Static test builder setting; no runtime conditional/context metadata overloads. |
| `Test.info` | method | Partial | `TestContext.info` ([source](crates/ferrite-e2e/src/runner.rs#L286)) | Provided through test_with_context; fewer live metadata fields and mutators. |
| `Test.only` | method | Partial | `Test.only` ([source](crates/ferrite-e2e/src/runner.rs#L87)) | Static test builder setting; no runtime conditional/context metadata overloads. |
| `Test.setTimeout` | method | Partial | `Test.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L129)) | Static test builder setting; no runtime conditional/context metadata overloads. |
| `Test.skip` | method | Partial | `Test.skip` ([source](crates/ferrite-e2e/src/runner.rs#L80)) | Static test builder setting; no runtime conditional/context metadata overloads. |
| `Test.slow` | method | Partial | `Test.slow` ([source](crates/ferrite-e2e/src/runner.rs#L115)) | Static test builder setting; no runtime conditional/context metadata overloads. |
| `Test.step` | method | Partial | `Page.step` ([source](crates/ferrite-e2e/src/page.rs#L2270)) | Named closure logged on Page; no structured step tree, boxing, timeout, subtitle/params or TestStepInfo. |
| `Test.step.skip` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.use` | method | Partial | `ContextOptions` ([source](crates/ferrite-e2e/src/context.rs#L31)) | Manual Browser.new_context options; no test.use fixture override/scoping system. |

## TestCase

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-testcase.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestCase.annotations` | property | Partial | `Test.annotations` ([source](crates/ferrite-e2e/src/runner.rs#L55)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.expectedStatus` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestCase.id` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestCase.location` | property | Partial | `Test.file` ([source](crates/ferrite-e2e/src/runner.rs#L57)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.ok` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestCase.outcome` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestCase.parent` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestCase.repeatEachIndex` | property | Partial | `TestResult.repeat_each_index` ([source](crates/ferrite-e2e/src/report.rs#L59)) | Result metadata only; no TestCase/Suite reporter tree. |
| `TestCase.results` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestCase.retries` | property | Partial | `Test.retries` ([source](crates/ferrite-e2e/src/runner.rs#L122)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.tags` | property | Partial | `Test.tags` ([source](crates/ferrite-e2e/src/runner.rs#L53)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.timeout` | property | Partial | `Test.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L129)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.title` | property | Partial | `Test.name` ([source](crates/ferrite-e2e/src/runner.rs#L47)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.titlePath` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestCase.type` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |

## TestConfig

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-testconfig.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestConfig.build` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.captureGitInfo` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.expect` | property | Partial | `E2eConfig.expect_timeout_ms` ([source](crates/ferrite-config/src/lib.rs#L418)) | Config field exists but is not consumed by ferrite-e2e; assertion default stays 5000 ms. Set assertion timeout explicitly. |
| `TestConfig.failOnFlakyTests` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.forbidOnly` | property | Partial | `Runner.forbid_only` ([source](crates/ferrite-e2e/src/runner.rs#L858)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.fullyParallel` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.globalSetup` | property | Partial | `Runner.global_setup` ([source](crates/ferrite-e2e/src/runner.rs#L771)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.globalTeardown` | property | Partial | `Runner.global_teardown` ([source](crates/ferrite-e2e/src/runner.rs#L782)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.globalTimeout` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.grep` | property | Partial | `Runner.grep` ([source](crates/ferrite-e2e/src/runner.rs#L704)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.grepInvert` | property | Partial | `Runner.grep_invert` ([source](crates/ferrite-e2e/src/runner.rs#L713)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.ignoreSnapshots` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.maxFailures` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.metadata` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.name` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.outputDir` | property | Partial | `E2eConfig.output_dir` ([source](crates/ferrite-config/src/lib.rs#L426)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.snapshotDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.snapshotPathTemplate` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.preserveOutput` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.projects` | property | Partial | `Runner.project` ([source](crates/ferrite-e2e/src/runner.rs#L842)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.quiet` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.repeatEach` | property | Partial | `Runner.repeat_each` ([source](crates/ferrite-e2e/src/runner.rs#L849)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.reporter` | property | Partial | `E2eConfig.reporter` ([source](crates/ferrite-config/src/lib.rs#L424)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.reportSlowTests` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.respectGitIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.retries` | property | Partial | `Runner.retries` ([source](crates/ferrite-e2e/src/runner.rs#L679)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.retryStrategy` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.shard` | property | Partial | `Runner.shard` ([source](crates/ferrite-e2e/src/runner.rs#L800)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.tag` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.testDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.testIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.testMatch` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.timeout` | property | Partial | `Runner.test_timeout` ([source](crates/ferrite-e2e/src/runner.rs#L686)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.tsconfig` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.updateSnapshots` | property | Partial | `E2eConfig.update_snapshots` ([source](crates/ferrite-config/src/lib.rs#L436)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.updateSourceMethod` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.use` | property | Partial | `ContextOptions` ([source](crates/ferrite-e2e/src/context.rs#L31)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.webServer` | property | Partial | `E2eConfig.web_server` ([source](crates/ferrite-config/src/lib.rs#L440)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.workers` | property | Partial | `Runner.workers` ([source](crates/ferrite-e2e/src/runner.rs#L672)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |

## TestError

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-testerror.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestError.cause` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestError.message` | property | Partial | `TestResult.error` ([source](crates/ferrite-e2e/src/report.rs#L44)) | String-formatted Rust error; no structured cause/stack/error-context object. |
| `TestError.stack` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestError.value` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestError.location` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestError.snippet` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## TestInfo

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-testinfo.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestInfo.annotations` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.attachments` | property | Partial | `TestInfo.attachments` ([source](crates/ferrite-e2e/src/runner.rs#L242)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.attach` | method | Partial | `TestInfo.attach` ([source](crates/ferrite-e2e/src/runner.rs#L217)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.column` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.config` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.duration` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.error` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.errors` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.expectedStatus` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.fail` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.file` | property | Partial | `TestInfo.file` ([source](crates/ferrite-e2e/src/runner.rs#L190)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.fixme` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.fn` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.tags` | property | Partial | `TestInfo.tags` ([source](crates/ferrite-e2e/src/runner.rs#L194)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.testId` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.line` | property | Partial | `TestInfo.line` ([source](crates/ferrite-e2e/src/runner.rs#L192)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.snapshotDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.outputDir` | property | Partial | `TestInfo.output_dir` ([source](crates/ferrite-e2e/src/runner.rs#L204)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.outputPath` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.parallelIndex` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.project` | property | Partial | `TestInfo.project` ([source](crates/ferrite-e2e/src/runner.rs#L206)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.repeatEachIndex` | property | Partial | `TestInfo.repeat_each_index` ([source](crates/ferrite-e2e/src/runner.rs#L200)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.retry` | property | Partial | `TestInfo.retry` ([source](crates/ferrite-e2e/src/runner.rs#L196)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.setTimeout` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.skip` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.slow` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.snapshotPath` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.snapshotSuffix` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.status` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.timeout` | property | Partial | `TestInfo.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L202)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.title` | property | Partial | `TestInfo.title` ([source](crates/ferrite-e2e/src/runner.rs#L188)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.titlePath` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.workerIndex` | property | Partial | `TestInfo.worker_index` ([source](crates/ferrite-e2e/src/runner.rs#L198)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |

## TestInfoError

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-testinfoerror.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestInfoError.cause` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfoError.message` | property | Partial | `TestResult.error` ([source](crates/ferrite-e2e/src/report.rs#L44)) | String-formatted Rust error; no structured cause/stack/error-context object. |
| `TestInfoError.stack` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfoError.errorContext` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfoError.value` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## TestOptions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-testoptions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestOptions.acceptDownloads` | property | Partial | `ContextOptions.accept_downloads` ([source](crates/ferrite-e2e/src/context.rs#L206)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.baseURL` | property | Partial | `E2eConfig.base_url` ([source](crates/ferrite-config/src/lib.rs#L414)) | Runner/launch config field; no automatic named-fixture/project options equivalence. |
| `TestOptions.browserName` | property | Partial | `E2eConfig.browser` ([source](crates/ferrite-config/src/lib.rs#L397)) | Runner/launch config field; no automatic named-fixture/project options equivalence. |
| `TestOptions.actionTimeout` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.bypassCSP` | property | Partial | `ContextOptions.bypass_csp` ([source](crates/ferrite-e2e/src/context.rs#L199)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.channel` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.clientCertificates` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.colorScheme` | property | Partial | `Page.emulate_media` ([source](crates/ferrite-e2e/src/page.rs#L2773)) | Chromium page-level manual emulation; no context/test option binding. |
| `TestOptions.connectOptions` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.contextOptions` | property | Partial | `ContextOptions` ([source](crates/ferrite-e2e/src/context.rs#L31)) | Manual context constructor options; runner does not create a fresh context per test. |
| `TestOptions.contrast` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.deviceScaleFactor` | property | Partial | `ContextOptions.device_scale_factor` ([source](crates/ferrite-e2e/src/context.rs#L171)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.extraHTTPHeaders` | property | Partial | `ContextOptions.extra_http_headers` ([source](crates/ferrite-e2e/src/context.rs#L164)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.forcedColors` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.geolocation` | property | Partial | `ContextOptions.geolocation` ([source](crates/ferrite-e2e/src/context.rs#L136)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.hasTouch` | property | Partial | `ContextOptions.has_touch` ([source](crates/ferrite-e2e/src/context.rs#L185)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.headless` | property | Partial | `E2eConfig.headless` ([source](crates/ferrite-config/src/lib.rs#L399)) | Runner/launch config field; no automatic named-fixture/project options equivalence. |
| `TestOptions.httpCredentials` | property | Partial | `ContextOptions.http_credentials` ([source](crates/ferrite-e2e/src/context.rs#L157)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.ignoreHTTPSErrors` | property | Partial | `ContextOptions.ignore_https_errors` ([source](crates/ferrite-e2e/src/context.rs#L41)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.isMobile` | property | Partial | `ContextOptions.is_mobile` ([source](crates/ferrite-e2e/src/context.rs#L178)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.javaScriptEnabled` | property | Partial | `ContextOptions.java_script_enabled` ([source](crates/ferrite-e2e/src/context.rs#L192)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.launchOptions` | property | Partial | `LaunchOptions` ([source](crates/ferrite-e2e/src/browser.rs#L60)) | Launch struct subset; no full Playwright options, persistent profile or channels. |
| `TestOptions.locale` | property | Partial | `ContextOptions.locale` ([source](crates/ferrite-e2e/src/context.rs#L122)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.navigationTimeout` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.offline` | property | Partial | `ContextOptions.offline` ([source](crates/ferrite-e2e/src/context.rs#L150)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.permissions` | property | Partial | `ContextOptions.permissions` ([source](crates/ferrite-e2e/src/context.rs#L143)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.proxy` | property | Partial | `ContextOptions.proxy_server` ([source](crates/ferrite-e2e/src/context.rs#L39)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.reducedMotion` | property | Partial | `Page.emulate_media` ([source](crates/ferrite-e2e/src/page.rs#L2773)) | Chromium page-level manual emulation; no context/test option binding. |
| `TestOptions.reuseContext` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.screenshot` | property | Partial | `E2eConfig.screenshot` ([source](crates/ferrite-config/src/lib.rs#L428)) | Runner/launch config field; no automatic named-fixture/project options equivalence. |
| `TestOptions.storageState` | property | Partial | `ContextOptions.storage_state` ([source](crates/ferrite-e2e/src/context.rs#L220)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.testIdAttribute` | property | Partial | `set_test_id_attribute` ([source](crates/ferrite-e2e/src/locator.rs#L22)) | Process-global setter, not project/test-specific fixture option. |
| `TestOptions.timezoneId` | property | Partial | `ContextOptions.timezone_id` ([source](crates/ferrite-e2e/src/context.rs#L129)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.trace` | property | Partial | `BrowserContext.start_tracing` ([source](crates/ferrite-e2e/src/context.rs#L666)) | Manual custom JSON traces plus runner JSON; no trace mode policy or Trace Viewer compatibility. |
| `TestOptions.userAgent` | property | Partial | `ContextOptions.user_agent` ([source](crates/ferrite-e2e/src/context.rs#L115)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.video` | property | Partial | `E2eConfig.video` ([source](crates/ferrite-config/src/lib.rs#L430)) | Runner/launch config field; no automatic named-fixture/project options equivalence. |
| `TestOptions.viewport` | property | Partial | `ContextOptions.viewport` ([source](crates/ferrite-e2e/src/context.rs#L108)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |
| `TestOptions.serviceWorkers` | property | Partial | `ContextOptions.service_workers` ([source](crates/ferrite-e2e/src/context.rs#L227)) | Available during manual context creation; no per-project/test.use override; Firefox restrictions and narrower options apply. |

## TestProject

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-testproject.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestProject.dependencies` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.expect` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.fullyParallel` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.grep` | property | Partial | `Project.grep` ([source](crates/ferrite-e2e/src/runner.rs#L347)) | Project name/filter/retry/timeout override only; all projects run against the same Browser supplied to Runner.run. |
| `TestProject.grepInvert` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.ignoreSnapshots` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.metadata` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.name` | property | Partial | `Project.name` ([source](crates/ferrite-e2e/src/runner.rs#L326)) | Project name/filter/retry/timeout override only; all projects run against the same Browser supplied to Runner.run. |
| `TestProject.snapshotDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.snapshotPathTemplate` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.outputDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.repeatEach` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.respectGitIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.retries` | property | Partial | `Project.retries` ([source](crates/ferrite-e2e/src/runner.rs#L354)) | Project name/filter/retry/timeout override only; all projects run against the same Browser supplied to Runner.run. |
| `TestProject.teardown` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.testDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.testIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.testMatch` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.timeout` | property | Partial | `Project.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L361)) | Project name/filter/retry/timeout override only; all projects run against the same Browser supplied to Runner.run. |
| `TestProject.use` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.workers` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## TestResult

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-testresult.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestResult.attachments` | property | Partial | `TestResult.attachments` ([source](crates/ferrite-e2e/src/report.rs#L65)) | Aggregate result fields; attempts aggregates retries, not one TestResult per attempt; errors are strings, no reporter callbacks. |
| `TestResult.annotations` | property | Partial | `TestResult.annotations` ([source](crates/ferrite-e2e/src/report.rs#L62)) | Aggregate result fields; attempts aggregates retries, not one TestResult per attempt; errors are strings, no reporter callbacks. |
| `TestResult.duration` | property | Partial | `TestResult.duration_ms` ([source](crates/ferrite-e2e/src/report.rs#L41)) | Aggregate result fields; attempts aggregates retries, not one TestResult per attempt; errors are strings, no reporter callbacks. |
| `TestResult.error` | property | Partial | `TestResult.error` ([source](crates/ferrite-e2e/src/report.rs#L44)) | Aggregate result fields; attempts aggregates retries, not one TestResult per attempt; errors are strings, no reporter callbacks. |
| `TestResult.errors` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestResult.retry` | property | Partial | `TestResult.attempts` ([source](crates/ferrite-e2e/src/report.rs#L39)) | Aggregate result fields; attempts aggregates retries, not one TestResult per attempt; errors are strings, no reporter callbacks. |
| `TestResult.startTime` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestResult.status` | property | Partial | `TestResult.status` ([source](crates/ferrite-e2e/src/report.rs#L37)) | Aggregate result fields; attempts aggregates retries, not one TestResult per attempt; errors are strings, no reporter callbacks. |
| `TestResult.stderr` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestResult.stdout` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestResult.steps` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestResult.workerIndex` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestResult.parallelIndex` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## TestRun

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-testrun.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestRun.exclude` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestRun.fail` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestRun.fixme` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestRun.skip` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestRun.skipSharding` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |

## TestStep

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-teststep.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestStep.category` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestStep.duration` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestStep.location` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestStep.error` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestStep.parent` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestStep.params` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestStep.startTime` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestStep.steps` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestStep.annotations` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestStep.attachments` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestStep.title` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestStep.subtitle` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestStep.titlePath` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |

## TestStepInfo

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-teststepinfo.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestStepInfo.annotations` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestStepInfo.attach` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestStepInfo.skip` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestStepInfo.titlePath` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |

## TimeoutError

No own JS-applicable member headings. The class/error type is not exposed as a Ferrite class; use `E2eError`.

## Touchscreen

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-touchscreen.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Touchscreen.tap` | method | Partial | `Page.touchscreen_tap` ([source](crates/ferrite-e2e/src/page.rs#L2263)) | Coordinate tap available; no separate Touchscreen object. |

## Tracing

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-tracing.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Tracing.start` | method | Partial | `BrowserContext.start_tracing` ([source](crates/ferrite-e2e/src/context.rs#L666)) | Custom JSON actions/logs/requests; optional screenshots at Page.step only, no DOM/ARIA/source snapshots. |
| `Tracing.startChunk` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Tracing.startHar` | method | Partial | `Page.start_request_capture` ([source](crates/ferrite-e2e/src/page.rs#L2984)) | Page capture + save_har_with; no Tracing.startHar API or full browser/API-request tracing. |
| `Tracing.group` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Tracing.groupEnd` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Tracing.stop` | method | Partial | `BrowserContext.stop_tracing` ([source](crates/ferrite-e2e/src/context.rs#L679)) | Writes JSON, not a Trace Viewer-compatible zip archive. |
| `Tracing.stopChunk` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Tracing.stopHar` | method | Partial | `Page.save_har_with` ([source](crates/ferrite-e2e/src/page.rs#L3024)) | HAR exporter; no Tracing.stopHar interface, update/rewrite mode or full timing/body coverage. |

## Video

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-video.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Video.delete` | method | Idiomatic | — | Caller deletes artifact through Rust filesystem operations; no Video object. |
| `Video.path` | method | Partial | `TestResult.video` ([source](crates/ferrite-e2e/src/report.rs#L53)) | Runner records an optional artifact path, not Page.video()/Video object. |
| `Video.saveAs` | method | Partial | `Page.stop_video` ([source](crates/ferrite-e2e/src/page.rs#L3661)) | Stop recording to a path; no independently awaitable Video handle. |

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
| `WebSocket.close` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1491)) | PageEvent::WebSocket direction Closed; Chromium-only observation without a WebSocket object. |
| `WebSocket.frameReceived` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1491)) | PageEvent::WebSocket direction Received; Chromium-only observation without a WebSocket object. |
| `WebSocket.frameSent` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1491)) | PageEvent::WebSocket direction Sent; Chromium-only observation without a WebSocket object. |
| `WebSocket.socketError` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `WebSocket.isClosed` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `WebSocket.url` | method | Partial | `WebSocketEvent.url` ([source](crates/ferrite-e2e/src/page.rs#L217)) | Captured socket URL, Chromium only. |
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
| `WebStorage.getItem` | method | Partial | `Page.local_storage_get` ([source](crates/ferrite-e2e/src/page.rs#L2318)) | Page helper methods for local and session storage; no WebStorage object. |
| `WebStorage.setItem` | method | Partial | `Page.local_storage_set` ([source](crates/ferrite-e2e/src/page.rs#L2323)) | Page helper methods for local and session storage; no WebStorage object. |
| `WebStorage.removeItem` | method | Partial | `Page.local_storage_remove` ([source](crates/ferrite-e2e/src/page.rs#L2328)) | Page helper methods for local and session storage; no WebStorage object. |
| `WebStorage.clear` | method | Partial | `Page.local_storage_clear` ([source](crates/ferrite-e2e/src/page.rs#L2337)) | Page helper methods for local and session storage; no WebStorage object. |

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
| `WorkerInfo.config` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `WorkerInfo.parallelIndex` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `WorkerInfo.project` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `WorkerInfo.workerIndex` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
