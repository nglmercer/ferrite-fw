---
title: 快速上手
order: 2
---

# 快速上手

你只需要 Rust 工具链（stable，1.88+），别无其他。Node.js 可选，
仅 tier-3 外部插件才会用到。

## 安装 CLI

```bash
cargo install --path crates/ferrite-cli
ferrite --version
```

也可以不安装，直接从仓库运行：

```bash
cargo run -p ferrite-cli -- --help
```

## 第一个项目

```bash
ferrite create my-app
cd my-app
ferrite dev
```

打开打印出的 URL（默认 `http://127.0.0.1:5173`）。任意模块的修改
都会热重载；Markdown 或样式的修改会重建页面 CSS。

最小项目只是一个 `index.html` 加一个模块脚本：

```html
<!doctype html>
<html>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.js"></script>
  </body>
</html>
```

```js
// src/main.js
document.getElementById("app").textContent = "你好，Ferrite。";
```

## 开发循环

1. `ferrite dev` —— 按模块原生 ESM，WebSocket HMR。
2. `ferrite build` —— 哈希化生产包输出到 `dist/`。
3. `ferrite preview` —— 本地 serve `dist/`，检查真实产物。

发现异常时，`ferrite compat` 会针对真实流水线做存活自检，
大声报告失败原因，而不是猜测。
