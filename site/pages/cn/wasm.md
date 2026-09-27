---
title: WASM
order: 13
---

# WASM

Ferrite 两种方式集成 Rust/WASM（`ferrite-wasm`，spec §45、§74）：
预构建 `.wasm` 导入，与按需构建的 `rust:<crate>` 包。

## `.wasm` 导入

```js
import wasm from "./add.wasm?wasm";
const instance = await WebAssembly.instantiateStreaming(fetch(wasm));
```

`?wasm` 把模块作为 WASM 入口构建：哈希输出、glue 伴随 serve，
dev 与构建 URL 一致。其他后缀见[资源](assets)。

## `rust:` 包

```js
import { hash } from "rust:my_crypto";
```

解析跑的是真流水线，不是 stub：

```text
rust:my_crypto
  → Cargo 元数据
  → cargo build --target wasm32-unknown-unknown
  → wasm-bindgen glue
  → 虚拟 JS 入口
```

`ferrite_wasm::ensure_glue` 只在 crate 源码比 glue 新时重跑
`cargo build` + `wasm-bindgen`（按 mtime）。缺工具链则明确报错并
点名缺件，永不静默。内部 id 为 `\0rust:<name>`；构建把 glue 作
哈希 chunk 与他模块一并输出。

## Dev 与构建

- **Dev**：glue 首个导入时懒构建；源码变化时重建
  （watcher 感知）。
- **构建**：分块前确保 glue，清单与 standalone 二进制都包含最终
  哈希文件。

## 示例

`examples/rust-wasm` 把最小 UI 与 `rust:` 导入端到端串起：
dev 实时 serve、`ferrite build` 哈希、`--standalone` 嵌入。
缺 `wasm32-unknown-unknown` 或 `wasm-bindgen` 时，错误会给出确切
安装命令。
