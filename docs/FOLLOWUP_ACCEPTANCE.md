# Follow-up acceptance after PR #1

## Framework coverage and SSR routing

Linux x86_64, Rust 1.99.1, Node 24.22.1, Chromium headless shell 153.0.8010.0,
official Firefox 157.0. Tests are selected explicitly; ignored fixtures are never
counted as executed by a plain `cargo test`.

- 171 unit tests in ferrite/config/frameworks/server passed (three ignored excluded).
- `ssr_routes`: development source revalidation, emitted artifacts with scope
  hoisting on/off, lazy evaluation isolation, route precedence, query/method and
  status/header preservation, no-fallback 404 and prerender passed.
- `dev_hmr`: React/Vue/Svelte × JS/TS in both engines exercises nested props,
  keyed lists, memo/computed/derived values, hydration identity, child edits,
  CSS, state/reset contracts and syntax recovery. Vue script edits reset the
  component without replacing the document.
- Three standalone tests compile generated applications offline and exercise
  route-specific dotted URLs with buffered embedded, buffered process and streamed
  process runtimes, empty PATH and unrelated working directory.
- Official compiler resource/map/cache and library/server parity fixture passed.
- Strict all-target Clippy for config/server/frameworks/integration crates;
  formatting and whitespace checks.

These are bounded fixtures, not complete Vite or framework parity. See
[VITE_COMPATIBILITY.md](VITE_COMPATIBILITY.md) and the consolidated roadmap.
