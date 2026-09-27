---
title: Tailwind vendor
order: 7
---

# Tailwind vendor

Ferrite styles this site with utility CSS compiled from a vendored
copy of [tailwind-rs](https://github.com/nglmercer/tailwind-rs)
(`utilitycss`: a runtime-agnostic utility compiler written in Rust).

## Why vendored

- **Hermetic builds**: docs CSS never depends on a live upstream.
- **Pinned semantics**: the pin in `vendor/README.md` names the exact
  upstream commit every build uses.
- **Auditable diffs**: re-vendoring is a plain directory replacement.

## Layout

```text
vendor/
  README.md                     # origin, pin, update procedure
  materialize-tailwind-manifests.py  # inlines workspace inheritance
  tailwind-rs/                  # pristine upstream + materialized manifests
    crates/utilitycss-compiler  # the API ferrite-tailwind uses
    crates/utilitycss-span      # SourceId
    ...                         # rest of the upstream workspace
```

`ferrite-tailwind` links `utilitycss-compiler`, `utilitycss-span`,
and `utilitycss-theme`; the other crates ship for reference and future
extractors.

## Theme extension

Upstream's default theme is minimal on purpose (pre-1.0): a few hues
at `500`, red at `50`/`600`, gray at `50`/`100`/`500`/`900`. Ferrite
extends it through the public `Theme::builder()` API with the full
stone and orange scales plus the `6xl` width (`ferrite_theme()` in
`crates/ferrite-tailwind`). No vendored source is modified.

A handful of utility families are still missing upstream and are
covered by a compat block in `site/src/docs.css` under identical
class names: margin `auto`, border edge widths, `leading-6`,
`shrink-0`, and `cursor-pointer`. Each line is marked for deletion
once upstream emits it.

## The manifest wrinkle

Cargo resolves `*.workspace = true` against the *outer* workspace for
path dependencies, so the vendored crates' inherited `[package]` keys
would fail to load. The materialize script inlines the seven package
keys plus the `[lints]` tables — the same transform `cargo vendor`
performs. It also drops the inherited `readme` key (per-crate READMEs
do not exist upstream). Re-run it after every re-vendor.

## Updating

```bash
rm -rf vendor/tailwind-rs
git clone https://github.com/nglmercer/tailwind-rs /tmp/tailwind-rs
cp -r /tmp/tailwind-rs vendor/tailwind-rs
rm -rf vendor/tailwind-rs/.git
python3 vendor/materialize-tailwind-manifests.py
# record the new HEAD in vendor/README.md
cargo test -p ferrite-tailwind
```
