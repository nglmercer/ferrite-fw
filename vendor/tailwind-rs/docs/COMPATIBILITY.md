# Compatibility Policy

## Default stance

The project is not automatically Tailwind-compatible.

Similarity of syntax does not imply compatibility.

## Compatibility dimensions

A compatibility preset may define:

- candidate grammar,
- default theme tokens,
- utility names,
- variant names,
- ordering behavior,
- generated CSS semantics.

## Conformance

Any compatibility claim must have fixtures.

Prefer statements like:

> Supports the documented subset X.

Avoid:

> Drop-in compatible.

unless demonstrated by broad conformance testing.

## Host compatibility

Semantic output must be consistent across:

- Rust native,
- CLI,
- Node,
- Bun,
- Deno,
- Vite.

Host APIs can differ, compiler meaning cannot.

## Browser support

Generated CSS browser support is part of the serializer/config policy. The supported deterministic
targets are `modern`, `evergreen`, `safari-15`, and `legacy`. The compiler analyzes the CSS IR and
reports unsupported feature use without silently changing semantics; a future lowering pass MAY
add explicit fallbacks. Browser policy is included in config fingerprints and capability manifests.

`safari-15` represents Safari 15.0 (September 2021), the 15.x series floor. Output MUST run on
every Safari 15.x release, so features added in later 15.x releases report unsupported. Tracked
feature floors, verified against MDN browser-compat-data:

- `oklch()`, unprefixed masking: Safari 15.4 (unsupported by `safari-15`);
- `color-mix()`: Safari 16.2 (unsupported);
- `light-dark()`: Safari 17.5 (unsupported);
- `content-visibility`, unprefixed `backdrop-filter`: Safari 18.0 (unsupported);
- `field-sizing`: Safari 26.2 (unsupported).

`modern` and `evergreen` accept all tracked features; `legacy` accepts none. Fixtures live in
`crates/utilitycss-css-ir/tests/fixtures/browser_targets.json` and run through
`crates/utilitycss-css-ir/tests/browser_support.rs`.
