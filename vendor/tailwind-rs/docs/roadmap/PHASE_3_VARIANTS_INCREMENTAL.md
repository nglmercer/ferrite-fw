# Phase 3 — Variants and Incremental Compiler

## Objective

Add composable variants and a first-class incremental engine.

## Initial variants

- hover
- focus
- active
- disabled
- dark
- sm/md/lg or configured breakpoints
- group-hover subset
- peer-checked subset

## Variant model

Variants transform CSS IR/rule context rather than duplicating utility logic.

## Incremental indexes

Implement:

```text
source -> candidate set
candidate -> compiled result
candidate -> source ref count
```

Optional later:

```text
theme token -> affected candidate set
```

## Update API

Support:

- add/update source,
- remove source,
- config replacement,
- full reset.

## Correctness invariant

For every sequence of source changes:

```text
incremental result == clean rebuild result
```

## Exit criteria

- variant composition fixtures pass,
- incremental/full differential tests pass,
- no-op update avoids recompilation,
- one-file update work scales with changed candidates,
- watch benchmarks exist.

## Won't yet

- granular config-token invalidation if complexity is high,
- SWC extraction.
