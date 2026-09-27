---
title: Vendor Tailwind
order: 7
---

# Vendor Tailwind

Ferrite estiliza este sitio con CSS de utilidades compilado desde una
copia vendored de [tailwind-rs](https://github.com/nglmercer/tailwind-rs)
(`utilitycss`: un compilador de utilidades agnóstico del runtime, en Rust).

## Por qué vendored

- **Builds herméticos**: el CSS de docs nunca depende de un upstream vivo.
- **Semántica fijada**: el pin en `vendor/README.md` nombra el commit
  exacto que usa cada build.
- **Diffs auditables**: re-vendorear es un reemplazo plano de directorio.

## Layout

```text
vendor/
  README.md                     # origen, pin, procedimiento de update
  materialize-tailwind-manifests.py  # inlina herencia del workspace
  tailwind-rs/                  # upstream prístino + manifests materializados
    crates/utilitycss-compiler  # la API que usa ferrite-tailwind
    crates/utilitycss-span      # SourceId
    ...                         # resto del workspace upstream
```

`ferrite-tailwind` enlaza `utilitycss-compiler`, `utilitycss-span` y
`utilitycss-theme`; los demás crates vienen de referencia y para
futuros extractores.

## Extensión del tema

El tema default de upstream es mínimo a propósito (pre-1.0): pocos
hues en `500`, red en `50`/`600`, gray en `50`/`100`/`500`/`900`.
Ferrite lo extiende por la API pública `Theme::builder()` con las
escalas stone y orange completas más el ancho `6xl` (`ferrite_theme()`
en `crates/ferrite-tailwind`). No se modifica fuente vendored.

Algunas familias de utilidades aún faltan upstream y se cubren con un
bloque compat en `site/src/docs.css` bajo nombres de clase idénticos:
`auto` en márgenes, anchos de borde por lado, `leading-6`, `shrink-0`
y `cursor-pointer`. Cada línea está marcada para borrarse cuando
upstream la emita.

## Actualizar

```bash
rm -rf vendor/tailwind-rs
git clone https://github.com/nglmercer/tailwind-rs /tmp/tailwind-rs
cp -r /tmp/tailwind-rs vendor/tailwind-rs
rm -rf vendor/tailwind-rs/.git
python3 vendor/materialize-tailwind-manifests.py
# registra el nuevo HEAD en vendor/README.md
cargo test -p ferrite-tailwind
```
