import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { existsSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";

const PATH_SEP = process.platform === "win32" ? ";" : ":";
const MAX_ENV_VALUE = 16 * 1024;
const MAX_PATH_ENTRIES = 64;

/**
 * Copy process env for child processes without exceeding OS ARG_MAX.
 * Bun and npm inject thousands of `npm_package_*` keys; those plus a long PATH
 * make `posix_spawn("/usr/bin/bash", …)` fail with E2BIG.
 */
export function spawnEnv(source = process.env) {
  const env = {};
  for (const [key, value] of Object.entries(source)) {
    if (value == null || value === "") {
      continue;
    }
    if (value.length > MAX_ENV_VALUE) {
      continue;
    }
    if (key.startsWith("npm_package_")) {
      continue;
    }
    if (key.startsWith("npm_config_") && key !== "npm_config_registry") {
      continue;
    }
    if (key.startsWith("npm_lifecycle_")) {
      continue;
    }
    if (key.startsWith("BUN_") && key !== "BUN_INSTALL") {
      continue;
    }
    env[key] = value;
  }
  if (source.PATH) {
    env.PATH = compactPath(source.PATH);
  }
  return env;
}

export function compactPath(pathValue) {
  const seen = new Set();
  const out = [];
  for (const part of pathValue.split(PATH_SEP)) {
    if (!part || seen.has(part)) {
      continue;
    }
    seen.add(part);
    out.push(part);
    if (out.length >= MAX_PATH_ENTRIES) {
      break;
    }
  }
  return out.join(PATH_SEP);
}

export function tokenize(command) {
  return command.trim().split(/\s+/).filter(Boolean);
}

export function expandArg(cwd, arg) {
  if (!arg.includes("*") || arg.startsWith("-")) {
    return [arg];
  }
  const slash = arg.lastIndexOf("/");
  const dir = slash === -1 ? "." : arg.slice(0, slash);
  const pattern = slash === -1 ? arg : arg.slice(slash + 1);
  const regex = new RegExp(`^${pattern.replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*")}$`);
  const target = join(cwd, dir);
  if (!existsSync(target)) {
    throw new Error(`no files match ${arg}: ${target} does not exist`);
  }
  const entries = readdirSync(target, { withFileTypes: true });
  const matches = entries
    .filter((entry) => entry.isFile() && regex.test(entry.name))
    .map((entry) => (dir === "." ? entry.name : `${dir}/${entry.name}`))
    .sort();
  if (matches.length === 0) {
    throw new Error(`no files match ${arg} in ${cwd}`);
  }
  return matches;
}

function tryResolve(requireFrom, specifier) {
  try {
    return requireFrom.resolve(specifier);
  } catch {
    return undefined;
  }
}

/**
 * The executable to use for `node ...` child processes.
 * Under `bun run`, `process.execPath` is the Bun binary, which does not accept
 * Node CLI flags such as `--test`, so fall back to `node` on PATH.
 */
export function nodeExecutable() {
  const exe = process.execPath;
  if (/(?:^|[\\/])bun(?:\.exe)?$/i.test(exe)) {
    return "node";
  }
  return exe;
}

export function resolveBin(cwd, repositoryRoot, name) {
  const requirers = [
    createRequire(join(cwd, "package.json")),
    createRequire(join(repositoryRoot, "package.json")),
    createRequire(join(repositoryRoot, "packages", "utilitycss-node", "package.json")),
    createRequire(join(repositoryRoot, "packages", "utilitycss-napi", "package.json"))
  ];
  const resolveFromWorkspaces = (specifier) => {
    for (const requireFrom of requirers) {
      const resolved = tryResolve(requireFrom, specifier);
      if (resolved) {
        return resolved;
      }
    }
    return undefined;
  };
  if (name === "node") {
    return [nodeExecutable()];
  }
  if (name === "bun") {
    return ["bun"];
  }
  if (name === "tsc") {
    const bin = resolveFromWorkspaces("typescript/bin/tsc");
    if (!bin) {
      throw new Error("typescript is not installed (cannot resolve tsc)");
    }
    return [nodeExecutable(), bin];
  }
  if (name === "napi") {
    const pkg = resolveFromWorkspaces("@napi-rs/cli/package.json");
    if (!pkg) {
      throw new Error("@napi-rs/cli is not installed");
    }
    return [nodeExecutable(), join(dirname(pkg), "dist", "cli.js")];
  }
  const shim = join(repositoryRoot, "node_modules", ".bin", name);
  if (existsSync(shim)) {
    return [shim];
  }
  throw new Error(`unable to resolve command ${name} (no ${shim})`);
}

export function runCommand(file, args, { cwd, env } = {}) {
  const result = spawnSync(file, args, {
    cwd,
    env: spawnEnv(env ?? process.env),
    stdio: "inherit",
    shell: false
  });
  if (result.error) {
    throw result.error;
  }
  if ((result.status ?? 1) !== 0) {
    process.exitCode = result.status ?? 1;
    throw new Error(`${file} ${args.join(" ")} exited with ${result.status}`);
  }
}

export function runShellFreeCommand(cwd, repositoryRoot, command) {
  const tokens = tokenize(command);
  if (tokens.length === 0) {
    throw new Error("empty command");
  }
  const [file, ...resolved] = resolveBin(cwd, repositoryRoot, tokens[0]);
  const args = [...resolved];
  for (const token of tokens.slice(1)) {
    args.push(...expandArg(cwd, token));
  }
  runCommand(file, args, { cwd });
}

export function runPackageScriptLine(cwd, repositoryRoot, script) {
  const parts = script.split("&&").map((part) => part.trim()).filter(Boolean);
  for (const part of parts) {
    runShellFreeCommand(cwd, repositoryRoot, part);
  }
}
