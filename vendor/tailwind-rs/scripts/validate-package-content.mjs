import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";

import { npmCommand } from "./lib/npm.mjs";

const npm = npmCommand();
const packages = [
  { name: "@utilitycss/napi", cwd: "packages/utilitycss-napi", api: "index.d.ts" },
  { name: "@utilitycss/node", cwd: "packages/utilitycss-node", api: "dist/index.d.ts" },
  { name: "@utilitycss/bun", cwd: "packages/utilitycss-bun", api: "dist/index.js" },
  { name: "@utilitycss/vite", cwd: "packages/utilitycss-vite", api: "dist/index.js" },
  { name: "@utilitycss/wasm", cwd: "packages/utilitycss-wasm", api: "dist/index.d.ts" },
  { name: "@utilitycss/napi-darwin-arm64", cwd: "packages/utilitycss-napi/npm/darwin-arm64" },
  { name: "@utilitycss/napi-darwin-x64", cwd: "packages/utilitycss-napi/npm/darwin-x64" },
  { name: "@utilitycss/napi-linux-x64-gnu", cwd: "packages/utilitycss-napi/npm/linux-x64-gnu" },
  { name: "@utilitycss/napi-win32-x64-msvc", cwd: "packages/utilitycss-napi/npm/win32-x64-msvc" }
];
const forbidden = /(^|\/)(?:test|tests|fixtures|node_modules|\.git)(\/|$)|(?:\.log|\.map)$/;

for (const { name, cwd, api } of packages) {
  const metadata = JSON.parse(readFileSync(join(cwd, "package.json"), "utf8"));
  if (metadata.license !== "MIT OR Apache-2.0") {
    throw new Error(`${name} does not declare the repository license`);
  }
  const output = execFileSync(npm[0], [...npm.slice(1), "pack", "--dry-run", "--json"], {
    encoding: "utf8",
    cwd
  });
  const result = JSON.parse(output)[0];
  for (const entry of result.files) {
    if (forbidden.test(entry.path)) {
      throw new Error(`${name} contains forbidden package content: ${entry.path}`);
    }
  }
  if (!result.files.some((entry) => entry.path === "package.json")) {
    throw new Error(`${name} does not contain package.json`);
  }
  if (api) {
    const apiPath = join(cwd, api);
    if (!readFileSync(apiPath, "utf8").includes("transformStylesheet")) {
      throw new Error(`${name} does not contain the stylesheet transformation API`);
    }
  }
  console.log(`${name}: ${result.files.length} files`);
}
