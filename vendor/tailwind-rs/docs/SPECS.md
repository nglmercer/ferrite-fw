# Product and Compiler Specification

## Terminology

A **candidate** is a source token that may represent a utility class.

A **parsed candidate** is a syntactically valid candidate represented as an AST.

A **resolved candidate** has semantic meaning after utility/variant/theme lookup.

A **rule** is one or more CSS IR nodes generated from a resolved candidate.

## Functional requirements

### Source input

The compiler MUST accept source content supplied by the host.

The semantic core MUST NOT require direct filesystem access.

Each source unit SHOULD have:

- stable source id,
- optional path,
- bytes/string content,
- optional content kind hint.

### Candidate extraction

The scanner MUST:

- support batch input,
- return source spans,
- avoid catastrophic backtracking,
- support escaped delimiters,
- tolerate unrelated programming-language syntax,
- avoid requiring a full host-language AST.

The scanner MAY return false-positive candidates; the parser/resolver may reject them.

### Parsing

The parser MUST understand:

- variants,
- base utility family,
- named values,
- arbitrary values,
- optional modifiers,
- important marker if supported,
- negative marker if supported.

Parsing MUST be deterministic and allocation-conscious.

### Theme

The theme system MUST provide typed access to:

- colors,
- spacing,
- breakpoints,
- typography tokens,
- radii,
- widths/heights where applicable.

Unknown theme keys MUST produce predictable fallback/rejection behavior.

### Utilities

Each utility family SHOULD define:

- accepted syntax,
- accepted value kinds,
- optional negative support,
- optional modifier support,
- CSS lowering behavior,
- ordering category.

### Variants

Variants MUST be composable according to documented ordering and compatibility rules.

Variants may transform:

- selectors,
- rule wrappers,
- declarations,
- layer/order metadata.

### Output

Output MUST be deterministic.

Output SHOULD be minimal and stable enough for snapshot tests.

The compiler SHOULD be able to return structured rules in addition to a serialized CSS string.

## Non-functional requirements

### Performance

Target budgets are defined in `PERFORMANCE.md`.

### Portability

Core crates SHOULD compile on major Rust stable targets that do not require JS runtimes.

### Observability

Debug builds or explicit diagnostic modes MAY expose:

- scan timing,
- parse timing,
- cache hit rates,
- rule counts,
- invalidation counts.

### Error stability

Public error codes SHOULD remain stable within a major version.

## Compatibility philosophy

Do not promise Tailwind compatibility by default.

If compatibility layers are added, they should be explicit presets with their own documented scope.

## Deterministic ordering

Ordering must not depend on:

- filesystem enumeration order,
- hash-map random seed,
- thread scheduling,
- adapter callback order.

The compiler should derive an explicit ordering key.

## Caching

Caches MUST be invalidated by semantic input, not only timestamps.

Cache keys may include:

- candidate text,
- compiled theme/config fingerprint,
- compiler version/schema version,
- feature flags.

## Thread safety

Core immutable configuration types SHOULD be `Send + Sync` where practical.

Mutation-heavy incremental state may be instance-local and explicitly synchronized.
