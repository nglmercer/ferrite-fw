# Performance

## Principle

Performance claims must be measured.

Optimize hot paths only after correctness and determinism are established.

## Critical paths

1. source scanning,
2. candidate deduplication,
3. candidate parsing,
4. semantic resolution,
5. incremental invalidation,
6. CSS rule ordering,
7. serialization,
8. FFI crossing for JS adapters.

## Benchmark dimensions

Track:

- cold build time,
- warm rebuild time,
- single-file edit rebuild,
- no-op rebuild,
- memory usage,
- candidate throughput,
- CSS serialization throughput,
- adapter overhead.

## Target budgets

Initial engineering targets, not promises:

- no-op incremental update: effectively constant with respect to total project size,
- single-file update: work proportional to changed source + affected candidates,
- scanning: hundreds of MB/s on modern desktop hardware where realistic,
- duplicate candidates: near-zero repeated semantic work after caching,
- adapter overhead: small relative to compile work.

Use measured baselines before turning these into hard release gates.

## Benchmark corpus

Include:

- tiny app,
- medium app,
- large synthetic app,
- many duplicate candidates,
- many unique candidates,
- arbitrary-value-heavy input,
- variant-heavy input,
- large TSX files.
- authored CSS with 100, 1,000, and 10,000 `@apply` directives,
- repeated and unique composition candidates,
- large CSS with no `@apply` directives.

## Methodology

- pin benchmark fixtures,
- record CPU/OS/Rust version,
- run multiple iterations,
- report median and variance,
- compare against previous main branch,
- avoid claiming improvements from one noisy run.

## Allocation strategy

Do not introduce arenas/interners globally before measurement.

Potential techniques after profiling:

- string interning,
- bump allocation for parse batches,
- small-vector optimization,
- candidate hash caching,
- zero-copy source slices,
- parallel scanning.

Each technique must preserve API clarity and deterministic behavior.

## Parallelism

Parallelize independent work.

Avoid parallelism for tiny workloads when scheduling overhead dominates.

Final serialization can remain single-threaded if that improves stable ordering and is not a bottleneck.

## JS boundary

Batch calls across N-API/WASM boundaries.

Bad:

```text
JS calls Rust once per candidate
```

Good:

```text
JS sends file or candidate batch
Rust processes all candidates
```

## Regression policy

A statistically meaningful hot-path regression should require explanation before merge.

Correctness fixes may justify regressions, but document them.
