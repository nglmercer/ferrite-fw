---
title: Servidor dev
order: 5
---

# Servidor dev

`ferrite dev` sirve ESM nativo por módulo: resolve → load → transform →
reescritura de imports → serve. No hay bundle en dev, así que los edits
se ven recargando (o al instante vía HMR) exactamente un módulo.

```bash
ferrite dev [--host 127.0.0.1] [--port 5173] [--open] [--no-hmr]
```

## Pipeline

1. **Resolve** del specifier (`ferrite-resolver`: alias, npm,
   `exports`/`imports`, conditions, extensiones).
2. **Load** vía hooks de plugins (Markdown, SFCs, glue WASM, `?raw`).
3. **Transform** con Oxc (o SWC opt-in): strip de TS, JSX, lowering.
4. **Reescritura** de desnudos a `/@npm/<pkg>@<ver>/…` (o se dejan
   desnudos con `[npm] dev_strategy = "import-map"`).
5. **Serve** con ETag + cachés; los errores salen como overlays ruidosos.

Los virtuales resuelven a `\0…` internos y sirven en `/@id/…`.
Los assets se reescriben a `…?asset-shim` (export JS de URL); las URLs
planas sirven bytes crudos. Los `<link>` de estilos van a `…?direct`.

## HMR

El servidor observa el proyecto (ignorando `node_modules`, `dist`,
`target`, `.ferrite`) y difunde por WebSocket:

- `ferrite-hmr` calcula el boundary desde el grafo (`plan_update`):
  los importadores exactos aceptan, el resto recarga completa.
- El cliente (`ferrite_hmr::client_source`, tipado en
  `packages/ferrite-client`) aplica el update o recarga la página.
- `import.meta.hot` se reescribe por módulo; React Refresh añade su
  preamble/footer solo en dev (ver [Frameworks](frameworks)).
- Apágalo con `--no-hmr` o `[server] hmr = false`.

```js
if (import.meta.hot) {
  import.meta.hot.accept((next) => render(next));
}
```

## Import maps (solo dev)

```toml
[npm]
dev_strategy = "import-map" # rewrite (default) | import-map
```

Con `import-map`, dev deja los specifiers desnudos e inyecta un
`<script type="importmap">` inline (del entry closure) antes del primer
module script. Los builds conservan reescrituras hasheadas; SSR sigue
reescribiendo en servidor. Úsalo cuando quieras que el navegador
resuelva los desnudos (shims de CDN, debug de externals).

## Variables de entorno

Los `.env` cargan por modo (`.env`, `.env.[mode]`, `.env.local`);
solo `FERRITE_*` / `PUBLIC_*` (configurable en `[env] prefix`) llegan a
`import.meta.env`. El resto queda en servidor. Los `define` aplican encima.

## Modo middleware

Embebe el pipeline dev en tu app Axum en vez de escuchar:

```rust
let server = ferrite::create_server(ferrite::Config::default()).await?;
let app = my_router.merge(server.router());
```

Pon `[server] middleware_mode = true` (o simplemente no llames `listen()`).
HMR, watcher y caché de transform siguen funcionando; tu router es dueño
de puertos, TLS y rutas fallback.

## Manejo de HTML

Los entry scripts se descubren desde `index.html` (módulos primero).
El pipeline corre pre hooks → rewrites core → hooks normales → post hooks:
reescritura de desnudos, inyección del cliente HMR, preloads/tags y el
import map de dev. Ver [Configuración](configuration) para entries.
