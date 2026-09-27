# rust-wasm example

Demonstrates both WASM paths (spec §45):

1. `import init from "./module.wasm?wasm"` — served by the asset pipeline
   with a fetch+instantiate loader shim.
2. `import { hash } from "rust:my_crypto"` — resolved by
   `ferrite_wasm::RustWasmPlugin` to prebuilt `wasm-bindgen` glue.

Build the glue first:

```bash
cargo build -p my_crypto --target wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/debug/my_crypto.wasm \
  --out-dir ./wasm-glue --target web
```

then register `RustWasmPlugin::new(Some("./wasm-glue".into()))` via
`ferrite::Config::plugin`. Without glue, the import fails with build
instructions instead of a cryptic error.
