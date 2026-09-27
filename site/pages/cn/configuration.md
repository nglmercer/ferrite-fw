---
title: 配置
order: 4
---

# 配置

项目用根目录的 `ferrite.toml` 配置 Ferrite，另有可选的、git 忽略的
`ferrite.local.toml` 放本机覆盖。所有文件都可选；空项目按默认构建。
没有任何 TOML 时回退到静态解析的 `ferrite.config.*`
（或 `vite.config.*`）；两者并存时 TOML 永远优先。
优先级：

1. CLI flags（`--host`、`--port`、`--out-dir` 等）
2. `ferrite.local.toml`
3. `ferrite.toml`
4. `ferrite.config.*` / `vite.config.*`（仅无 TOML 时）
5. 内置默认

`ferrite inspect` 打印合并结果；`ferrite inspect --json` 输出供脚本用。

## 最小示例

```toml
# ferrite.toml
base = "/"
mode = "development"

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

## JS 配置文件

`ferrite.config.mts|.cts|.ts|.tsx|.mjs|.cjs|.js|.jsx`
（回退到同等扩展名的 `vite.config.*`）无需 JS 运行时即可加载：
Ferrite 用 Oxc 解析文件并静态求值。只读纯数据——字符串/数字/
布尔字面量、数组与对象字面量，可包一层 `defineConfig(...)`，
顶层 `const` 经 `export default NAME` 或 `module.exports = { ... }`
追踪。

```js
// ferrite.config.ts
export default {
  base: "/",
  server: { port: 5173, proxy: { "/api": "http://localhost:3000" } },
  resolve: { alias: { "@": "./src" } },
};
```

支持的键与下述 TOML 小节对应：`root`、`base`、`mode`、
`define`（标量）、`server`（`host`、`port`、`strictPort`、`open`、
`hmr`、`middlewareMode`、`proxy` 为前缀 → URL 或 `{ target }`
对象）、`build`（`outDir`、`sourcemap`、`minify`、`target`、
`lib`）、`resolve`（对象或 `[{ find, replacement }]` 别名——
alias 是字面量前缀，长得像正则的 `find` 会告警——另加
`conditions`、`extensions`、`preserveSymlinks`）、
`envPrefix` / `env.prefix`、`ssr`（`external`、`noExternal`；
`noExternal: true` 打包全部）、`npm`、`compiler`。动态内容——
其他函数调用、不可解析的标识符、展开、`plugins: [react()]`——
逐键跳过并告警；default export 非对象则是可操作的错误。调
`ferrite::load_config_from_file(dir)`（`loadConfigFromFile`
等价）以 `LoadedConfigFile { path, config, warnings }` 取出告警；
`define_config` / `merge_config` 对应 `defineConfig` /
`mergeConfig`。新项目首选 TOML：它是文档化的接口且永远优先。

## 顶层

| 键 | 默认 | 用途 |
|---|---|---|
| `root` | 配置所在目录 | 项目根（少用；`--config` 优先）。 |
| `base` | `/` | 构建 URL 的公共 base。 |
| `mode` | `development` / `production` | dev 为前者，build 为后者。 |
| `define` | `{}` | 编译期字符串替换（`{ "process.env.X": "\"y\"" }`）。 |

## [server]

| 键 | 默认 | 用途 |
|---|---|---|
| `host` | `127.0.0.1` | 绑定 host。 |
| `port` | `5173` | 绑定端口（`preview` 默认 `4173`）。 |
| `strict_port` | `false` | 端口占用时失败，而非另选空闲端口。 |
| `open` | `false` | 启动时打开浏览器。 |
| `hmr` | `true` | 启用 HMR（`--no-hmr` 关闭）。 |
| `middleware_mode` | `false` | 经 `server.router()` 嵌入，不自己 listen。 |
| `proxy` | `{}` | dev/preview 代理规则：路径前缀 → 目标源。 |

```toml
[server.proxy]
"/api" = "http://localhost:3000"
```

最长前缀获胜；转发时保留路径。见[开发服务器](dev-server)。

## [build]

| 键 | 默认 | 用途 |
|---|---|---|
| `out_dir` | `dist` | 输出目录。 |
| `sourcemap` | `true` | `true`（外部）、`false`、`"inline"` 或 `"hidden"`。 |
| `minify` | `true` | 压缩，并在 tree-shaking 后重新压缩。 |
| `target` | `es2022` | 编译目标。 |
| `entries` | `["index.html"]` | HTML 或模块入口。 |
| `scope_hoist` | `false` | 拼合每个 entry 闭包（见[构建流水线](pipeline)）。 |
| `[build.lib]` | 无 | 库模式：`entry`、`name`、`formats = ["es", "cjs"]`。 |

## [ssr]

| 键 | 默认 | 用途 |
|---|---|---|
| `entry` | `src/server.rs` | SSR 入口（使 `build_app` 产出 `ssr` 环境）。 |
| `external` | `[]` | 保持外部的依赖（Node 内置永远外部）。 |
| `no_external` | `[]` | 永远打包的依赖。 |
| `bundle_all` | `false` | 打包所有依赖。 |

`.node` 二进制永远外部；见 [SSR](ssr)。

## [resolve]

| 键 | 默认 | 用途 |
|---|---|---|
| `conditions` | `["browser", "module", "import"]` | `exports` 条件。 |
| `extensions` | `.mjs .js .mts .ts .jsx .tsx .json` | 探测顺序。 |
| `alias` | `{}` | 导入别名（`"@" = "./src"`）。 |
| `preserve_symlinks` | `false` | 保留 symlink 而非解析。 |

见[包与解析](npm)。

## [npm] / [compiler] / [env]

```toml
[npm]
registry = "https://registry.npmjs.org"
lockfile = "ferrite.lock"
dev_strategy = "rewrite" # 或 "import-map"

