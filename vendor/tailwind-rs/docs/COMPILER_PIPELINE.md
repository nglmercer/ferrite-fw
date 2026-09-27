# Compiler Pipeline

## Pipeline

```text
SourceInput
   ↓
Scan
   ↓
CandidateToken
   ↓
Parse
   ↓
CandidateAst
   ↓
Resolve
   ↓
ResolvedCandidate
   ↓
Lower
   ↓
CssIr
   ↓
Order
   ↓
Serialize
   ↓
CompileOutput
```

## Stage 1 — Scan

Input:

```rust
SourceInput {
    id,
    path,
    content,
}
```

Output:

```rust
CandidateToken {
    raw,
    span,
}
```

Scanner responsibility ends at candidate discovery.

## Stage 2 — Parse

The parser validates class DSL structure.

It MUST NOT look up theme values.

This allows parser tests to remain independent from configuration.

## Stage 3 — Resolve

Resolution combines:

- parsed candidate,
- utility registry,
- variant registry,
- compiled theme.

Output contains semantic intent but need not contain final CSS text.

## Stage 4 — Lower

Lower semantic intent into CSS IR.

Example:

```text
ResolvedUtility::Padding(All, Spacing(4))
```

to:

```text
Declaration("padding", "calc(var(--spacing) * 4)")
```

## Stage 5 — Variant transformation

A utility can first lower to a base rule, then variants transform it.

This separation prevents each utility from duplicating hover/responsive/dark behavior.

## Stage 6 — Ordering

Assign explicit stable ordering.

A possible composite ordering key:

```text
layer
variant rank
utility group
utility-specific rank
candidate tie-breaker
```

The exact ordering system requires benchmarks and compatibility tests.

## Stage 7 — Serialize

Serializer requirements:

- deterministic whitespace mode,
- valid escaping,
- predictable minified/dev modes,
- no random ordering,
- no adapter-specific differences.

## Incremental pipeline

For a file update:

```text
old candidates = file_index[file]
new candidates = scan(new_content)

added   = new - old
removed = old - new
kept    = old ∩ new
```

Compile only `added` candidates unless config changed.

For removed candidates, decrement rule references.

If a candidate reaches zero references, remove its generated rules from active output.

## Configuration invalidation

Config/theme changes can invalidate many candidates.

The compiler should fingerprint compiled configuration.

At first, a config change MAY trigger full semantic recompilation while preserving scan results.

Later phases may implement more granular invalidation.

## Diagnostics

Each stage may emit diagnostics, but diagnostics should preserve source identity and spans when available.

Adapters may format diagnostics for terminals or dev servers.
