---
title: Pipeline de build
order: 9
---

# Pipeline de build

Dev y build comparten un pipeline (`ferrite-transform` + `ferrite-graph`
+ `ferrite-bundler`): el bundler consume módulos con el mismo
`ModuleLoader` del servidor, así que lo que ves en dev es lo que sale.

## Transform

`JsCompiler` es el boundary estable: grafo, servidor y bundler solo ven
`ParsedModule` / `TransformResult`, nunca ASTs del motor.

- **Oxc (default)**: strip de TS, JSX automatic/classic, lowering,
  rewrites de `import.meta.hot`, reemplazo de defines, inyección del
  cliente HMR.
- **SWC (opt-in)**: compila con `--features swc` (o
  `[compiler] engine = "swc"`), cambiando el motor de transform/minify
  a `swc_core` (strip TS, JSX, lowering es2015–2022, DCE top-level).
  Los builds default conservan el frontend Oxc de costo cero.

Depura un archivo sin un build completo:

```bash
ferrite transform src/app.tsx --out /tmp/app.js --sourcemap
```

## Bundler

`ferrite build` recorre cada entry closure, trocea, hashea y emite
(`ferrite-bundler`, spec §37–§39):

1. Resolve + load vía hooks de plugins.
2. Transform (Oxc o SWC).
3. Tree-shake a nivel de sentencia, luego re-minify.
4. Chunks (hasheados, un ESM por módulo) o `--scope-hoist`.
5. Extracción de CSS (un `.css` hasheado por hoja, orden `@import`).
6. Emisión del manifiesto + source maps.

## Tree-shaking

On por default en producción (`BundleRequest.treeshake`). Los exports
no usados caen por sentencia con un fixpoint de used-exports en el
grafo; los barrel re-exports sobreviven si algún consumidor los necesita.

## Scope hoisting

```bash
ferrite build --scope-hoist
```

```toml
[build]
scope_hoist = true
```

Concatena cada entry closure en un módulo con locales prefijados
`$f{index}$`. Los grafos no demostrablemente seguros (namespaces,
`eval`, …) vuelven a chunks en vez de miscompilar.

## Source maps

`[build] sourcemap`: `true` (externo, default), `false`, `"inline"` o
`"hidden"` (se emite, sin comentario). El minify emite su propio mapa y
lo encadena con el de transform
(`ferrite_transform::chain_source_maps`), así que lo minificado sigue
apuntando a las fuentes. Lo irresoluble pasa sin fuente.

## Manifiesto

`ferrite-manifest` (spec §40) escribe un manifiesto cliente compatible
con Vite (`file`, `src`, `isEntry`, `css`, `imports`, `dynamicImports`)
más un mapa SSR módulo→chunk. El servidor standalone y los loaders SSR
lo leen para resolver URLs hasheadas en runtime.
