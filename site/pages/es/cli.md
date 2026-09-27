---
title: Referencia CLI
order: 3
---

# Referencia CLI

```bash
ferrite <comando> [opciones]
ferrite --config ./ferrite.toml <comando>
```

## Comandos

| Comando    | Propósito                                            |
|------------|------------------------------------------------------|
| `dev`      | Inicia el servidor dev con HMR                       |
| `ssr`      | Servidor dev en modo SSR                             |
| `build`    | Bundle de producción en `dist/`                      |
| `preview`  | Sirve un build de producción en local                |
| `compat`   | Auto-chequeos en vivo contra el pipeline real        |
| `inspect`  | Muestra config resuelta / grafo de módulos           |
| `transform`| Pasa un archivo por el pipeline de transform        |
| `create`   | Crea un proyecto nuevo                               |
| `install` / `add` / `remove` / `update` | gestión de paquetes npm |
| `migrate`  | Migra una config de Vite a `ferrite.toml`            |
| `clean`    | Borra cachés de build                                |

## Opciones de build

```bash
ferrite build [root] [--out-dir dist] [--minify] [--env client]
ferrite build --standalone [--target x86_64-unknown-linux-musl]
ferrite build --scope-hoist
```

- `--standalone` embebe `dist/` en un binario servidor autocontenido.
- `--target` cross-compila ese binario (solo triples validados).
- `--scope-hoist` concatena cada entry closure en un solo módulo.
- `--env` compila un solo entorno en vez de cliente + SSR.

## Opciones de dev

`ferrite dev` acepta `--host`, `--port`, `--no-hmr` y `--open`.
La precedencia siempre es: flags CLI, luego `ferrite.local.toml`,
luego `ferrite.toml`, luego los defaults internos.
