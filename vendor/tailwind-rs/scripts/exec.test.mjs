import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";
import test from "node:test";

import { compactPath, expandArg, spawnEnv, tokenize } from "./lib/exec.mjs";
import { npmCommand } from "./lib/npm.mjs";

test("spawnEnv drops npm_package metadata and oversized values", () => {
  const env = spawnEnv({
    PATH: ["/usr/bin", "/usr/bin", "/opt/bin"].join(delimiter),
    HOME: "/home/dev",
    npm_package_dependencies_foo: "1.0.0",
    npm_config_cache: "/tmp/npm",
    npm_config_registry: "https://registry.npmjs.org/",
    BUN_DEBUG: "1",
    BUN_INSTALL: "/home/dev/.bun",
    HUGE: "x".repeat(20_000)
  });
  assert.equal(env.HOME, "/home/dev");
  assert.equal(env.BUN_INSTALL, "/home/dev/.bun");
  assert.equal(env.npm_config_registry, "https://registry.npmjs.org/");
  assert.equal(env.npm_package_dependencies_foo, undefined);
  assert.equal(env.npm_config_cache, undefined);
  assert.equal(env.BUN_DEBUG, undefined);
  assert.equal(env.HUGE, undefined);
  assert.equal(env.PATH.split(delimiter).filter((part) => part === "/usr/bin").length, 1);
});

test("compactPath caps unique entries", () => {
  const parts = Array.from({ length: 80 }, (_, index) => `/p${index}`);
  const compacted = compactPath(parts.join(delimiter));
  assert.equal(compacted.split(delimiter).length, 64);
});

test("tokenize splits on whitespace", () => {
  assert.deepEqual(tokenize("  napi build --release  "), ["napi", "build", "--release"]);
});

test("npmCommand honours npm-cli.js and never runs Bun as npm", () => {
  assert.deepEqual(
    npmCommand({
      npmExecpath: "/usr/lib/node_modules/npm/bin/npm-cli.js",
      platform: "linux",
      node: "/usr/bin/node"
    }),
    ["/usr/bin/node", "/usr/lib/node_modules/npm/bin/npm-cli.js"]
  );
  assert.deepEqual(
    npmCommand({
      npmExecpath: "C:\\Program Files\\nodejs\\node_modules\\npm\\bin\\npm-cli.js",
      platform: "win32",
      node: "C:\\Program Files\\nodejs\\node.exe"
    }),
    [
      "C:\\Program Files\\nodejs\\node.exe",
      "C:\\Program Files\\nodejs\\node_modules\\npm\\bin\\npm-cli.js"
    ]
  );
  // `bun run` points npm_execpath at the Bun executable; the bundled npm next
  // to the real Node binary is used instead.
  assert.deepEqual(
    npmCommand({
      npmExecpath: "C:\\Users\\dev\\.bun\\bin\\bun.exe",
      platform: "win32",
      node: "C:\\Program Files\\nodejs\\node.exe"
    }),
    [
      "C:\\Program Files\\nodejs\\node.exe",
      "C:\\Program Files\\nodejs\\node_modules\\npm\\bin\\npm-cli.js"
    ]
  );
  // Direct `bun script.mjs` has no Node path; Bun resolves bare `npm` itself.
  // (`null` simulates an unset variable; explicit `undefined` would read the
  // real test-runner environment through the default parameter.)
  assert.deepEqual(
    npmCommand({ npmExecpath: null, platform: "win32", node: "node" }),
    ["npm"]
  );
  assert.deepEqual(
    npmCommand({ npmExecpath: null, platform: "linux", node: "/usr/bin/node" }),
    ["npm"]
  );
});

test("expandArg expands a directory glob without a shell", () => {
  const dir = join(tmpdir(), `utilitycss-glob-${process.pid}`);
  mkdirSync(join(dir, "test"), { recursive: true });
  writeFileSync(join(dir, "test", "index.test.mjs"), "");
  writeFileSync(join(dir, "test", "native.test.mjs"), "");
  writeFileSync(join(dir, "test", "skip.txt"), "");
  assert.deepEqual(expandArg(dir, "test/*.test.mjs"), [
    "test/index.test.mjs",
    "test/native.test.mjs"
  ]);
});
