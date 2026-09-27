---
title: Configuration
order: 4
---

# Configuration

Projects configure Ferrite with `ferrite.toml` at the root (plus an
optional git-ignored `ferrite.local.toml` for machine-local overrides).
Every file is optional; bare projects build with defaults.

## Example

```toml
# ferrite.toml
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

## Conventions

- Dev serves native ESM per module; bare imports rewrite to
  `/@npm/<pkg>@<ver>/…`, or stay bare under the `import-map` strategy.
- Virtual modules resolve to `\0…` internally and serve at `/@id/…`.
- Asset imports rewrite to `…?asset-shim` (a JS URL export); plain
  URLs serve raw bytes.
- Stylesheet `<link>`s rewrite to `…?direct` (CSS, not the JS wrapper).
- npm installs live in `.ferrite/npm/packages/`; `ferrite.lock` pins them.
- Only `FERRITE_*` / `PUBLIC_*` env vars reach `import.meta.env`.

## Environments

`ferrite build` produces the `client` environment, plus `ssr` when an
SSR entry is configured. Use `--env` to build one environment alone.
