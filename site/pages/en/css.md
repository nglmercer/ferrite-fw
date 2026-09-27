---
title: CSS
order: 6
---

# CSS

Ferrite handles `.css` imports, CSS modules, `@import`/`url()`
rewriting, dev-time injection, production extraction, and minification
(`ferrite-css`, spec §29).

## Imports

```js
import "./app.css";            // side-effect: inject (dev) / extract (build)
import styles from "./a.module.css"; // CSS modules: class map
```

- Bare `import "./a.css"` statements are stripped from production JS;
  CSS-only modules produce no JS chunk.
- `@import` order is preserved through extraction.
- `url()` references are rebased onto the output; hashed assets get
  content hashes (`logo.4ad83f.svg`).

## CSS modules

`.module.css` files export a class-name map; dev and build use the same
scoped-name scheme so SSR and client agree. Plain `.css` keeps global
semantics.

```css
/* button.module.css */
.primary { color: orange; }
```

```js
import styles from "./button.module.css";
el.className = styles.primary;
```

## Dev vs build

- **Dev**: styles inject via a JS wrapper; edits hot-update without a
  page reload. Stylesheet `<link>`s rewrite to `…?direct` so the browser
  gets CSS, not the wrapper.
- **Build**: one hashed `.css` file per stylesheet module, injected as
  `<link>` tags into the built HTML in `@import` order. Minification
  runs after extraction; source maps chain through the transform map.

## Utility CSS (Tailwind)

```js
import "ferrite:tailwind.css";
```

The `ferrite:tailwind` plugin compiles this virtual module by scanning
project content (HTML, JS/TS, Markdown, Vue/Svelte/Astro — never
`node_modules`, `dist`, `target`, `vendor`) with the vendored
`tailwind-rs` compiler. Importing the specifier is the only opt-in;
projects that never import it pay nothing. This site's colors live in
`site/src/docs.css` on top. See [Tailwind vendor](tailwind-vendor).

## Minify

Production minify is on by default (`[build] minify = true`). It emits
its own map and chains it through the transform map
(`ferrite_transform::chain_source_maps`), so minified output still
points at original sources. Unresolvable positions pass through
sourceless instead of failing the build.
