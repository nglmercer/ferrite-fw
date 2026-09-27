# Release and Versioning

## Versioning

Use semantic versioning.

Before `1.0`, treat minor versions as potentially breaking but document migrations.

After `1.0`:

- patch: bug fixes, no intentional public breakage,
- minor: backward-compatible features,
- major: breaking public API or stabilized grammar changes.

## Version surfaces

Version separately or in a coordinated workspace:

- Rust crates,
- CLI,
- Node/Bun package,
- WASM/Deno package,
- Vite plugin.

The Bun bundler integration is published as `@utilitycss/bun`; it depends on the generic
`@utilitycss/node` lifecycle adapter and does not publish a second native compiler.

Prefer synchronized core semantic versions to reduce compatibility confusion.

## Protocol compatibility

Bindings/adapters that communicate through serialized data should expose a protocol/schema version.

## Release checklist

- all CI green,
- full test suite,
- adapter conformance suite,
- benchmark comparison,
- changelog,
- migration notes for breaking changes,
- docs match implementation,
- package smoke tests,
- reproducible build checks where practical.

## Artifact validation

The native package uses N-API's platform-package layout. Each advertised target
MUST be built in its native CI job. The CI workflow uploads those artifacts and
assembles them in a separate packaging job. After all target artifacts are present,
run `npm run prepare:release --workspace=@utilitycss/napi`; this copies binaries into
the four `packages/utilitycss-napi/npm/*` packages and adds them as optional
dependencies of the public `@utilitycss/napi` package. Publish those platform
packages and the root package from the same immutable tag.

Before publishing, run `cargo package --allow-dirty --workspace`,
`npm run validate:packages`, `npm run smoke:packed-node`, and `npm run smoke:packed-bun` in a clean
checkout that contains a native artifact. The smoke tests MUST install packed tarballs in separate
temporary projects and compile CSS without workspace symlinks. `@utilitycss/bun` MUST be packed and
installed together with `@utilitycss/node`, `@utilitycss/napi`, and the current platform package.

## Local release sequence

When GitHub Actions is unavailable because of an external account or billing limitation, use the
local release gate and record platform limitations honestly:

```text
npm run release:check
    ↓
cargo package --allow-dirty --workspace
    ↓
npm run validate:packages
    ↓
npm run smoke:packed-node
npm run smoke:packed-bun
    ↓
verify current-platform native artifact
    ↓
record SKIP entries for unavailable architectures
    ↓
create an immutable release candidate tag only after all required evidence is collected
```

A GitHub workflow with zero executed steps MUST NOT be described as a green release check. CI
configuration and all required jobs should remain in the repository for later execution.

## Changelog categories

- Added
- Changed
- Fixed
- Performance
- Deprecated
- Removed
- Security

## Deprecation

After `1.0`, deprecated APIs should normally survive at least one minor release before removal in a major release.

## Grammar stability

Class DSL syntax is a public API.

A syntax change can be more disruptive than a Rust API change and must be treated accordingly.

## Release channels

Possible future channels:

- stable,
- beta,
- nightly/next.

Do not create channels until there is a concrete user need.
