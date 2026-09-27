---
title: 排障
order: 17
---

# 排障

出问题先用明确的工具：`compat` 查流水线、`inspect` 看解析后状态、
`transform` 隔离单个文件。Ferrite 宁明确失败也不猜——下面的错误
都是系统按设计工作。

## compat

```bash
ferrite compat
```

针对真实流水线的存活自检（无需 Node）：resolver、Oxc
transform、HMR 规划、CSS、清单 round-trip。逐项打印
`PASS`/`FAIL`；`FAIL` 点名下一步查哪个子系统。

## inspect

```bash
ferrite inspect
ferrite inspect --json
```

展示合并后配置（root、mode、server、build、compiler）、活动插件
与锁定包。`--json` 供脚本/CI。若 dev 与构建不一致，在各 `--mode`
下 diff `inspect`。

## transform

```bash
ferrite transform src/suspect.tsx
ferrite transform src/suspect.tsx --out /tmp/out.js --sourcemap
```

对单个文件跑 resolve + transform + 导入重写。用于区分
“这个文件的语法”与“周围的图”。

## clean

```bash
ferrite clean
```

删除 `.ferrite/`（npm 安装、transform/远端缓存）。全新
checkout 重新拉取；`.ferrite/` 之外不受影响。怀疑缓存过期时，
`clean` + 重跑是官方重置。

## migrate

```bash
ferrite migrate vite.config.js --out ferrite.toml
```

best-effort 的 Vite → `ferrite.toml` 翻译，带注释与告警。
务必复核输出：alias、define、代理多需手工调。
未知键只告警，永不静默丢弃。

## 常见故障

| 症状 | 可能原因 | 修法 |
|---|---|---|
| dev 下裸导入 404 | 包未安装 | `ferrite add <pkg>`；查 `ferrite.lock` |
| `.node` 运行时抛错 | 未进 allowlist | 加 `[runtime] native_allow` + 完整性；需 `napi-vm` |
| `https://` 导入被拒 | 远端未开 / host 未允许 | `[remote] enabled = true`，`allow = […]` |
| WASM 构建报错 | 缺 target/`wasm-bindgen` | 按 hint 安装；重跑 |
| 白屏、无 HMR | `--no-hmr` 或 `[server] hmr = false` | 重开；查 WS 代理 |
| 构建与 dev 不一致 | env/mode 不一致 | 按 mode 对比 `inspect`；查 `[env] prefix` |
| 端口占用 | 默认 5173 被占 | `--port 0`/换端口，或 `strict_port = true` 快速失败 |

## 求助

明确的错误就是 bug 报告：点名 crate、hook、模块 id。
用 `transform` 或最小 `TempProject` 复现（见 [Rust
API](api)），附失败输入加 `inspect --json` 提 issue。
