# Snapshot font fixture

`snapshot-probe.ttf` is an original three-glyph font (`.notdef`, space, `F`),
created from the polygon coordinates in `build_snapshot_font.py`. Its glyph
advance is 1,000 units/em, so `FFFF` at 40 px measures 160 px after loading;
the tests prove the actual fallback width is different. It uses the repository's
[MIT license](../../../../LICENSE-MIT) and copies no external font outlines.

The committed 1,116-byte file has SHA-256
`eb22d9ac770bd9abf4e4caf1c5285ac033d76df7171ab6fbb170d1d13113748c`.
The generator fixes the font timestamps for reproducibility. Regeneration needs
development-only `fonttools==4.66.1` and Python:

```sh
python3 -m pip install fonttools==4.66.1
python3 crates/ferrite-e2e/tests/fixtures/build_snapshot_font.py
```

The generator follows the public
[FontBuilder table setup contract](https://github.com/fonttools/fonttools/blob/4.66.1/Lib/fontTools/fontBuilder.py).
Rust tests use `include_bytes!` and need no Python/fontTools installation or system
font. A local HTTP server holds the real font response until explicitly released;
the browser's FontFaceSet promises are never replaced by the fixture.
