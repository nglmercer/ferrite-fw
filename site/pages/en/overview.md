---
title: Overview
order: 1
---

# Ferrite

Ferrite is a Rust-native web toolchain: dev server, production bundler,
and SSR runtime in one binary. It speaks Vite conventions (native ESM in
dev, plugin hooks, import maps) with an Oxc-powered pipeline and no
Node.js required for the core workflow.

This site is built with Ferrite itself: Markdown pages become JS modules
through the `ferrite:markdown` plugin, layout utilities are compiled from
the vendored `tailwind-rs` workspace, and the whole thing ships as a
single binary.

## What you get

- **Dev server** with per-module native ESM, HMR, and import-map support.
- **Production builds** with hashed assets, manifest, CSS extraction,
  statement-level tree-shaking, and optional scope hoisting.
- **Frameworks**: React Refresh in dev, plus Vue/Svelte SFC experiments.
- **SSR**: adapters, `ssrLoadModule`, externals, streaming, islands, RPC.
- **Single binary**: embed `dist/` and serve it with no runtime beside it.
- **Markdown + utility CSS**: this docs stack, zero JavaScript toolchain.

## Start here

New to Ferrite? Read [Getting started](getting-started) for install, your
first project, and the dev loop. Then pick a track:

| Track    | Pages                                            |
|----------|--------------------------------------------------|
| Build    | [CLI](cli), [Configuration](configuration)      |
| Extend   | [Plugins](plugins)                               |
| Ship     | [Production](production)                         |
| Vendored | [Tailwind vendor](tailwind-vendor)               |

> These docs live in `site/pages/*/*.md` in the Ferrite repo and are
> the project's production-verification target: if this site builds
> and serves, the framework's integration status is green.

## Theme & languages

Use the header controls to switch between light and dark mode (saved
locally, follows your OS setting by default) and between English,
Spanish (`es`), and Chinese (`cn`). Every locale ships the same seven
pages under `#/{locale}/{page}`.
