---
title: Plugins
order: 14
---

# Plugins

Los plugins implementan el trait `ferrite_plugin::Plugin`: `resolve_id`,
`load`, `transform`, `transform_index_html` y hooks de ciclo de vida de
build/dev. Los plugins nativos Rust vienen con el CLI; los plugins ESM
extranjeros pueden correr en JS embebido (nivel 2) o Node real (nivel 3).

## Plugins integrados

| Plugin              | Qué hace                                               |
|---------------------|--------------------------------------------------------|
| `ferrite:markdown`  | `.md` se vuelve módulo JS que exporta HTML + meta      |
| `ferrite:tailwind`  | `ferrite:tailwind.css` virtual compilado del contenido |
| `ferrite:react-refresh` | Preamble + footers de React Refresh solo en dev    |
| Vue / Svelte        | Splitting SFC experimental (bloques script/style)      |
| Rust WASM           | Orquestación de `cargo build` para imports `.wasm`     |
| Raw text            | Imports con sufijo `?raw`                              |

## Módulos Markdown

```js
import page, { title, headings } from "./guide.md";

page.html; // string HTML renderizado
page.title; // título del frontmatter, o primer h1
page.headings; // [{ level, text, id }] para tablas de contenido
page.order; // orden del frontmatter para sidebars
```

El frontmatter es un bloque `---` con `title:` y `order:`.
GitHub-flavored Markdown (tablas, footnotes, tachado, task lists)
está activado, y cada encabezado recibe un anchor slugificado.

## Módulos de CSS de utilidades

```js
import "ferrite:tailwind.css";
```

Al cargar, el plugin escanea la raíz del proyecto (HTML, JS/TS,
Markdown, Vue/Svelte/Astro; nunca `node_modules`, `dist`, `target`,
`vendor`) y compila las utilidades encontradas con el compilador
vendored `tailwind-rs`. Importar el specifier es el único opt-in; los
proyectos que nunca lo importan no pagan nada.

## Escribe el tuyo

Los plugins nativos nuevos van en un crate del workspace, se
re-exportan desde la fachada `ferrite` y se suman a `default_plugins`
en el CLI para que dev y build los tomen. Mantén los hooks totales:
devuelve `None` para ids que no son tuyos, y falla con un
`FerriteError::Build` en voz alta en vez de adivinar.
