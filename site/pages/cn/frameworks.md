---
title: Frameworks
order: 12
---

# 框架

组件编译、浏览器运行和 SSR 渲染是不同的能力。
[能力注册表](../../../crates/ferrite-frameworks/src/registry.rs)
是生成器和诊断工具的依据。目前所有应用配置仍处于实验阶段。

| 框架 | 客户端编译 | 编译宿主 | SSR 配置 |
|---|---|---|---|
| React 19.2.0 | 原生 Oxc/SWC；部分 React Refresh | 原生 | 实验性 napi-vm |
| Vue 3.5.22 | 与项目版本匹配的官方编译器 | 显式启用 Node | 实验性 napi-vm |
| Svelte 5.39.6 | 与项目版本匹配的官方编译器 | 显式启用 Node | 实验性 napi-vm |

Vue 和 Svelte 使用真实的官方编译器，不再提供模板占位实现。未配置编译宿主时，
组件编译会明确报错。Node 是这些配置在开发和构建时的编译依赖，与嵌入式 SSR
运行时及发布的独立程序运行依赖分开。

## 创建项目

```bash
ferrite create my-react --framework react --language ts --rendering client
ferrite create my-vue --framework vue --language ts --rendering client --compiler-host node
ferrite create my-svelte --framework svelte --language js --rendering client --compiler-host node
```

React Refresh 已有组件、自定义和导入的 hooks、重新导出、memo、默认导出、混合导出、
错误恢复与生产预览的浏览器测试，但完整矩阵尚未完成。生产输出不包含 Refresh 代码。
识别 Preact、Solid 或 Qwik 的包名并不代表提供这些框架的适配器。

Vue/Svelte 在 Chromium 和 Firefox 中验证真实模板。编译器、宿主和依赖文件都参与
缓存及失效判断。组件修改可能重新加载页面；完整的保留状态 HMR 尚未提供。

## 实验性 Vue 和 Svelte SSR

使用 `--features napi-vm` 构建 Ferrite，并显式选择两个宿主：

```bash
ferrite create my-app --framework vue --language ts --rendering ssr --compiler-host node --ssr-runtime napi-vm
```

JS/TS 验收覆盖 JavaScript 执行前的 HTTP HTML、水合、交互、文件监听、语法和样式
错误恢复、生产预览以及 `/app/` 基础路径。这不代表已验证流式渲染、SSG、全部 Vue
功能或跨平台独立程序水合。Svelte JS/TS 已通过 Chromium 和 Firefox 的生产构建、
预览、水合、DOM 保留、交互和 CSS 验收；使用 `--framework svelte` 生成实验性 SSR 项目。
React JS/TS 也通过了两个浏览器的同步生产 SSR 与水合验收。使用
`--framework react --compiler-host native --ssr-runtime napi-vm`，不需要 Node。
新配置的完整开发和独立程序验收矩阵仍未完成。

`ferrite doctor` 检查配置的前置条件，不执行编译器；`ferrite inspect` 显示解析后的配置。
剩余任务和复现命令见[完成计划](../../../docs/COMPLETION_PLAN.md)。
