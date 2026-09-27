---
title: 概述
order: 1
---

# Ferrite

Ferrite 是一个 Rust 原生 Web 工具链：开发服务器、生产打包器和 SSR
运行时合于一个二进制文件。它遵循 Vite 约定（开发环境原生 ESM、
插件钩子、import map），流水线由 Oxc 驱动，核心流程不需要 Node.js。

本站本身就是用 Ferrite 构建的：Markdown 页面通过 `ferrite:markdown`
插件变为 JS 模块，布局工具类由 vendored 的 `tailwind-rs` 编译，
最终以单个二进制文件发布。

## 你能得到

- **开发服务器**：按模块原生 ESM、HMR、支持 import map。
- **生产构建**：哈希资源、清单、CSS 抽取、语句级 tree-shaking、
  可选 scope hoisting。
- **框架**：开发环境 React Refresh，以及 Vue/Svelte SFC 实验。
- **SSR**：适配器、`ssrLoadModule`、externals、流式、islands、RPC。
- **单个二进制**：嵌入 `dist/`，无需其他运行时即可 serve。
- **Markdown + 工具类 CSS**：即本文档站技术栈，零 JS 工具链。

## 从这里开始

初次接触 Ferrite？请读[快速上手](getting-started)：安装、第一个
项目与开发循环。然后选择一条路线：

| 路线     | 页面 |
|----------|------|
| 开发     | [开发服务器](dev-server)、[CSS](css)、[资源](assets) |
| 依赖     | [包与解析](npm) |
| 渲染     | [SSR](ssr)、[运行时](runtime)、[框架](frameworks)、[WASM](wasm) |
| 构建     | [CLI](cli)、[配置](configuration)、[构建流水线](pipeline) |
| 扩展     | [插件](plugins)、[Rust API](api) |
| 发布     | [生产环境](production)、[排障](troubleshooting) |
| Vendored | [Tailwind Vendor](tailwind-vendor) |

> 本文档位于 Ferrite 仓库的 `site/pages/*/*.md`，也是项目的生产验证
> 目标：本站能构建、能 serve，即代表框架集成状态为绿。

## 主题与语言

用页眉控件在浅色 / 深色间切换（存于本地，默认跟随系统），
也可在英语、西班牙语（`es`）、中文（`cn`）间切换。每种语言都
是相同的十八个页面，路由为 `#/{语言}/{页面}`。
