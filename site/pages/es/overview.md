---
title: Resumen
order: 1
---

# Ferrite

Ferrite es una toolchain web nativa de Rust: servidor de desarrollo,
empaquetador de producción y runtime SSR en un solo binario. Habla las
convenciones de Vite (ESM nativo en dev, hooks de plugins, import maps)
con un pipeline basado en Oxc y sin necesidad de Node.js para el flujo
principal.

Este sitio está construido con el propio Ferrite: las páginas Markdown
se convierten en módulos JS mediante el plugin `ferrite:markdown`, las
utilidades de maquetación se compilan desde el workspace vendored
`tailwind-rs`, y todo se distribuye como un único binario.

## Qué obtienes

- **Servidor dev** con ESM nativo por módulo, HMR y soporte de import maps.
- **Builds de producción** con assets hasheados, manifiesto, extracción
  de CSS, tree-shaking a nivel de sentencia y scope hoisting opcional.
- **Frameworks**: React Refresh en dev, más experimentos SFC de Vue/Svelte.
- **SSR**: adaptadores, `ssrLoadModule`, externals, streaming, islands, RPC.
- **Binario único**: embebe `dist/` y sírvelo sin nada más al lado.
- **Markdown + CSS de utilidades**: esta stack de docs, sin toolchain JS.

## Empieza aquí

¿Nuevo en Ferrite? Lee [Primeros pasos](getting-started): instalación,
tu primer proyecto y el ciclo dev. Después elige una ruta:

| Ruta     | Páginas |
|----------|---------|
| Develop  | [Servidor dev](dev-server), [CSS](css), [Assets](assets) |
| Depend   | [Paquetes y resolución](npm) |
| Render   | [SSR](ssr), [Runtime](runtime), [Frameworks](frameworks), [WASM](wasm) |
| Build    | [CLI](cli), [Configuración](configuration), [Pipeline de build](pipeline) |
| Extender | [Plugins](plugins), [API Rust](api) |
| Ship     | [Producción](production), [Solución de problemas](troubleshooting) |
| Vendored | [Vendor Tailwind](tailwind-vendor) |

> Estos docs viven en `site/pages/*/*.md` del repo de Ferrite y son el
> objetivo de verificación de producción: si este sitio compila y sirve,
> el estado de integración del framework está en verde.

## Tema e idiomas

Usa los controles del header para cambiar entre modo claro y oscuro
(se guarda en local, por defecto sigue tu SO) y entre inglés, español
(`es`) y chino (`cn`). Cada idioma trae las mismas dieciocho páginas bajo
`#/{idioma}/{pagina}`.
