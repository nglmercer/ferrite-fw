# Generated capability data

The compiler is the source of truth for utility families, variant families, value namespaces,
theme tokens, diagnostic codes, and compatibility labels.

Use the native CLI to generate deterministic artifacts:

```bash
cargo run -p utilitycss-cli -- capabilities --output-dir dist/utilitycss
```

The output contains:

- `capabilities.json` — machine-readable registry and theme manifest;
- `capabilities.schema.json` — JSON Schema for manifest consumers;
- `REFERENCE.md` — generated utility and variant reference;
- `llms.txt` and `llms-full.txt` — compact and expanded agent references;
- `compatibility-report.json` — explicit profile and feature status data.

The manifest also records the active browser target. Candidate explanation returns the same target
and uses `UnsupportedBrowser` plus a warning diagnostic when the generated CSS requires a feature
outside that target's support policy.

LSP, protocol, N-API, and WASM integrations delegate to the same compiler APIs. They MUST NOT
maintain independent utility-name lists.
