---
title: Plugins
order: 5
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

## Writing your own

New native plugins go in a workspace crate, get re-exported from the
`ferrite` facade, and join `default_plugins` in the CLI so dev and
build both pick them up. Keep hooks total: return `None` for ids you
do not own, and fail with a loud `FerriteError::Build` instead of
guessing.
