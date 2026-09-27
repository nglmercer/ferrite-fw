---
title: Configuración
order: 4
---

# Configuración

Los proyectos configuran Ferrite con `ferrite.toml` en la raíz (más un
opcional `ferrite.local.toml` ignorado por git para overrides locales).
Todos los archivos son opcionales; los proyectos vacíos compilan con defaults.

## Ejemplo

```toml
# ferrite.toml
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

## Convenciones

- Dev sirve ESM nativo por módulo; los imports desnudos se reescriben a
  `/@npm/<pkg>@<ver>/…`, o se quedan desnudos con `import-map`.
- Los módulos virtuales resuelven a `\0…` internos y sirven en `/@id/…`.
- Los imports de assets se reescriben a `…?asset-shim` (export JS de URL);
  las URLs planas sirven bytes crudos.
- Los `<link>` de hojas de estilo se reescriben a `…?direct` (CSS, no el wrapper JS).
- Las instalaciones npm viven en `.ferrite/npm/packages/`; `ferrite.lock` las fija.
- Solo las vars `FERRITE_*` / `PUBLIC_*` llegan a `import.meta.env`.

## Entornos

`ferrite build` produce el entorno `client`, más `ssr` cuando hay un
entry SSR configurado. Usa `--env` para compilar un solo entorno.
