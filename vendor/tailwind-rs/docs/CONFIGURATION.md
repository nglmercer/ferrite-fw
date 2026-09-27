# Configuration Specification

## Goals

Configuration should be:

- deterministic,
- runtime-independent,
- cacheable,
- serializable where practical,
- usable from CSS-first workflows,
- extensible without arbitrary code execution in core.

## Configuration layers

Suggested precedence:

1. built-in defaults,
2. preset(s),
3. project configuration,
4. inline compiler options,
5. command-specific overrides.

Later layers override earlier layers according to field-specific rules.

## CSS-first configuration

A CSS-based configuration format is attractive because it works across runtimes.

Conceptual example:

```css
@theme {
  --color-brand-500: oklch(62% 0.2 260);
  --spacing: 0.25rem;
  --breakpoint-md: 48rem;
}
```

The parser should lower this into a typed compiled theme.

The implemented CSS-first subset also accepts one-declaration `@utility` blocks and selector or
at-rule `@variant` statements:

```css
@utility content-auto { content-visibility: auto; }
@variant theme-midnight (&:where([data-theme="midnight"] *));
```

The same parser validates custom-property names, balanced blocks, comments, and UTF-8 without
executing host code.

## Structured configuration

A serializable JSON/TOML configuration MAY also exist.

Example:

```json
{
  "preset": "utilitycss",
  "browserTarget": "modern",
  "theme": {
    "spacing": { "4": "1rem" }
  },
  "utilities": {
    "content-center": {
      "type": "static",
      "property": "place-content",
      "value": "center",
      "order": 63
    }
  },
  "variants": {
    "motion-safe": {
      "type": "media",
      "name": "media",
      "prelude": "(prefers-reduced-motion: no-preference)",
      "order": 170
    }
  }
}
```

The accepted preset names are `utilitycss` and the explicitly limited
`tailwind-v4-subset` and `tailwind-v3-subset`. The latter profiles are compatibility profiles for
the semantics implemented by this workspace; they MUST NOT be treated as full Tailwind
compatibility. Utility and variant plugin entries are declarative, deterministic, and validated
before compilation. The active preset is included in compiler provenance and capability manifests.

`browserTarget` accepts `modern`, `evergreen`, `safari-15`, or `legacy`. The compiler keeps the
generated CSS deterministic and reports unsupported modern features as structured warnings. The
target participates in configuration fingerprints so changing browser policy invalidates cached
semantic results. `safari-15` pins Safari 15.0 (September 2021) as the 15.x series floor; see
`COMPATIBILITY.md` for the per-feature support table.

The CLI `watch` command watches the file supplied through `--config` and reloads a valid change
without restarting. If a changed configuration is invalid, the watcher reports the diagnostic and
retains the last valid configuration until the file is fixed.

## JavaScript configuration

Core MUST NOT require executing JavaScript.

A Node adapter may offer an optional JS configuration bridge later, but it must resolve JS objects into a plain serialized config before entering core.

## Compiled config

Hot paths should use immutable compiled config.

Example conceptual type:

```rust
pub struct CompiledConfig {
    pub theme: CompiledTheme,
    pub utilities: UtilityRegistry,
    pub variants: VariantRegistry,
    pub features: FeatureSet,
    pub fingerprint: ConfigFingerprint,
}
```

## Fingerprinting

The fingerprint should change when semantic output can change.

It should not depend on unstable memory addresses or unordered serialization.

## Validation

Configuration errors should be detected before compiling many candidates.

Examples:

- duplicate variant name,
- invalid breakpoint,
- invalid token syntax,
- conflicting registry entry,
- unsupported feature combination.

## Extension policy

Extension points should prefer declarative descriptions.

Native Rust extension APIs may be added for embedding use cases.

Cross-runtime plugin APIs should use a versioned protocol if introduced.
