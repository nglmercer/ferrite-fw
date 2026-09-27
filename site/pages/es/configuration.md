---
title: Configuración
order: 4
---

# Configuración

Los proyectos configuran Ferrite con `ferrite.toml` en la raíz, más un
opcional `ferrite.local.toml` ignorado por git para overrides locales.
Todos los archivos son opcionales; los proyectos vacíos compilan con defaults.
Sin ningún TOML, Ferrite usa un `ferrite.config.*` (o `vite.config.*`)
parseado estáticamente; el TOML siempre gana si hay ambos. Precedencia:

1. Flags CLI (`--host`, `--port`, `--out-dir`, …)
2. `ferrite.local.toml`
3. `ferrite.toml`
4. `ferrite.config.*` / `vite.config.*` (solo sin TOML)
5. Defaults internos

`ferrite inspect` imprime el resultado mezclado; `ferrite inspect --json`
lo emite para scripts.

## Ejemplo mínimo

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
dev_strategy = "rewrite" # o "import-map"

[react]
refresh = true
```

## Archivos de config JS

`ferrite.config.mts|.cts|.ts|.tsx|.mjs|.cjs|.js|.jsx` (o `vite.config.*`
con las mismas extensiones) carga sin runtime JS: Ferrite parsea el
archivo con Oxc y lo evalúa estáticamente. Solo se leen datos planos —
literales string/número/booleano, arrays y objetos, opcionalmente
envueltos en `defineConfig(...)`, siguiendo los `const` de nivel
superior en `export default NAME` o `module.exports = { ... }`.

```js
// ferrite.config.ts
export default {
  base: "/",
  server: { port: 5173, proxy: { "/api": "http://localhost:3000" } },
  resolve: { alias: { "@": "./src" } },
};
```

Las claves soportadas reflejan las secciones TOML: `root`, `base`,
`mode`, `define` (escalares), `server` (`host`, `port`,
`strictPort`, `open`, `hmr`, `middlewareMode`, `proxy` como prefijo →
URL u objetos `{ target }`), `build` (`outDir`, `sourcemap`,
`minify`, `target`, `lib`), `resolve` (alias como objeto o
`[{ find, replacement }]` — un `find` con pinta de regex avisa porque
los alias son prefijos literales — más `conditions`, `extensions`,
`preserveSymlinks`), `envPrefix` / `env.prefix`, `ssr` (`external`,
`noExternal`; `noExternal: true` empaqueta todo), `npm`, `compiler`.
Lo dinámico — otras llamadas, identificadores irresolubles, spreads,
`plugins: [react()]` — se salta con un warning por clave; un default
export que no sea objeto es un error accionable. Llama a
`ferrite::load_config_from_file(dir)` (el equivalente a
`loadConfigFromFile`) para ver los warnings como `LoadedConfigFile {
path, config, warnings }`; los helpers `define_config` /
`merge_config` equivalen a `defineConfig` / `mergeConfig`. Para
proyectos nuevos prefiere TOML: es la superficie documentada y
siempre gana.

## Nivel superior

| Clave | Default | Propósito |
|---|---|---|
| `root` | dir del config | Raíz del proyecto (raro; `--config` gana). |
| `base` | `/` | Base pública para URLs construidas. |
| `mode` | `development` / `production` | `development` en dev, `production` en build. |
| `define` | `{}` | Reemplazos en compilación (`{ "process.env.X": "\"y\"" }`). |

## [server]

| Clave | Default | Propósito |
|---|---|---|
| `host` | `127.0.0.1` | Host de bind. |
| `port` | `5173` | Puerto (`preview` usa `4173`). |
| `strict_port` | `false` | Fallar en vez de elegir otro puerto si está ocupado. |
| `open` | `false` | Abrir un navegador al arrancar. |
| `hmr` | `true` | Habilitar HMR (`--no-hmr` lo apaga). |
| `middleware_mode` | `false` | Embeber vía `server.router()` sin listen propio. |
| `proxy` | `{}` | Reglas de proxy dev/preview: prefijo de path → origen destino. |

```toml
[server.proxy]
"/api" = "http://localhost:3000"
```

Gana el prefijo más largo; el path se preserva al reenviar. Ver
[Servidor dev](dev-server).

## [build]

| Clave | Default | Propósito |
|---|---|---|
| `out_dir` | `dist` | Directorio de salida. |
| `sourcemap` | `true` | `true` (externo), `false`, `"inline"` o `"hidden"`. |
| `minify` | `true` | Minificar + re-minificar tras tree-shaking. |
| `target` | `es2022` | Target de compilación. |
| `entries` | `["index.html"]` | Entradas HTML o de módulo. |
| `scope_hoist` | `false` | Concatenar cada entry closure (ver [Pipeline de build](pipeline)). |
| `[build.lib]` | ninguno | Modo librería: `entry`, `name`, `formats = ["es", "cjs"]`. |

## [ssr]

| Clave | Default | Propósito |
|---|---|---|
| `entry` | `src/server.rs` | Entry SSR (habilita el env `ssr` en `build_app`). |
| `external` | `[]` | Deps a dejar externas (builtins de Node siempre lo son). |
| `no_external` | `[]` | Deps a empaquetar siempre. |
| `bundle_all` | `false` | Empaquetar todas las dependencias. |

Los binarios `.node` siempre son externos; ver [SSR](ssr).

## [resolve]

| Clave | Default | Propósito |
|---|---|---|
| `conditions` | `["browser", "module", "import"]` | Condiciones de `exports`. |
| `extensions` | `.mjs .js .mts .ts .jsx .tsx .json` | Orden de prueba. |
| `alias` | `{}` | Alias (`"@" = "./src"`). |
| `preserve_symlinks` | `false` | Conservar symlinks en vez de resolverlos. |

Ver [Paquetes y resolución](npm).

## [npm] / [compiler] / [env]

```toml
[npm]
registry = "https://registry.npmjs.org"
lockfile = "ferrite.lock"
dev_strategy = "rewrite" # o "import-map"

