---
title: 生产环境
order: 16
---

# 生产环境

`ferrite build` 把模块图变成哈希资源加清单。下面每个生产特性都
被 workspace 测试套件以及本站自己的构建覆盖。

## 流水线

1. **Resolve + load** 经插件钩子（Markdown、SFC、WASM glue）。
2. **Transform** 用 Oxc（或 opt-in 的 SWC 后端，`--features swc`）。
3. **Tree-shake** 按语句删除未用导出，再 re-minify。
4. **分包或 hoist**：默认哈希分包，或 `--scope-hoist` 把每个
   entry 闭包拼成一个文件。
5. **CSS 抽取**：每个样式模块一个哈希 `.css`，按 `@import` 顺序
   以 `<link>` 注入构建后的 HTML。
6. **输出清单**，映射源码 id 到哈希产物。

## 单个二进制

```bash
ferrite build --standalone
./dist/ferrite-standalone # serve 嵌入的站点，不需要旁边的 dist/
```

standalone 脚手架用 `include_bytes!` + gzip 嵌入 `dist/` 并以 HTTP
serve。加 `--target` 即交叉编译（musl 已在 ignored 测试中验证）。

## 发布前验证

```bash
ferrite build site --standalone
PORT=8080 ./site/dist/ferrite-site
curl -s localhost:8080/ | head -c 200
```

绿灯标准：HTTP 200、构建后的 `<script>` 与 `<link>` 存在、哈希
资源 URL 可达、二进制旁边不需要 `dist/`。本仓库的自证就是这个
文档站：`site/` 正走这条路构建与 serve。
