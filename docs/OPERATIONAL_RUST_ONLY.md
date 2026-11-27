# Operational Rust-only status

Operational Rust-only means normal use does not require a Node, npm, bun or
pnpm executable. It does not yet mean the implementation is Rust-native, and it
does not yet mean every framework profile satisfies that requirement.

## Current support tiers

| Profile | Compiler execution | External Node required | Status |
|---|---|---:|---|
| Vanilla generated applications | Rust | No | Covered by generated-app and SSR/standalone acceptance |
| React generated applications | Rust/native transform | No for compilation and execution | Registry-backed package installation remains network-dependent; refresh coverage is incomplete |
| Vue 3.5.22 | Official browser compiler embedded in napi-vm | No for the default embedded host | Experimental operational Rust-only profile; explicit Node remains a transitional fallback |
| Svelte 5.39.6 | Official compiler in a Node child process | Yes, when explicitly enabled | Transitional experimental profile, not operational Rust-only |

The SSR embedded runtime remains separate from framework compilation. Vue 3.5.22
now uses the same embedded runtime technology for its official browser compiler
by default, while Svelte still requires the explicit external Node compiler host.
Selecting `runtime.backend = "napi-vm"` or `napi-vm-process` never by itself
implies that a framework compiler is embedded.

## Inspect contract

`ferrite inspect --json` exposes `compilerHosts` for the selected framework
profile. Each entry reports:

- `framework`: the enabled framework or default `vanilla` pipeline;
- `host`: `native`, `embedded`, `node` or `disabled`;
- `execution`: `rust`, `embedded-napi-vm`, `external-node-process` or `unavailable`;
- `externalNodeRequired`: whether normal component compilation requires an
  external Node executable;
- `support`: the current evidence tier.

Human-readable `ferrite inspect` prints the same operational classification.
Use this before claiming a project is operational Rust-only.

## Acceptance profile

```bash
python3 scripts/acceptance.py --profile rust-only
```

The profile intentionally does not require Node. It currently proves the parts
that can execute without it: compiler-host reporting, generated vanilla
applications in real Chromium/Firefox, generated React and embedded Vue
production builds/previews using only checked-in pinned package tarballs,
embedded SSR packaging, streamed prerendering and standalone process/streaming
recovery. The generated tests run
their Ferrite child processes with an empty `PATH`.

The profile is a stronger baseline, not final production proof. Svelte
compilation still requires the explicit external Node compiler host. Vue is
operational Rust-only through the embedded official compiler, but that is still
a transitional bridge rather than a Rust-native compiler; dev/HMR, SSR, type
dependency edge cases and cross-platform release matrices require continued
acceptance before the full production claim is complete.