[compiler]
engine = "oxc" # 或 "swc"（需 `--features swc`）

[env]
prefix = ["FERRITE_", "PUBLIC_"]
```

- `dev_strategy = "import-map"` 在 dev 下保留裸导入，并注入内联
  `<script type="importmap">`；构建仍用哈希重写。SSR 保持服务端重写。
- 只有 `FERRITE_*` / `PUBLIC_*`（或你的 `prefix`）能进入
  `import.meta.env`；其余留在服务端。

## [remote] / [node_compat]

```toml
[remote]
enabled = false
allow = ["esm.example", "*.cdn.example"]

[node_compat]
enabled = true
mode = "browser-shims"
```

远端 `https://` 导入默认关闭；纯 `http` 仅 loopback 可用。
见[包与解析](npm)。

## [package] / [react] / [runtime]

```toml
[package]
standalone = false
embed_assets = true
compress_assets = true
# target = "x86_64-unknown-linux-musl"

[react]
refresh = true
runtime = "automatic" # 或 "classic"

[runtime]
backend = "auto" # auto | none | napi-vm
fuel_budget = 0  # 0 = 引擎默认
loop_budget = 0
native_allow = ["native/addon.node"]

[runtime.native_integrity]
"native/addon.node" = "<64 hex>"
```

见[生产](production)、[框架](frameworks)与[运行时](runtime)。

## 约定

- 开发环境按模块 serve 原生 ESM；裸导入重写为
  `/@npm/<pkg>@<ver>/…`，`import-map` 策略下保持裸导入。
- 虚拟模块内部解析为 `\0…`，以 `/@id/…` serve。
- 资源导入重写为 `…?asset-shim`（JS URL 导出）；普通 URL 直接 serve 原始字节。
- 样式 `<link>` 重写为 `…?direct`（CSS，而非 JS 包装）。
- npm 安装在 `.ferrite/npm/packages/`；`ferrite.lock` 锁定版本。

## 环境

`ferrite build` 产出 `client` 环境；配置了 SSR 入口时还会产出 `ssr`。
用 `--env`（或 `Config::entry`）只构建单个环境。
每个环境有自己的 target、define 与模块图。
