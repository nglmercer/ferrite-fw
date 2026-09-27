---
title: Assets
order: 7
---

# Assets

Static assets use query suffixes (`ferrite-assets`, spec §30). The
suffix picks the representation; the pipeline hashes, inlines, or shims
accordingly.

## Queries

```js
import url from "./logo.svg";        // hashed URL (dev path / build hash)
import text from "./doc.txt?raw";    // file text as a string
import url2 from "./a.bin?url";      // always a URL, never inlined
import data from "./icon.svg?inline";// always a data: URL
import Worker from "./w.js?worker";  // built as a worker entry
import wasm from "./add.wasm?wasm";  // built as a WASM entry
```

| Suffix | Dev | Build |
|---|---|---|
| (none) | served path | content-hashed file (`logo.4ad83f.svg`) |
| `?raw` | inline text | inline text |
| `?url` | served path | hashed file URL |
| `?inline` | data: URL | data: URL |
| `?worker` | worker shim | separate worker chunk |
| `?wasm` | WASM loader | hashed WASM + glue (see [WASM](wasm)) |

## How imports resolve

Asset imports rewrite to `…?asset-shim`: a tiny JS module exporting the
URL (or text/data). Requesting the plain URL serves raw bytes with a
content-derived MIME type. This keeps `import url` working under native
ESM in dev and hashed output in build with no loader config.

## Hashing and caching

Build output embeds a content hash in the filename, so assets are
immutable and cacheable forever. The [manifest](pipeline) maps source
ids to hashed outputs; HTML `<link>`/`<script>` tags and CSS `url()`
references are rewritten to match.

## Workers

`?worker` entries are built as separate chunks with their own import
closure. The import yields a constructor/URL you pass to
`new Worker(url, { type: "module" })`. Shared deps are deduplicated
through the same chunk graph as the main entries.
