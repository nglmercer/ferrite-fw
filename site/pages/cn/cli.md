---
title: CLI 参考
order: 3
---

# CLI 参考

```bash
ferrite <命令> [选项]
ferrite --config ./ferrite.toml <命令>
```

## 命令

| 命令       | 用途                                   |
|------------|----------------------------------------|
| `dev`      | 启动带 HMR 的开发服务器                |
| `ssr`      | SSR 模式开发服务器                     |
| `build`    | 生产包输出到 `dist/`                   |
| `preview`  | 本地 serve 生产构建                    |
| `compat`   | 针对真实流水线的存活自检               |
| `inspect`  | 查看解析后配置 / 模块图                |
| `transform`| 对单个文件跑 transform 流水线         |
| `create`   | 脚手架新项目                           |
| `install` / `add` / `remove` / `update` | npm 包管理 |
| `migrate`  | 把 Vite 配置迁移为 `ferrite.toml`      |
| `clean`    | 清理构建缓存                           |

## 构建选项

```bash
ferrite build [root] [--out-dir dist] [--minify] [--env client]
ferrite build --standalone [--target x86_64-unknown-linux-musl]
ferrite build --scope-hoist
```

- `--standalone` 把 `dist/` 嵌入自包含的服务器二进制。
- `--target` 交叉编译该二进制（仅校验过的 triple）。
- `--scope-hoist` 把每个 entry 闭包拼成一个模块。
- `--env` 只构建单个环境，而非 client + SSR。

## 开发选项

`ferrite dev` 接受 `--host`、`--port`、`--no-hmr`、`--open`。
优先级永远是：CLI flags > `ferrite.local.toml` >
`ferrite.toml` > 内置默认。
