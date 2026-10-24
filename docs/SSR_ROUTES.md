# Route-specific SSR entries

Assign base-relative paths to project-relative JavaScript/TypeScript renderers:

```toml
base = "/app/"

[ssr]
entry = "src/entry-server.ts" # optional fallback; conventional discovery also works

[[ssr.entries]]
route = "/admin/*"
entry = "src/admin-server.ts"

[[ssr.entries]]
route = "/admin/report.json"
entry = "src/report-server.ts"
```

Exact paths win over prefixes. The longest matching prefix wins over shorter
prefixes; `*` is last. `/admin/*` includes `/admin` and its descendants, but does
not match `/administrator`. Route configuration order does not change selection.
The base is removed only for selection; `render(url, request)` receives the
original URL, query, method, headers and body. Each module exports `render` using
the existing response contract. Status and headers are preserved.

Unmatched requests use `ssr.entry` or a conventionally discovered server entry.
Without either, the dispatcher returns 404. The optional `ssr.routes` list is an
outer allowlist and still applies to every renderer. Reserved asset/internal
paths cannot reach a renderer, including when a handler declares `*`. Duplicate
patterns, malformed patterns, unsafe entry paths and missing entry files fail
before traffic is served.

Ferrite generates an internal, SSR-only module containing literal dynamic
imports. Every entry and dependency is compiled and included in the executable
graph, while only the selected module is evaluated for a request. Thus syntax
errors can fail compilation even in an unselected entry; an unselected module's
top-level execution error does not affect a different route. The development
adapter revalidates sources on each request, including nested route dependencies.

The same dispatcher is used by dev, production preview, prerender and embedded
standalone artifacts. A deployment consumes emitted graphs rather than importing
project source. Existing single-entry artifacts and configs remain supported.
`ssr.streaming` is a global contract: every selected renderer must return a stream
when it is enabled. The generated no-match response uses that same contract.

Current CSS handling retains the existing eager graph closure. Per-route CSS
isolation and client lazy CSS timing are production-correctness follow-ups; route
entries do not claim separate document shells or independently configured runtimes.
