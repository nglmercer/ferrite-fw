# Contributing

## Before coding

Read:

- `LLMS.md`
- `docs/VISION.md`
- `docs/ARCHITECTURE.md`
- the relevant specification
- the active roadmap phase

## Issue types

Suggested labels:

- `scanner`
- `parser`
- `utilities`
- `variants`
- `theme`
- `incremental`
- `performance`
- `vite`
- `node`
- `bun`
- `deno`
- `swc`
- `docs`
- `rfc`
- `bug`

## Change categories

### Small implementation change

Examples:

- parser bug,
- utility fix,
- diagnostic improvement.

Open a PR with tests.

### Semantic change

Examples:

- new grammar,
- variant ordering change,
- config precedence change.

Requires an RFC or ADR when behavior is externally visible.

### Architectural change

Examples:

- new compiler IR,
- crate boundary movement,
- plugin model,
- public binding protocol.

Requires an ADR before or with implementation.

## Pull request expectations

PR should explain:

- problem,
- approach,
- tests,
- compatibility impact,
- performance impact if relevant,
- documentation updated.

## Commit style

Use scope-first concise messages.

Examples:

```text
scanner: fix bracket nesting
parser: reject empty modifier
compiler: cache resolved candidates
vite: reuse compiler on HMR
docs: clarify config precedence
```

## Review standards

Reviewers should check:

- correct layer,
- deterministic behavior,
- error quality,
- test coverage,
- incremental equivalence,
- performance hot paths,
- docs.

## Backward compatibility

Before `1.0`, breaking changes are allowed but must be documented.

After `1.0`, follow `RELEASE.md`.
