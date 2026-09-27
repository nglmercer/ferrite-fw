# `@apply`-compatible composition

`utilitycss` supports an `@apply`-compatible composition directive backed by its own utility
grammar and semantic registry. Exact Tailwind CSS compatibility is not guaranteed unless covered by
an explicit compatibility preset and conformance tests.

## Syntax

Use `@apply` directly inside a CSS style rule:

```css
.button {
  @apply flex items-center gap-2 rounded bg-brand-600 p-4 text-white;
}
```

Multiple directives and ordinary declarations may be mixed. Expanded declarations occupy the
directive's author-order position:

```css
.button {
  color: red;
  @apply p-4;
  display: block;
}
```

Candidates are parsed by the normal utility candidate parser. Arbitrary values and registered
utility definitions therefore work without a second set of utility semantics:

```css
.card {
  @apply w-[42rem] bg-[#123456] custom-surface;
}
```

Unknown utilities are errors in explicit `@apply` directives. Normal source scanning may still
ignore unknown candidate-looking text because scanning is intentionally conservative.

## Variants

Variants use the normal `utilitycss-variants` registry:

```css
.button {
  @apply hover:bg-red-500 focus:bg-white dark:bg-black md:p-8;
}
```

Pseudo, ancestor, attribute, arbitrary, dark, and registered responsive variants are supported
when they are valid normal candidates. Variant rules are emitted with the caller's selector; the
transformer does not generate a utility selector and perform string replacement.

Applied utilities within one directive use the compiler's deterministic utility ordering. Repeated
declarations are retained so normal CSS cascade behavior remains visible. Identical input,
configuration, theme, utility registry, and variant registry produce byte-identical output.

`!important` is supported at the end of a directive and is applied to every declaration expanded
from that directive:

```css
.button {
  @apply p-4 text-white !important;
}
```

`!important` is not a utility candidate.

## Diagnostics and limitations

Diagnostics include source IDs, byte spans, stable codes, severity, and help where useful. Common
codes include `apply.invalid-syntax`, `apply.unknown-utility`, `apply.invalid-context`,
`apply.invalid-selector`, `apply.variant-error`, and `apply.unsupported`.

`@apply` at stylesheet root, inside declaration-only at-rules such as `@font-face`, and inside
CSS-nested style rules is rejected or reported as unsupported. The surrounding CSS is preserved.
Custom CSS class composition is intentionally not supported:

```css
.base { @apply p-4; }
.button { @apply base; } /* unknown utility */
```

CSS Modules are transformed using the selector text supplied by the CSS module. Cross-file custom
utility or custom-class resolution is not provided; projects requiring that behavior should keep
composition local or wait for a future explicit registry/reference design.

`@reference` is not required or interpreted by this release. Shared theme, utility, and variant
references are a separate build-context feature and MUST NOT be assumed to work across files.

## APIs

The native binding exposes:

```ts
compiler.transformStylesheet(id, content, path?): {
  css: string;
  diagnostics: readonly Diagnostic[];
}
```

`@utilitycss/node` normalizes this native result without implementing CSS or utility semantics.
The Bun and Vite adapters call the same method for imported CSS. The CLI accepts authored CSS from
input directories or explicitly with:

```bash
utilitycss build src --stylesheet src/app.css -o dist/app.css
```

Authored transformed CSS is emitted before independently discovered utility rules. A utility used
only by `@apply` is not emitted as a standalone utility selector.

The WASM binding exposes `transformStylesheet(id, content)` with the same result shape. Shared
fixtures under `fixtures/apply/` document the cross-adapter semantic contract.

## Compatibility statement

`utilitycss` supports an `@apply`-compatible composition directive backed by its own utility grammar
and semantic registry. Exact Tailwind CSS compatibility is not guaranteed unless covered by an
explicit compatibility preset and conformance tests.
