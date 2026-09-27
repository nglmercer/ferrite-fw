# API Design Guidelines

## Rust core

Prefer instance-based APIs.

Conceptual:

```rust
let config = Config::builder()
    .theme(theme)
    .build()?;

let mut compiler = Compiler::new(config);

compiler.update_source(SourceInput {
    id: SourceId::new("src/app.tsx"),
    path: Some("src/app.tsx".into()),
    content: source.into(),
})?;

let output = compiler.build()?;
```

## API properties

Public APIs SHOULD be:

- batch-friendly,
- incremental-friendly,
- deterministic,
- explicit about ownership,
- independent from JS runtime conventions.

## Source updates

Use stable source IDs rather than assuming filesystem paths are identity.

Paths may be metadata.

## Diagnostics

Prefer returning diagnostics as structured data.

Adapters can render them for CLI/dev server UI.

## Async

Core compilation APIs SHOULD remain synchronous unless actual asynchronous core work exists.

JS adapters may expose async wrappers for I/O/lifecycle reasons.

Do not make CPU compilation async merely because JavaScript commonly uses promises.

## Cancellation

If large projects justify it later, cancellation should be cooperative and explicit.

## Versioned serialized types

If types cross process/runtime boundaries, use schema versions.

Do not serialize private Rust implementation details accidentally.
