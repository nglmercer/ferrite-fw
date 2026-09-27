---
title: WASM
order: 13
---

# WASM

Ferrite integra Rust/WASM de dos formas (`ferrite-wasm`, spec §45, §74):
imports `.wasm` precompilados y paquetes `rust:<crate>` que compilan a demanda.

## Imports `.wasm`

```js
import wasm from "./add.wasm?wasm";
const instance = await WebAssembly.instantiateStreaming(fetch(wasm));
```

`?wasm` compila el módulo como entry WASM: salida hasheada, glue servido
al lado, dev y build de acuerdo en URLs. Ver [Assets](assets) para los
demás sufijos.

## Paquetes `rust:`

```js
import { hash } from "rust:my_crypto";
```

La resolución corre un pipeline real, no un stub:

```text
rust:my_crypto
  → metadata de Cargo
  → cargo build --target wasm32-unknown-unknown
  → glue de wasm-bindgen
  → entry JS virtual
```

`ferrite_wasm::ensure_glue` re-corre `cargo build` + `wasm-bindgen` solo
cuando las fuentes son más nuevas que el glue (por mtime). Los
toolchains ausentes fallan con un hint ruidoso que nombra la pieza,
nunca en silencio. Internamente el id es `\0rust:<name>`; los builds
emiten el glue como un chunk hasheado más.

## Dev vs build

- **Dev**: el glue compila lazy al primer import; recompila cuando
  cambian las fuentes (el watcher lo ve).
- **Build**: el glue se asegura antes de trocear, así que el manifiesto
  y el binario standalone incluyen los archivos finales.

## Ejemplo

`examples/rust-wasm` une una UI mínima con un import `rust:` de punta a
punta: dev lo sirve en vivo, `ferrite build` lo hashea y `--standalone`
lo embebe. Si te faltan `wasm32-unknown-unknown` o `wasm-bindgen`, el
error te dice el comando exacto de instalación.
