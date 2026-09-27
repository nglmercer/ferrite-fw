import { build } from "vite";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { utilitycss } from "../packages/utilitycss-vite/dist/index.js";

const root = mkdtempSync(join(tmpdir(), "utilitycss-vite-smoke-"));
try {
  writeFileSync(
    join(root, "index.html"),
    '<!doctype html><html><body class="flex p-4"></body><script type="module" src="/main.js"></script></html>'
  );
  writeFileSync(join(root, "main.js"), 'import "./app.css"; import "virtual:utilitycss.css";');
  writeFileSync(join(root, "app.css"), ".button { @apply flex p-4 hover:bg-red-500; }");

  const result = await build({
    root,
    logLevel: "silent",
    plugins: [utilitycss()],
    build: { write: false }
  });
  const css = result.output
    .filter((entry) => entry.type === "asset")
    .map((entry) => String(entry.source))
    .join("\n");
  if (!css.includes(".button{") || !css.includes("display:flex") || !css.includes("padding:1rem") || !css.includes(".button:hover{background-color:#ef4444}")) {
    throw new Error(`Vite build emitted unexpected CSS: ${css}`);
  }
  console.log("Vite build smoke passed");
} finally {
  rmSync(root, { recursive: true, force: true });
}
