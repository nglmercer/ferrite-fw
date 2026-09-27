---
title: Configuration
order: 4
---

# Configuration

Projects configure Ferrite with `ferrite.toml` at the root, plus an
optional git-ignored `ferrite.local.toml` for machine-local overrides.
Every file is optional; bare projects build with defaults. When no
TOML file exists, Ferrite falls back to a statically parsed
`ferrite.config.*` (or `vite.config.*`); TOML always wins when both
exist. Precedence:

1. CLI flags (`--host`, `--port`, `--out-dir`, …)
2. `ferrite.local.toml`
3. `ferrite.toml`
4. `ferrite.config.*` / `vite.config.*` (only when no TOML exists)
5. Built-in defaults

`ferrite inspect` prints the merged result; `ferrite inspect --json`
emits it for scripts.

## Minimal example

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
dev_strategy = "rewrite" # or "import-map"

[react]
refresh = true
```

## JS config files

`ferrite.config.mts|.cts|.ts|.tsx|.mjs|.cjs|.js|.jsx` (falling back to
`vite.config.*` with the same extensions) loads with no JS runtime:
Ferrite parses the file with Oxc and evaluates it statically. Only
plain data is read — string/number/boolean literals, arrays, and
object literals, optionally wrapped in `defineConfig(...)`, with
top-level `const` locals followed through `export default NAME` or
`module.exports = { ... }`.

```js
// ferrite.config.ts
export default {
  base: "/",
  server: { port: 5173, proxy: { "/api": "http://localhost:3000" } },
  resolve: { alias: { "@": "./src" } },
};
```

Supported keys mirror the TOML sections below: `root`, `base`,
`mode`, `define` (scalars), `server` (`host`, `port`, `strictPort`,
`open`, `hmr`, `middlewareMode`, `proxy` as prefix → URL or
`{ target }` objects), `build` (`outDir`, `sourcemap`, `minify`,
`target`, `lib`), `resolve` (object or `[{ find, replacement }]`
aliases — regex-looking `find` warns since aliases are literal
prefixes — plus `conditions`, `extensions`, `preserveSymlinks`),
`envPrefix` / `env.prefix`, `ssr` (`external`, `noExternal`;
`noExternal: true` bundles everything), `npm`, `compiler`. Anything
dynamic — other function calls, unresolvable identifiers, spreads,
`plugins: [react()]` — is skipped with a warning per key; a
non-object default export is an actionable error. Call
`ferrite::load_config_from_file(dir)` (the `loadConfigFromFile`
equivalent) to surface warnings as `LoadedConfigFile { path, config,
warnings }`; the `define_config` / `merge_config` helpers match
`defineConfig` / `mergeConfig`. Prefer TOML for new projects: it is
the documented surface and always wins.

## Top level

| Key | Default | Purpose |
|---|---|---|
| `root` | config file dir | Project root (rarely set; CLI `--config` wins). |
| `base` | `/` | Public base path for built URLs. |
| `mode` | `development` / `production` | `development` in dev, `production` in build. |
| `define` | `{}` | Compile-time string replacements (`{ "process.env.X": "\"y\"" }`). |

## [server]

| Key | Default | Purpose |
|---|---|---|
| `host` | `127.0.0.1` | Bind host. |
| `port` | `5173` | Bind port (`preview` defaults to `4173`). |
| `strict_port` | `false` | Fail instead of picking a free port when busy. |
| `open` | `false` | Open a browser on start. |
| `hmr` | `true` | Enable HMR broadcast (`--no-hmr` disables). |
| `middleware_mode` | `false` | Embed via `server.router()` instead of listening. |
| `proxy` | `{}` | Dev/preview proxy rules: path prefix → target origin. |

```toml
[server.proxy]
"/api" = "http://localhost:3000"
```

Longest prefix wins; the path is preserved on forward. See [Dev
server](dev-server).

## [build]

| Key | Default | Purpose |
|---|---|---|
| `out_dir` | `dist` | Output directory. |
| `sourcemap` | `true` | `true` (external), `false`, `"inline"`, or `"hidden"`. |
| `minify` | `true` | Minify + re-minify after tree-shaking. |
| `target` | `es2022` | Compilation target. |
| `entries` | `["index.html"]` | HTML or module entries. |
| `scope_hoist` | `false` | Concatenate each entry closure (see [Build pipeline](pipeline)). |
| `[build.lib]` | none | Library mode: `entry`, `name`, `formats = ["es", "cjs"]`. |

## [ssr]

| Key | Default | Purpose |
|---|---|---|
| `entry` | `src/server.rs` | SSR entry (enables the `ssr` env in `build_app`). |
| `external` | `[]` | Deps to leave external (Node builtins always are). |
| `no_external` | `[]` | Deps to always bundle. |
| `bundle_all` | `false` | Bundle every dependency. |

`.node` binaries are always external; see [SSR](ssr).

## [resolve]

| Key | Default | Purpose |
|---|---|---|
| `conditions` | `["browser", "module", "import"]` | `exports` conditions. |
| `extensions` | `.mjs .js .mts .ts .jsx .tsx .json` | Probing order. |
| `alias` | `{}` | Import aliases (`"@" = "./src"`). |
| `preserve_symlinks` | `false` | Keep symlinks instead of resolving them. |

See [Packages & resolution](npm).

## [npm] / [compiler] / [env]

```toml
[npm]
registry = "https://registry.npmjs.org"
lockfile = "ferrite.lock"
dev_strategy = "rewrite" # or "import-map"

[compiler]
engine = "oxc" # or "swc" (needs `--features swc`)

[env]
prefix = ["FERRITE_", "PUBLIC_"]
```

- `dev_strategy = "import-map"` leaves bare specifiers untouched in dev
  and injects an inline `<script type="importmap">`; builds keep hashed
  rewrites. SSR keeps server-side rewriting either way.
- Only `FERRITE_*` / `PUBLIC_*` (or your `prefix`) reach
  `import.meta.env`; the rest stay server-side.

## [remote] / [node_compat]

```toml
[remote]
enabled = false
allow = ["esm.example", "*.cdn.example"]

[node_compat]
enabled = true
mode = "browser-shims"
```

Remote `https://` imports are disabled by default; plain `http` works
only for loopback. See [Packages & resolution](npm).

## [package] / [react] / [runtime]

```toml
[package]
standalone = false
embed_assets = true
compress_assets = true
# target = "x86_64-unknown-linux-musl"

[react]
refresh = true
runtime = "automatic" # or "classic"

[runtime]
backend = "auto" # auto | none | napi-vm
fuel_budget = 0  # 0 = engine default
loop_budget = 0
native_allow = ["native/addon.node"]

[runtime.native_integrity]
"native/addon.node" = "<64 hex chars>"
```

See [Production](production), [Frameworks](frameworks), and
[Runtime](runtime).

## Conventions

- Dev serves native ESM per module; bare imports rewrite to
  `/@npm/<pkg>@<ver>/…`, or stay bare under `import-map`.
- Virtual modules resolve to `\0…` internally and serve at `/@id/…`.
- Asset imports rewrite to `…?asset-shim` (a JS URL export); plain
  URLs serve raw bytes.
- Stylesheet `<link>`s rewrite to `…?direct` (CSS, not the JS wrapper).
- npm installs live in `.ferrite/npm/packages/`; `ferrite.lock` pins them.

## Environments

`ferrite build` produces the `client` environment, plus `ssr` when an
SSR entry is configured. Use `--env` (or `Config::entry`) to build one
environment alone. Each environment gets its own target, defines, and
module graph.
