import { dirname, join } from "node:path";

import { nodeExecutable } from "./exec.mjs";

/**
 * Resolve the `[file, ...prefixArgs]` needed to run npm without a shell.
 *
 * Under `bun run`, `npm_execpath` points at the Bun executable rather than
 * npm-cli.js, so it is only honoured when it really is npm. All inputs are
 * injectable so tests can cover every runner without spawning one.
 */
export function npmCommand({
  npmExecpath = process.env.npm_execpath,
  platform = process.platform,
  node = nodeExecutable()
} = {}) {
  const npmCli = /npm-cli\.js$/.test(npmExecpath ?? "") ? npmExecpath : undefined;
  if (npmCli) {
    return [node, npmCli];
  }
  if (platform === "win32" && node !== "node") {
    return [node, join(dirname(node), "node_modules", "npm", "bin", "npm-cli.js")];
  }
  return ["npm"];
}
