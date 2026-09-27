import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { runPackageScriptLine } from "./lib/exec.mjs";
import { packageScript } from "./lib/workspaces.mjs";

const repositoryRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

function usage() {
  console.error("usage: node scripts/run-workspace.mjs <script> <packageName>...");
  process.exit(2);
}

const [scriptName, ...packageNames] = process.argv.slice(2);
if (!scriptName || packageNames.length === 0) {
  usage();
}

for (const packageName of packageNames) {
  const { dir, script } = packageScript(repositoryRoot, packageName, scriptName);
  runPackageScriptLine(dir, repositoryRoot, script);
}
