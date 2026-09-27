---
title: Producción
order: 6
---

# Producción

`ferrite build` convierte el grafo de módulos en assets hasheados más
un manifiesto. Cada feature de producción está cubierta por la suite de
tests del workspace y por el build de este mismo sitio.

## Pipeline

1. **Resolve + load** por hooks de plugins (Markdown, SFCs, glue WASM).
2. **Transform** con Oxc (o el backend SWC opt-in, `--features swc`).
3. **Tree-shake** de exports sin uso por sentencia, luego re-minify.
4. **Chunks o hoist**: chunks hasheados por defecto, o `--scope-hoist`
   para concatenar cada entry closure en un archivo.
5. **Extracción de CSS**: un `.css` hasheado por módulo de estilos,
   inyectado como tags `<link>` en el HTML en orden de `@import`.
6. **Emisión de manifiesto** mapeando ids fuente a salidas hasheadas.

## Binario único

```bash
ferrite build --standalone
./dist/ferrite-standalone # sirve el sitio embebido, sin dist/ al lado
```

El scaffold standalone embebe `dist/` con `include_bytes!` + gzip y lo
sirve por HTTP. Suma `--target` para cross-compilar (musl está
verificado en tests ignorados estilo CI).

## Verifica antes de shipear

```bash
ferrite build site --standalone
PORT=8080 ./site/dist/ferrite-site
curl -s localhost:8080/ | head -c 200
```

Un chequeo en verde es: HTTP 200, los tags `<script>` y `<link>`
presentes, las URLs de assets hasheadas resolviendo, y ningún `dist/`
hermano necesario junto al binario. La prueba de este repo es este
sitio de docs: `site/` compila y sirve por exactamente esta vía.
