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

Inventory: **73 classes, 1018 distinct members**. Equivalent: 15; Partial: 511; Idiomatic: 42; Missing: 450. These counts are inventory labels, not a percentage of behavioral compatibility.

## APIRequest

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-apirequest.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `APIRequest.newContext` | method | Partial | `ApiClient.with_options` ([source](crates/ferrite-e2e/src/api.rs#L160)) | Cookie jar, base URL, headers, TLS, proxy, timeout, redirects and Basic auth; optional browser cookie link, no full Playwright options. |

## APIRequestContext

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-apirequestcontext.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `APIRequestContext.delete` | method | Partial | `ApiClient.delete` ([source](crates/ferrite-e2e/src/api.rs#L295)) | HTTP verb available; use fetch_with for query/body/header/timeout/retry/status options. Client has cookies/auth/proxy/redirect settings; no API storage-state export/disposal lifecycle. |
| `APIRequestContext.dispose` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIRequestContext.fetch` | method | Partial | `ApiClient.fetch_with` ([source](crates/ferrite-e2e/src/api.rs#L203)) | Generic HTTP method; headers/query/JSON/form/multipart/raw payloads, timeout, status checks and connection retries. Narrower redirect/storage/disposal options. |
| `APIRequestContext.get` | method | Partial | `ApiClient.get` ([source](crates/ferrite-e2e/src/api.rs#L288)) | HTTP verb available; use fetch_with for query/body/header/timeout/retry/status options. Client has cookies/auth/proxy/redirect settings; no API storage-state export/disposal lifecycle. |
| `APIRequestContext.head` | method | Partial | `ApiClient.head` ([source](crates/ferrite-e2e/src/api.rs#L323)) | HTTP verb available; use fetch_with for query/body/header/timeout/retry/status options. Client has cookies/auth/proxy/redirect settings; no API storage-state export/disposal lifecycle. |
| `APIRequestContext.patch` | method | Partial | `ApiClient.patch_json` ([source](crates/ferrite-e2e/src/api.rs#L316)) | HTTP verb available; use fetch_with for query/body/header/timeout/retry/status options. Client has cookies/auth/proxy/redirect settings; no API storage-state export/disposal lifecycle. |
| `APIRequestContext.post` | method | Partial | `ApiClient.post_json` ([source](crates/ferrite-e2e/src/api.rs#L302)) | HTTP verb available; use fetch_with for query/body/header/timeout/retry/status options. Client has cookies/auth/proxy/redirect settings; no API storage-state export/disposal lifecycle. |
| `APIRequestContext.put` | method | Partial | `ApiClient.put_json` ([source](crates/ferrite-e2e/src/api.rs#L309)) | HTTP verb available; use fetch_with for query/body/header/timeout/retry/status options. Client has cookies/auth/proxy/redirect settings; no API storage-state export/disposal lifecycle. |
| `APIRequestContext.storageState` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIRequestContext.tracing` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## APIResponse

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-apiresponse.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `APIResponse.body` | method | Equivalent | `ApiResponse.bytes` ([source](crates/ferrite-e2e/src/api.rs#L67)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.dispose` | method | Partial | `ApiResponse.dispose` ([source](crates/ferrite-e2e/src/api.rs#L33)) | Final response URL/status text and explicit body-buffer disposal; clones own their buffers. |
| `APIResponse.headers` | method | Partial | `ApiResponse.headers` ([source](crates/ferrite-e2e/src/api.rs#L52)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.headersArray` | method | Partial | `ApiResponse.headers` ([source](crates/ferrite-e2e/src/api.rs#L52)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.json` | method | Equivalent | `ApiResponse.json` ([source](crates/ferrite-e2e/src/api.rs#L78)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.ok` | method | Equivalent | `ApiResponse.ok` ([source](crates/ferrite-e2e/src/api.rs#L46)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.securityDetails` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIResponse.serverAddr` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIResponse.status` | method | Equivalent | `ApiResponse.status` ([source](crates/ferrite-e2e/src/api.rs#L40)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.statusText` | method | Partial | `ApiResponse.status_text` ([source](crates/ferrite-e2e/src/api.rs#L28)) | Final response URL/status text and explicit body-buffer disposal; clones own their buffers. |
| `APIResponse.text` | method | Partial | `ApiResponse.text` ([source](crates/ferrite-e2e/src/api.rs#L73)) | Buffered response equivalent for basic HTTP values; headers are pairs and text uses lossy UTF-8. |
| `APIResponse.timing` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `APIResponse.url` | method | Partial | `ApiResponse.url` ([source](crates/ferrite-e2e/src/api.rs#L23)) | Final response URL/status text and explicit body-buffer disposal; clones own their buffers. |

## APIResponseAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-apiresponseassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `APIResponseAssertions.not` | property | Idiomatic | `ApiResponse.ok` ([source](crates/ferrite-e2e/src/api.rs#L46)) | Use assert!(response.ok()) or its negation; no dedicated retrying assertion object. |
| `APIResponseAssertions.toBeOK` | method | Idiomatic | `ApiResponse.ok` ([source](crates/ferrite-e2e/src/api.rs#L46)) | Use assert!(response.ok()) or its negation; no dedicated retrying assertion object. |

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
| `Browser.browserType` | method | Partial | `Browser.kind` ([source](crates/ferrite-e2e/src/browser.rs#L699)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.close` | method | Partial | `Browser.close` ([source](crates/ferrite-e2e/src/browser.rs#L1053)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.contexts` | method | Partial | `Browser.contexts` ([source](crates/ferrite-e2e/src/browser.rs#L1019)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.isConnected` | method | Partial | `Browser.is_connected` ([source](crates/ferrite-e2e/src/browser.rs#L916)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.newBrowserCDPSession` | method | Partial | `Browser.cdp` ([source](crates/ferrite-e2e/src/browser.rs#L893)) | Raw shared browser CDP connection; no independently detachable CDPSession. |
| `Browser.newContext` | method | Partial | `Browser.new_context` ([source](crates/ferrite-e2e/src/browser.rs#L924)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |
| `Browser.newPage` | method | Partial | `Browser.new_page` ([source](crates/ferrite-e2e/src/browser.rs#L1037)) | Fresh owning context; closing the page disposes it, including its popups. |
| `Browser.bind` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.removeAllListeners` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.startTracing` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.stopTracing` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.unbind` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Browser.version` | method | Partial | `Browser.version` ([source](crates/ferrite-e2e/src/browser.rs#L910)) | Similar lifecycle/introspection; option sets and connection/context ownership differ. |

## BrowserContext

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-browsercontext.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `BrowserContext.backgroundPage` (deprecated) | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `BrowserContext.clock` | property | Partial | `Page.clock_install` ([source](crates/ferrite-e2e/src/page.rs#L2620)) | Page-document-local clock; no context-wide clock object. |
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
| `BrowserContext.addCookies` | method | Partial | `BrowserContext.add_cookies` ([source](crates/ferrite-e2e/src/context.rs#L615)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.addInitScript` | method | Partial | `BrowserContext.add_init_script` ([source](crates/ferrite-e2e/src/context.rs#L1101)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.backgroundPages` (deprecated) | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.browser` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.clearCookies` | method | Partial | `BrowserContext.clear_cookies` ([source](crates/ferrite-e2e/src/context.rs#L607)) | Clears all cookies; no name/domain/path filters. |
| `BrowserContext.clearPermissions` | method | Partial | `BrowserContext.clear_permissions` ([source](crates/ferrite-e2e/src/context.rs#L895)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.close` | method | Partial | `BrowserContext.close` ([source](crates/ferrite-e2e/src/context.rs#L1175)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.cookies` | method | Partial | `BrowserContext.cookies` ([source](crates/ferrite-e2e/src/context.rs#L599)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.exposeBinding` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.exposeFunction` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.grantPermissions` | method | Partial | `BrowserContext.grant_permissions` ([source](crates/ferrite-e2e/src/context.rs#L866)) | No origin argument; Chromium grants broadly, Firefox grants after navigation for the current origin. |
| `BrowserContext.isClosed` | method | Partial | `BrowserContext.is_closed` ([source](crates/ferrite-e2e/src/context.rs#L1170)) | Tracks explicit context disposal; no full remote-disconnection lifecycle semantics. |
| `BrowserContext.newCDPSession` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.newPage` | method | Partial | `BrowserContext.new_page` ([source](crates/ferrite-e2e/src/context.rs#L395)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.pages` | method | Partial | `BrowserContext.pages` ([source](crates/ferrite-e2e/src/context.rs#L552)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.removeAllListeners` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.request` | property | Partial | `BrowserContext.request` ([source](crates/ferrite-e2e/src/context.rs#L590)) | Context-linked HTTP client sharing cookies. Transport settings are configured separately through ApiClientOptions. |
| `BrowserContext.route` | method | Partial | `BrowserContext.route_with_handler` ([source](crates/ferrite-e2e/src/context.rs#L654)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.routeFromHAR` | method | Partial | `BrowserContext.route_from_har` ([source](crates/ferrite-e2e/src/context.rs#L821)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.routeWebSocket` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.serviceWorkers` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserContext.setDefaultNavigationTimeout` | method | Partial | `BrowserContext.set_default_navigation_timeout` ([source](crates/ferrite-e2e/src/context.rs#L568)) | Shared settings update existing and future pages; protocol calls still have backend timeouts. |
| `BrowserContext.setDefaultTimeout` | method | Partial | `BrowserContext.set_default_timeout` ([source](crates/ferrite-e2e/src/context.rs#L557)) | Shared settings update existing and future pages; protocol calls still have backend timeouts. |
| `BrowserContext.setExtraHTTPHeaders` | method | Partial | `BrowserContext.set_extra_http_headers` ([source](crates/ferrite-e2e/src/context.rs#L1011)) | Chromium only; HTTP credentials hold one username/password pair, no origin list. |
| `BrowserContext.setGeolocation` | method | Partial | `BrowserContext.set_geolocation` ([source](crates/ferrite-e2e/src/context.rs#L886)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.setHTTPCredentials` | method | Partial | `BrowserContext.set_http_credentials` ([source](crates/ferrite-e2e/src/context.rs#L985)) | Chromium only; HTTP credentials hold one username/password pair, no origin list. |
| `BrowserContext.setOffline` | method | Partial | `BrowserContext.set_offline` ([source](crates/ferrite-e2e/src/context.rs#L975)) | Chromium only; HTTP credentials hold one username/password pair, no origin list. |
| `BrowserContext.storageState` | method | Partial | `BrowserContext.storage_state` ([source](crates/ferrite-e2e/src/context.rs#L1143)) | Playwright cookies/origins JSON, localStorage from live pages across origins; closed-origin inventory and IndexedDB/OPFS are deferred. |
| `BrowserContext.setStorageState` | method | Partial | `BrowserContext.load_storage_state` ([source](crates/ferrite-e2e/src/context.rs#L1116)) | Playwright multi-origin state and legacy files; cookies restore before navigation and localStorage before app scripts. IndexedDB/OPFS not persisted. |
| `BrowserContext.tracing` | property | Partial | `BrowserContext.start_tracing` ([source](crates/ferrite-e2e/src/context.rs#L720)) | Custom JSON trace, no Trace Viewer-compatible archive, DOM snapshots or chunks. |
| `BrowserContext.unrouteAll` | method | Partial | `BrowserContext.unroute_all` ([source](crates/ferrite-e2e/src/context.rs#L800)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
| `BrowserContext.unroute` | method | Partial | `BrowserContext.unroute` ([source](crates/ferrite-e2e/src/context.rs#L774)) | Context API exists, with fewer options and engine restrictions; see the feature audit. |
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
| `BrowserType.connectOverCDP` | method | Partial | `Browser.connect_over_cdp` ([source](crates/ferrite-e2e/src/browser.rs#L654)) | Chromium HTTP or browser WebSocket endpoint; no Playwright remote protocol or headers/options surface. |
| `BrowserType.executablePath` | method | Partial | `find_chromium` ([source](crates/ferrite-e2e/src/browser.rs#L168)) | Stock-browser discovery/launch; no channels/installer or full Playwright connection options. |
| `BrowserType.launch` | method | Partial | `Browser.launch` ([source](crates/ferrite-e2e/src/browser.rs#L364)) | Stock-browser discovery/launch; no channels/installer or full Playwright connection options. |
| `BrowserType.launchPersistentContext` | method | Partial | `LaunchOptions.user_data_dir` ([source](crates/ferrite-e2e/src/browser.rs#L137)) | Reusable Chromium/Firefox profile; obtain browser.default_context(). Dedicated contexts remain isolated from persistent storage. |
| `BrowserType.launchServer` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `BrowserType.name` | method | Partial | `BrowserKind.name` ([source](crates/ferrite-e2e/src/browser.rs#L47)) | Stock-browser discovery/launch; no channels/installer or full Playwright connection options. |

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
| `Clock.fastForward` | method | Partial | `Page.clock_fast_forward` ([source](crates/ferrite-e2e/src/page.rs#L2668)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.install` | method | Partial | `Page.clock_install_at` ([source](crates/ferrite-e2e/src/page.rs#L2688)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.runFor` | method | Partial | `Page.clock_run_for` ([source](crates/ferrite-e2e/src/page.rs#L2673)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.pauseAt` | method | Partial | `Page.clock_pause_at` ([source](crates/ferrite-e2e/src/page.rs#L2683)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.resume` | method | Partial | `Page.clock_resume` ([source](crates/ferrite-e2e/src/page.rs#L2732)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.setFixedTime` | method | Partial | `Page.clock_set_fixed_time` ([source](crates/ferrite-e2e/src/page.rs#L2644)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |
| `Clock.setSystemTime` | method | Partial | `Page.clock_set_system_time` ([source](crates/ferrite-e2e/src/page.rs#L2678)) | Distinct timer/Date/jump/progression behavior; document clock persists via init scripts but resets on navigation, is not context-wide, and idle callbacks are approximated. |

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
| `Dialog.accept` | method | Partial | `DialogDecision.accept_with` ([source](crates/ferrite-e2e/src/page.rs#L973)) | Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page. |
| `Dialog.defaultValue` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Dialog.dismiss` | method | Partial | `DialogDecision.dismiss` ([source](crates/ferrite-e2e/src/page.rs#L978)) | Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page. |
| `Dialog.message` | method | Partial | `DialogInfo.message` ([source](crates/ferrite-e2e/src/page.rs#L214)) | Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page. |
| `Dialog.page` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Dialog.type` | method | Partial | `DialogInfo.dialog_type` ([source](crates/ferrite-e2e/src/page.rs#L212)) | Handler returns a decision; wait_for_dialog handles the dialog before returning metadata; no live Dialog object/defaultValue/page. |

## Disposable

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-disposable.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Disposable.dispose` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## Download

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-download.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Download.cancel` | method | Partial | `Page.cancel_downloads` ([source](crates/ferrite-e2e/src/page.rs#L3592)) | Cancels page-tracked downloads on Chromium; no per-Download.cancel method. |
| `Download.createReadStream` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Download.delete` | method | Partial | `Download.delete` ([source](crates/ferrite-e2e/src/page.rs#L1291)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.failure` | method | Partial | `Download.failure` ([source](crates/ferrite-e2e/src/page.rs#L1253)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.page` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Download.path` | method | Partial | `Download.path` ([source](crates/ferrite-e2e/src/page.rs#L1245)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.saveAs` | method | Partial | `Download.save_as` ([source](crates/ferrite-e2e/src/page.rs#L1276)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.suggestedFilename` | method | Partial | `Download.suggested_filename` ([source](crates/ferrite-e2e/src/page.rs#L1247)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |
| `Download.url` | method | Partial | `Download.url` ([source](crates/ferrite-e2e/src/page.rs#L1250)) | Represents completed files; URL/failure data are Chromium-only and filename derives from the saved path. |

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
| `Fixtures.browser` | property | Partial | `Browser` ([source](crates/ferrite-e2e/src/browser.rs#L319)) | Manual Browser/Context/client setup or injected Page; runner injects a fresh context/page for every attempt. |
| `Fixtures.browserName` | property | Partial | `Browser.kind` ([source](crates/ferrite-e2e/src/browser.rs#L699)) | Manual Browser/Context/client setup or injected Page; runner injects a fresh context/page for every attempt. |
| `Fixtures.context` | property | Partial | `BrowserContext` ([source](crates/ferrite-e2e/src/context.rs#L326)) | Manual Browser/Context/client setup or injected Page; runner injects a fresh context/page for every attempt. |
| `Fixtures.mount` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Fixtures.page` | property | Partial | `test` ([source](crates/ferrite-e2e/src/runner.rs#L147)) | Manual Browser/Context/client setup or injected Page; runner injects a fresh context/page for every attempt. |
| `Fixtures.request` | property | Partial | `ApiClient` ([source](crates/ferrite-e2e/src/api.rs#L135)) | Manual Browser/Context/client setup or injected Page; runner injects a fresh context/page for every attempt. |

## Frame

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-frame.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Frame.addScriptTag` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.addStyleTag` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.check` | method | Partial | `Locator.check` ([source](crates/ferrite-e2e/src/locator.rs#L1464)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.childFrames` | method | Partial | `Frame.child_frames` ([source](crates/ferrite-e2e/src/page.rs#L436)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.click` | method | Partial | `Locator.click` ([source](crates/ferrite-e2e/src/locator.rs#L1199)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.content` | method | Equivalent | `Frame.content` ([source](crates/ferrite-e2e/src/page.rs#L466)) | Basic document access. |
| `Frame.dblclick` | method | Partial | `Locator.dblclick` ([source](crates/ferrite-e2e/src/locator.rs#L1235)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.dispatchEvent` | method | Partial | `Locator.dispatch_event` ([source](crates/ferrite-e2e/src/locator.rs#L1324)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.dragAndDrop` | method | Partial | `Locator.drag_to` ([source](crates/ferrite-e2e/src/locator.rs#L1289)) | Frame.locator(source).drag_to(target, steps); fewer options and frame-coordinate restrictions. |
| `Frame.evalOnSelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.evalOnSelectorAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.evaluate` | method | Partial | `Frame.evaluate` ([source](crates/ferrite-e2e/src/page.rs#L475)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.evaluateHandle` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.fill` | method | Partial | `Locator.fill` ([source](crates/ferrite-e2e/src/locator.rs#L1376)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.focus` | method | Partial | `Locator.focus` ([source](crates/ferrite-e2e/src/locator.rs#L1261)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.frameElement` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.frameLocator` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.getAttribute` | method | Partial | `Locator.attribute` ([source](crates/ferrite-e2e/src/locator.rs#L1569)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.getByAltText` | method | Partial | `Frame.get_by_alt` ([source](crates/ferrite-e2e/src/page.rs#L533)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByLabel` | method | Partial | `Frame.get_by_label` ([source](crates/ferrite-e2e/src/page.rs#L523)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByPlaceholder` | method | Partial | `Frame.get_by_placeholder` ([source](crates/ferrite-e2e/src/page.rs#L528)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByRole` | method | Partial | `Frame.get_by_role_with` ([source](crates/ferrite-e2e/src/page.rs#L514)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByTestId` | method | Partial | `Frame.get_by_test_id` ([source](crates/ferrite-e2e/src/page.rs#L499)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByText` | method | Partial | `Frame.get_by_text` ([source](crates/ferrite-e2e/src/page.rs#L504)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.getByTitle` | method | Partial | `Frame.get_by_title` ([source](crates/ferrite-e2e/src/page.rs#L538)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.goto` | method | Partial | `Frame.goto` ([source](crates/ferrite-e2e/src/page.rs#L418)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.hover` | method | Partial | `Locator.hover` ([source](crates/ferrite-e2e/src/locator.rs#L1246)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.innerHTML` | method | Partial | `Locator.inner_html` ([source](crates/ferrite-e2e/src/locator.rs#L1601)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.innerText` | method | Partial | `Locator.text` ([source](crates/ferrite-e2e/src/locator.rs#L1525)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.inputValue` | method | Partial | `Locator.input_value` ([source](crates/ferrite-e2e/src/locator.rs#L1530)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isChecked` | method | Partial | `Locator.is_checked` ([source](crates/ferrite-e2e/src/locator.rs#L1672)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isDetached` | method | Partial | `Frame.is_detached` ([source](crates/ferrite-e2e/src/page.rs#L447)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.isDisabled` | method | Partial | `Locator.is_disabled` ([source](crates/ferrite-e2e/src/locator.rs#L1667)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isEditable` | method | Partial | `Locator.is_editable` ([source](crates/ferrite-e2e/src/locator.rs#L1677)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isEnabled` | method | Partial | `Locator.is_enabled` ([source](crates/ferrite-e2e/src/locator.rs#L1662)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isHidden` | method | Partial | `Locator.is_hidden` ([source](crates/ferrite-e2e/src/locator.rs#L1656)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.isVisible` | method | Partial | `Locator.is_visible` ([source](crates/ferrite-e2e/src/locator.rs#L1651)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.locator` | method | Partial | `Frame.locator` ([source](crates/ferrite-e2e/src/page.rs#L494)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Frame.name` | method | Partial | `Frame.name` ([source](crates/ferrite-e2e/src/page.rs#L407)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.page` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.parentFrame` | method | Partial | `Frame.parent` ([source](crates/ferrite-e2e/src/page.rs#L423)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
| `Frame.press` | method | Partial | `Locator.press` ([source](crates/ferrite-e2e/src/locator.rs#L1388)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.querySelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.querySelectorAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.selectOption` | method | Partial | `Locator.select_options` ([source](crates/ferrite-e2e/src/locator.rs#L1498)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.setChecked` | method | Partial | `Locator.set_checked` ([source](crates/ferrite-e2e/src/locator.rs#L1708)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.setContent` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Frame.setInputFiles` | method | Partial | `Locator.set_input_files` ([source](crates/ferrite-e2e/src/locator.rs#L1432)) | Use Frame.locator(selector); path uploads only and fewer options. |
| `Frame.tap` | method | Partial | `Locator.tap` ([source](crates/ferrite-e2e/src/locator.rs#L1273)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.textContent` | method | Partial | `Locator.text` ([source](crates/ferrite-e2e/src/locator.rs#L1525)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.title` | method | Equivalent | `Frame.title` ([source](crates/ferrite-e2e/src/page.rs#L457)) | Basic document access. |
| `Frame.type` (deprecated) | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L1400)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.uncheck` | method | Partial | `Locator.uncheck` ([source](crates/ferrite-e2e/src/locator.rs#L1469)) | Frame.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Frame.url` | method | Partial | `Frame.url` ([source](crates/ferrite-e2e/src/page.rs#L413)) | Frame handles/evaluation exist; Firefox frame names are empty, navigation returns unit, and coordinate actions may miss offset iframes. |
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
| `FullConfig.forbidOnly` | property | Partial | `Runner.forbid_only` ([source](crates/ferrite-e2e/src/runner.rs#L924)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.fullyParallel` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.globalSetup` | property | Partial | `Runner.global_setup` ([source](crates/ferrite-e2e/src/runner.rs#L837)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.globalTeardown` | property | Partial | `Runner.global_teardown` ([source](crates/ferrite-e2e/src/runner.rs#L848)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.globalTimeout` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.grep` | property | Partial | `Runner.grep` ([source](crates/ferrite-e2e/src/runner.rs#L770)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.grepInvert` | property | Partial | `Runner.grep_invert` ([source](crates/ferrite-e2e/src/runner.rs#L779)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.maxFailures` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.metadata` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.preserveOutput` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.projects` | property | Partial | `Runner.project` ([source](crates/ferrite-e2e/src/runner.rs#L908)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.quiet` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.reporter` | property | Partial | `E2eConfig.reporter` ([source](crates/ferrite-config/src/lib.rs#L424)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.reportSlowTests` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.rootDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.shard` | property | Partial | `Runner.shard` ([source](crates/ferrite-e2e/src/runner.rs#L866)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.tags` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.updateSnapshots` | property | Partial | `E2eConfig.update_snapshots` ([source](crates/ferrite-config/src/lib.rs#L436)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.updateSourceMethod` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.version` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullConfig.webServer` | property | Partial | `E2eConfig.web_server` ([source](crates/ferrite-config/src/lib.rs#L440)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `FullConfig.workers` | property | Partial | `Runner.workers` ([source](crates/ferrite-e2e/src/runner.rs#L738)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |

## FullProject

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-fullproject.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `FullProject.dependencies` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.grep` | property | Partial | `Project.grep` ([source](crates/ferrite-e2e/src/runner.rs#L403)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `FullProject.grepInvert` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.ignoreSnapshots` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.metadata` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.name` | property | Partial | `Project.name` ([source](crates/ferrite-e2e/src/runner.rs#L363)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `FullProject.snapshotDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.outputDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.repeatEach` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.retries` | property | Partial | `Project.retries` ([source](crates/ferrite-e2e/src/runner.rs#L410)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `FullProject.teardown` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.testDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.testIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.testMatch` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `FullProject.timeout` | property | Partial | `Project.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L417)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `FullProject.use` | property | Partial | `Project.context_options` ([source](crates/ferrite-e2e/src/runner.rs#L387)) | Project-specific context settings; optional browser/launch overrides, no test.use suite hierarchy or project dependencies. |

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
| `Keyboard.down` | method | Partial | `Page.key_down` ([source](crates/ferrite-e2e/src/page.rs#L2379)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |
| `Keyboard.insertText` | method | Partial | `Page.insert_text` ([source](crates/ferrite-e2e/src/page.rs#L2326)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |
| `Keyboard.press` | method | Partial | `Page.press_key_with` ([source](crates/ferrite-e2e/src/page.rs#L2340)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |
| `Keyboard.type` | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L1400)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |
| `Keyboard.up` | method | Partial | `Page.key_up` ([source](crates/ferrite-e2e/src/page.rs#L2386)) | Basic trusted inputs available via Page/Locator; fewer modifiers/options and different typing defaults. |

## Location

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-location.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Location.file` | property | Partial | `TestInfo.file` ([source](crates/ferrite-e2e/src/runner.rs#L201)) | Rust track_caller metadata; no column. |
| `Location.line` | property | Partial | `TestInfo.line` ([source](crates/ferrite-e2e/src/runner.rs#L203)) | Rust track_caller metadata; no column. |
| `Location.column` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## Locator

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-locator.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Locator.all` | method | Equivalent | `Locator.all` ([source](crates/ferrite-e2e/src/locator.rs#L1040)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.allInnerTexts` | method | Partial | `Locator.all_inner_texts` ([source](crates/ferrite-e2e/src/locator.rs#L807)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.allTextContents` | method | Partial | `Locator.all_text_contents` ([source](crates/ferrite-e2e/src/locator.rs#L801)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.and` | method | Equivalent | `Locator.and_` ([source](crates/ferrite-e2e/src/locator.rs#L989)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.ariaSnapshot` | method | Partial | `Locator.aria_snapshot` ([source](crates/ferrite-e2e/src/locator.rs#L843)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.ariaSnapshotJSON` | method | Partial | `Locator.aria_snapshot_json` ([source](crates/ferrite-e2e/src/locator.rs#L837)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.blur` | method | Partial | `Locator.blur` ([source](crates/ferrite-e2e/src/locator.rs#L1267)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.boundingBox` | method | Partial | `Locator.bounding_box` ([source](crates/ferrite-e2e/src/locator.rs#L1692)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.check` | method | Partial | `Locator.check` ([source](crates/ferrite-e2e/src/locator.rs#L1464)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.clear` | method | Partial | `Locator.clear` ([source](crates/ferrite-e2e/src/locator.rs#L1424)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.click` | method | Partial | `Locator.click_with_options` ([source](crates/ferrite-e2e/src/locator.rs#L1204)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.count` | method | Equivalent | `Locator.count` ([source](crates/ferrite-e2e/src/locator.rs#L1094)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.dblclick` | method | Partial | `Locator.dblclick` ([source](crates/ferrite-e2e/src/locator.rs#L1235)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.describe` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.description` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.dispatchEvent` | method | Partial | `Locator.dispatch_event` ([source](crates/ferrite-e2e/src/locator.rs#L1324)) | Always creates a bubbling CustomEvent with detail; not Playwright eventInit/type-specific event behavior. |
| `Locator.dragTo` | method | Partial | `Locator.drag_to` ([source](crates/ferrite-e2e/src/locator.rs#L1289)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.drop` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.elementHandle` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.elementHandles` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.contentFrame` | method | Partial | `Locator.content_frame` ([source](crates/ferrite-e2e/src/locator.rs#L742)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.evaluate` | method | Partial | `Locator.evaluate_with_arg` ([source](crates/ferrite-e2e/src/locator.rs#L778)) | Function with element and JSON argument; no JSHandle arguments or arbitrary JS result serialization. |
| `Locator.evaluateAll` | method | Partial | `Locator.evaluate_all` ([source](crates/ferrite-e2e/src/locator.rs#L791)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.evaluateHandle` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.fill` | method | Partial | `Locator.fill` ([source](crates/ferrite-e2e/src/locator.rs#L1376)) | Retries visibility/enabled/editability, uses native input setters or contenteditable text plus input/change events; not all native input validation/event semantics. |
| `Locator.filter` | method | Partial | `Locator.filter_with` ([source](crates/ferrite-e2e/src/locator.rs#L952)) | Relative has/hasNot/text and visibility filters; exact/regex matching through locator builders. Inner locators must share the document. |
| `Locator.first` | method | Partial | `Locator.first` ([source](crates/ferrite-e2e/src/locator.rs#L897)) | Narrows the resolved set to its first match, including count/all; selector semantics remain narrower. |
| `Locator.focus` | method | Partial | `Locator.focus` ([source](crates/ferrite-e2e/src/locator.rs#L1261)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.frameLocator` | method | Partial | `Locator.frame_locator` ([source](crates/ferrite-e2e/src/locator.rs#L747)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.getAttribute` | method | Partial | `Locator.attribute` ([source](crates/ferrite-e2e/src/locator.rs#L1569)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.getByAltText` | method | Partial | `Locator.get_by_alt` ([source](crates/ferrite-e2e/src/locator.rs#L1083)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByLabel` | method | Partial | `Locator.get_by_label` ([source](crates/ferrite-e2e/src/locator.rs#L1071)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByPlaceholder` | method | Partial | `Locator.get_by_placeholder` ([source](crates/ferrite-e2e/src/locator.rs#L1077)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByRole` | method | Partial | `Locator.get_by_role_with` ([source](crates/ferrite-e2e/src/locator.rs#L1065)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByTestId` | method | Partial | `Locator.get_by_test_id` ([source](crates/ferrite-e2e/src/locator.rs#L1047)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByText` | method | Partial | `Locator.get_by_text` ([source](crates/ferrite-e2e/src/locator.rs#L1053)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.getByTitle` | method | Partial | `Locator.get_by_title` ([source](crates/ferrite-e2e/src/locator.rs#L1089)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.hideHighlight` | method | Partial | `Locator.hide_highlight` ([source](crates/ferrite-e2e/src/locator.rs#L831)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.highlight` | method | Partial | `Locator.highlight` ([source](crates/ferrite-e2e/src/locator.rs#L1697)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.hover` | method | Partial | `Locator.hover` ([source](crates/ferrite-e2e/src/locator.rs#L1246)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.innerHTML` | method | Partial | `Locator.inner_html` ([source](crates/ferrite-e2e/src/locator.rs#L1601)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.innerText` | method | Partial | `Locator.inner_text` ([source](crates/ferrite-e2e/src/locator.rs#L822)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.inputValue` | method | Partial | `Locator.input_value` ([source](crates/ferrite-e2e/src/locator.rs#L1530)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isChecked` | method | Partial | `Locator.is_checked` ([source](crates/ferrite-e2e/src/locator.rs#L1672)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isDisabled` | method | Partial | `Locator.is_disabled` ([source](crates/ferrite-e2e/src/locator.rs#L1667)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isEditable` | method | Partial | `Locator.is_editable` ([source](crates/ferrite-e2e/src/locator.rs#L1677)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isEnabled` | method | Partial | `Locator.is_enabled` ([source](crates/ferrite-e2e/src/locator.rs#L1662)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isHidden` | method | Partial | `Locator.is_hidden` ([source](crates/ferrite-e2e/src/locator.rs#L1656)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.isVisible` | method | Partial | `Locator.is_visible` ([source](crates/ferrite-e2e/src/locator.rs#L1651)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.last` | method | Partial | `Locator.last` ([source](crates/ferrite-e2e/src/locator.rs#L903)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.locator` | method | Partial | `Locator.locator` ([source](crates/ferrite-e2e/src/locator.rs#L891)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Locator.normalize` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Locator.nth` | method | Equivalent | `Locator.nth` ([source](crates/ferrite-e2e/src/locator.rs#L909)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.or` | method | Equivalent | `Locator.or_` ([source](crates/ferrite-e2e/src/locator.rs#L984)) | Equivalent basic collection/selection operation; selector limitations still apply. |
| `Locator.page` | method | Partial | `Locator.page` ([source](crates/ferrite-e2e/src/locator.rs#L752)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.press` | method | Partial | `Locator.press_with` ([source](crates/ferrite-e2e/src/locator.rs#L1394)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.pressSequentially` | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L1400)) | Trusted per-character key input with optional delay; fewer keyboard layout/modifier options. |
| `Locator.screenshot` | method | Partial | `Locator.screenshot` ([source](crates/ferrite-e2e/src/locator.rs#L1361)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.scrollIntoViewIfNeeded` | method | Partial | `Locator.scroll_into_view` ([source](crates/ferrite-e2e/src/locator.rs#L1318)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.selectOption` | method | Partial | `Locator.select_options` ([source](crates/ferrite-e2e/src/locator.rs#L1498)) | Value/label/index matching exists; returns unit rather than selected values, with fewer options. |
| `Locator.selectText` | method | Partial | `Locator.select_text` ([source](crates/ferrite-e2e/src/locator.rs#L1339)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.setChecked` | method | Partial | `Locator.set_checked` ([source](crates/ferrite-e2e/src/locator.rs#L1708)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.setInputFiles` | method | Partial | `Locator.set_input_files` ([source](crates/ferrite-e2e/src/locator.rs#L1432)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.tap` | method | Partial | `Locator.tap` ([source](crates/ferrite-e2e/src/locator.rs#L1273)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.textContent` | method | Partial | `Locator.text_content` ([source](crates/ferrite-e2e/src/locator.rs#L813)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |
| `Locator.toString` | method | Idiomatic | `Locator.selector` ([source](crates/ferrite-e2e/src/locator.rs#L874)) | Selector string available; no Playwright locator expression representation. |
| `Locator.type` (deprecated) | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L1400)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.uncheck` | method | Partial | `Locator.uncheck` ([source](crates/ferrite-e2e/src/locator.rs#L1469)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.visible` | method | Partial | `Locator.visible` ([source](crates/ferrite-e2e/src/locator.rs#L757)) | Lazy visibility filter reapplied when resolving; uses the shared DOM visibility approximation. |
| `Locator.waitFor` | method | Partial | `Locator.wait_for_state` ([source](crates/ferrite-e2e/src/locator.rs#L1121)) | Strict single-target operation with actionability retries; option sets and some input/event semantics remain narrower. |
| `Locator.waitForFunction` | method | Partial | `Locator.wait_for_function` ([source](crates/ferrite-e2e/src/locator.rs#L852)) | Dedicated counterpart; strict/DOM/accessibility/frame/options differences remain. Lazy frame selection is same-origin only. |

## LocatorAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-locatorassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `LocatorAssertions.not` | property | Partial | `LocatorExpect.not` ([source](crates/ferrite-e2e/src/expect.rs#L557)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeAttached` | method | Partial | `LocatorExpect.attached` ([source](crates/ferrite-e2e/src/expect.rs#L908)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeChecked` | method | Partial | `LocatorExpect.checked` ([source](crates/ferrite-e2e/src/expect.rs#L738)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeDisabled` | method | Partial | `LocatorExpect.disabled` ([source](crates/ferrite-e2e/src/expect.rs#L801)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeEditable` | method | Partial | `LocatorExpect.editable` ([source](crates/ferrite-e2e/src/expect.rs#L826)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeEmpty` | method | Partial | `LocatorExpect.empty` ([source](crates/ferrite-e2e/src/expect.rs#L854)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeEnabled` | method | Partial | `LocatorExpect.enabled` ([source](crates/ferrite-e2e/src/expect.rs#L776)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeFocused` | method | Partial | `LocatorExpect.focused` ([source](crates/ferrite-e2e/src/expect.rs#L883)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeHidden` | method | Partial | `LocatorExpect.hidden` ([source](crates/ferrite-e2e/src/expect.rs#L591)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toBeInViewport` | method | Partial | `LocatorExpect.in_viewport` ([source](crates/ferrite-e2e/src/expect.rs#L1073)) | Checks rectangle overlap only; no IntersectionObserver ratio option. |
| `LocatorAssertions.toBeVisible` | method | Partial | `LocatorExpect.visible` ([source](crates/ferrite-e2e/src/expect.rs#L563)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toContainClass` | method | Partial | `LocatorExpect.contains_class` ([source](crates/ferrite-e2e/src/expect.rs#L966)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toContainText` | method | Partial | `LocatorExpect.contains_text` ([source](crates/ferrite-e2e/src/expect.rs#L647)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveAccessibleDescription` | method | Partial | `LocatorExpect.accessible_description` ([source](crates/ferrite-e2e/src/expect.rs#L1129)) | Approximate DOM computation; no full accessibility algorithm or regex matching. |
| `LocatorAssertions.toHaveAccessibleErrorMessage` | method | Partial | `LocatorExpect.accessible_error_message` ([source](crates/ferrite-e2e/src/expect.rs#L532)) | Retrying dedicated matcher; ARIA snapshots use exact text and a DOM approximation, not full YAML matching. |
| `LocatorAssertions.toHaveAccessibleName` | method | Partial | `LocatorExpect.accessible_name` ([source](crates/ferrite-e2e/src/expect.rs#L1098)) | Approximate DOM computation; no full accessibility algorithm or regex matching. |
| `LocatorAssertions.toHaveAttribute` | method | Partial | `LocatorExpect.attribute` ([source](crates/ferrite-e2e/src/expect.rs#L933)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveClass` | method | Partial | `LocatorExpect.class` ([source](crates/ferrite-e2e/src/expect.rs#L490)) | Retrying dedicated matcher; ARIA snapshots use exact text and a DOM approximation, not full YAML matching. |
| `LocatorAssertions.toHaveCount` | method | Partial | `LocatorExpect.count` ([source](crates/ferrite-e2e/src/expect.rs#L709)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveCSS` | method | Partial | `LocatorExpect.css` ([source](crates/ferrite-e2e/src/expect.rs#L1007)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveId` | method | Partial | `LocatorExpect.id` ([source](crates/ferrite-e2e/src/expect.rs#L1002)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveJSProperty` | method | Partial | `LocatorExpect.js_property` ([source](crates/ferrite-e2e/src/expect.rs#L1040)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveRole` | method | Partial | `LocatorExpect.role` ([source](crates/ferrite-e2e/src/expect.rs#L524)) | Retrying dedicated matcher; ARIA snapshots use exact text and a DOM approximation, not full YAML matching. |
| `LocatorAssertions.toHaveScreenshot` | method | Partial | `LocatorExpect.screenshot_with` ([source](crates/ferrite-e2e/src/expect.rs#L1169)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveText` | method | Partial | `LocatorExpect.text` ([source](crates/ferrite-e2e/src/expect.rs#L616)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveValue` | method | Partial | `LocatorExpect.value` ([source](crates/ferrite-e2e/src/expect.rs#L678)) | Retrying assertion available; string/regex/array/options, accessibility and snapshot semantics differ. |
| `LocatorAssertions.toHaveValues` | method | Partial | `LocatorExpect.values` ([source](crates/ferrite-e2e/src/expect.rs#L495)) | Retrying dedicated matcher; ARIA snapshots use exact text and a DOM approximation, not full YAML matching. |
| `LocatorAssertions.toMatchAriaSnapshot` | method | Partial | `LocatorExpect.aria_snapshot` ([source](crates/ferrite-e2e/src/expect.rs#L540)) | Retrying dedicated matcher; ARIA snapshots use exact text and a DOM approximation, not full YAML matching. |

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
| `Mouse.click` | method | Partial | `Page.mouse_click_with` ([source](crates/ferrite-e2e/src/page.rs#L2307)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.dblclick` | method | Partial | `Page.mouse_click` ([source](crates/ferrite-e2e/src/page.rs#L2300)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.down` | method | Partial | `Page.mouse_down` ([source](crates/ferrite-e2e/src/page.rs#L2351)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.move` | method | Partial | `Page.mouse_move` ([source](crates/ferrite-e2e/src/page.rs#L2319)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.up` | method | Partial | `Page.mouse_up` ([source](crates/ferrite-e2e/src/page.rs#L2358)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |
| `Mouse.wheel` | method | Partial | `Page.mouse_wheel` ([source](crates/ferrite-e2e/src/page.rs#L2372)) | Input methods on Page; down/up/wheel also take coordinates; fewer modifier/steps/button options. |

## Page

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-page.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Page.clock` | property | Partial | `Page.clock_install` ([source](crates/ferrite-e2e/src/page.rs#L2620)) | Fake clock is scoped to the current document and resets on navigation; semantic differences below. |
| `Page.close` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1575)) | PageEvent.Closed via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.console` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1575)) | PageEvent.Console via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.crash` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.dialog` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1575)) | PageEvent.Dialog via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.dialogClosed` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.DOMContentLoaded` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.download` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1575)) | PageEvent.Download via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.fileChooser` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.frameAttached` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.frameDetached` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.frameNavigated` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.load` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.pageError` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.popup` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1575)) | PageEvent.Popup via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.request` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1575)) | PageEvent.Request via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.requestFailed` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.requestFinished` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.response` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1575)) | PageEvent.Response via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.webSocket` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1575)) | PageEvent.WebSocket via broadcast receiver; smaller payload and lifecycle; socket events are Chromium-only. |
| `Page.worker` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `Page.addInitScript` | method | Partial | `Page.add_init_script` ([source](crates/ferrite-e2e/src/page.rs#L3307)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.addScriptTag` | method | Partial | `Page.add_script_tag_url` ([source](crates/ferrite-e2e/src/page.rs#L3312)) | URL/content helpers return unit; no ElementHandle return, file/type options; other helper: Page.add_script_tag_content |
| `Page.addStyleTag` | method | Partial | `Page.add_style_tag_url` ([source](crates/ferrite-e2e/src/page.rs#L3338)) | URL/content helpers return unit; no ElementHandle return, file/type options; other helper: Page.add_style_tag_content |
| `Page.bringToFront` | method | Partial | `Page.bring_to_front` ([source](crates/ferrite-e2e/src/page.rs#L1950)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.cancelPickLocator` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.check` | method | Partial | `Locator.check` ([source](crates/ferrite-e2e/src/locator.rs#L1464)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.click` | method | Partial | `Locator.click` ([source](crates/ferrite-e2e/src/locator.rs#L1199)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.close` | method | Partial | `Page.close` ([source](crates/ferrite-e2e/src/page.rs#L3955)) | Closes the target and its owning convenience context; no runBeforeUnload/reason options. |
| `Page.content` | method | Equivalent | `Page.content` ([source](crates/ferrite-e2e/src/page.rs#L1939)) | Basic document access. |
| `Page.context` | method | Partial | `Page.context` ([source](crates/ferrite-e2e/src/page.rs#L1522)) | Owning context while registered; returns Option and becomes None after context disposal. |
| `Page.coverage` | property | Partial | `Page.coverage` ([source](crates/ferrite-e2e/src/page.rs#L1492)) | Dedicated API exists; coverage is Chromium-only, lazy frame locators are same-origin, exceptions have ConsoleMessage shape. |
| `Page.dblclick` | method | Partial | `Locator.dblclick` ([source](crates/ferrite-e2e/src/locator.rs#L1235)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.dispatchEvent` | method | Partial | `Locator.dispatch_event` ([source](crates/ferrite-e2e/src/locator.rs#L1324)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.dragAndDrop` | method | Partial | `Locator.drag_to` ([source](crates/ferrite-e2e/src/locator.rs#L1289)) | Page.locator(source).drag_to(target, steps); fewer options and frame-coordinate restrictions. |
| `Page.emulateMedia` | method | Partial | `Page.emulate_media` ([source](crates/ferrite-e2e/src/page.rs#L3018)) | Chromium only; color scheme/reduced motion only, no full media/forcedColors/contrast surface. |
| `Page.evalOnSelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.evalOnSelectorAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.evaluate` | method | Partial | `Page.evaluate_with_arg` ([source](crates/ferrite-e2e/src/page.rs#L1955)) | JSON-serializable arguments and results; no JSHandle argument or arbitrary JS value serialization. |
| `Page.evaluateHandle` | method | Partial | `Page.evaluate_handle` ([source](crates/ferrite-e2e/src/page.rs#L2003)) | Remote JSHandle supported; no ElementHandle conversion or separate evaluation argument. |
| `Page.exposeBinding` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.exposeFunction` | method | Partial | `Page.expose_function` ([source](crates/ferrite-e2e/src/page.rs#L3405)) | Survives navigation using init scripts; polled Rust callbacks, no context bindings, frame-wide dispatch or async callbacks. |
| `Page.fill` | method | Partial | `Locator.fill` ([source](crates/ferrite-e2e/src/locator.rs#L1376)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.focus` | method | Partial | `Locator.focus` ([source](crates/ferrite-e2e/src/locator.rs#L1261)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.frame` | method | Partial | `Page.frame_by_name` ([source](crates/ferrite-e2e/src/page.rs#L3383)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.frameLocator` | method | Partial | `Page.frame_locator` ([source](crates/ferrite-e2e/src/page.rs#L1480)) | Dedicated API exists; coverage is Chromium-only, lazy frame locators are same-origin, exceptions have ConsoleMessage shape. |
| `Page.frames` | method | Partial | `Page.document_frames` ([source](crates/ferrite-e2e/src/page.rs#L3366)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.getAttribute` | method | Partial | `Locator.attribute` ([source](crates/ferrite-e2e/src/locator.rs#L1569)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.getByAltText` | method | Partial | `Page.get_by_alt` ([source](crates/ferrite-e2e/src/page.rs#L2118)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByLabel` | method | Partial | `Page.get_by_label` ([source](crates/ferrite-e2e/src/page.rs#L2106)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByPlaceholder` | method | Partial | `Page.get_by_placeholder` ([source](crates/ferrite-e2e/src/page.rs#L2112)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByRole` | method | Partial | `Page.get_by_role_with` ([source](crates/ferrite-e2e/src/page.rs#L2094)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByTestId` | method | Partial | `Page.get_by_test_id` ([source](crates/ferrite-e2e/src/page.rs#L2076)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByText` | method | Partial | `Page.get_by_text` ([source](crates/ferrite-e2e/src/page.rs#L2082)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.getByTitle` | method | Partial | `Page.get_by_title` ([source](crates/ferrite-e2e/src/page.rs#L2124)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.goBack` | method | Partial | `Page.go_back` ([source](crates/ferrite-e2e/src/page.rs#L1919)) | Returns unit, not a navigation Response; fewer navigation options. Relative URLs resolve through the configured base URL. |
| `Page.goForward` | method | Partial | `Page.go_forward` ([source](crates/ferrite-e2e/src/page.rs#L1924)) | Returns unit, not a navigation Response; fewer navigation options. Relative URLs resolve through the configured base URL. |
| `Page.requestGC` | method | Partial | `Page.request_gc` ([source](crates/ferrite-e2e/src/page.rs#L2295)) | Chromium only; Firefox returns an unsupported error. |
| `Page.goto` | method | Partial | `Page.goto_with_options` ([source](crates/ferrite-e2e/src/page.rs#L1841)) | Returns unit, not a navigation Response; fewer navigation options. Relative URLs resolve through the configured base URL. |
| `Page.hideHighlight` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.hover` | method | Partial | `Locator.hover` ([source](crates/ferrite-e2e/src/locator.rs#L1246)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.innerHTML` | method | Partial | `Locator.inner_html` ([source](crates/ferrite-e2e/src/locator.rs#L1601)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.innerText` | method | Partial | `Locator.text` ([source](crates/ferrite-e2e/src/locator.rs#L1525)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.inputValue` | method | Partial | `Locator.input_value` ([source](crates/ferrite-e2e/src/locator.rs#L1530)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isChecked` | method | Partial | `Locator.is_checked` ([source](crates/ferrite-e2e/src/locator.rs#L1672)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isClosed` | method | Partial | `Page.is_closed` ([source](crates/ferrite-e2e/src/page.rs#L1653)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.isDisabled` | method | Partial | `Locator.is_disabled` ([source](crates/ferrite-e2e/src/locator.rs#L1667)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isEditable` | method | Partial | `Locator.is_editable` ([source](crates/ferrite-e2e/src/locator.rs#L1677)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isEnabled` | method | Partial | `Locator.is_enabled` ([source](crates/ferrite-e2e/src/locator.rs#L1662)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isHidden` | method | Partial | `Locator.is_hidden` ([source](crates/ferrite-e2e/src/locator.rs#L1656)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.isVisible` | method | Partial | `Locator.is_visible` ([source](crates/ferrite-e2e/src/locator.rs#L1651)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.keyboard` | property | Partial | `Page.press_key` ([source](crates/ferrite-e2e/src/page.rs#L2333)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.clearConsoleMessages` | method | Partial | `Page.clear_console_messages` ([source](crates/ferrite-e2e/src/page.rs#L1540)) | Dedicated API exists; coverage is Chromium-only, lazy frame locators are same-origin, exceptions have ConsoleMessage shape. |
| `Page.clearPageErrors` | method | Partial | `Page.clear_page_errors` ([source](crates/ferrite-e2e/src/page.rs#L1557)) | Dedicated API exists; coverage is Chromium-only, lazy frame locators are same-origin, exceptions have ConsoleMessage shape. |
| `Page.localStorage` | property | Partial | `Page.local_storage_get` ([source](crates/ferrite-e2e/src/page.rs#L2448)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.sessionStorage` | property | Partial | `Page.session_storage_get` ([source](crates/ferrite-e2e/src/page.rs#L2473)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.consoleMessages` | method | Partial | `Page.console_messages` ([source](crates/ferrite-e2e/src/page.rs#L1795)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.pageErrors` | method | Partial | `Page.page_errors` ([source](crates/ferrite-e2e/src/page.rs#L1549)) | Dedicated API exists; coverage is Chromium-only, lazy frame locators are same-origin, exceptions have ConsoleMessage shape. |
| `Page.locator` | method | Partial | `Page.locator` ([source](crates/ferrite-e2e/src/page.rs#L2070)) | Strict single-target actions; exact/regex matching and open shadow roots supported. Full accessible-name/selector-extension semantics remain narrower. |
| `Page.mainFrame` | method | Partial | `Page.document_frames` ([source](crates/ferrite-e2e/src/page.rs#L3366)) | Enumerate frames and choose the root manually; no dedicated main_frame API. |
| `Page.mouse` | property | Partial | `Page.mouse_click` ([source](crates/ferrite-e2e/src/page.rs#L2300)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.opener` | method | Partial | `Page.opener` ([source](crates/ferrite-e2e/src/page.rs#L1686)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.pause` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.pdf` | method | Partial | `Page.pdf` ([source](crates/ferrite-e2e/src/page.rs#L2601)) | PDF export exists; no PDF options builder. Engine behavior differs. |
| `Page.pickLocator` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.press` | method | Partial | `Locator.press` ([source](crates/ferrite-e2e/src/locator.rs#L1388)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.querySelector` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.querySelectorAll` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.requests` | method | Partial | `Page.requests` ([source](crates/ferrite-e2e/src/page.rs#L3250)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.addLocatorHandler` | method | Partial | `Page.add_locator_handler_with` ([source](crates/ferrite-e2e/src/page.rs#L2149)) | Visibility-based overlay handlers run before actions and state/custom assertions; no full dismissal/noWaitAfter semantics. |
| `Page.removeAllListeners` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.removeLocatorHandler` | method | Partial | `Page.remove_locator_handler` ([source](crates/ferrite-e2e/src/page.rs#L2171)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.reload` | method | Partial | `Page.reload` ([source](crates/ferrite-e2e/src/page.rs#L1914)) | Returns unit, not a navigation Response; fewer navigation options. Relative URLs resolve through the configured base URL. |
| `Page.request` | property | Partial | `Page.request` ([source](crates/ferrite-e2e/src/page.rs#L1533)) | HTTP client sharing the owning context cookies; returns an error after context disposal. Transport settings configured separately. |
| `Page.route` | method | Partial | `Page.route_with_handler` ([source](crates/ferrite-e2e/src/page.rs#L3043)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.routeFromHAR` | method | Partial | `Page.route_from_har` ([source](crates/ferrite-e2e/src/page.rs#L3146)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.routeWebSocket` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.screencast` | property | Partial | `Page.frames` ([source](crates/ferrite-e2e/src/page.rs#L3925)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.screenshot` | method | Partial | `Page.screenshot` ([source](crates/ferrite-e2e/src/page.rs#L2517)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.selectOption` | method | Partial | `Locator.select_options` ([source](crates/ferrite-e2e/src/locator.rs#L1498)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.setChecked` | method | Partial | `Locator.set_checked` ([source](crates/ferrite-e2e/src/locator.rs#L1708)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.setContent` | method | Partial | `Page.set_content` ([source](crates/ferrite-e2e/src/page.rs#L1945)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.setDefaultNavigationTimeout` | method | Partial | `Page.set_navigation_timeout` ([source](crates/ferrite-e2e/src/page.rs#L1747)) | Navigation default distinct from locator timeout. |
| `Page.setDefaultTimeout` | method | Partial | `Page.set_timeout` ([source](crates/ferrite-e2e/src/page.rs#L1738)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.setExtraHTTPHeaders` | method | Partial | `Page.set_extra_http_headers` ([source](crates/ferrite-e2e/src/page.rs#L2956)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.setInputFiles` | method | Partial | `Page.set_input_files` ([source](crates/ferrite-e2e/src/page.rs#L3281)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.setViewportSize` | method | Partial | `Page.set_viewport` ([source](crates/ferrite-e2e/src/page.rs#L2606)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.ariaSnapshot` | method | Partial | `Page.aria_snapshot` ([source](crates/ferrite-e2e/src/page.rs#L3630)) | Structured role/name/state DOM approximation, including open shadow roots; no full ARIA/YAML matching, mode/depth/boxes options. |
| `Page.ariaSnapshotJSON` | method | Partial | `Page.aria_snapshot_json` ([source](crates/ferrite-e2e/src/page.rs#L3624)) | Nested role/name/state DOM tree without name/node truncation; not the complete accessibility algorithm. |
| `Page.tap` | method | Partial | `Locator.tap` ([source](crates/ferrite-e2e/src/locator.rs#L1273)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.textContent` | method | Partial | `Locator.text` ([source](crates/ferrite-e2e/src/locator.rs#L1525)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.title` | method | Equivalent | `Page.title` ([source](crates/ferrite-e2e/src/page.rs#L1929)) | Basic document access. |
| `Page.touchscreen` | property | Partial | `Page.touchscreen_tap` ([source](crates/ferrite-e2e/src/page.rs#L2393)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.type` (deprecated) | method | Partial | `Locator.press_sequentially_with` ([source](crates/ferrite-e2e/src/locator.rs#L1400)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.uncheck` | method | Partial | `Locator.uncheck` ([source](crates/ferrite-e2e/src/locator.rs#L1469)) | Page.locator(selector) followed by this operation; no selector-method facade. Locator/input semantics remain partial. |
| `Page.unrouteAll` | method | Partial | `Page.unroute_all` ([source](crates/ferrite-e2e/src/page.rs#L3125)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.unroute` | method | Partial | `Page.unroute` ([source](crates/ferrite-e2e/src/page.rs#L3099)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.url` | method | Equivalent | `Page.url` ([source](crates/ferrite-e2e/src/page.rs#L1934)) | Basic document access. |
| `Page.video` | method | Partial | `Page.start_video` ([source](crates/ferrite-e2e/src/page.rs#L3856)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.viewportSize` | method | Partial | `Page.viewport_size` ([source](crates/ferrite-e2e/src/page.rs#L2284)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.waitForEvent` | method | Partial | `Page.wait_for_event` ([source](crates/ferrite-e2e/src/page.rs#L1584)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.waitForFunction` | method | Partial | `Page.wait_for_function` ([source](crates/ferrite-e2e/src/page.rs#L2018)) | Awaits Promise predicates correctly; polling returns unit, with no JSHandle/polling-mode/argument options. |
| `Page.waitForLoadState` | method | Partial | `Page.wait_for_load_state` ([source](crates/ferrite-e2e/src/page.rs#L2063)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.waitForNavigation` (deprecated) | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Page.waitForRequest` | method | Partial | `Page.wait_for_request` ([source](crates/ferrite-e2e/src/page.rs#L3729)) | URL substring match; returns RecordedRequest rather than rich Request/Response; no predicate overload. |
| `Page.waitForResponse` | method | Partial | `Page.wait_for_response` ([source](crates/ferrite-e2e/src/page.rs#L3760)) | URL substring match; returns RecordedRequest rather than rich Request/Response; no predicate overload. |
| `Page.waitForSelector` | method | Partial | `Page.wait_for_selector_with` ([source](crates/ferrite-e2e/src/page.rs#L1665)) | Waits for requested state and returns Locator, not ElementHandle. |
| `Page.waitForTimeout` | method | Partial | `Page.wait_for_timeout` ([source](crates/ferrite-e2e/src/page.rs#L2038)) | Similar operation, with a smaller option/result/event surface; see feature audit. |
| `Page.waitForURL` | method | Partial | `Page.wait_for_url` ([source](crates/ferrite-e2e/src/page.rs#L2044)) | Substring match only; no exact/glob/regex/predicate or waitUntil option. |
| `Page.workers` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |

## PageAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-pageassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `PageAssertions.not` | property | Partial | `PageExpect.not` ([source](crates/ferrite-e2e/src/expect.rs#L214)) | Retrying page assertion; exact/regex title and URL helpers, with narrower options and different screenshot/ARIA algorithms. |
| `PageAssertions.toMatchAriaSnapshot` | method | Partial | `PageExpect.aria_snapshot` ([source](crates/ferrite-e2e/src/expect.rs#L404)) | Retrying page assertion; exact/regex title and URL helpers, with narrower options and different screenshot/ARIA algorithms. |
| `PageAssertions.toHaveScreenshot` | method | Partial | `PageExpect.screenshot_with` ([source](crates/ferrite-e2e/src/expect.rs#L353)) | Retrying page assertion; exact/regex title and URL helpers, with narrower options and different screenshot/ARIA algorithms. |
| `PageAssertions.toHaveTitle` | method | Partial | `PageExpect.title` ([source](crates/ferrite-e2e/src/expect.rs#L248)) | Retrying page assertion; exact/regex title and URL helpers, with narrower options and different screenshot/ARIA algorithms. |
| `PageAssertions.toHaveURL` | method | Partial | `PageExpect.url` ([source](crates/ferrite-e2e/src/expect.rs#L302)) | Exact relative/base URL assertion plus url_matches() and url_contains(); narrower options and regex syntax. |

## Playwright

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-playwright.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Playwright.chromium` | property | Partial | `Browser.launch` ([source](crates/ferrite-e2e/src/browser.rs#L364)) | Select BrowserKind with LaunchOptions; no BrowserType object or bundled browser installer. |
| `Playwright.devices` | property | Partial | `DeviceDescriptor` ([source](crates/ferrite-e2e/src/page.rs#L567)) | Seven metrics presets; no full device catalog or device user-agent metadata. |
| `Playwright.errors` | property | Idiomatic | `E2eError` ([source](crates/ferrite-e2e/src/error.rs#L8)) | Rust error enum, with different variants and diagnostics. |
| `Playwright.firefox` | property | Partial | `Browser.launch` ([source](crates/ferrite-e2e/src/browser.rs#L364)) | Select BrowserKind with LaunchOptions; no BrowserType object or bundled browser installer. |
| `Playwright.request` | property | Partial | `ApiClient` ([source](crates/ferrite-e2e/src/api.rs#L135)) | Standalone or context-linked HTTP client; smaller APIRequest option surface. |
| `Playwright.selectors` | property | Partial | `set_test_id_attribute` ([source](crates/ferrite-e2e/src/locator.rs#L22)) | Only test-id configuration; no custom selector registration. |
| `Playwright.webkit` | property | Missing | — | No WebKit backend; BrowserKind contains Chromium and Firefox only. |

## PlaywrightAssertions

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-playwrightassertions.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `PlaywrightAssertions.expectAPIResponse` | method | Idiomatic | `ApiResponse.ok` ([source](crates/ferrite-e2e/src/api.rs#L46)) | Use native Rust assertions over the standalone response. |
| `PlaywrightAssertions.expectGeneric` | method | Idiomatic | — | Use native Rust assertions and explicit pattern/container checks; no Playwright expect matcher library. |
| `PlaywrightAssertions.expectLocator` | method | Partial | `Locator.expect` ([source](crates/ferrite-e2e/src/expect.rs#L1245)) | Rust assertion builders; fewer matcher/expect configuration capabilities. |
| `PlaywrightAssertions.expectPage` | method | Partial | `Page.expect` ([source](crates/ferrite-e2e/src/expect.rs#L1227)) | Rust assertion builders; fewer matcher/expect configuration capabilities. |

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
| `Request.allHeaders` | method | Partial | `RecordedRequest.headers` ([source](crates/ferrite-e2e/src/page.rs#L157)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |
| `Request.failure` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.frame` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.headers` | method | Partial | `RecordedRequest.headers` ([source](crates/ferrite-e2e/src/page.rs#L157)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |
| `Request.headersArray` | method | Partial | `RecordedRequest.headers` ([source](crates/ferrite-e2e/src/page.rs#L157)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |
| `Request.headerValue` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.isNavigationRequest` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.method` | method | Partial | `RecordedRequest.method` ([source](crates/ferrite-e2e/src/page.rs#L150)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |
| `Request.postData` | method | Partial | `RecordedRequest.post_data` ([source](crates/ferrite-e2e/src/page.rs#L160)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |
| `Request.postDataBuffer` | method | Partial | `RecordedRequest.post_data` ([source](crates/ferrite-e2e/src/page.rs#L160)) | Text capture only; no lossless binary request body API. |
| `Request.postDataJSON` | method | Partial | `RecordedRequest.post_data` ([source](crates/ferrite-e2e/src/page.rs#L160)) | Caller parses the captured request text with serde_json; body_json() is RESPONSE JSON, not request postDataJSON. |
| `Request.redirectedFrom` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.redirectedTo` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.resourceType` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.response` | method | Partial | `RecordedRequest.status` ([source](crates/ferrite-e2e/src/page.rs#L154)) | Captured response data are merged into the record; no separate live Response or request lifecycle object. |
| `Request.existingResponse` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.serviceWorker` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.sizes` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Request.timing` | method | Partial | `RecordedRequest.duration_ms` ([source](crates/ferrite-e2e/src/page.rs#L163)) | Total elapsed time only; no DNS/connect/TLS/response timing breakdown. |
| `Request.url` | method | Partial | `RecordedRequest.url` ([source](crates/ferrite-e2e/src/page.rs#L152)) | Captured record fields, not a live Request object; post_data is Chromium-only and response/request metadata are merged. |

## Response

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-response.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Response.allHeaders` | method | Partial | `RecordedRequest.response_headers` ([source](crates/ferrite-e2e/src/page.rs#L172)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.body` | method | Partial | `RecordedRequest.body` ([source](crates/ferrite-e2e/src/page.rs#L184)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.finished` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.frame` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.fromServiceWorker` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.headers` | method | Partial | `RecordedRequest.response_headers` ([source](crates/ferrite-e2e/src/page.rs#L172)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.headersArray` | method | Partial | `RecordedRequest.response_headers` ([source](crates/ferrite-e2e/src/page.rs#L172)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.headerValue` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.headerValues` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.httpVersion` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.json` | method | Partial | `RecordedRequest.body_json` ([source](crates/ferrite-e2e/src/page.rs#L201)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.ok` | method | Idiomatic | `RecordedRequest.status` ([source](crates/ferrite-e2e/src/page.rs#L154)) | Check (200..300).contains(&record.status); no dedicated Response.ok() method. |
| `Response.request` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.securityDetails` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.serverAddr` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Response.status` | method | Partial | `RecordedRequest.status` ([source](crates/ferrite-e2e/src/page.rs#L154)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.statusText` | method | Partial | `RecordedRequest.status_text` ([source](crates/ferrite-e2e/src/page.rs#L166)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.text` | method | Partial | `RecordedRequest.body_text` ([source](crates/ferrite-e2e/src/page.rs#L193)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |
| `Response.url` | method | Partial | `RecordedRequest.url` ([source](crates/ferrite-e2e/src/page.rs#L152)) | Captured request/response record; Chromium bodies capped at 1 MiB, no body channel on Firefox. |

## Route

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-route.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Route.abort` | method | Partial | `RouteAction.abort_with` ([source](crates/ferrite-e2e/src/page.rs#L990)) | Rule/action model, with glob patterns and fewer options; response modification and URL override are Chromium-only. |
| `Route.continue` | method | Partial | `RouteRule.continue_with` ([source](crates/ferrite-e2e/src/page.rs#L1140)) | Rule/action model, with glob patterns and fewer options; response modification and URL override are Chromium-only. |
| `Route.fallback` | method | Partial | `RouteAction.fallback` ([source](crates/ferrite-e2e/src/page.rs#L1027)) | Rule/action model, with glob patterns and fewer options; response modification and URL override are Chromium-only. |
| `Route.fetch` | method | Partial | `RouteInfo.fetch` ([source](crates/ferrite-e2e/src/page.rs#L890)) | Standalone reqwest client, not a shared browser cookie context; no full redirect/retry/options surface. |
| `Route.fulfill` | method | Partial | `RouteAction.fulfill_full` ([source](crates/ferrite-e2e/src/page.rs#L1006)) | Rule/action model, with glob patterns and fewer options; response modification and URL override are Chromium-only. |
| `Route.request` | method | Partial | `RouteInfo` ([source](crates/ferrite-e2e/src/page.rs#L876)) | Rule/action model, with glob patterns and fewer options; response modification and URL override are Chromium-only. |

## Screencast

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-screencast.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Screencast.start` | method | Partial | `Page.start_video` ([source](crates/ferrite-e2e/src/page.rs#L3856)) | Video recording/live frames available; fewer formats/options, no screencast overlay/action system. |
| `Screencast.stop` | method | Partial | `Page.stop_video` ([source](crates/ferrite-e2e/src/page.rs#L3893)) | Explicit output path; Chromium assembles frames with ffmpeg, Firefox records natively. |
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
| `Test.(call)` | method | Partial | `test` ([source](crates/ferrite-e2e/src/runner.rs#L147)) | Rust test closure; dynamic details/locks/options differ. |
| `Test.afterAll` | method | Partial | `Runner.after_all` ([source](crates/ferrite-e2e/src/runner.rs#L826)) | Global runner hooks; not scoped by describe suite or isolated worker lifecycle. |
| `Test.afterEach` | method | Partial | `Runner.after_each` ([source](crates/ferrite-e2e/src/runner.rs#L798)) | Global runner hooks; not scoped by describe suite or isolated worker lifecycle. |
| `Test.beforeAll` | method | Partial | `Runner.before_all` ([source](crates/ferrite-e2e/src/runner.rs#L813)) | Global runner hooks; not scoped by describe suite or isolated worker lifecycle. |
| `Test.beforeEach` | method | Partial | `Runner.before_each` ([source](crates/ferrite-e2e/src/runner.rs#L786)) | Global runner hooks; not scoped by describe suite or isolated worker lifecycle. |
| `Test.describe` | method | Partial | `describe` ([source](crates/ferrite-e2e/src/runner.rs#L185)) | Prefixes names only; no nested suite execution/hook scope or scheduling model. |
| `Test.describe.configure` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.fixme` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.only` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.parallel` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.parallel.only` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.serial` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.serial.only` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.describe.skip` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.expect` | property | Partial | `Locator.expect` ([source](crates/ferrite-e2e/src/expect.rs#L1245)) | Rust builders, expect_poll, expect_to_pass and SoftAsserts; no generic matcher registry/configure API. |
| `Test.extend` | method | Partial | `Runner.fixture_with_teardown` ([source](crates/ferrite-e2e/src/runner.rs#L960)) | Typed per-attempt fixtures; no dependency graph, worker scope, auto fixtures, named overrides or lazy fixture resolution. |
| `Test.abort` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.fail` | method | Partial | `Test.fail` ([source](crates/ferrite-e2e/src/runner.rs#L111)) | Static test builder setting; no runtime conditional/context metadata overloads. |
| `Test.fail.only` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.fixme` | method | Partial | `Test.fixme` ([source](crates/ferrite-e2e/src/runner.rs#L104)) | Static test builder setting; no runtime conditional/context metadata overloads. |
| `Test.info` | method | Partial | `TestContext.info` ([source](crates/ferrite-e2e/src/runner.rs#L317)) | Provided through test_with_context; fewer live metadata fields and mutators. |
| `Test.only` | method | Partial | `Test.only` ([source](crates/ferrite-e2e/src/runner.rs#L97)) | Static test builder setting; no runtime conditional/context metadata overloads. |
| `Test.setTimeout` | method | Partial | `Test.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L139)) | Static test builder setting; no runtime conditional/context metadata overloads. |
| `Test.skip` | method | Partial | `Test.skip` ([source](crates/ferrite-e2e/src/runner.rs#L90)) | Static test builder setting; no runtime conditional/context metadata overloads. |
| `Test.slow` | method | Partial | `Test.slow` ([source](crates/ferrite-e2e/src/runner.rs#L125)) | Static test builder setting; no runtime conditional/context metadata overloads. |
| `Test.step` | method | Partial | `Page.step` ([source](crates/ferrite-e2e/src/page.rs#L2400)) | Named closure logged on Page; no structured step tree, boxing, timeout, subtitle/params or TestStepInfo. |
| `Test.step.skip` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Test.use` | method | Partial | `ContextOptions` ([source](crates/ferrite-e2e/src/context.rs#L31)) | Manual Browser.new_context options; no test.use fixture override/scoping system. |

## TestCase

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-reporter-api/class-testcase.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestCase.annotations` | property | Partial | `Test.annotations` ([source](crates/ferrite-e2e/src/runner.rs#L59)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.expectedStatus` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestCase.id` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestCase.location` | property | Partial | `Test.file` ([source](crates/ferrite-e2e/src/runner.rs#L61)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.ok` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestCase.outcome` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestCase.parent` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestCase.repeatEachIndex` | property | Partial | `TestResult.repeat_each_index` ([source](crates/ferrite-e2e/src/report.rs#L59)) | Result metadata only; no TestCase/Suite reporter tree. |
| `TestCase.results` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestCase.retries` | property | Partial | `Test.retries` ([source](crates/ferrite-e2e/src/runner.rs#L132)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.tags` | property | Partial | `Test.tags` ([source](crates/ferrite-e2e/src/runner.rs#L57)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.timeout` | property | Partial | `Test.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L139)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.title` | property | Partial | `Test.name` ([source](crates/ferrite-e2e/src/runner.rs#L51)) | Test definition field only; no TestCase/Suite reporter tree. |
| `TestCase.titlePath` | method | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |
| `TestCase.type` | property | Missing | — | No corresponding live reporter/suite/step/worker API; Ferrite exposes aggregate TestReport/TestResult and fixed report serializers. |

## TestConfig

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-testconfig.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestConfig.build` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.captureGitInfo` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.expect` | property | Partial | `E2eConfig.expect_timeout_ms` ([source](crates/ferrite-config/src/lib.rs#L418)) | Consumed by CLI environment bridge and Runner; per-page/expect overrides supported. |
| `TestConfig.failOnFlakyTests` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.forbidOnly` | property | Partial | `Runner.forbid_only` ([source](crates/ferrite-e2e/src/runner.rs#L924)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.fullyParallel` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.globalSetup` | property | Partial | `Runner.global_setup` ([source](crates/ferrite-e2e/src/runner.rs#L837)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.globalTeardown` | property | Partial | `Runner.global_teardown` ([source](crates/ferrite-e2e/src/runner.rs#L848)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.globalTimeout` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.grep` | property | Partial | `Runner.grep` ([source](crates/ferrite-e2e/src/runner.rs#L770)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.grepInvert` | property | Partial | `Runner.grep_invert` ([source](crates/ferrite-e2e/src/runner.rs#L779)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.ignoreSnapshots` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.maxFailures` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.metadata` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.name` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.outputDir` | property | Partial | `E2eConfig.output_dir` ([source](crates/ferrite-config/src/lib.rs#L426)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.snapshotDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.snapshotPathTemplate` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.preserveOutput` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.projects` | property | Partial | `Runner.project` ([source](crates/ferrite-e2e/src/runner.rs#L908)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.quiet` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.repeatEach` | property | Partial | `Runner.repeat_each` ([source](crates/ferrite-e2e/src/runner.rs#L915)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.reporter` | property | Partial | `E2eConfig.reporter` ([source](crates/ferrite-config/src/lib.rs#L424)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.reportSlowTests` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.respectGitIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.retries` | property | Partial | `Runner.retries` ([source](crates/ferrite-e2e/src/runner.rs#L745)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.retryStrategy` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.shard` | property | Partial | `Runner.shard` ([source](crates/ferrite-e2e/src/runner.rs#L866)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.tag` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.testDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.testIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.testMatch` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.timeout` | property | Partial | `Runner.test_timeout` ([source](crates/ferrite-e2e/src/runner.rs#L752)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.tsconfig` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.updateSnapshots` | property | Partial | `E2eConfig.update_snapshots` ([source](crates/ferrite-config/src/lib.rs#L436)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.updateSourceMethod` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestConfig.use` | property | Partial | `ContextOptions` ([source](crates/ferrite-e2e/src/context.rs#L31)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.webServer` | property | Partial | `E2eConfig.web_server` ([source](crates/ferrite-config/src/lib.rs#L440)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |
| `TestConfig.workers` | property | Partial | `Runner.workers` ([source](crates/ferrite-e2e/src/runner.rs#L738)) | Comparable knob, but no FullConfig/TestConfig object; runner/CLI wiring and scheduling semantics differ. |

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
| `TestInfo.attachments` | property | Partial | `TestInfo.attachments` ([source](crates/ferrite-e2e/src/runner.rs#L273)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.attach` | method | Partial | `TestInfo.attach` ([source](crates/ferrite-e2e/src/runner.rs#L248)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.column` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.config` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.duration` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.error` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.errors` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.expectedStatus` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.fail` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.file` | property | Partial | `TestInfo.file` ([source](crates/ferrite-e2e/src/runner.rs#L201)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.fixme` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.fn` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.tags` | property | Partial | `TestInfo.tags` ([source](crates/ferrite-e2e/src/runner.rs#L205)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.testId` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.line` | property | Partial | `TestInfo.line` ([source](crates/ferrite-e2e/src/runner.rs#L203)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.snapshotDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.outputDir` | property | Partial | `TestInfo.output_dir` ([source](crates/ferrite-e2e/src/runner.rs#L215)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.outputPath` | method | Partial | `TestInfo.output_path` ([source](crates/ferrite-e2e/src/runner.rs#L224)) | Attempt-specific artifact path; parent traversal and absolute paths rejected. No snapshot-path templates. |
| `TestInfo.parallelIndex` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.project` | property | Partial | `TestInfo.project` ([source](crates/ferrite-e2e/src/runner.rs#L217)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.repeatEachIndex` | property | Partial | `TestInfo.repeat_each_index` ([source](crates/ferrite-e2e/src/runner.rs#L211)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.retry` | property | Partial | `TestInfo.retry` ([source](crates/ferrite-e2e/src/runner.rs#L207)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.setTimeout` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.skip` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.slow` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.snapshotPath` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.snapshotSuffix` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.status` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.timeout` | property | Partial | `TestInfo.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L213)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.title` | property | Partial | `TestInfo.title` ([source](crates/ferrite-e2e/src/runner.rs#L199)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |
| `TestInfo.titlePath` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestInfo.workerIndex` | property | Partial | `TestInfo.worker_index` ([source](crates/ferrite-e2e/src/runner.rs#L209)) | Similar per-execution metadata; project is only an optional name and other metadata/mutation facilities differ. |

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
| `TestOptions.acceptDownloads` | property | Partial | `ContextOptions.accept_downloads` ([source](crates/ferrite-e2e/src/context.rs#L206)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.baseURL` | property | Partial | `E2eConfig.base_url` ([source](crates/ferrite-config/src/lib.rs#L414)) | Runner/launch config field; no automatic named-fixture/project options equivalence. |
| `TestOptions.browserName` | property | Partial | `Project.browser` ([source](crates/ferrite-e2e/src/runner.rs#L374)) | Projects select Chromium/Firefox independently; no WebKit backend. |
| `TestOptions.actionTimeout` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.bypassCSP` | property | Partial | `ContextOptions.bypass_csp` ([source](crates/ferrite-e2e/src/context.rs#L199)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.channel` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.clientCertificates` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.colorScheme` | property | Partial | `Page.emulate_media` ([source](crates/ferrite-e2e/src/page.rs#L3018)) | Chromium page-level manual emulation; no context/test option binding. |
| `TestOptions.connectOptions` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.contextOptions` | property | Partial | `Project.context_options` ([source](crates/ferrite-e2e/src/runner.rs#L387)) | Isolated context per attempt, runner defaults and per-project overrides; no full named-fixture test.use model. |
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
| `TestOptions.launchOptions` | property | Partial | `Project.launch_options` ([source](crates/ferrite-e2e/src/runner.rs#L380)) | Dedicated project launch settings; persistent profiles supported, no managed channels. |
| `TestOptions.locale` | property | Partial | `ContextOptions.locale` ([source](crates/ferrite-e2e/src/context.rs#L122)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.navigationTimeout` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.offline` | property | Partial | `ContextOptions.offline` ([source](crates/ferrite-e2e/src/context.rs#L150)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.permissions` | property | Partial | `ContextOptions.permissions` ([source](crates/ferrite-e2e/src/context.rs#L143)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.proxy` | property | Partial | `ContextOptions.proxy_server` ([source](crates/ferrite-e2e/src/context.rs#L39)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.reducedMotion` | property | Partial | `Page.emulate_media` ([source](crates/ferrite-e2e/src/page.rs#L3018)) | Chromium page-level manual emulation; no context/test option binding. |
| `TestOptions.reuseContext` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestOptions.screenshot` | property | Partial | `E2eConfig.screenshot` ([source](crates/ferrite-config/src/lib.rs#L428)) | Runner/launch config field; no automatic named-fixture/project options equivalence. |
| `TestOptions.storageState` | property | Partial | `ContextOptions.storage_state` ([source](crates/ferrite-e2e/src/context.rs#L220)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.testIdAttribute` | property | Partial | `set_test_id_attribute` ([source](crates/ferrite-e2e/src/locator.rs#L22)) | Process-global setter, not project/test-specific fixture option. |
| `TestOptions.timezoneId` | property | Partial | `ContextOptions.timezone_id` ([source](crates/ferrite-e2e/src/context.rs#L129)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.trace` | property | Partial | `BrowserContext.start_tracing` ([source](crates/ferrite-e2e/src/context.rs#L720)) | Manual custom JSON traces plus runner JSON; no trace mode policy or Trace Viewer compatibility. |
| `TestOptions.userAgent` | property | Partial | `ContextOptions.user_agent` ([source](crates/ferrite-e2e/src/context.rs#L115)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.video` | property | Partial | `E2eConfig.video` ([source](crates/ferrite-config/src/lib.rs#L430)) | Runner/launch config field; no automatic named-fixture/project options equivalence. |
| `TestOptions.viewport` | property | Partial | `ContextOptions.viewport` ([source](crates/ferrite-e2e/src/context.rs#L108)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |
| `TestOptions.serviceWorkers` | property | Partial | `ContextOptions.service_workers` ([source](crates/ferrite-e2e/src/context.rs#L227)) | Available through runner defaults and per-project context options; no suite-scoped test.use; Firefox restrictions and narrower options apply. |

## TestProject

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/test-api/class-testproject.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `TestProject.dependencies` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.expect` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.fullyParallel` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.grep` | property | Partial | `Project.grep` ([source](crates/ferrite-e2e/src/runner.rs#L403)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `TestProject.grepInvert` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.ignoreSnapshots` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.metadata` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.name` | property | Partial | `Project.name` ([source](crates/ferrite-e2e/src/runner.rs#L363)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `TestProject.snapshotDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.snapshotPathTemplate` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.outputDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.repeatEach` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.respectGitIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.retries` | property | Partial | `Project.retries` ([source](crates/ferrite-e2e/src/runner.rs#L410)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `TestProject.teardown` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.testDir` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.testIgnore` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.testMatch` | property | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `TestProject.timeout` | property | Partial | `Project.timeout` ([source](crates/ferrite-e2e/src/runner.rs#L417)) | Project name/filter/retry/timeout/context/browser overrides; no dependency graph or suite-scoped test.use. |
| `TestProject.use` | property | Partial | `Project.context_options` ([source](crates/ferrite-e2e/src/runner.rs#L387)) | Project-specific context settings; optional browser/launch overrides, no test.use suite hierarchy or project dependencies. |
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
| `Touchscreen.tap` | method | Partial | `Page.touchscreen_tap` ([source](crates/ferrite-e2e/src/page.rs#L2393)) | Coordinate tap available; no separate Touchscreen object. |

## Tracing

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-tracing.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Tracing.start` | method | Partial | `BrowserContext.start_tracing` ([source](crates/ferrite-e2e/src/context.rs#L720)) | Custom JSON actions/logs/requests; optional screenshots at Page.step only, no DOM/ARIA/source snapshots. |
| `Tracing.startChunk` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Tracing.startHar` | method | Partial | `Page.start_request_capture` ([source](crates/ferrite-e2e/src/page.rs#L3229)) | Page capture + save_har_with; no Tracing.startHar API or full browser/API-request tracing. |
| `Tracing.group` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Tracing.groupEnd` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Tracing.stop` | method | Partial | `BrowserContext.stop_tracing` ([source](crates/ferrite-e2e/src/context.rs#L733)) | Writes JSON, not a Trace Viewer-compatible zip archive. |
| `Tracing.stopChunk` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `Tracing.stopHar` | method | Partial | `Page.save_har_with` ([source](crates/ferrite-e2e/src/page.rs#L3269)) | HAR exporter; no Tracing.stopHar interface, update/rewrite mode or full timing/body coverage. |

## Video

[Pinned upstream definition](https://github.com/microsoft/playwright/blob/v1.63.0/docs/src/api/class-video.md)

| Playwright member | Kind | Status | Ferrite counterpart / evidence | Difference or limitation |
|---|---|---|---|---|
| `Video.delete` | method | Idiomatic | — | Caller deletes artifact through Rust filesystem operations; no Video object. |
| `Video.path` | method | Partial | `TestResult.video` ([source](crates/ferrite-e2e/src/report.rs#L53)) | Runner records an optional artifact path, not Page.video()/Video object. |
| `Video.saveAs` | method | Partial | `Page.stop_video` ([source](crates/ferrite-e2e/src/page.rs#L3893)) | Stop recording to a path; no independently awaitable Video handle. |

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
| `WebSocket.close` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1575)) | PageEvent::WebSocket direction Closed; Chromium-only observation without a WebSocket object. |
| `WebSocket.frameReceived` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1575)) | PageEvent::WebSocket direction Received; Chromium-only observation without a WebSocket object. |
| `WebSocket.frameSent` | event | Partial | `Page.subscribe` ([source](crates/ferrite-e2e/src/page.rs#L1575)) | PageEvent::WebSocket direction Sent; Chromium-only observation without a WebSocket object. |
| `WebSocket.socketError` | event | Missing | — | No matching public event variant/emitter; Page.subscribe exposes only the documented PageEvent variants. |
| `WebSocket.isClosed` | method | Missing | — | No dedicated public equivalent found in exported ferrite-e2e APIs. Raw protocol calls/general evaluate are not counted as implemented API parity. |
| `WebSocket.url` | method | Partial | `WebSocketEvent.url` ([source](crates/ferrite-e2e/src/page.rs#L256)) | Captured socket URL, Chromium only. |
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
| `WebStorage.getItem` | method | Partial | `Page.local_storage_get` ([source](crates/ferrite-e2e/src/page.rs#L2448)) | Page helper methods for local and session storage; no WebStorage object. |
| `WebStorage.setItem` | method | Partial | `Page.local_storage_set` ([source](crates/ferrite-e2e/src/page.rs#L2453)) | Page helper methods for local and session storage; no WebStorage object. |
| `WebStorage.removeItem` | method | Partial | `Page.local_storage_remove` ([source](crates/ferrite-e2e/src/page.rs#L2458)) | Page helper methods for local and session storage; no WebStorage object. |
| `WebStorage.clear` | method | Partial | `Page.local_storage_clear` ([source](crates/ferrite-e2e/src/page.rs#L2467)) | Page helper methods for local and session storage; no WebStorage object. |

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
