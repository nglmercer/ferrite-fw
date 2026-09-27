---
title: CLI reference
order: 3
---

# CLI reference

```bash
ferrite <command> [options]
ferrite --config ./ferrite.toml <command>
ferrite --mode production <command>
ferrite --log-level debug <command>
```

Global flags apply to every command. Precedence is always CLI flags,
then `ferrite.local.toml`, then `ferrite.toml`, then built-in defaults.
Run `ferrite <command> --help` for the authoritative flag list; this page
documents intent and examples.

## Global flags

| Flag | Default | Purpose |
|---|---|---|
| `--config <path>` | project root | Config file or directory containing `ferrite.toml`. |
| `--mode <mode>` | `development` (dev) / `production` (build) | Build mode; selects `is_production` and env files. |
| `--log-level <level>` | `info` | `trace`, `debug`, `info`, `warn`, `error`. |

## Commands

| Command | Purpose |
|---|---|
| `dev [root]` | Start the dev server with HMR. |
| `ssr [root]` | Dev server in SSR mode (shell or `napi-vm` entry-server). |
| `build [root]` | Production bundle into `dist/`. |
| `preview [root]` | Serve a production build locally. |
| `add <spec...>` | Add npm packages (no Node required). |
| `remove <name...>` | Remove npm packages. |
| `update [spec...]` | Update npm packages (default: all). |
| `install` | Install locked/declared npm packages. |
| `inspect [root]` | Show resolved config, plugins, lockfile. |
| `transform <file>` | Run one file through the transform pipeline. |
| `migrate <file>` | Migrate a Vite config to `ferrite.toml`. |
| `compat` | Live self-checks against the real pipeline. |
| `clean` | Remove `.ferrite/` caches. |
| `create <name>` | Scaffold a new project. |

## dev / ssr

```bash
ferrite dev [--host 127.0.0.1] [--port 5173] [--open] [--no-hmr] [--runtime auto]
ferrite ssr --runtime napi-vm
```

- Serves native ESM per module; HMR runs over WebSocket unless `--no-hmr`.
- `--open` launches a browser; `--host`/`--port` override `[server]`.
- `--runtime` selects the SSR backend: `auto` (default, no JS engine),
  `none` (force off), or `napi-vm` (requires the `napi-vm` cargo feature).
- `ssr` serves the `index.html` shell with preload injection, or the
  `src/entry-server.*` `render(url)` export when `napi-vm` is selected.
- See [Dev server](dev-server) and [SSR](ssr) for the full loop.

## build

```bash
ferrite build [root] [--out-dir dist] [--minify] [--env client]
ferrite build --standalone [--target x86_64-unknown-linux-musl]
ferrite build --scope-hoist
```

- `--out-dir` overrides `[build] out_dir`; `--minify` defaults to true
  (pass `--minify=false` to disable).
- `--env` builds one environment (`client` or `ssr`) instead of both.
- `--standalone` embeds `dist/` into a self-contained server binary via
  `include_bytes!` + gzip; reported as `standalone_binary`.
- `--target` cross-compiles that binary with `cargo build --target`
  (validated triples only; web assets are target-independent).
- `--scope-hoist` concatenates each entry closure into one module;
  graphs that cannot be proven safe fall back to chunked output.
- See [Production](production) and [Build pipeline](pipeline).

## preview

```bash
ferrite preview [--port 4173]
```

Serves the last `dist/` over HTTP for a production-fidelity check:
static files, SPA fallback, `[server] proxy` rules, and the plugin
preview hooks (`configResolved`, `configure_preview_server`, then
`configure_preview`) with the default plugin set — extra headers,
mounts, and proxy rules included. See [Plugins](plugins). SSR routes
still need the standalone binary or `ferrite ssr`.

## npm: add / remove / update / install

```bash
ferrite add react react-dom three
ferrite add lodash --dev
ferrite remove lodash
ferrite update
ferrite update react@latest
ferrite install
```

- Specs look like `react`, `three@latest`, `@scope/name@^1.0.0`.
- Packages install into `.ferrite/npm/packages/`; `ferrite.lock` pins them.
- `--dev` records a dev dependency; `--root` targets another project.
- No `node`, `npm`, or `node_modules` is ever required.
- See [Packages & resolution](npm).

## inspect

```bash
ferrite inspect
ferrite inspect --json
```

Prints the resolved config (root, mode, server, build, compiler),
the active plugin list, and locked packages. `--json` emits
machine-readable output for scripts and CI. See
[Troubleshooting](troubleshooting).

## transform

```bash
ferrite transform src/main.ts
ferrite transform src/app.tsx --out /tmp/app.js --sourcemap
```

Runs one file through resolve + transform + import rewriting and prints
the result. `--out` writes to a file; `--sourcemap` emits a `.map` next
to `--out` (or an inline comment to stdout). Useful for debugging the
[Build pipeline](pipeline) without a full build.

## migrate

```bash
ferrite migrate vite.config.js
ferrite migrate vite.config.ts --out ferrite.toml
```

Best-effort Vite → `ferrite.toml` translation (server host/port, `outDir`,
`sourcemap`, aliases, defines). Output is commented and always needs a
human review; unknown keys produce warnings, never silent drops. See
[Troubleshooting](troubleshooting). Static `vite.config.*` files also
load directly without migrating (see
[Configuration](configuration)); use `migrate` when you want TOML.

## compat / clean / create

```bash
ferrite compat
ferrite clean
ferrite create my-app
ferrite create my-ssr --template ssr
```

- `compat` runs live checks (resolver, Oxc transform, HMR plan, CSS,
  manifest round-trip) and prints `PASS`/`FAIL` per check.
- `clean` removes `.ferrite/` (npm installs, transform/remote caches).
  Fresh checkouts refetch; nothing outside `.ferrite/` is touched.
- `create` scaffolds `vanilla` (default) or `ssr` templates: `index.html`,
  a module entry, and a starter `ferrite.toml`.
