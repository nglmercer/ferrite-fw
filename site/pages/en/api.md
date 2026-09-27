---
title: Rust API
order: 15
---

# Rust API

The `ferrite` facade is the programmatic equivalent of the CLI (spec
§9): create servers and builders, add plugins, resolve config — all
from Rust, all without Node.

## Servers and builders

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
builder.build_app().await?; // client (+ ssr when an entry exists)
```

One-shots: `ferrite::build(config)` returns every `BuildReport`;
`ferrite::preview(config)` serves the output dir. `Config` merges file
config under programmatic values, then CLI overrides on top:

```rust
let config = ferrite::Config::default()
    .alias("@", "./src")
    .entry("marketing.html")
    .plugin(MyPlugin);
```

`Config::resolve()` runs `config` hooks and yields
`(ResolvedConfig, Vec<Arc<dyn Plugin>>)`.

## Middleware mode

```rust
let server = ferrite::create_server(ferrite::Config::default()).await?;
let app = my_router.merge(server.router());
```

Your Axum router owns ports, TLS, and fallbacks; Ferrite contributes
the dev pipeline, HMR socket, and transform cache. See [Dev
server](dev-server).

## Plugins

Implement `ferrite::Plugin` (`resolve_id`, `load`, `transform`,
`transform_index_html`, lifecycle hooks), re-export from your crate, and
join `default_plugins` (CLI) or `Config::plugin` (API). Rules: return
`None` for ids you do not own, fail with `FerriteError::Build` instead
of guessing. See [Plugins](plugins).

## Errors and types

The prelude has the vocabulary: `Config`, `Environment`,
`EnvironmentKind`, `ModuleId`, `ModuleType`, `ResolvedConfig`,
`UserConfig`, `Target`, `Hash`, `SourceMap`, `VERSION`, plus `Plugin`,
`PluginContainer`, `Apply`, `Enforce`.

```rust
use ferrite::prelude::*;
```

## Testing

`ferrite-test` (spec §67) gives temp projects, fixtures, and assertions
with no Node needed:

```rust
let project = ferrite_test::TempProject::new(&[
    ("index.html", "<script type=\"module\" src=\"/src/main.js\"></script>"),
    ("src/main.js", "export const n = 1;"),
]);
// … build or serve `project.root`, assert on outputs …
```

The `tests/vite-compat/` suite is built on these helpers. Pair them
with `ferrite compat` (live self-checks) and `ferrite inspect --json`
(machine-readable resolved config) for CI gates.
