# Testing Strategy

## Test pyramid

### Unit tests

Use for:

- scanner edge cases,
- parser grammar,
- theme resolution,
- utility lowering,
- variant transforms,
- ordering keys,
- escaping,
- cache behavior.

### Snapshot/golden tests

Use for:

- CSS output,
- diagnostics,
- serialized AST/IR,
- fixture projects.

Snapshots must be deterministic.

### Integration tests

Test full compilation from source input to CSS.

Representative fixtures should include:

- HTML,
- JSX/TSX,
- Vue-like templates,
- Svelte-like templates,
- mixed files,
- duplicate candidates,
- arbitrary values,
- variant chains.

### Adapter conformance tests

All adapters should run the same semantic fixture suite.

If Node and Vite produce different CSS for the same compiler version/config/input, that is a bug.

## Scanner tests

Include:

- Unicode,
- escaped punctuation,
- long tokens,
- malformed brackets,
- comments/strings,
- template literals,
- pathological input,
- false positives.

## Parser tests

Use table-driven tests.

For each candidate:

```text
input
expected AST or expected error
```

## Fuzzing

Fuzz:

- scanner,
- candidate parser,
- arbitrary-value parser,
- CSS escaping/serialization.

Core property:

> User-controlled input must not panic or violate memory safety.

## Property tests

Useful properties:

- parse/serialize internal forms are deterministic,
- scanning the same bytes always returns the same spans,
- output ordering is independent of candidate insertion order,
- incremental result equals clean full rebuild result.

The last property is especially important.

## Differential incremental test

For random edit sequences:

```text
incremental_output(edit sequence)
==
full_rebuild(final sources)
```

This should become a permanent regression suite.

## Performance tests

Benchmarks are not correctness tests, but major regressions should be visible in CI or scheduled benchmark jobs.

## Fixture policy

Keep small focused fixtures in git.

Large benchmark corpora may be generated or stored separately if repository size becomes a concern.

## Error tests

Diagnostics should test:

- stable code,
- message,
- source span,
- help text when relevant.

Avoid snapshotting volatile absolute paths or timing.
