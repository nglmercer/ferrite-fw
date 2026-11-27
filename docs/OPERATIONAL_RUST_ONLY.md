# Operational Rust-only status

Operational Rust-only means normal use does not require a Node, npm, bun or
pnpm executable. It does not yet mean the implementation is Rust-native, and it
does not yet mean every framework profile satisfies that requirement.

## Current support tiers

| Profile | Compiler execution | External Node required | Status |
|---|---|---:|---|
| Vanilla generated applications | Rust | No | Covered by generated-app and SSR/standalone acceptance |
| React generated applications | Rust/native transform | No for compilation and execution | Registry-backed package installation remains network-dependent; refresh coverage is incomplete |
| Vue 3.5.22 | Official compiler in a Node child process | Yes, when explicitly enabled | Transitional experimental profile, not operational Rust-only |
| Svelte 5.39.6 | Official compiler in a Node child process | Yes, when explicitly enabled | Transitional experimental profile, not operational Rust-only |

The SSR embedded runtime is separate from framework compilation. Selecting
`runtime.backend = "napi-vm"` or `napi-vm-process` never implies that Vue or
Svelte components can compile without their explicitly configured compiler
host.

## Inspect contract

`ferrite inspect --json` exposes `compilerHosts` for the selected framework
profile. Each entry reports:

- `framework`: the enabled framework or default `vanilla` pipeline;
- `host`: `native`, `node` or `disabled`;
- `execution`: `rust`, `external-node-process` or `unavailable`;
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
applications in real Chromium/Firefox, embedded SSR packaging, streamed
prerendering and standalone process/streaming recovery. The generated tests run
their Ferrite child processes with an empty `PATH`.

The profile is a baseline, not final production proof. Generated React/Vue/
Svelte applications still depend on registry-backed package installation for
their official runtime packages, and Vue/Svelte compilation still requires the
explicit external Node compiler host. A no-network fixture registry is still
required before framework acceptance can be reproducible on a clean offline
machine.
