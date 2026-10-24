# Vite replacement acceptance target

Ferrite remains experimental. A feature has implementation evidence only for the
versions, hosts and fixtures actually executed; this is not a claim that arbitrary
Vite applications or plugins can migrate unchanged.

| Area | Current evidence | Remaining replacement gate |
|---|---|---|
| React / Vue / Svelte | Generated JS/TS SSR hydration and nested component edits in Chromium/Firefox; official compilers/renderers | Broader framework/version/component libraries; full checker integration |
| Development HMR | React state preservation, official Svelte resets, Vue template/style rerender and script reload; nested props/lists/computed values and syntax recovery | Wider hook signatures, resources, async components and cross-platform matrix |
| SSR hosting | Buffered/streamed rendering, request metadata, route-specific lazy entries, preview, static generation and embedded standalone | Islands, independent shells/runtime per entry, external/native contracts |
| Production output | Executed lazy CSS timing/retry, shared chunks, cascade order, assets and final native maps with minify/scope-hoist combinations | Broader cyclic/worker/Wasm closure, render-hook maps and plugin output validation |
| Packages | Portable contextual lock graph, peers, optional dependencies and workspace links | Remaining package-map URL semantics and CJS/native interoperability |
| Plugins | Explicit supported hook subset and bounded external host | Wider context/lifecycle parity; unsupported hooks must remain explicit |
| Distribution | Executed Linux browser/standalone acceptance with Node-free serving | Windows/macOS isolation, release matrix and measured performance budgets |

Follow-up drafts should keep framework/SSR changes separate from production
correctness so each behavior and its regression evidence can be reviewed. The
consolidated remaining checklist is in [COMPLETION_PLAN.md](COMPLETION_PLAN.md).
No paid CI is required to run the local gates in `scripts/acceptance.py`.
