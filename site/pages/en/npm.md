---
title: Packages & resolution
order: 8
---

# Packages & resolution

Ferrite resolves npm without Node.js: registry client, semver
resolution, integrity-checked tarballs into `.ferrite/npm/packages/`,
and a `ferrite.lock` lockfile (`ferrite-npm`, spec §16).

## Installing

```bash
ferrite add react react-dom three   # → .ferrite/npm/
ferrite add lodash --dev
ferrite install                      # locked/declared packages
ferrite update [spec...]             # default: all
ferrite remove lodash
```

Specs look like `react`, `three@latest`, `@scope/name@^1.0.0`.
No `node`, `npm`, or `node_modules` is ever required.

## Dev serving

Bare imports rewrite to `/@npm/<pkg>@<ver>/…` in dev. With
`[npm] dev_strategy = "import-map"`, dev leaves bare specifiers
untouched and injects an inline `<script type="importmap">` collected
from the entry closure; builds always use hashed rewrites and SSR
always rewrites server-side.

## Resolver

`ferrite-resolver` (spec §15) handles relative/absolute imports,
aliases, bare npm imports, package `exports`/`imports`, conditions,
directory indexes, symlinks, CSS/URL imports, and virtual modules.

```toml
[resolve]
conditions = ["browser", "module", "import"]
extensions = [".mjs", ".js", ".mts", ".ts", ".jsx", ".tsx", ".json"]
preserve_symlinks = false

[resolve.alias]
"@" = "./src"
```

Or programmatically: `ferrite::Config::default().alias("@", "./src")`.

## Lockfile

`ferrite.lock` (configurable via `[npm] lockfile`) pins exact versions
plus integrity hashes. `ferrite install` reproduces the lock; `ferrite
add`/`update` re-resolve and rewrite it. `ferrite inspect` lists locked
packages; `--json` includes them for CI.

## Remote imports

```toml
[remote]
enabled = true
allow = ["esm.example", "*.cdn.example"]
```

Disabled by default. Allowed `https://` imports (plain `http` only for
loopback) resolve to virtual modules, fetch once, and cache by URL hash
under `.ferrite/cache/remote/`. Relative imports inside remote modules
rebase onto the remote origin. Fresh checkouts refetch; `ferrite clean`
drops the cache.

## Node compat

`[node_compat] enabled = true` (default) shims `node:*` imports for the
browser (`mode = "browser-shims"`). Server-only builtins stay external
under SSR instead of silently shipping a stub.
