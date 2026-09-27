# @utilitycss/bun

`@utilitycss/bun` is the Bun bundler and fullstack integration for `utilitycss`.
It collects supported source modules through Bun's plugin lifecycle and serves generated CSS from
the virtual `utilitycss` stylesheet specifier. `config` accepts the same declarative JSON or
CSS-first source supported by the native compiler, while `browserTarget` enables target-aware
diagnostics.

```ts
import { utilitycss } from "@utilitycss/bun";

const result = await Bun.build({
  entrypoints: ["src/server.ts"],
  outdir: "dist",
  target: "bun",
  plugins: [utilitycss({
    config: '@theme { --color-brand-600: oklch(55% 0.2 260); }',
    browserTarget: "modern"
  })]
});

if (!result.success) {
  throw new Error("Bun build failed");
}
```

In Bun fullstack development, configure the plugin in `bunfig.toml`:

```toml
[serve.static]
plugins = ["@utilitycss/bun"]
```

Then reference the virtual stylesheet from HTML:

```html
<link rel="stylesheet" href="utilitycss" />
```

The adapter rebuilds a compiler from a persistent normalized-module snapshot for each Bun build
cycle. This retains unchanged modules when Bun omits them from an incremental callback while its
dependency graph tracking removes deleted modules. Imported `.css` files are transformed through
the same native `transformStylesheet` API used by Node and Vite.

Pass `debug: true` to log build numbers, retained source counts, successful rebuilds, and HMR
failures. Diagnostics include source locations when the source is available.
An invalid incremental build is rejected while Bun keeps the last successful bundle active, so the
development process can recover on the next save.

For fullstack development, the application server should explicitly enable Bun frontend HMR:

```ts
Bun.serve({
  development: { hmr: true, console: true },
  routes: { "/": homepage }
});
```

The default export is a zero-options plugin object for Bun's `bunfig.toml` loader; use the named
`utilitycss(options?)` export when passing options to `Bun.build()`.
