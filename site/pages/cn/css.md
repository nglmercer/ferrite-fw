---
title: CSS
order: 6
---

# CSS

Ferrite 处理 `.css` 导入、CSS modules、`@import`/`url()` 重写、
dev 注射、生产抽取与压缩（`ferrite-css`，spec §29）。

## 导入

```js
import "./app.css";            // 副作用：dev 注射 / 构建抽取
import styles from "./a.module.css"; // CSS modules：类名映射
```

- 裸 `import "./a.css"` 从生产 JS 中剥离；
  纯 CSS 模块不产出 JS chunk。
- `@import` 顺序在抽取中保留。
- `url()` 引用 rebase 到输出；哈希资源带内容哈希
  （`logo.4ad83f.svg`）。

## CSS modules

`.module.css` 导出类名映射；dev 与构建用同一作用域命名，
SSR 与客户端一致。普通 `.css` 保持全局语义。

```css
/* button.module.css */
.primary { color: orange; }
```

```js
import styles from "./button.module.css";
el.className = styles.primary;
```

## Dev 与构建

- **Dev**：样式经 JS 包装注射；改动无刷新热更新。
  样式 `<link>` 重写为 `…?direct`，浏览器拿到的是 CSS 而非包装。
- **构建**：每个样式模块一个哈希 `.css`，按 `@import` 顺序以
  `<link>` 注入构建后 HTML。抽取后压缩；source map 串起 transform 映射。

## 工具类 CSS（Tailwind）

```js
import "ferrite:tailwind.css";
```

`ferrite:tailwind` 插件扫描项目内容（HTML、JS/TS、Markdown、
Vue/Svelte/Astro——从不扫 `node_modules`、`dist`、`target`、
`vendor`），用 vendored `tailwind-rs` 编译该虚拟模块。
导入该 specifier 是唯一的 opt-in；从不导入的项目零开销。
本站颜色在其上的 `site/src/docs.css` 里。见 [Tailwind vendor](tailwind-vendor)。

## 压缩

生产压缩默认开（`[build] minify = true`）。
压缩产出自己的 map 并串起 transform map
（`ferrite_transform::chain_source_maps`），压缩后仍指向原始源码。
无法解析的位置以无源通过，而非构建失败。
