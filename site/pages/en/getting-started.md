---
title: Getting started
order: 2
---

# Getting started

You need a Rust toolchain (stable, 1.88+) and nothing else. Node.js is
optional and only used by tier-3 foreign plugins.

## Install the CLI

```bash
cargo install --path crates/ferrite-cli
ferrite --version
```

Or run it from the repo without installing:

```bash
cargo run -p ferrite-cli -- --help
```

## Your first project

```bash
ferrite create my-app
cd my-app
ferrite dev
```

Open the printed URL (default `http://127.0.0.1:5173`). Edits to any
module hot-reload; edits to Markdown or styles rebuild the page CSS.

A minimal project is just an `index.html` plus a module script:

```html
<!doctype html>
<html>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.js"></script>
  </body>
</html>
```

```js
// src/main.js
document.getElementById("app").textContent = "Hello, Ferrite.";
```

## The dev loop

1. `ferrite dev` — native ESM per module, HMR over WebSocket.
2. `ferrite build` — hashed production bundle into `dist/`.
3. `ferrite preview` — serve `dist/` locally to check the real output.

When something looks wrong, `ferrite compat` runs live self-checks
against the real pipeline and reports what failed loudly instead of
guessing.
