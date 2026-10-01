# Documentation

Start with the [project README](../README.md) for installation, commands and a
workspace overview, or the [getting started guide](../site/pages/en/getting-started.md)
for a first application.

## Framework guides

The website's Markdown pages remain in `site/pages/` so the documentation site
and this index use the same guides.

| Topic | Guides |
|---|---|
| Introduction | [Overview](../site/pages/en/overview.md), [getting started](../site/pages/en/getting-started.md) |
| Configuration and commands | [Configuration](../site/pages/en/configuration.md), [CLI](../site/pages/en/cli.md), [Rust API](../site/pages/en/api.md) |
| Development | [Dev server](../site/pages/en/dev-server.md), [pipeline](../site/pages/en/pipeline.md), [plugins](../site/pages/en/plugins.md) |
| Dependencies and assets | [npm](../site/pages/en/npm.md), [assets](../site/pages/en/assets.md), [CSS](../site/pages/en/css.md), [Tailwind](../site/pages/en/tailwind-vendor.md) |
| Application integration | [Frameworks](../site/pages/en/frameworks.md), [SSR](../site/pages/en/ssr.md), [runtime](../site/pages/en/runtime.md), [WebAssembly](../site/pages/en/wasm.md) |
| Shipping and diagnosis | [Production](../site/pages/en/production.md), [troubleshooting](../site/pages/en/troubleshooting.md) |

Translated guides: [Español](../site/pages/es/overview.md),
[中文](../site/pages/cn/overview.md).

## E2E testing and Playwright compatibility

Read the [E2E usage guide](../examples/e2e/README.md) first. For compatibility,
use the engine table and comparison report before the detailed API inventory.

| Document | Purpose |
|---|---|
| [Engine capabilities](e2e/E2E-ENGINE-CAPABILITIES.md) | Chromium and Firefox support, differences and limitations |
| [Playwright comparison](e2e/PLAYWRIGHT-PARITY.md) | Feature comparisons, implementation notes and regression evidence |
| [API member matrix](e2e/PLAYWRIGHT-API-MATRIX.md) | Generated inventory of the pinned Playwright API and Ferrite counterparts |
| [Implementation checklist](e2e/E2E-PARITY-TODO.md) | Completed task acceptance criteria and historical implementation checkpoints |
| [Completion audit](e2e/E2E-PARITY-AUDIT.md) | Final task evidence index and verified regression gates |
| [Conformance corpus](../scripts/e2e-conformance/README.md) | Pinned Playwright observations and reproduction instructions |
| [Matrix generator](../scripts/playwright-parity/README.md) | Regenerate the API inventory and source links |
| [Browser fixtures](../crates/ferrite-e2e/tests/fixtures/README.md) | Test fixture provenance and usage |

The checklist's 51 tasks are complete. The comparison and engine documents
still describe intentional limitations and deferred features; completion of
the checklist does not imply full Playwright compatibility. Older checkpoint
notes in the long reports are historical; the completion audit records the
final verification.

## Examples and package references

- [Vanilla TypeScript](../examples/vanilla-ts/README.md)
- [React](../examples/react/README.md)
- [SSR](../examples/ssr/README.md)
- [Rust and WebAssembly](../examples/rust-wasm/README.md)
- [Browser client package](../packages/ferrite-client/README.md)
- [Vite compatibility tests](../tests/vite-compat/README.md)

## Maintaining documentation

Keep project-wide reference documents under `docs/`, grouped by topic, and add
them to this index. Keep usage instructions beside their examples, scripts and
packages, and website guides in `site/pages/`. Use relative links from each
document's directory. Regenerate the API matrix with its script instead of
editing generated entries directly.
