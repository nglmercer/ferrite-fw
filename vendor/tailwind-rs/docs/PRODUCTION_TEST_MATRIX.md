# Production test matrix

The repository uses layered tests so one failing surface is easy to localize.

## Fast local loop

```bash
cargo fmt --all -- --check
cargo test -p utilitycss-swc
cargo test -p utilitycss-extractor
cargo test -p utilitycss-compiler --test conformance
cargo test -p utilitycss-lsp
npm run build --workspace=@utilitycss/node
npm test --workspace=@utilitycss/node
npm run build --workspace=@utilitycss/bun
npm test --workspace=@utilitycss/bun
bun install --cwd examples/bun --frozen-lockfile
npm --prefix examples/bun run typecheck
npm --prefix examples/bun run setup
npm --prefix examples/bun run verify
```

The compiler conformance fixtures cover basic utilities, variant ordering, arbitrary values,
multi-source deduplication, and duplicate diagnostic spans. SWC tests cover JSX, TSX, helper calls,
dynamic templates, parser failures, and byte-span round trips. Framework tests cover Vue bindings,
Svelte directives, and Astro `class:list` expressions.

## Full local gate

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo bench -p utilitycss-bench
npm run lint
npm run typecheck
npm run build
npm test
```

## Local release gate

Run the reproducible local orchestration command before preparing a release candidate:

```bash
npm run release:check
```

It runs the Rust format, lint, tests, conformance, audit, dependency-policy, packaging, JavaScript
adapter, packed-artifact, WASM, Vite, native-current-platform, Bun plugin, and Bun example checks
that are available on the host. It reports unavailable external architectures explicitly as
`SKIP` and exits non-zero for an executed check that fails. A skipped architecture is not release
evidence for that architecture.

GitHub Actions remains configured with the full matrix, but a workflow that cannot start steps due
to an external billing/account issue is not counted as compiler or release evidence. Local results
and externally verified target results must be recorded separately.

## Native adapter gate

The native package must be built for each declared target before running the Node smoke test:

```bash
npm run build:native
npm run build --workspace=@utilitycss/node
npm test --workspace=@utilitycss/node
```

The current-platform native smoke should also be followed by the Vite and packed Bun checks:

```bash
npm run smoke:vite
npm run smoke:packed-bun
```

The native test is skipped when no platform binary is installed, but the native CI matrix MUST run
it on Linux, Windows, and macOS. A skipped test is not release evidence.

## Bun integration gate

The Bun gate uses Bun's plugin lifecycle, not a CLI watcher:

```bash
npm run build --workspace=@utilitycss/bun
npm test --workspace=@utilitycss/bun
bun install --cwd examples/bun --frozen-lockfile
npm --prefix examples/bun run typecheck
npm --prefix examples/bun run setup
npm --prefix examples/bun run verify
npm --prefix examples/bun run build
```

The example's `bunfig.toml` loads the plugin for `bun --hot src/server.ts`; its production script
passes the plugin explicitly to `Bun.build()`. Both paths must prove that the virtual `utilitycss`
stylesheet contains expected rules. The test suite also proves that source replacement and graph
deletion remove stale CSS.

## Reliability properties

The scanner and candidate parser use `proptest` to exercise arbitrary valid UTF-8 input. The key
properties are that malformed input never panics, candidate spans remain source-valid, and an
incremental rebuild matches a clean rebuild. New compiler or extractor behavior MUST add a focused
fixture or regression test before it is released.
