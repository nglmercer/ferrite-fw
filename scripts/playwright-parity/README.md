# Regenerating the Playwright API inventory

The matrix uses official API documents pinned to Playwright v1.63.0.
`sources.json` records every source path, including test, reporter, Android and
Electron classes. No browser installation or Node dependency is required.

```bash
python3 scripts/playwright-parity/build_matrix.py --fetch
```

To reuse an existing directory of the pinned Markdown documents:

```bash
python3 scripts/playwright-parity/build_matrix.py --upstream /path/to/upstream
```

The default cache is `~/.cache/ferrite-playwright-audit/upstream`. Every manifest
document must exist; incomplete inventories fail explicitly. The script writes
`PLAYWRIGHT-API-MATRIX.md`, plus `classified-inventory.json` and `summary.json`
inside the upstream directory. It resolves local evidence links from actual
public Rust methods/types/fields and rejects nonexistent mapping targets.

Mappings are reviewed manually. A similarly named method does not establish
behavioral compatibility; `Partial` entries retain known semantic, option and
engine differences. Arbitrary evaluation and raw protocol access do not count
as dedicated equivalents. Counts describe documented members, not a parity
percentage. See `PLAYWRIGHT-PARITY.md` for runtime evidence and deferred work.
