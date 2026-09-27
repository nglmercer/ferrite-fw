---
title: CSS
order: 6
---

# CSS

Ferrite maneja imports `.css`, CSS modules, reescritura de
`@import`/`url()`, inyección en dev, extracción en producción y
minificación (`ferrite-css`, spec §29).

## Imports

```js
import "./app.css";            // side-effect: inyecta (dev) / extrae (build)
import styles from "./a.module.css"; // CSS modules: mapa de clases
```

- Los `import "./a.css"` desnudos se eliminan del JS de producción;
  los módulos solo-CSS no producen chunk JS.
- El orden `@import` se preserva en la extracción.
- Las referencias `url()` se rebasan a la salida; los assets hasheados
  llevan hash de contenido (`logo.4ad83f.svg`).

## CSS modules

Los `.module.css` exportan un mapa de clases; dev y build usan el mismo
esquema de nombres para que SSR y cliente coincidan. El `.css` plano
conserva semántica global.

```css
/* button.module.css */
.primary { color: orange; }
```

```js
import styles from "./button.module.css";
el.className = styles.primary;
```

## Dev vs build

- **Dev**: los estilos se inyectan vía un wrapper JS; los edits se
  actualizan sin recargar. Los `<link>` van a `…?direct` para que el
  navegador reciba CSS, no el wrapper.
- **Build**: un `.css` hasheado por módulo de estilos, inyectado como
  tags `<link>` en el HTML en orden `@import`. La minificación corre
  tras extraer; los source maps encadenan el mapa de transform.

## CSS de utilidades (Tailwind)

```js
import "ferrite:tailwind.css";
```

El plugin `ferrite:tailwind` compila este módulo virtual escaneando el
contenido (HTML, JS/TS, Markdown, Vue/Svelte/Astro — nunca
`node_modules`, `dist`, `target`, `vendor`) con el compilador vendored
`tailwind-rs`. Importar el specifier es el único opt-in; quien nunca lo
importa no paga nada. Los colores de este sitio viven en
`site/src/docs.css` encima. Ver [Tailwind vendor](tailwind-vendor).

## Minify

El minify de producción está on por default (`[build] minify = true`).
Emite su propio mapa y lo encadena con el de transform
(`ferrite_transform::chain_source_maps`), así que lo minificado sigue
apuntando a las fuentes. Las posiciones irresolubles pasan sin fuente
en vez de romper el build.
