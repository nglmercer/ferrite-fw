---
title: Assets
order: 7
---

# Assets

Los assets estáticos usan sufijos de query (`ferrite-assets`, spec §30).
El sufijo elige la representación; el pipeline hashea, inlinea o genera
el shim correspondiente.

## Queries

```js
import url from "./logo.svg";        // URL hasheada (path en dev / hash en build)
import text from "./doc.txt?raw";    // texto del archivo como string
import url2 from "./a.bin?url";      // siempre URL, nunca inline
import data from "./icon.svg?inline";// siempre data: URL
import Worker from "./w.js?worker";  // se compila como worker entry
import wasm from "./add.wasm?wasm";  // se compila como entry WASM
```

| Sufijo | Dev | Build |
|---|---|---|
| (ninguno) | path servido | archivo con hash (`logo.4ad83f.svg`) |
| `?raw` | texto inline | texto inline |
| `?url` | path servido | URL de archivo hasheado |
| `?inline` | data: URL | data: URL |
| `?worker` | shim de worker | chunk de worker separado |
| `?wasm` | loader WASM | WASM hasheado + glue (ver [WASM](wasm)) |

## Cómo resuelven los imports

Los imports de assets se reescriben a `…?asset-shim`: un módulo JS
mínimo que exporta la URL (o texto/data). Pedir la URL plana sirve bytes
crudos con MIME derivado del contenido. Así `import url` funciona bajo
ESM nativo en dev y con salida hasheada en build sin config de loaders.

## Hashing y caché

La salida de build incrusta un hash de contenido en el nombre, así que
los assets son inmutables y cacheables para siempre. El
[manifiesto](pipeline) mapea ids fuente a salidas hasheadas; los tags
`<link>`/`<script>` del HTML y los `url()` del CSS se reescriben igual.

## Workers

Los entries `?worker` se compilan como chunks separados con su propio
closure de imports. El import da un constructor/URL para
`new Worker(url, { type: "module" })`. Las deps compartidas se
deduplican en el mismo grafo de chunks que los entries principales.
