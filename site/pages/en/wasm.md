---
title: WASM
order: 13
---

# WASM

Ferrite integrates Rust/WASM two ways (`ferrite-wasm`, spec §45, §74):
prebuilt `.wasm` imports, and `rust:<crate>` packages that build on demand.

## `.wasm` imports

```js
import wasm from "./add.wasm?wasm";
const instance = await WebAssembly.instantiateStreaming(fetch(wasm));
```

`?wasm` builds the module as a WASM entry: hashed output, glue served
alongside, dev and build agreeing on URLs. See [Assets](assets) for the
other query suffixes.

## `rust:` packages

```js
import { hash } from "rust:my_crypto";
```

Resolution runs a real pipeline, not a stub:

```text
rust:my_crypto
  → Cargo metadata
  → cargo build --target wasm32-unknown-unknown
  → wasm-bindgen glue
  → virtual JS entry
```

`ferrite_wasm::ensure_glue` re-runs `cargo build` + `wasm-bindgen` only
when crate sources are newer than the glue (mtime-based). Missing
toolchains fail with a loud hint naming the missing piece, never
silently. Internally the id is `\0rust:<name>`; builds emit the glue as
a hashed chunk like any other module.

## Dev vs build

- **Dev**: glue builds lazily on first import; rebuilds when sources
  change (watcher-aware).
- **Build**: glue is ensured before chunking, so the manifest and the
  standalone binary both include the final hashed files.

## Example

`examples/rust-wasm` pairs a minimal UI with a `rust:` import end to
end: dev serves it live, `ferrite build` hashes it, and `--standalone`
embeds it. If your toolchain is missing `wasm32-unknown-unknown` or
`wasm-bindgen`, the error tells you the exact install command.
