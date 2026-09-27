---
title: CLI 参考
order: 3
---

# CLI 参考

```bash
ferrite <命令> [选项]
ferrite --config ./ferrite.toml <命令>
ferrite --mode production <命令>
ferrite --log-level debug <命令>
```

全局 flags 适用于所有命令。优先级永远是：CLI flags、
`ferrite.local.toml`、`ferrite.toml`、内置默认。
`ferrite <命令> --help` 是权威 flags 列表；本页讲意图与示例。

## 全局 flags

| Flag | 默认 | 用途 |
|---|---|---|
| `--config <path>` | 项目根 | 含 `ferrite.toml` 的文件或目录。 |
| `--mode <mode>` | `development`（dev）/ `production`（build） | 构建模式；决定 `is_production` 与 env 文件。 |
| `--log-level <level>` | `info` | `trace`、`debug`、`info`、`warn`、`error`。 |

## 命令

| 命令 | 用途 |
|---|---|
| `dev [root]` | 启动带 HMR 的开发服务器。 |
| `ssr [root]` | SSR 模式开发服务器（shell 或 `napi-vm` entry-server）。 |
| `build [root]` | 生产包输出到 `dist/`。 |
| `preview [root]` | 本地 serve 生产构建。 |
| `add <spec...>` | 添加 npm 包（无需 Node）。 |
| `remove <name...>` | 移除 npm 包。 |
| `update [spec...]` | 更新 npm 包（默认全部）。 |
| `install` | 安装已声明/锁定的 npm 包。 |
| `inspect [root]` | 查看解析后配置、插件、lockfile。 |
| `transform <file>` | 对单个文件跑 transform 流水线。 |
| `migrate <file>` | 把 Vite 配置迁移为 `ferrite.toml`。 |
| `compat` | 针对真实流水线的存活自检。 |
| `clean` | 清理 `.ferrite/` 缓存。 |
| `create <name>` | 脚手架新项目。 |

## dev / ssr

```bash
ferrite dev [--host 127.0.0.1] [--port 5173] [--open] [--no-hmr] [--runtime auto]
ferrite ssr --runtime napi-vm
```

- 按模块 serve 原生 ESM；除非 `--no-hmr`，HMR 走 WebSocket。
- `--open` 打开浏览器；`--host`/`--port` 覆盖 `[server]`。
- `--runtime` 选择 SSR 后端：`auto`（默认，无 JS 引擎）、
  `none`（强制关闭）或 `napi-vm`（需 `napi-vm` cargo feature）。
- `ssr` serve 带 preload 注入的 `index.html` shell，或 `napi-vm` 下
  `src/entry-server.*` 的 `render(url)` 导出。
- 见[开发服务器](dev-server)与 [SSR](ssr)。

## build

```bash
ferrite build [root] [--out-dir dist] [--minify] [--env client]
ferrite build --standalone [--target x86_64-unknown-linux-musl]
ferrite build --scope-hoist
```

- `--out-dir` 覆盖 `[build] out_dir`；`--minify` 默认为 true
 （传 `--minify=false` 关闭）。
- `--env` 只构建单个环境（`client` 或 `ssr`），而非两者。
- `--standalone` 经 `include_bytes!` + gzip 把 `dist/` 嵌入自包含
  服务器二进制；报告为 `standalone_binary`。
- `--target` 用 `cargo build --target` 交叉编译该二进制
  （仅校验过的 triple；web 资源与 target 无关）。
- `--scope-hoist` 把每个 entry 闭包拼成一个模块；
  无法证明安全的图回退到 chunked 输出。
- 见[生产](production)与[构建流水线](pipeline)。

## preview

```bash
ferrite preview [--port 4173]
```

经 HTTP serve 上一次 `dist/`，做生产级检查。
只 serve 静态文件；SSR 路由需要 standalone 二进制或 `ferrite ssr`。

## npm：add / remove / update / install

```bash
ferrite add react react-dom three
ferrite add lodash --dev
ferrite remove lodash
ferrite update
ferrite update react@latest
ferrite install
```

- spec 形如 `react`、`three@latest`、`@scope/name@^1.0.0`。
- 包安装到 `.ferrite/npm/packages/`；`ferrite.lock` 锁定版本。
- `--dev` 记为 dev 依赖；`--root` 指向另一项目。
- 永远不需要 `node`、`npm` 或 `node_modules`。
- 见[包与解析](npm)。

## inspect

```bash
ferrite inspect
ferrite inspect --json
```

打印解析后配置（root、mode、server、build、compiler）、
活动插件列表与锁定包。`--json` 输出供脚本/CI 用的机器可读结果。
见[排障](troubleshooting)。

## transform

```bash
ferrite transform src/main.ts
ferrite transform src/app.tsx --out /tmp/app.js --sourcemap
```

对单个文件跑 resolve + transform + 导入重写并打印结果。
`--out` 写文件；`--sourcemap` 在 `--out` 旁输出 `.map`
（stdout 时为 inline 注释）。无需完整构建即可调试[构建流水线](pipeline)。

## migrate

```bash
ferrite migrate vite.config.js
ferrite migrate vite.config.ts --out ferrite.toml
```

best-effort 的 Vite → `ferrite.toml` 翻译（host/port、`outDir`、
`sourcemap`、alias、define）。输出带注释，务必人工复核；
未知键只告警，永不静默丢弃。见[排障](troubleshooting)。

## compat / clean / create

```bash
ferrite compat
ferrite clean
ferrite create my-app
ferrite create my-ssr --template ssr
```

- `compat` 跑存活检查（resolver、Oxc transform、HMR 规划、CSS、
  清单 round-trip），逐项打印 `PASS`/`FAIL`。
- `clean` 删除 `.ferrite/`（npm 安装、transform/远端缓存）。
  全新 checkout 会重新拉取；`.ferrite/` 之外不受影响。
- `create` 生成 `vanilla`（默认）或 `ssr` 模板：`index.html`、
  模块入口与初始 `ferrite.toml`。
