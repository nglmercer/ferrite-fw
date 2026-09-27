---
title: Rust API
order: 15
---

# Rust API

`ferrite` facade 是 CLI 的编程式等价（spec §9）：创建服务器与
builder、加插件、解析配置——全在 Rust，全无需 Node。

## 服务器与 builder

```rust
#[tokio::main]
async fn main() -> ferrite::Result<()> {
    let server = ferrite::create_server(ferrite::Config::default()).await?;
    server.listen().await?;
    Ok(())
}
```

```rust
let builder = ferrite::create_builder(ferrite::Config::default()).await?;
builder.build_app().await?; // client（有入口时加 ssr）
```

One-shot：`ferrite::build(config)` 返回全部 `BuildReport`；
`ferrite::preview(config)` serve 输出目录。`Config` 把文件配置合在
编程式值之下，CLI 覆盖在最上：

```rust
let config = ferrite::Config::default()
    .alias("@", "./src")
    .entry("marketing.html")
    .plugin(MyPlugin);
```

`Config::resolve()` 跑 `config` hooks，产出
`(ResolvedConfig, Vec<Arc<dyn Plugin>>)`。

## 中间件模式

```rust
let server = ferrite::create_server(ferrite::Config::default()).await?;
let app = my_router.merge(server.router());
```

你的 Axum 路由拥有端口、TLS 与回退；Ferrite 贡献 dev 流水线、
HMR socket 与 transform 缓存。见[开发服务器](dev-server)。

## 插件

实现 `ferrite::Plugin`（`resolve_id`、`load`、`transform`、
`transform_index_html`、生命周期 hooks），从你的 crate 重导出，
加入 `default_plugins`（CLI）或 `Config::plugin`（API）。规则：
不归你的 id 返回 `None`，宁以 `FerriteError::Build` 明确失败也不
猜。见[插件](plugins)。

## 错误与类型

prelude 有全套词汇：`Config`、`Environment`、`EnvironmentKind`、
`ModuleId`、`ModuleType`、`ResolvedConfig`、`UserConfig`、
`Target`、`Hash`、`SourceMap`、`VERSION`，加 `Plugin`、
`PluginContainer`、`Apply`、`Enforce`。

```rust
use ferrite::prelude::*;
```

## 测试

`ferrite-test`（spec §67）提供临时项目、fixture 与断言，无需 Node：

```rust
let project = ferrite_test::TempProject::new(&[
    ("index.html", "<script type=\"module\" src=\"/src/main.js\"></script>"),
    ("src/main.js", "export const n = 1;"),
]);
// … 构建或 serve `project.root`，对输出断言 …
```

`tests/vite-compat/` 套件即建于这些 helper 之上。配 `ferrite compat`
（存活自检）与 `ferrite inspect --json`（机器可读配置）作 CI 门禁。
