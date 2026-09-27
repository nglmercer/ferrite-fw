---
title: 框架
order: 12
---

# 框架

`ferrite-frameworks` 自带 React 插件与实验性的 Vue/Svelte
单文件组件支持。三者 CLI 默认全开；每个都是普通 `Plugin`，
编程式用户可随意去掉。

## React

```toml
[react]
refresh = true
runtime = "automatic" # 或 "classic"
```

- 仅 dev 的 Refresh preamble + 逐模块 footer：组件改动带状态热换；
  非组件改动回退 HMR。
- `detect_components` 逐文件找组件；`refresh_footer` 向
  React Refresh 运行时注册。
- `runtime = "automatic"` 用 JSX transform；`classic` 保留
  `React.createElement`。两种 transform 引擎（Oxc/SWC）都支持。
- 生产构建剥离全部 Refresh 代码——零运行时开销。

```bash
ferrite add react react-dom
```

## Vue（实验）

`VuePlugin` 把 `.vue` 切成 script/style 块作虚拟模块 serve。
识别 `<script setup>`；template 块目前编译为明确的 stub
（`template_stub`）——完整模板编译是路线图，stub 在调用点明确说明。

## Svelte（实验）

`SveltePlugin` 同理切分 `.svelte`（`split_svelte`、markup 抽取、
`markup_stub`）。script 响应性走普通 JS 流水线编译；模板是明确 stub。

## SSR + islands

三者都经同一 [SSR](ssr) 适配器渲染，以 [islands](ssr) 水合：
服务端 HTML 加每 island 客户端闭包。框架选择不改变 dev 服务器、
清单或 standalone 打包。

## 示例

`examples/react` 是参考应用（dev、HMR、Refresh、构建）。
`examples/vanilla-ts` 是零框架基线；`examples/ssr` 是服务端渲染；
`examples/rust-wasm` 是框架 UI 配 [WASM](wasm)。
