import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { npmCommand } from "./lib/npm.mjs";

const npm = npmCommand();
const root = mkdtempSync(join(tmpdir(), "utilitycss-packed-bun-"));

try {
  const napiTarball = pack("@utilitycss/napi");
  const nodeTarball = pack("@utilitycss/node");
  const bunTarball = pack("@utilitycss/bun");
  const platformTarballs = [
    ["@utilitycss/napi-darwin-arm64", "packages/utilitycss-napi/npm/darwin-arm64", "darwin", "arm64"],
    ["@utilitycss/napi-darwin-x64", "packages/utilitycss-napi/npm/darwin-x64", "darwin", "x64"],
    ["@utilitycss/napi-linux-x64-gnu", "packages/utilitycss-napi/npm/linux-x64-gnu", "linux", "x64"],
    ["@utilitycss/napi-win32-x64-msvc", "packages/utilitycss-napi/npm/win32-x64-msvc", "win32", "x64"]
  ]
    .filter(([, cwd, platform, arch]) =>
      process.platform === platform && process.arch === arch && readdirSync(cwd).some((entry) => entry.endsWith(".node"))
    )
    .map(([name, cwd]) => packDirectory(name, cwd));

  execFileSync(npm[0], [...npm.slice(1), "init", "-y"], { cwd: root, stdio: "ignore" });
  execFileSync(
    npm[0],
    [...npm.slice(1), "install", "--ignore-scripts", "--omit=optional", napiTarball, nodeTarball, bunTarball, ...platformTarballs],
    { cwd: root, stdio: "inherit" }
  );

  writeFileSync(join(root, "entry.html"), '<link rel="stylesheet" href="./app.css"><link rel="stylesheet" href="utilitycss"><main class="flex p-4"></main>\n');
  writeFileSync(join(root, "app.css"), '.button { @apply flex p-4; }\n');
  writeFileSync(
    join(root, "smoke.ts"),
    `import "./app.css";
import { utilitycss } from "@utilitycss/bun";
const result = await Bun.build({
  entrypoints: ["entry.html"],
  outdir: "dist",
  target: "browser",
  plugins: [utilitycss()]
});
if (!result.success) throw new Error(JSON.stringify(result.logs));
const outputs = await Promise.all(result.outputs.map((output) => output.text()));
const css = outputs.join("\\n");
if (!css.includes(".flex") || !css.includes(".p-4") || !css.includes(".button")) throw new Error(css);
console.log("packed Bun smoke passed");
`
  );
  execFileSync("bun", ["run", "smoke.ts"], { cwd: root, stdio: "inherit" });
} finally {
  rmSync(root, { recursive: true, force: true });
}

function pack(workspace) {
  const output = execFileSync(npm[0], [...npm.slice(1), "pack", "--workspace", workspace, "--pack-destination", root], {
    encoding: "utf8"
  }).trim();
  return join(root, output.split(/\r?\n/).at(-1));
}

function packDirectory(name, cwd) {
  const output = execFileSync(npm[0], [...npm.slice(1), "pack", "--pack-destination", root], {
    encoding: "utf8",
    cwd
  }).trim();
  const tarball = join(root, output.split(/\r?\n/).at(-1));
  if (!existsSync(tarball)) {
    throw new Error(`${name} did not produce a tarball`);
  }
  return tarball;
}
