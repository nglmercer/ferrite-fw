import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { npmCommand } from "./lib/npm.mjs";

const npm = npmCommand();
const root = mkdtempSync(join(tmpdir(), "utilitycss-packed-smoke-"));
try {
  const napiTarball = pack("@utilitycss/napi");
  const nodeTarball = pack("@utilitycss/node");
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
  execFileSync(npm[0], [...npm.slice(1), "init", "-y"], {
    cwd: root,
    stdio: "ignore"
  });
  execFileSync(npm[0], [...npm.slice(1), "install", "--ignore-scripts", "--omit=optional", napiTarball, nodeTarball, ...platformTarballs], {
    cwd: root,
    stdio: "inherit"
  });
  const smoke = `
    import { createCompiler } from "@utilitycss/node";
    const compiler = createCompiler();
    compiler.updateSource("src/app.html", '<div class="flex p-4"></div>', "src/app.html");
    const result = compiler.build();
    if (!result.css.includes(".flex{display:flex;}") || !result.css.includes(".p-4{padding:1rem;}")) {
      throw new Error(\`packed compiler emitted unexpected CSS: \${result.css}\`);
    }
    const stylesheet = compiler.transformStylesheet(
      "src/app.css",
      ".button { @apply flex p-4; }",
      "src/app.css"
    );
    if (stylesheet.diagnostics.length !== 0 || stylesheet.css !== ".button{display:flex;padding:1rem;}") {
      throw new Error(\`packed stylesheet transform emitted unexpected CSS: \${stylesheet.css}\`);
    }
    console.log("packed Node smoke passed");
  `;
  execFileSync(process.execPath, ["--input-type=module", "-e", smoke], {
    cwd: root,
    stdio: "inherit"
  });
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
