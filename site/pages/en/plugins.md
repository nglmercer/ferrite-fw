---
title: Plugins
order: 14
---

# Plugins

Plugins implement the `ferrite_plugin::Plugin` trait: `resolve_id`,
`load`, `transform`, `transform_index_html`, and build/dev lifecycle
hooks. Native Rust plugins ship with the CLI; foreign ESM plugins can
run in tier-2 embedded JS or tier-3 real Node.

## Built-in plugins

| Plugin              | What it does                                           |
|---------------------|--------------------------------------------------------|
| `ferrite:markdown`  | `.md` files become JS modules exporting HTML + meta    |
| `ferrite:tailwind`  | Virtual `ferrite:tailwind.css` compiled from content   |
| `ferrite:react-refresh` | Dev-only React Refresh preamble + footers          |
| Vue / Svelte        | Experimental SFC splitting (script/style blocks)       |
| Rust WASM           | `cargo build` orchestration for `.wasm` imports        |
| Raw text            | `?raw` suffixed imports                                |

## Markdown modules

```js
import page, { title, headings } from "./guide.md";

page.html; // rendered HTML string
page.title; // frontmatter title, or first h1
page.headings; // [{ level, text, id }] for tables of contents
page.order; // frontmatter order for sidebar sorting
```

Frontmatter is a `---` block supporting `title:` and `order:`.
GitHub-flavored Markdown (tables, footnotes, strikethrough, task
lists) is enabled, and every heading gets a slugified anchor id.

## Utility CSS modules

```js
import "ferrite:tailwind.css";
```

On load, the plugin scans the project root (HTML, JS/TS, Markdown,
Vue/Svelte/Astro; never `node_modules`, `dist`, `target`, `vendor`)
and compiles the found utilities with the vendored `tailwind-rs`
compiler. Importing the specifier is the only opt-in; projects that
never import it pay nothing.

## Hook reference

Beyond `resolve_id` / `load` / `transform` /
`transform_index_html`, plugins get the Vite/Rollup parity hooks:

| Hook | When it runs |
|---|---|
| `options` | Mutate input options (`BundleOptions`: entries, treeshake, minify, sourcemap, scope-hoist) before the build starts. |
| `output_options` | Mutate output patterns (`OutputOptions`: `chunk_pattern`, `css_pattern`, `asset_pattern` with `[name]` / `[hash]` / `[ext]`) before chunks are named. |
| `resolve_dynamic_import` | Resolve a dynamic `import()` specifier; falls back to `resolve_id` when every hook returns `None`. |
| `should_transform_cached_module` | `Some(true)` forces a re-transform of a cached module; `None` / `Some(false)` keeps the cache. |
| `watch_change` | Observe a file-watcher event (`WatchEvent { path, kind }`: create / modify / remove). |
| `resolve_file_url` | Map an emitted file to its public URL; first `Some` wins (rewrites the dev `?url` shim too). |
| `hot_update` | Vite 6 `hotUpdate`; runs before the legacy `handle_hot_update`, first `Some` across both wins. |
| `render_start` | Rendering starts: entries known, chunks not yet planned. |
| `render_chunk` | Replace one chunk's code pre-rewrite (the replacement is what gets hashed and rewritten; hooks see dev-URL specifiers). |
| `augment_chunk_hash` | Contribute extra chunk-hash input. |
| `banner` / `intro` / `outro` / `footer` | Wrap a chunk; folded into the hash, applied post-rewrite. |
| `build_end` / `close_bundle` | End of build; `build_end` carries `Some(message)` on failure and `close_bundle` still runs. |

See [Build pipeline](pipeline) for the render-phase order.

## Preview hooks

`ferrite preview` runs `configResolved`, then the legacy
`configure_preview_server` and the newer `configure_preview`
(implement the latter). The `PreviewControl` collects:

- extra response headers (`add_header`),
- static mounts checked before the output dir
  (`add_mount("/docs", dir)`),
- proxy rules (`add_proxy("/api", "http://localhost:3000")`, longest
  prefix wins, seeded from `[server] proxy`).

## Server handles

Dev-server hooks receive a `ServerControl` with the Vite
equivalents: `module_graph()` (`server.moduleGraph`),
`local_addr()`, `server_urls()` / `print_urls()`, `hmr_clients()` /
`send_full_reload(path)`, and `watcher_alive()` /
`watcher_add(path)` (`server.watcher.add`).

## Writing your own

New native plugins go in a workspace crate, get re-exported from the
`ferrite` facade, and join `default_plugins` in the CLI so dev and
build both pick them up. Keep hooks total: return `None` for ids you
do not own, and fail with a loud `FerriteError::Build` instead of
guessing.
