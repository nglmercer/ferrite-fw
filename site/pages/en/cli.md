---
title: CLI reference
order: 3
---

# CLI reference

```bash
ferrite <command> [options]
ferrite --config ./ferrite.toml <command>
```

## Commands

| Command    | Purpose                                              |
|------------|------------------------------------------------------|
| `dev`      | Start the dev server with HMR                        |
| `ssr`      | Dev server in SSR mode                               |
| `build`    | Production bundle into `dist/`                       |
| `preview`  | Serve a production build locally                     |
| `compat`   | Live self-checks against the real pipeline           |
| `inspect`  | Show resolved config / module graph                  |
| `transform`| Run one file through the transform pipeline          |
| `create`   | Scaffold a new project                               |
| `install` / `add` / `remove` / `update` | npm package management |
| `migrate`  | Migrate a Vite config to `ferrite.toml`              |
| `clean`    | Remove build caches                                  |

## Build options

```bash
ferrite build [root] [--out-dir dist] [--minify] [--env client]
ferrite build --standalone [--target x86_64-unknown-linux-musl]
ferrite build --scope-hoist
```

- `--standalone` embeds `dist/` into a self-contained server binary.
- `--target` cross-compiles that binary (validated triples only).
- `--scope-hoist` concatenates each entry closure into one module.
- `--env` builds a single environment instead of client + SSR.

## Dev options

`ferrite dev` accepts `--host`, `--port`, `--no-hmr`, and `--open`.
Precedence is always CLI flags, then `ferrite.local.toml`, then
`ferrite.toml`, then built-in defaults.
