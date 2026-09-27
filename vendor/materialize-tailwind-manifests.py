#!/usr/bin/env python3
"""Materialize `[package].*.workspace` inheritance in vendor/tailwind-rs.

Cargo resolves `*.workspace = true` against the *outer* workspace for path
dependencies, so a vendored workspace root's `[workspace.package]` keys are
invisible (this is why `cargo vendor` inlines them too). This script inlines
the seven inherited package keys with the values from
`vendor/tailwind-rs/Cargo.toml [workspace.package]`, making each crate
manifest self-contained. Re-run after re-vendoring.

`readme` is dropped (per-crate READMEs don't exist; it only matters for
publishing).
"""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent / "tailwind-rs"
VALUES = {
    "edition": '"2021"',
    "rust-version": '"1.88"',
    "version": '"0.1.0"',
    "license": '"MIT OR Apache-2.0"',
    "repository": '"https://github.com/nglmercer/tailwind-rs"',
    "homepage": '"https://github.com/nglmercer/tailwind-rs"',
}


def main() -> int:
    root_manifest = (ROOT / "Cargo.toml").read_text()
    for key, value in VALUES.items():
        needle = f"{key} = {value}"
        if needle not in root_manifest:
            print(f"pin drift: inner root lacks `{needle}`; update VALUES", file=sys.stderr)
            return 1
    touched = 0
    for manifest in sorted(ROOT.joinpath("crates").glob("*/Cargo.toml")):
        text = manifest.read_text()
        original = text
        for key, value in VALUES.items():
            text = text.replace(f"{key}.workspace = true", f"{key} = {value}")
        # Drop readme inheritance (no per-crate README to point at).
        lines = [line for line in text.splitlines(True) if line.strip() != "readme.workspace = true"]
        text = "".join(lines)
        # Inline `[lints] workspace = true` (same outer-workspace problem).
        text = text.replace(
            "[lints]\nworkspace = true",
            "[lints.rust]\n"
            'unsafe_code = "deny"\n'
            'missing_docs = "warn"\n'
            'rust_2018_idioms = { level = "deny", priority = -1 }\n'
            "\n"
            "[lints.clippy]\n"
            'all = "warn"',
        )
        if text != original:
            manifest.write_text(text)
            touched += 1
    print(f"materialized {touched} manifests")
    return 0


if __name__ == "__main__":
    sys.exit(main())
