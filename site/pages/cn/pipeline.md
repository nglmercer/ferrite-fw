---
title: 构建流水线
order: 9
---

# 构建流水线

dev 与构建共享一条流水线（`ferrite-transform` + `ferrite-graph`
+ `ferrite-bundler`）：bundler 经与服务器相同的 `ModuleLoader`
消费模块，所见即所得。

## Transform

`JsCompiler` 是稳定的边界：图、服务器、bundler 只看到
`ParsedModule` / `TransformResult`，从不见引擎 AST 类型。

- **Oxc（默认）**：TS 剥离、JSX automatic/classic、lowering、
  `import.meta.hot` 重写、define 替换、HMR 客户端注入。
- **SWC（可选）**：`--features swc` 构建（或
  `[compiler] engine = "swc"`），把 transform/minify 引擎换成
  `swc_core`（TS 剥离、JSX、es2015–2022 lowering、顶层 DCE）。
  默认构建保留零成本 Oxc 前端。

无需完整构建即可调试单个文件：

```bash
ferrite transform src/app.tsx --out /tmp/app.js --sourcemap
```

## Bundler

`ferrite build` 遍历每个 entry 闭包、分块、哈希、输出
（`ferrite-bundler`，spec §37–§39）：

1. 经插件 hooks resolve + load。
2. Transform（Oxc 或 SWC）。
3. 语句级 tree-shake，再重新压缩。
4. 分块（哈希、每模块一个 ESM）或 `--scope-hoist`。
5. 抽取 CSS（每样式一个哈希 `.css`，`@import` 顺序）。
6. 输出清单 + source map。

## 渲染流水线

transform 与输出之间，builder 经插件钩子跑 Rollup 风格的渲染阶段
（`ferrite-bundler` 隔在 `BundleHooks` trait 后保持插件无关；
单测用 `NoHooks` 驱动）：

1. `options` 改输入选项；`output_options` 改 chunk/CSS/asset
   模式（`[name]` / `[hash]` / `[ext]`）。
2. `render_start` 携带 entry 模块 id 触发。
3. 逐 chunk：`render_chunk`（重写前替换）、
   `augment_chunk_hash`（额外哈希输入）、导入重写，
   再套 `banner` / `intro` / `outro` / `footer` 包裹。
4. `generate_bundle` / `write_bundle`，然后是 `build_end`
   （失败时携带 `Some(消息)`）与必定执行的 `close_bundle`。

`render_chunk` 跑在重写之前——与 Rollup 相反——以便内容哈希覆盖
钩子改后的代码；钩子因此看到模块解析后（dev URL）的 specifier。
包裹文本计入哈希、重写后应用，永不破坏 specifier。
钩子表见[插件](plugins)。

## Tree-shaking

生产默认开（`BundleRequest.treeshake`）。无用导出按语句删除，
used-exports 在全图 fixpoint；barrel 重导出只要有下游用就保留。

## Scope hoisting

```bash
ferrite build --scope-hoist
```

```toml
[build]
scope_hoist = true
```

把每个 entry 闭包拼成一个模块，局部变量加 `$f{index}$` 前缀。
无法证明安全的图（namespace、`eval` 等）回退到 chunked 输出，
绝不错编译。

## Source map

`[build] sourcemap`：`true`（外部，默认）、`false`、`"inline"`、
`"hidden"`（输出但无注释）。压缩产出自己的 map 并串起
transform map（`ferrite_transform::chain_source_maps`），压缩后仍
指向原始源码。不可解析的位置无源通过。

## 清单

`ferrite-manifest`（spec §40）写 Vite 兼容的客户端清单
（`file`、`src`、`isEntry`、`css`、`imports`、
`dynamicImports`）加 SSR 模块→chunk 映射。standalone 服务器与
SSR loader 读它在运行时解析哈希 URL。
