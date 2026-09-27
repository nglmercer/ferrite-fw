---
title: 开发服务器
order: 5
---

# 开发服务器

`ferrite dev` 按模块 serve 原生 ESM：resolve → load → transform →
导入重写 → serve。dev 下没有打包步骤，改动重载即见
（或经 HMR 即时生效），粒度精确到单个模块。

```bash
ferrite dev [--host 127.0.0.1] [--port 5173] [--open] [--no-hmr]
```

## 流水线

1. **Resolve** specifier（`ferrite-resolver`：alias、npm、
   `exports`/`imports`、conditions、扩展名）。
2. 经插件 hooks **Load**（Markdown、SFC、WASM glue、`?raw`）。
3. 用 Oxc（或可选 SWC）**Transform**：TS 剥离、JSX、lowering。
4. 裸导入**重写**为 `/@npm/<pkg>@<ver>/…`（`[npm] dev_strategy =
   "import-map"` 下保留裸导入）。
5. 带 ETag + 缓存头 **Serve**；错误变为醒目的 overlay。

虚拟模块内部解析为 `\0…`，以 `/@id/…` serve。
资源导入重写为 `…?asset-shim`（JS URL 导出）；普通 URL 直接 serve
原始字节。样式 `<link>` 重写为 `…?direct`。

## HMR

服务器监听项目（忽略 `node_modules`、`dist`、`target`、
`.ferrite`），经 WebSocket 广播：

- `ferrite-hmr` 从模块图算边界（`plan_update`）：
  精确 importer 接受更新，其余整页重载。
- 浏览器客户端（`ferrite_hmr::client_source`，
  `packages/ferrite-client` 中有类型版）应用更新或重载页面。
- `import.meta.hot` 逐模块重写；React Refresh 仅在 dev 追加
  preamble/footer（见[框架](frameworks)）。
- `--no-hmr` 或 `[server] hmr = false` 可关闭。

```js
if (import.meta.hot) {
  import.meta.hot.accept((next) => render(next));
}
```

## Import map（仅 dev）

```toml
[npm]
dev_strategy = "import-map" # rewrite（默认） | import-map
```

`import-map` 下 dev 保留裸导入，并在首个 module script 前注入内联
`<script type="importmap">`（从 entry 闭包收集）。构建仍用哈希重写；
SSR 保持服务端重写。想让浏览器自己解析裸导入
（CDN shim、externals 调试）时用它。

## 环境变量

`.env` 按 mode 分层加载，后者覆盖前者（Vite `loadEnv` 风格）：
`.env`、`.env.local`、`.env.{mode}`、`.env.{mode}.local`。
值支持 `$VAR`、`${VAR}`、`${VAR:-default}` 与 `$$` 转义，
单遍展开，先查进程环境，再查已加载的文件值
（无默认值且未知则为空）。行支持 `export KEY=value`、`#`
注释、单/双引号（双引号解释 `\"` `\\` `\n` `\r` `\t`）
与未引号值后的 ` # 注释`。

只有 `FERRITE_*` / `PUBLIC_*`（`[env] prefix` 可配）能进入
`import.meta.env`，其余留在服务端；且只返回文件变量——
进程环境只参与展开，不进入结果。编译期 `define` 在其上生效。
编程式接口是 `ferrite::load_env(mode, root, prefixes)` 加
`ferrite::expand_vars(value, loaded)`。

## 代理

```toml
[server.proxy]
"/api" = "http://localhost:3000"
```

前缀规则把命中的 dev/preview 请求转发到目标源，保留路径与
query（Vite `server.proxy` 简写）。最长前缀获胜；按分段边界
匹配，`/api` 覆盖 `/api/users` 而不覆盖 `/apix`。状态码、
header、字节原样透传；hop-by-hop 头（`connection`、`upgrade`、
`transfer-encoding` 等）双向剥离。JS 配置用 `server.proxy`，
字符串或 `{ target }` 对象均可；preview 插件可经
`configure_preview` 追加规则（见[插件](plugins)）。

## 中间件模式

把 dev 流水线嵌入已有 Axum 应用，而非自己 listen：

```rust
let server = ferrite::create_server(ferrite::Config::default()).await?;
let app = my_router.merge(server.router());
```

设 `[server] middleware_mode = true`（或干脆不调 `listen()`）。
HMR、watcher、transform 缓存照常工作；你的路由拥有端口、TLS 与回退路由。

## HTML 处理

entry 脚本从 `index.html` 发现（module 脚本优先）。
流水线跑 pre hooks → 核心重写 → 普通 hooks → post hooks：
裸导入重写、HMR 客户端注入、preload/tag 注入、dev import map。
entry 见[配置](configuration)。
