---
title: Paquetes y resolución
order: 8
---

# Paquetes y resolución

Ferrite resuelve npm sin Node.js: cliente de registry, resolución
semver, tarballs con integridad verificada en `.ferrite/npm/packages/` y
lockfile `ferrite.lock` (`ferrite-npm`, spec §16).

## Instalación

```bash
ferrite add react react-dom three   # → .ferrite/npm/
ferrite add lodash --dev
ferrite install                      # paquetes declarados/bloqueados
ferrite update [spec...]             # default: todos
ferrite remove lodash
```

Los specs son `react`, `three@latest`, `@scope/name@^1.0.0`.
Nunca se requiere `node`, `npm` ni `node_modules`.

## Servido en dev

Los desnudos se reescriben a `/@npm/<pkg>@<ver>/…` en dev. Con
`[npm] dev_strategy = "import-map"`, dev deja los specifiers desnudos e
inyecta un `<script type="importmap">` inline del entry closure; los
builds siempre usan reescrituras hasheadas y SSR siempre reescribe en
servidor.

## Resolver

`ferrite-resolver` (spec §15) maneja imports relativos/absolutos, alias,
desnudos de npm, `exports`/`imports` de paquetes, conditions, índices de
directorios, symlinks, imports CSS/URL y módulos virtuales.

```toml
[resolve]
conditions = ["browser", "module", "import"]
extensions = [".mjs", ".js", ".mts", ".ts", ".jsx", ".tsx", ".json"]
preserve_symlinks = false

[resolve.alias]
"@" = "./src"
```

O programático: `ferrite::Config::default().alias("@", "./src")`.

## Lockfile

`ferrite.lock` (vía `[npm] lockfile`) fija versiones exactas más hashes
de integridad. `ferrite install` reproduce el lock; `ferrite add`/`update`
re-resuelven y lo reescriben. `ferrite inspect` lista los paquetes
bloqueados; `--json` los incluye para CI.

## Imports remotos

```toml
[remote]
enabled = true
allow = ["esm.example", "*.cdn.example"]
```

Desactivados por default. Los `https://` permitidos (`http` plano solo
para loopback) resuelven a módulos virtuales, se descargan una vez y se
cachean por hash de URL en `.ferrite/cache/remote/`. Los relativos
dentro de módulos remotos se rebasan al origen remoto. Los checkouts
frescos re-descargan; `ferrite clean` borra la caché.

## Compat Node

`[node_compat] enabled = true` (default) pone shims de `node:*` para el
navegador (`mode = "browser-shims"`). Los builtins solo-servidor quedan
externos bajo SSR en vez de embarcar un stub silencioso.
