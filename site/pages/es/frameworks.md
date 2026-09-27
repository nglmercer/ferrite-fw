---
title: Frameworks
order: 12
---

# Frameworks

`ferrite-frameworks` trae un plugin React más soporte experimental de
single-file components Vue/Svelte. Los tres están on por default en el
CLI; cada uno es un `Plugin` plano, así que los usuarios programáticos
pueden quitar cualquiera.

## React

```toml
[react]
refresh = true
runtime = "automatic" # o "classic"
```

- Preamble de Refresh + footers por módulo solo en dev: los edits de
  componentes se intercambian con estado; el resto cae a HMR.
- `detect_components` encuentra componentes por archivo; `refresh_footer`
  los registra con el runtime de React Refresh.
- `runtime = "automatic"` usa el transform JSX; `classic` conserva
  `React.createElement`. El motor (Oxc/SWC) maneja ambos.
- Los builds de producción eliminan todo Refresh — costo cero.

```bash
ferrite add react react-dom
```

## Vue (experimental)

`VuePlugin` parte los `.vue` en bloques script/style servidos como
módulos virtuales. `<script setup>` se reconoce; el bloque template hoy
compila a un stub explícito (`template_stub`) — la compilación completa
es roadmap, y el stub lo dice ruidosamente en el call site.

## Svelte (experimental)

`SveltePlugin` parte los `.svelte` igual (`split_svelte`, extracción de
markup, `markup_stub`). La reactividad del script compila por el
pipeline JS normal; el template es un stub explícito.

## SSR + islands

Los tres renderizan con los mismos adaptadores [SSR](ssr) e hidratan
como [islands](ssr): HTML server-side más un closure cliente por
island. El framework nunca cambia el dev server, el manifiesto ni el
empaquetado standalone.

## Ejemplos

`examples/react` es la app de referencia (dev, HMR, Refresh, build).
`examples/vanilla-ts` muestra la base sin framework; `examples/ssr` el
render de servidor; `examples/rust-wasm` una UI con [WASM](wasm).
