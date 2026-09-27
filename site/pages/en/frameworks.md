---
title: Frameworks
order: 12
---

# Frameworks

`ferrite-frameworks` ships a React plugin plus experimental Vue/Svelte
single-file-component support. All three are enabled by default in the
CLI; each is a plain `Plugin`, so programmatic users can drop any of them.

## React

```toml
[react]
refresh = true
runtime = "automatic" # or "classic"
```

- Dev-only Refresh preamble + per-module footers: component edits
  hot-swap statefully; non-component edits fall back to HMR.
- `detect_components` finds components per file; `refresh_footer`
  registers them with the React Refresh runtime.
- `runtime = "automatic"` uses the JSX transform; `classic` keeps
  `React.createElement`. The transform engine (Oxc/SWC) handles both.
- Production builds strip all Refresh code — zero runtime cost.

```bash
ferrite add react react-dom
```

## Vue (experimental)

`VuePlugin` splits `.vue` files into script/style blocks served as
virtual modules. `<script setup>` is recognized; the template block
currently compiles to an explicit stub (`template_stub`) — full template
compilation is roadmap, and the stub says so loudly at the call site.

## Svelte (experimental)

`SveltePlugin` splits `.svelte` files the same way (`split_svelte`,
markup extraction, `markup_stub`). Script reactivity compiles through
the normal JS pipeline; template compilation is an explicit stub.

## SSR + islands

All three frameworks render through the same [SSR](ssr) adapters and can
hydrate as [islands](ssr): server-rendered HTML plus a per-island client
closure. Framework choice never changes the dev server, manifest, or
standalone packaging.

## Examples

`examples/react` is the reference app (dev, HMR, Refresh, build).
`examples/vanilla-ts` shows the zero-framework baseline; `examples/ssr`
shows server rendering; `examples/rust-wasm` pairs a framework UI with
[WASM](wasm).
