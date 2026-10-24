# SSR documents and selective hydration

This experimental profile combines route-specific renderer graphs, isolated server
style closures, independent HTML shells and framework islands. It uses the same
metadata in dev, preview, prerender and standalone output.

## Documents

```toml
base = "/app/"

[ssr]
entry = "src/entry-server.ts"
shell = "layouts/main.html" # optional; index.html is the default

[[ssr.entries]]
route = "/admin/*"
entry = "src/admin-server.ts"
shell = "layouts/admin.html"
```

Each shell must contain exactly one `<!--ssr-outlet-->`. Its module scripts are
built as normal client entries. A route without its own shell inherits `ssr.shell`
or the default document. Exact paths win over the longest prefix, then `*`.
The existing outer `ssr.routes` allowlist and reserved-path protection still apply.

Development selects only the matching source graph and revalidates that graph,
its styles and its explicit shell on each request. All documents share one
configured runtime and admission limits. Production writes renderer artifact v2
with a graph, ordered stylesheet closure and transformed HTML for each document.
Preview and standalone consume this emitted metadata without project source.
Artifact v1 and legacy manifest-only single-entry output remain readable.

Only the selected document's server CSS is linked. Dependencies reachable inside
that renderer, including its asynchronous components, retain their CSS closure.
A shell's own client CSS also remains intentional; use an independent shell when
its client application must differ from another route. Unselected server graphs
are not evaluated. Builds still compile all configured entries.

The existing streaming contract applies to every renderer when `ssr.streaming`
is enabled. Document selection preserves status/headers, HEAD behavior, shell
prefix/suffix handling and process cancellation. See [SSR_STREAMING.md](SSR_STREAMING.md).

## Islands

Register client modules whose only required export is `hydrate(element, props)`:

```toml
[ssr.islands]
Counter = "src/counter-client.ts"
```

Render independent roots with server HTML and JSON props. Rust applications can
use `island_tag_with_strategy(&Island { name, props, html }, "visible")`; JavaScript
renderers can emit the equivalent attributes:

```html
<div data-ferrite-island="Counter"
     data-ferrite-hydrate="visible"
     data-props="{&quot;start&quot;:2}">
  <button>count: 2</button>
</div>
```

Attribute values must be escaped for HTML, including ampersands before quotes.
The Rust helper does that. Server HTML is trusted renderer output.

Ferrite injects a registry bootstrap into document shells. Literal imports make
hydrators, framework code, styles and assets discoverable to the production
bundler. A registry declaration alone does not load or hydrate that component.
The client module must use its framework's actual hydration API:

```js
import React from 'react';
import { hydrateRoot } from 'react-dom/client';
import Counter from './Counter';
export function hydrate(element, props) {
  const root = hydrateRoot(element, React.createElement(Counter, props));
  return () => root.unmount();
}
```

Vue uses `createSSRApp(Component, props).mount(element)` and Svelte uses
`hydrate(Component, { target: element, props })`. The server must render the same
component and initial props through the corresponding official server API.
A hydrator can return a cleanup function, including one returning a promise.

| Strategy | Trigger |
|---|---|
| `load` (default) | Bootstrap runs |
| `idle` | Browser idle callback, or timer fallback |
| `visible` | Intersection, with eager fallback when the API is absent |
| `interaction` | Focus or click; the triggering click is replayed after hydration |
| `manual` | Dispatch a bubbling `ferrite:hydrate` event on the root |

Applications can also import `hydrateIsland` from `ferrite:islands`. Concurrent
calls share one hydration attempt. Newly inserted roots are scanned. Removed
roots cancel pending scheduling and call cleanup once. Failures keep the root,
set its state to `error`, dispatch `ferrite:island-error`, and allow explicit
retry. Successful roots emit `ferrite:island-ready`. The `data-ferrite-state`
attribute reports pending/loading/ready/error; runtime ownership does not trust a
copied ready attribute.

Islands must have independent DOM ownership. Nested island roots fail explicitly;
use framework components within an island for nested trees. This is selective
hydration through user-provided adapters, not automatic extraction of arbitrary
components, React Server Components or a full framework router. Registry changes
require restarting dev; registered component/source edits use the existing HMR
pipeline. Native external/addon and full Web Streams limitations still apply.

## Acceptance

```bash
python3 scripts/acceptance.py --profile ssr
```

Requires Cargo, Node for official Vue/Svelte compilation, and explicitly installed
Chromium/Firefox. Serving built and standalone output requires no Node executable.
The fixtures execute React/Vue/Svelte hydration, interaction/visibility/idle/manual
triggers, retry, deduplication, cleanup, dynamic insertion and SSR node identity in
both browsers. Dev, source-hidden preview, static output and an empty-PATH
standalone are exercised. Separate fixtures cover per-route CSS/shell isolation,
streaming and legacy artifacts. These are bounded Linux acceptance cases, not a
claim of all framework versions or cross-platform release parity.
