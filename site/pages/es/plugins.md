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

## Referencia de hooks

Además de `resolve_id` / `load` / `transform` /
`transform_index_html`, los plugins tienen los hooks de paridad
Vite/Rollup:

| Hook | Cuándo corre |
|---|---|
| `options` | Muta las opciones de entrada (`BundleOptions`: entries, treeshake, minify, sourcemap, scope-hoist) antes del build. |
| `output_options` | Muta los patrones de salida (`OutputOptions`: `chunk_pattern`, `css_pattern`, `asset_pattern` con `[name]` / `[hash]` / `[ext]`) antes de nombrar chunks. |
| `resolve_dynamic_import` | Resuelve un `import()` dinámico; si todos devuelven `None` se usa `resolve_id`. |
| `should_transform_cached_module` | `Some(true)` fuerza re-transformar un módulo cacheado; `None` / `Some(false)` conserva la caché. |
| `watch_change` | Observa un evento del watcher (`WatchEvent { path, kind }`: create / modify / remove). |
| `resolve_file_url` | Mapea un archivo emitido a su URL pública; gana el primer `Some` (también reescribe el shim `?url` de dev). |
| `hot_update` | `hotUpdate` de Vite 6; corre antes del legacy `handle_hot_update`, gana el primer `Some` entre ambos. |
| `render_start` | Empieza el render: entries conocidos, chunks aún sin planear. |
| `render_chunk` | Reemplaza el código de un chunk pre-reescritura (el reemplazo es lo que se hashea y reescribe; los hooks ven specifiers de dev). |
| `augment_chunk_hash` | Aporta input extra al hash del chunk. |
| `banner` / `intro` / `outro` / `footer` | Envuelven un chunk; entran en el hash y se aplican post-reescritura. |
| `build_end` / `close_bundle` | Fin del build; `build_end` trae `Some(mensaje)` si falla y `close_bundle` corre igual. |

Ver [Pipeline de build](pipeline) para el orden de la fase de render.

## Hooks de preview

`ferrite preview` corre `configResolved`, luego el legacy
`configure_preview_server` y el nuevo `configure_preview` (implementa
este último). El `PreviewControl` junta:

- headers extra de respuesta (`add_header`),
- mounts estáticos que se consultan antes del out dir
  (`add_mount("/docs", dir)`),
- reglas de proxy (`add_proxy("/api", "http://localhost:3000")`, gana
  el prefijo más largo, precargadas desde `[server] proxy`).

## Handles del servidor

Los hooks del servidor dev reciben un `ServerControl` con los
equivalentes de Vite: `module_graph()` (`server.moduleGraph`),
`local_addr()`, `server_urls()` / `print_urls()`, `hmr_clients()` /
`send_full_reload(path)` y `watcher_alive()` / `watcher_add(path)`
(`server.watcher.add`).

## Escribe el tuyo

Los plugins nativos nuevos van en un crate del workspace, se
re-exportan desde la fachada `ferrite` y se suman a `default_plugins`
en el CLI para que dev y build los tomen. Mantén los hooks totales:
devuelve `None` para ids que no son tuyos, y falla con un
`FerriteError::Build` en voz alta en vez de adivinar.
