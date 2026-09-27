---
title: Troubleshooting
order: 17
---

# Troubleshooting

When something looks wrong, reach for the loud tools first: `compat`
checks the pipeline, `inspect` shows resolved state, `transform`
isolates one file. Ferrite fails loudly instead of guessing — the
errors below are the system working as designed.

## compat

```bash
ferrite compat
```

Live self-checks against the real pipeline (no Node): resolver,
Oxc transform, HMR plan, CSS, manifest round-trip. Each prints
`PASS`/`FAIL`; a `FAIL` names the subsystem to investigate next.

## inspect

```bash
ferrite inspect
ferrite inspect --json
```

Shows the merged config (root, mode, server, build, compiler), active
plugins, and locked packages. `--json` is for scripts and CI. If dev
and build disagree, diff `inspect` under each `--mode`.

## transform

```bash
ferrite transform src/suspect.tsx
ferrite transform src/suspect.tsx --out /tmp/out.js --sourcemap
```

Runs one file through resolve + transform + import rewriting. Use it to
separate "this file's syntax" from "the graph around it".

## clean

```bash
ferrite clean
```

Removes `.ferrite/` (npm installs, transform/remote caches). Fresh
checkouts refetch; nothing outside `.ferrite/` is touched. If a stale
cache is ever suspected, `clean` + rerun is the supported reset.

## migrate

```bash
ferrite migrate vite.config.js --out ferrite.toml
```

Best-effort Vite → `ferrite.toml` translation with comments and
warnings. Always review the output: aliases, defines, and proxy setups
usually need hand-tuning. Unknown keys warn, never silently drop.

## Common failures

| Symptom | Likely cause | Fix |
|---|---|---|
| Bare import 404 in dev | package not installed | `ferrite add <pkg>`; check `ferrite.lock` |
| `.node` throws at runtime | not allowlisted | add `[runtime] native_allow` + integrity; needs `napi-vm` |
| `https://` import refused | remote disabled / host not allowed | `[remote] enabled = true`, `allow = […]` |
| WASM build error | missing target/`wasm-bindgen` | install per the hint; rerun |
| Blank page, no HMR | `--no-hmr` or `[server] hmr = false` | re-enable; check WS proxy |
| Build differs from dev | env/mode mismatch | compare `inspect` per mode; check `[env] prefix` |
| Port busy | default 5173 taken | `--port 0`/another port, or `strict_port = true` to fail fast |

## Getting help

The loud error is the bug report: it names the crate, hook, and module
id. Reproduce with `transform` or a minimal `TempProject` (see [Rust
API](api)), then file the failing input plus `inspect --json`.
