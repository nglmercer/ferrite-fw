---
title: API Rust
order: 15
---

# API Rust

La fachada `ferrite` es el equivalente programático del CLI (spec §9):
crea servidores y builders, añade plugins, resuelve config — todo desde
Rust, todo sin Node.

## Servidores y builders

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
builder.build_app().await?; // client (+ ssr si hay entry)
```

One-shots: `ferrite::build(config)` devuelve cada `BuildReport`;
`ferrite::preview(config)` sirve el out dir pasando por `configResolved`
y los hooks del servidor preview. `Config` mezcla el config de archivo
bajo los valores programáticos, y los overrides CLI encima:

```rust
let config = ferrite::Config::default()
    .alias("@", "./src")
    .entry("marketing.html")
    .plugin(MyPlugin);
```

`Config::resolve()` corre los hooks `config` y da
`(ResolvedConfig, Vec<Arc<dyn Plugin>>)`.

## Modo middleware

```rust
let server = ferrite::create_server(ferrite::Config::default()).await?;
let app = my_router.merge(server.router());
```

Tu router Axum es dueño de puertos, TLS y fallbacks; Ferrite aporta el
pipeline dev, el socket HMR y la caché de transform. Ver [Servidor
dev](dev-server).

## Plugins

Implementa `ferrite::Plugin` (`resolve_id`, `load`, `transform`,
`transform_index_html`, hooks de ciclo), re-exporta desde tu crate y
únete a `default_plugins` (CLI) o `Config::plugin` (API). Reglas:
devuelve `None` para ids ajenos, falla con `FerriteError::Build` en vez
de adivinar. Ver [Plugins](plugins).

## Helpers

Utilidades de paridad Vite, re-exportadas desde la fachada:

- Config: `load_config_from_file(dir)` lee `ferrite.config.*` /
  `vite.config.*` estáticamente (`LoadedConfigFile { path, config,
  warnings }`); `define_config` / `merge_config` equivalen a
  `defineConfig` / `mergeConfig`. Ver [Configuración](configuration).
- Env: `load_env(mode, root, prefixes)` implementa el apilado `.env*`
  con expansión `$VAR` / `${VAR:-default}`;
  `expand_vars(value, loaded)` expande un valor. Ver [Servidor
  dev](dev-server).
- Paths: `normalize_path(path)` (slashes, `.`/`..` léxicos, el
  `normalizePath` de Vite) y `search_for_workspace_root(start)`
  (`pnpm-workspace.yaml`, `lerna.json`, `.git` o `package.json` con
  `workspaces` más cercano).
- SSR: `server.ssr_transform(code, url)`,
  `server.ssr_fix_stacktrace(stack)`, `server.module_runner()`. Ver
  [SSR](ssr).
- Preview/proxy: `PreviewControl` (headers, mounts, reglas proxy),
  `PreviewMount`, `ProxyRule`, `match_proxy`, `rules_from_config`.
- Pipeline de render: `BundleOptions`, `OutputOptions`,
  `ChunkWrapper`, `CachedModuleInfo`, `DynamicImportRequest`,
  `ResolveFileUrlRequest`, `WatchEvent`, `WatchKind`,
  `ServerControl`, `ServerUrls`. Ver [Plugins](plugins) y [Pipeline
  de build](pipeline).

## Errores y tipos

El preludio trae el vocabulario: `Config`, `Environment`,
`EnvironmentKind`, `ModuleId`, `ModuleType`, `ResolvedConfig`,
`UserConfig`, `Target`, `Hash`, `SourceMap`, `VERSION`, más `Plugin`,
`PluginContainer`, `Apply`, `Enforce`, y los helpers `define_config`,
`merge_config`, `load_env`, `normalize_path`,
`search_for_workspace_root`.

```rust
use ferrite::prelude::*;
```

## Testing

`ferrite-test` (spec §67) da proyectos temporales, fixtures y asserts
sin Node:

```rust
let project = ferrite_test::TempProject::new(&[
    ("index.html", "<script type=\"module\" src=\"/src/main.js\"></script>"),
    ("src/main.js", "export const n = 1;"),
]);
// … compila o sirve `project.root`, asserts sobre salidas …
```

La suite `tests/vite-compat/` se construye con estos helpers. Combínalos
con `ferrite compat` (chequeos vivos) y `ferrite inspect --json`
(config resuelta para máquinas) como gates de CI.
