import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { nodeExecutable, resolveBin, runCommand, runPackageScriptLine } from "./lib/exec.mjs";
import { packageScript } from "./lib/workspaces.mjs";

const repositoryRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

const TS_PACKAGES = [
  "@utilitycss/node",
  "@utilitycss/bun",
  "@utilitycss/vite",
  "@utilitycss/wasm"
];

function run(packageName, scriptName) {
  const { dir, script } = packageScript(repositoryRoot, packageName, scriptName);
  runPackageScriptLine(dir, repositoryRoot, script);
}

function runEach(scriptName, packages) {
  for (const packageName of packages) {
    run(packageName, scriptName);
  }
}

const tasks = {
  build() {
    runEach("build", TS_PACKAGES);
  },
  "build:native"() {
    run("@utilitycss/napi", "build:native");
  },
  "prepare:release"() {
    run("@utilitycss/napi", "prepare:release");
  },
  lint() {
    run("@utilitycss/node", "lint");
    run("@utilitycss/node", "build");
    runEach("lint", ["@utilitycss/bun", "@utilitycss/vite", "@utilitycss/wasm"]);
  },
  typecheck() {
    run("@utilitycss/node", "typecheck");
    run("@utilitycss/node", "build");
    runEach("typecheck", ["@utilitycss/bun", "@utilitycss/vite", "@utilitycss/wasm"]);
  },
  test() {
    runCommand(nodeExecutable(), ["--test", "scripts/exec.test.mjs"], { cwd: repositoryRoot });
    runCommand(nodeExecutable(), ["--test", "scripts/repo-consistency.test.mjs"], {
      cwd: repositoryRoot
    });
    run("@utilitycss/node", "build");
    run("@utilitycss/vite", "build");
    run("@utilitycss/wasm", "build");
    run("@utilitycss/node", "test");
    run("@utilitycss/vite", "test");
    run("@utilitycss/wasm", "test");
  },
  "test:bun"() {
    run("@utilitycss/bun", "test");
  },
  "example-setup"() {
    run("@utilitycss/napi", "build:native");
    run("@utilitycss/node", "build");
    run("@utilitycss/bun", "build");
  },
  "example-typecheck"() {
    const example = join(repositoryRoot, "examples", "bun");
    const [file, ...prefix] = resolveBin(example, repositoryRoot, "tsc");
    runCommand(file, [...prefix, "-p", "tsconfig.json", "--noEmit"], { cwd: example });
  }
};

const name = process.argv[2];
if (!name || !(name in tasks)) {
  console.error(`usage: node scripts/js-tasks.mjs <${Object.keys(tasks).join("|")}>`);
  process.exit(2);
}
tasks[name]();