[compiler]
engine = "oxc" # o "swc" (requiere `--features swc`)

[env]
prefix = ["FERRITE_", "PUBLIC_"]
```

- `dev_strategy = "import-map"` deja los specifiers desnudos en dev e
  inyecta un `<script type="importmap">` inline; los builds conservan
  reescrituras hasheadas. SSR sigue reescribiendo en servidor.
- Solo `FERRITE_*` / `PUBLIC_*` (o tu `prefix`) llegan a
  `import.meta.env`; el resto queda en servidor.

## [remote] / [node_compat]

```toml
[remote]
enabled = false
allow = ["esm.example", "*.cdn.example"]

[node_compat]
enabled = true
mode = "browser-shims"
```

Los imports remotos `https://` están desactivados por default; `http`
plano solo vale para loopback. Ver [Paquetes y resolución](npm).

## [package] / [react] / [runtime]

```toml
[package]
standalone = false
embed_assets = true
compress_assets = true
# target = "x86_64-unknown-linux-musl"

[react]
refresh = true
runtime = "automatic" # o "classic"

[runtime]
backend = "auto" # auto | none | napi-vm
fuel_budget = 0  # 0 = default del motor
loop_budget = 0
native_allow = ["native/addon.node"]

[runtime.native_integrity]
"native/addon.node" = "<64 hex>"
```

Ver [Producción](production), [Frameworks](frameworks) y [Runtime](runtime).

## Convenciones

- Dev sirve ESM nativo por módulo; los desnudos se reescriben a
  `/@npm/<pkg>@<ver>/…`, o quedan desnudos con `import-map`.
- Los virtuales resuelven a `\0…` internos y sirven en `/@id/…`.
- Los assets se reescriben a `…?asset-shim` (export JS de URL); las
  URLs planas sirven bytes crudos.
- Los `<link>` de estilos se reescriben a `…?direct` (CSS, no el wrapper JS).
- Las instalaciones npm viven en `.ferrite/npm/packages/`; `ferrite.lock` las fija.

## Entornos

`ferrite build` produce el entorno `client`, más `ssr` cuando hay entry
SSR configurado. Usa `--env` (o `Config::entry`) para compilar uno solo.
Cada entorno tiene su target, defines y grafo de módulos.
