import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

export function packageDirectories(repositoryRoot) {
  const packagesRoot = join(repositoryRoot, "packages");
  return readdirSync(packagesRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => join(packagesRoot, entry.name));
}

export function workspaceMap(repositoryRoot) {
  const map = new Map();
  for (const dir of packageDirectories(repositoryRoot)) {
    const manifest = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
    map.set(manifest.name, { dir, manifest });
  }
  return map;
}

export function packageScript(repositoryRoot, packageName, scriptName) {
  const entry = workspaceMap(repositoryRoot).get(packageName);
  if (!entry) {
    throw new Error(`unknown workspace package ${packageName}`);
  }
  const script = entry.manifest.scripts?.[scriptName];
  if (!script) {
    throw new Error(`${packageName} has no script ${scriptName}`);
  }
  return { dir: entry.dir, script };
}
