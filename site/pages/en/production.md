---
title: Production
order: 16
---

# Production

`ferrite build` turns the module graph into hashed assets plus a
manifest. Every production feature below is covered by the workspace
test suite and by this site's own build.

## Pipeline

1. **Resolve + load** through plugin hooks (Markdown, SFCs, WASM glue).
2. **Transform** with Oxc (or the opt-in SWC backend, `--features swc`).
3. **Tree-shake** unused exports per statement, then re-minify.
4. **Chunk or hoist**: hashed chunks by default, or `--scope-hoist`
   to concatenate each entry closure into one file.
5. **Extract CSS**: one hashed `.css` per stylesheet module, injected
   as `<link>` tags into the built HTML in `@import` order.
6. **Emit manifest** mapping source ids to hashed outputs.

## Single binary

```bash
ferrite build --standalone
./dist/ferrite-standalone # serves the embedded site, no dist/ needed
```

The standalone scaffold embeds `dist/` with `include_bytes!` + gzip
and serves it over HTTP. Add `--target` to cross-compile (musl is
verified in CI-style ignored tests).

## Verify before you ship

```bash
ferrite build site --standalone
PORT=8080 ./site/dist/ferrite-site
curl -s localhost:8080/ | head -c 200
```

A green check is: HTTP 200, the built `<script>` and `<link>` tags
present, hashed asset URLs resolving, and no sibling `dist/` needed
next to the binary. This repository's own proof is this docs site:
`site/` builds and serves through exactly this path.
