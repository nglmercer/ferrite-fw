---
title: Frameworks
order: 12
---

# Frameworks

Framework compilation, browser behavior and SSR rendering are separate
capabilities. The shared [capability registry](../../../crates/ferrite-frameworks/src/registry.rs)
backs generation and diagnostics. All current application profiles remain
experimental; a successful compiler invocation does not establish full framework
compatibility.

| Framework | Client compilation | Compiler host | SSR runtime profile |
|---|---|---|---|
| React 19.2.0 | Native Oxc/SWC; partial React Refresh | Native | Experimental napi-vm |
| Vue 3.5.22 | Official project-matched compiler | Explicit Node host | Experimental napi-vm |
| Svelte 5.39.6 | Official project-matched compiler | Explicit Node host | Experimental napi-vm |

Vue and Svelte use their actual official compilers; template stubs are not
supported. Without an explicitly configured compiler host, component compilation
fails with instructions instead of returning placeholder markup. Node is a
build/development compiler dependency for these profiles, separate from the
embedded SSR runtime and from the standalone application's runtime requirements.

## React

```toml
[react]
refresh = true
runtime = "automatic" # or "classic"
```

```bash
ferrite create my-react --framework react --language ts --rendering client
```

React Refresh has browser fixtures for component edits, imported/custom hooks,
barrels, memo/default exports, mixed exports, syntax recovery and production
preview. The complete component/hook matrix remains unfinished. Production output
excludes Refresh instrumentation. Detecting Preact, Solid or Qwik packages does
not enable an adapter for those frameworks.

## Vue and Svelte client applications

```bash
ferrite create my-vue --framework vue --language ts --rendering client --compiler-host node
ferrite create my-svelte --framework svelte --language js --rendering client --compiler-host node
```

Compiler versions must match the project's resolved packages. Compiler options,
loaded compiler/plugin dependencies, host identity and source dependencies take
part in caching and invalidation. Browser fixtures exercise actual templates and
production output in Chromium and Firefox. Current component changes may reload
the document; granular state-preserving HMR is not a complete supported contract.

## Experimental Vue and Svelte SSR

Build Ferrite with `--features napi-vm`, then explicitly select both hosts:

```bash
ferrite create my-app --framework vue --language ts --rendering ssr --compiler-host node --ssr-runtime napi-vm
```

The JS/TS fixtures cover meaningful HTTP HTML before JavaScript, hydration,
interaction, watched changes, syntax/style error recovery and production preview,
including a `/app/` base. Svelte JS/TS production builds and preview now pass real
hydration, DOM retention, interaction and scoped styles in Chromium and Firefox.
Use the same create command with `--framework svelte` for that experimental SSR
profile. These checks do not establish streaming, every component feature or
cross-platform standalone hydration. React JS/TS also passes synchronous production
SSR and hydration in both engines. Generate it with `--framework react
--compiler-host native --ssr-runtime napi-vm`; it uses the public edge renderer
and needs no Node executable. Complete development and standalone matrices remain
open for the new profiles.

Use `ferrite doctor` for configured capabilities and `ferrite inspect` for the
resolved profile. Doctor reports prerequisites; it does not execute a compiler or
prove runtime compatibility. See the [completion plan](../../../docs/COMPLETION_PLAN.md)
for remaining gates and their explicit reproduction commands.
