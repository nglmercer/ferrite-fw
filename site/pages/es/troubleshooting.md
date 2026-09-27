---
title: Solución de problemas
order: 17
---

# Solución de problemas

Cuando algo se ve mal, usa primero las herramientas ruidosas: `compat`
chequea el pipeline, `inspect` muestra el estado resuelto, `transform`
aísla un archivo. Ferrite falla ruidosamente en vez de adivinar — los
errores de abajo son el sistema funcionando como diseñado.

## compat

```bash
ferrite compat
```

Auto-chequeos vivos contra el pipeline real (sin Node): resolver,
transform Oxc, plan HMR, CSS, round-trip de manifiesto. Cada uno imprime
`PASS`/`FAIL`; un `FAIL` nombra el subsistema a investigar.

## inspect

```bash
ferrite inspect
ferrite inspect --json
```

Muestra el config mezclado (root, mode, server, build, compiler),
plugins activos y paquetes bloqueados. `--json` es para scripts y CI. Si
dev y build discrepan, difumina `inspect` bajo cada `--mode`.

## transform

```bash
ferrite transform src/sospechoso.tsx
ferrite transform src/sospechoso.tsx --out /tmp/out.js --sourcemap
```

Pasa un archivo por resolve + transform + reescritura. Úsalo para
separar "la sintaxis de este archivo" de "el grafo que lo rodea".

## clean

```bash
ferrite clean
```

Borra `.ferrite/` (instalaciones npm, cachés de transform/remotos). Los
checkouts frescos re-descargan; nada fuera de `.ferrite/` se toca. Ante
una caché rancia, `clean` + rerun es el reset soportado.

## migrate

```bash
ferrite migrate vite.config.js --out ferrite.toml
```

Traducción best-effort de Vite a `ferrite.toml` con comentarios y
avisos. Revisa siempre la salida: alias, defines y proxies suelen
necesitar mano. Las claves desconocidas avisan, nunca caen en silencio.

## Fallos comunes

| Síntoma | Causa probable | Arreglo |
|---|---|---|
| 404 de desnudo en dev | paquete no instalado | `ferrite add <pkg>`; revisa `ferrite.lock` |
| `.node` lanza en runtime | no está en allowlist | añade `[runtime] native_allow` + integridad; requiere `napi-vm` |
| `https://` rechazado | remoto off / host no permitido | `[remote] enabled = true`, `allow = […]` |
| Error de build WASM | falta target/`wasm-bindgen` | instala según el hint; reintenta |
| Página en blanco, sin HMR | `--no-hmr` o `[server] hmr = false` | reactiva; revisa el proxy WS |
| Build difiere de dev | mismatch de env/mode | compara `inspect` por modo; revisa `[env] prefix` |
| Puerto ocupado | el 5173 está tomado | `--port 0`/otro puerto, o `strict_port = true` para fallar rápido |

## Pedir ayuda

El error ruidoso es el bug report: nombra crate, hook y module id.
Reproduce con `transform` o un `TempProject` mínimo (ver [API
Rust](api)), y adjunta el input que falla más `inspect --json`.
