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

## 钩子参考

除 `resolve_id` / `load` / `transform` /
`transform_index_html` 外，插件还有 Vite/Rollup 对等钩子：

| 钩子 | 触发时机 |
|---|---|
| `options` | 构建开始前修改输入选项（`BundleOptions`：entries、treeshake、minify、sourcemap、scope-hoist）。 |
| `output_options` | chunk 命名前修改输出模式（`OutputOptions`：`chunk_pattern`、`css_pattern`、`asset_pattern`，含 `[name]` / `[hash]` / `[ext]`）。 |
| `resolve_dynamic_import` | 解析动态 `import()`；全部返回 `None` 时回退到 `resolve_id`。 |
| `should_transform_cached_module` | `Some(true)` 强制重变换缓存模块；`None` / `Some(false)` 保留缓存。 |
| `watch_change` | 观察文件监听事件（`WatchEvent { path, kind }`：create / modify / remove）。 |
| `resolve_file_url` | 把产物文件映射到公开 URL；首个 `Some` 获胜（也重写 dev 的 `?url` shim）。 |
| `hot_update` | Vite 6 `hotUpdate`；先于旧 `handle_hot_update`，两者间首个 `Some` 获胜。 |
| `render_start` | 渲染开始：entry 已知，chunk 尚未规划。 |
| `render_chunk` | 重写前替换单个 chunk 代码（替换结果参与哈希与重写；钩子看到 dev URL specifier）。 |
| `augment_chunk_hash` | 贡献额外的 chunk 哈希输入。 |
| `banner` / `intro` / `outro` / `footer` | 包裹 chunk；计入哈希，重写后应用。 |
| `build_end` / `close_bundle` | 构建结束；失败时 `build_end` 携带 `Some(消息)`，`close_bundle` 照样执行。 |

渲染阶段顺序见[构建流水线](pipeline)。

## Preview 钩子

`ferrite preview` 先跑 `configResolved`，再跑旧的
`configure_preview_server` 与新的 `configure_preview`
（新插件实现后者）。`PreviewControl` 收集：

- 额外的响应 header（`add_header`），
- 输出目录之前命中的静态挂载
  （`add_mount("/docs", dir)`），
- 代理规则（`add_proxy("/api", "http://localhost:3000")`，
  最长前缀获胜，初始来自 `[server] proxy`）。

## 服务器句柄

dev 服务器钩子收到 `ServerControl`，含 Vite 等价物：
`module_graph()`（`server.moduleGraph`）、`local_addr()`、
`server_urls()` / `print_urls()`、`hmr_clients()` /
`send_full_reload(path)`，以及 `watcher_alive()` /
`watcher_add(path)`（`server.watcher.add`）。

## 写自己的插件

新的原生插件放入 workspace crate，从 `ferrite` 门面 re-export，
并加入 CLI 的 `default_plugins`，dev 和 build 就都能用。钩子要
total：不属于你的 id 返回 `None`，失败时大声抛出
`FerriteError::Build`，不要猜测。
