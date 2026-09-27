---
title: 包与解析
order: 8
---

# 包与解析

Ferrite 无需 Node.js 解析 npm：registry 客户端、semver 解析、
完整性校验的 tarball 装到 `.ferrite/npm/packages/`，
`ferrite.lock` 锁定（`ferrite-npm`，spec §16）。

## 安装

```bash
ferrite add react react-dom three   # → .ferrite/npm/
ferrite add lodash --dev
ferrite install                      # 装已声明/锁定的包
ferrite update [spec...]             # 默认全部
ferrite remove lodash
```

spec 形如 `react`、`three@latest`、`@scope/name@^1.0.0`。
永远不需要 `node`、`npm` 或 `node_modules`。

## Dev serve

dev 下裸导入重写为 `/@npm/<pkg>@<ver>/…`。
`[npm] dev_strategy = "import-map"` 时 dev 保留裸导入，并注入从
entry 闭包收集的内联 `<script type="importmap">`；构建永远哈希重写，
SSR 永远服务端重写。

## Resolver

`ferrite-resolver`（spec §15）处理相对/绝对导入、alias、裸 npm
导入、包 `exports`/`imports`、conditions、目录索引、symlink、
CSS/URL 导入与虚拟模块。

```toml
[resolve]
conditions = ["browser", "module", "import"]
extensions = [".mjs", ".js", ".mts", ".ts", ".jsx", ".tsx", ".json"]
preserve_symlinks = false

[resolve.alias]
"@" = "./src"
```

或编程式：`ferrite::Config::default().alias("@", "./src")`。

## Lockfile

`ferrite.lock`（`[npm] lockfile` 可改）锁定精确版本加完整性哈希。
`ferrite install` 复现 lock；`ferrite add`/`update` 重新解析并重写。
`ferrite inspect` 列出锁定包；`--json` 供 CI 消费。

## 远端导入

```toml
[remote]
enabled = true
allow = ["esm.example", "*.cdn.example"]
```

默认关闭。允许的 `https://` 导入（纯 `http` 仅 loopback）解析为
虚拟模块，拉取一次，按 URL 哈希缓存在
`.ferrite/cache/remote/`。远端模块内的相对导入 rebase 到远端源。
全新 checkout 重新拉取；`ferrite clean` 清缓存。

## Node 兼容

`[node_compat] enabled = true`（默认）给浏览器垫 `node:*`
导入（`mode = "browser-shims"`）。纯服务端 builtin 在 SSR 下保持
外部，而非静默打包 stub。
