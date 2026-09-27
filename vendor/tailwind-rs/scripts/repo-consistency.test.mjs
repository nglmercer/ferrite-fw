import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

function readJson(path) {
  return JSON.parse(readFileSync(join(root, path), "utf8"));
}

test("MANIFEST.json matches git-tracked files", () => {
  const manifest = readJson("MANIFEST.json");
  const tracked = execFileSync("git", ["ls-files"], { cwd: root, encoding: "utf8" })
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "" && line !== "MANIFEST.json");
  assert.deepEqual(manifest.files, tracked);
  assert.equal(manifest.file_count, tracked.length);
});

test("adapter packages declare runtime engines matching the README", () => {
  const readme = readFileSync(join(root, "README.md"), "utf8");
  for (const name of ["node", "vite", "wasm", "napi"]) {
    const pkg = readJson(`packages/utilitycss-${name}/package.json`);
    assert.equal(pkg.engines?.node, ">=20.0.0", `@utilitycss/${name} declares Node 20+`);
  }
  const bun = readJson("packages/utilitycss-bun/package.json");
  assert.equal(bun.engines?.bun, ">=1.4.0", "@utilitycss/bun declares Bun 1.4+");
  assert.match(readme, /Node\.js\*\* 20\+/, "README declares the Node floor");
  assert.match(readme, /Bun\*\* 1\.4\+/, "README declares the Bun floor");
});
