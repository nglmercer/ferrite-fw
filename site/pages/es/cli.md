---
title: Referencia CLI
order: 3
---

# Referencia CLI

```bash
ferrite <comando> [opciones]
ferrite --config ./ferrite.toml <comando>
ferrite --mode production <comando>
ferrite --log-level debug <comando>
```

Los flags globales aplican a todo comando. La precedencia es: flags CLI,
`ferrite.local.toml`, `ferrite.toml`, defaults internos.
`ferrite <comando> --help` es la lista autoritativa; esta página documenta
intención y ejemplos.

## Flags globales

| Flag | Default | Propósito |
|---|---|---|
| `--config <path>` | raíz del proyecto | Archivo o directorio con `ferrite.toml`. |
| `--mode <mode>` | `development` (dev) / `production` (build) | Modo de build; elige `is_production` y env files. |
| `--log-level <level>` | `info` | `trace`, `debug`, `info`, `warn`, `error`. |

## Comandos

| Comando | Propósito |
|---|---|
| `dev [root]` | Inicia el servidor dev con HMR. |
| `ssr [root]` | Servidor dev en modo SSR (shell o entry-server `napi-vm`). |
| `build [root]` | Bundle de producción en `dist/`. |
| `preview [root]` | Sirve un build de producción en local. |
| `add <spec...>` | Añade paquetes npm (sin Node). |
| `remove <name...>` | Quita paquetes npm. |
| `update [spec...]` | Actualiza paquetes npm (default: todos). |
| `install` | Instala los paquetes declarados/bloqueados. |
| `inspect [root]` | Muestra config resuelta, plugins, lockfile. |
| `transform <file>` | Pasa un archivo por el pipeline de transform. |
| `migrate <file>` | Migra una config de Vite a `ferrite.toml`. |
| `compat` | Auto-chequeos en vivo contra el pipeline real. |
| `clean` | Borra cachés `.ferrite/`. |
| `create <name>` | Crea un proyecto nuevo. |

## dev / ssr

```bash
ferrite dev [--host 127.0.0.1] [--port 5173] [--open] [--no-hmr] [--runtime auto]
ferrite ssr --runtime napi-vm
```

- Sirve ESM nativo por módulo; HMR va por WebSocket salvo `--no-hmr`.
- `--open` abre un navegador; `--host`/`--port` sobreescriben `[server]`.
- `--runtime` elige el backend SSR: `auto` (default, sin motor JS),
  `none` (forzado off) o `napi-vm` (requiere el feature cargo `napi-vm`).
- `ssr` sirve el shell `index.html` con preloads, o el export
  `render(url)` de `src/entry-server.*` con `napi-vm`.
- Ver [Servidor dev](dev-server) y [SSR](ssr).

## build

```bash
ferrite build [root] [--out-dir dist] [--minify] [--env client]
ferrite build --standalone [--target x86_64-unknown-linux-musl]
ferrite build --scope-hoist
```

- `--out-dir` sobreescribe `[build] out_dir`; `--minify` default true
  (pasa `--minify=false` para desactivar).
- `--env` compila un solo entorno (`client` o `ssr`) en vez de ambos.
- `--standalone` embebe `dist/` en un binario servidor autocontenido vía
  `include_bytes!` + gzip; se reporta como `standalone_binary`.
- `--target` cross-compila ese binario con `cargo build --target`
  (solo triples validados; los assets web son independientes).
- `--scope-hoist` concatena cada entry closure en un módulo;
  los grafos no demostrablemente seguros vuelven a chunks.
- Ver [Producción](production) y [Pipeline de build](pipeline).

## preview

```bash
ferrite preview [--port 4173]
```

Sirve el último `dist/` por HTTP para un chequeo fiel a producción:
estáticos, fallback SPA, reglas `[server] proxy` y los hooks de
preview de plugins (`configResolved`, `configure_preview_server`,
luego `configure_preview`) con el set default — headers extra, mounts
y reglas de proxy incluidos. Ver [Plugins](plugins). Las rutas SSR
siguen necesitando el binario standalone o `ferrite ssr`.

## npm: add / remove / update / install

```bash
ferrite add react react-dom three
ferrite add lodash --dev
ferrite remove lodash
ferrite update
ferrite update react@latest
ferrite install
```

- Los specs son `react`, `three@latest`, `@scope/name@^1.0.0`.
- Los paquetes se instalan en `.ferrite/npm/packages/`; `ferrite.lock` los fija.
- `--dev` registra dev dependency; `--root` apunta a otro proyecto.
- Nunca se requiere `node`, `npm` ni `node_modules`.
- Ver [Paquetes y resolución](npm).

## inspect

```bash
ferrite inspect
ferrite inspect --json
```

Imprime la config resuelta (root, mode, server, build, compiler),
la lista de plugins activos y los paquetes bloqueados. `--json` emite
salida para scripts y CI. Ver [Solución de problemas](troubleshooting).

## transform

```bash
ferrite transform src/main.ts
ferrite transform src/app.tsx --out /tmp/app.js --sourcemap
```

Pasa un archivo por resolve + transform + reescritura de imports e
imprime el resultado. `--out` escribe a archivo; `--sourcemap` emite un
`.map` junto a `--out` (o un comentario inline a stdout). Útil para
depurar el [Pipeline de build](pipeline) sin un build completo.

## migrate

```bash
ferrite migrate vite.config.js
ferrite migrate vite.config.ts --out ferrite.toml
```

Traducción best-effort de Vite a `ferrite.toml` (host/port, `outDir`,
`sourcemap`, alias, defines). La salida es comentada y siempre requiere
revisión humana; las claves desconocidas avisan, nunca se pierden en
silencio. Ver [Solución de problemas](troubleshooting). Los
`vite.config.*` estáticos también cargan directo sin migrar (ver
[Configuración](configuration)); usa `migrate` si quieres TOML.

## compat / clean / create

```bash
ferrite compat
ferrite clean
ferrite create my-app
ferrite create my-ssr --template ssr
```

- `compat` corre chequeos vivos (resolver, transform Oxc, plan HMR, CSS,
  round-trip de manifiesto) e imprime `PASS`/`FAIL` por chequeo.
- `clean` borra `.ferrite/` (instalaciones npm, cachés de transform/remotos).
  Los checkouts frescos re-descargan; nada fuera de `.ferrite/` se toca.
- `create` genera las plantillas `vanilla` (default) o `ssr`: `index.html`,
  un entry de módulo y un `ferrite.toml` inicial.
