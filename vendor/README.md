# Vendored dependencies

Third-party sources kept in-tree so builds never depend on a live
upstream checkout. Do not edit vendored trees in place — update by
re-vendoring and note the new pin below.

## tailwind-rs (`tailwind-rs/`)

- Origin: <https://github.com/nglmercer/tailwind-rs>
- Pinned commit: `cd89ee7fc4a42ef108340b137358eeaa9990604a` (main)
- Vendored: 2026-09-27 (`.git` stripped; otherwise pristine)
- Used by: `crates/ferrite-tailwind` (path deps on
  `crates/utilitycss-compiler`, `crates/utilitycss-span`,
  `crates/utilitycss-theme`)
- Its own `[workspace]` root keeps it out of the ferrite workspace.
- Manifests are mechanically materialized (see below); all other files
  are pristine upstream.

## Manifest materialization

Cargo resolves `*.workspace = true` against the *outer* workspace for
path dependencies, so the vendored crates would fail to load. After
(re-)vendoring, run:

```bash
cargo run -p xtask -- materialize-tailwind
```

It inlines the seven inherited `[package]` keys, the `[lints]` tables,
and drops the inherited `readme` key (per-crate READMEs do not exist
upstream) — the same transform `cargo vendor` performs. It refuses to
run when the inner root's pinned values drift. Use `--check` to verify
without writing.

Update:

```bash
rm -rf vendor/tailwind-rs
git clone https://github.com/nglmercer/tailwind-rs /tmp/tailwind-rs
cp -r /tmp/tailwind-rs vendor/tailwind-rs
rm -rf vendor/tailwind-rs/.git
cargo run -p xtask -- materialize-tailwind
# record the new HEAD above
cargo test -p ferrite-tailwind
```
