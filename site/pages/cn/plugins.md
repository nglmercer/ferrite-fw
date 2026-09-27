---
title: 插件
order: 14
---

# 插件

插件实现 `ferrite_plugin::Plugin` trait：`resolve_id`、`load`、
`transform`、`transform_index_html` 以及构建/开发的生命周期钩子。
原生 Rust 插件随 CLI 发布；外部 ESM 插件可跑在 tier-2 嵌入式 JS
或 tier-3 真实 Node 中。

## 内置插件

| 插件                | 作用                                     |
|---------------------|------------------------------------------|
| `ferrite:markdown`  | `.md` 变为导出 HTML + 元数据的 JS 模块   |
| `ferrite:tailwind`  | 从内容编译的虚拟 `ferrite:tailwind.css`  |
| `ferrite:react-refresh` | 仅开发环境的 React Refresh 头尾     |
| Vue / Svelte        | SFC 切分实验（script/style 块）          |
| Rust WASM           | `.wasm` 导入的 `cargo build` 编排        |
| Raw text            | `?raw` 后缀导入                          |

## Markdown 模块

```js
import page, { title, headings } from "./guide.md";

page.html; // 渲染后的 HTML 字符串
page.title; // frontmatter 标题，或第一个 h1
page.headings; // [{ level, text, id }]，用于目录
page.order; // frontmatter 排序，用于侧栏
```

Frontmatter 是支持 `title:` / `order:` 的 `---` 块。
启用 GitHub 风格 Markdown（表格、脚注、删除线、任务列表），
每个标题都会得到 slug 化的锚点 id。

## 工具类 CSS 模块

```js
import "ferrite:tailwind.css";
```

加载时插件扫描项目根（HTML、JS/TS、Markdown、Vue/Svelte/Astro；
永不扫描 `node_modules`、`dist`、`target`、`vendor`），用 vendored
的 `tailwind-rs` 编译发现的工具类。导入该 specifier 是唯一的
opt-in；从不导入的项目零开销。

## 写自己的插件

新的原生插件放入 workspace crate，从 `ferrite` 门面 re-export，
并加入 CLI 的 `default_plugins`，dev 和 build 就都能用。钩子要
total：不属于你的 id 返回 `None`，失败时大声抛出
`FerriteError::Build`，不要猜测。
