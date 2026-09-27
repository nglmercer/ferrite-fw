# Observability and Debugging

## Goal

Make performance and invalidation behavior explainable without affecting normal output.

## Optional compiler stats

A build result may expose:

```text
sources_scanned
bytes_scanned
candidates_found
unique_candidates
candidates_parsed
cache_hits
rules_generated
rules_removed
scan_time
parse_time
resolve_time
serialize_time
```

Timing should be optional because instrumentation itself has overhead.

## Debug trace

A diagnostic/debug mode may answer:

- Why was this class included?
- Which source referenced it?
- Which utility handler compiled it?
- Which theme token was resolved?
- Which variants transformed it?
- What ordering key was assigned?

## Determinism

Debug metadata must not change semantic CSS output.

## Adapter logging

Adapters may add host-specific logging, but compiler trace formats should remain host-neutral.

## Privacy

Do not transmit source or telemetry externally by default.
