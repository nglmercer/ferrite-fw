# Security

## Threat model

The compiler processes untrusted project source and configuration.

Potential risks:

- denial of service from pathological input,
- parser panics,
- excessive memory allocation,
- arbitrary-value injection into malformed CSS,
- path traversal in CLI/adapters,
- unsafe FFI bugs,
- arbitrary code execution through configuration/plugin systems.

## Core security rules

- Do not execute source code to discover class names.
- Do not execute arbitrary JavaScript configuration in core.
- Avoid regexes with catastrophic backtracking.
- Validate arbitrary values according to utility grammar.
- Escape generated selectors correctly.
- Bound recursive parsing depth if recursion exists.
- Avoid panics on user input.
- Keep unsafe code minimal.

## Filesystem

Filesystem access belongs to CLI/adapters.

Normalize and validate paths according to host policy.

The compiler core should accept source IDs and content rather than opening arbitrary paths.

## Arbitrary CSS values

Arbitrary values must not bypass syntax validation blindly.

The project may intentionally allow raw CSS value fragments, but:

- delimiters must be balanced,
- generated selector escaping must remain correct,
- the serializer must not allow breaking out of the intended rule structure.

## Plugins

A future native plugin API must distinguish trusted in-process plugins from sandboxed/declarative extensions.

Cross-runtime arbitrary code execution is not a core requirement.

## FFI

All FFI boundaries should:

- validate lengths/types,
- avoid retaining invalid pointers,
- convert panics to errors where possible,
- document ownership.

## Reporting

Use the repository's private GitHub vulnerability reporting form. Do not submit
unpatched vulnerabilities through public issues or pull requests. The root
`.github/SECURITY.md` policy defines supported versions and coordinated
disclosure expectations.
