---
title: 配置
order: 4
---

# 配置

项目用根目录的 `ferrite.toml` 配置 Ferrite（另有可选的、
git 忽略的 `ferrite.local.toml` 放本机覆盖）。所有文件都可选；
空项目按默认构建。

## 示例

```toml
# ferrite.toml
[server]
port = 5173

[build]
minify = true
scope_hoist = false

[npm]
dev_strategy = "rewrite" # 或 "import-map"

[react]
refresh = true
```

## 约定

- 开发环境按模块 serve 原生 ESM；裸导入重写为
  `/@npm/<pkg>@<ver>/…`，`import-map` 策略下保持裸导入。
- 虚拟模块内部解析为 `\0…`，以 `/@id/…` serve。
- 资源导入重写为 `…?asset-shim`（JS URL 导出）；普通 URL
  直接 serve 原始字节。
- 样式 `<link>` 重写为 `…?direct`（CSS，而非 JS 包装）。
- npm 安装在 `.ferrite/npm/packages/`；`ferrite.lock` 锁定版本。
- 只有 `FERRITE_*` / `PUBLIC_*` 环境变量能进入 `import.meta.env`。

## 环境

`ferrite build` 产出 `client` 环境；配置了 SSR 入口时还会产出
`ssr`。用 `--env` 只构建单个环境。
