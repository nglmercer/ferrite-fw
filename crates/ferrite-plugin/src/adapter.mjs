// Tier-3 Node adapter guest driver (embedded by `node_adapter.rs`).
// Protocol: stdin JSON-lines `{id, cmd, ...}` → stdout JSON-lines
// `{id, ok, result?, error?}`. Boot prints `{"ferrite":3}`.
//
// Explicit hook profiles accept one factory/object or named functions:
// resolveId(id, importer, options), load(id, options), transform(code, id, options).
// A hook returning `null`/`undefined` means "skip" (Rust keeps its default).
import { createInterface } from "node:readline";
import { createHash } from "node:crypto";
import * as nodeModule from "node:module";
import { readFileSync, realpathSync } from "node:fs";
import { isAbsolute } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

// Protocol owns the original stdout writer. Ordinary guest output is logs.
const protocolWrite = process.stdout.write.bind(process.stdout);
process.stdout.write = process.stderr.write.bind(process.stderr);
const plugins = new Map();
const hookPlugins = new Map();
const invalidHookPlugins = new Set();
const registeredEntries = new Set();
const loadedFiles = new Map();
function fileHash(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}
if (typeof nodeModule.registerHooks !== 'function') throw new Error('Ferrite Node host requires node:module.registerHooks for dependency tracking; select a tested Node version (26.10.0) explicitly');
nodeModule.registerHooks({load(url, context, nextLoad) {
  const result = nextLoad(url, context);
  if (url.startsWith('file:')) {
    const path = realpathSync(fileURLToPath(url));
    loadedFiles.set(path, fileHash(path));
  }
  return result;
}});
const supportedHooks = new Set(['resolveId', 'load', 'transform']);
// Capture the running host before any guest is evaluated. This is capability
// identity, not a promise of compatibility for every Node version.
function environmentHash() {
  return createHash('sha256').update(JSON.stringify(Object.entries(process.env).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0))).digest('hex');
}
const hostProfile = {
  nodeVersion: process.version,
  executable: realpathSync(process.execPath),
  platform: process.platform,
  arch: process.arch,
  versions: {...process.versions},
  execArgs: [...process.execArgv],
  nodeOptions: process.env.NODE_OPTIONS ?? null,
  cwd: realpathSync(process.cwd()),
  environmentHash: environmentHash(),
};
function assertHostState(stage) {
  for (const [path, hash] of loadedFiles) {
    let current;
    try { current = fileHash(path); } catch { current = null; }
    if (current !== hash) throw new Error(`Node guest dependency changed: ${path}; recreate the explicit host before compiling`);
  }
  if (realpathSync(process.cwd()) !== hostProfile.cwd || environmentHash() !== hostProfile.environmentHash) {
    throw new Error(`Node worker environment or cwd changed ${stage}; recreate the explicit host and avoid mutating process.env/process.chdir in compiler or plugin hooks`);
  }
}
function dataKeys(value, name, moduleNamespace = false) {
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== null && prototype !== Object.prototype) {
    throw new Error(`unsupported foreign plugin prototype ${name}; return a plain hook object`);
  }
  return Reflect.ownKeys(value).filter(key => {
    if (moduleNamespace && key === Symbol.toStringTag) return false;
    if (typeof key === 'symbol') throw new Error(`unsupported foreign plugin symbol property ${name}.${String(key)}`);
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (descriptor.get || descriptor.set) throw new Error(`unsupported foreign plugin accessor ${name}.${key}; use data properties`);
    return true;
  });
}
function hookHandler(value, name) {
  if (value == null) return null;
  if (typeof value === 'function') return value;
  if (typeof value === 'object') {
    const keys = dataKeys(value, name);
    if (typeof value.handler !== 'function') throw new Error(`invalid hook ${name}; expected a function or {handler: function}`);
    for (const key of keys) {
      if (key !== 'handler' && !(key === 'order' && value.order == null)) {
        throw new Error(`unsupported hook metadata ${name}.${key}; use an unordered handler or the native plugin API`);
      }
    }
    return value.handler;
  }
  throw new Error(`invalid hook ${name}; expected a function or {handler: function}`);
}
const hookContext = new Proxy(Object.freeze({}), {
  get(_target, key) { throw new Error(`unsupported foreign hook context this.${String(key)}; use the native plugin API`); }
});

function validateJsonResult(value, path = '$', active = new Set(), depth = 0) {
  const fail = reason => { throw new Error(`non-JSON result at ${path}: ${reason}; return plain JSON data`); };
  if (depth > 128) fail('nesting exceeds 128 levels');
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return;
  if (typeof value === 'number') { if (!Number.isFinite(value)) fail('non-finite number'); return; }
  if (typeof value !== 'object') fail(`unsupported ${typeof value}`);
  if (active.has(value)) fail('cyclic reference');
  const array = Array.isArray(value);
  if (array && Object.getPrototypeOf(value) !== Array.prototype) fail('unsupported array prototype');
  if (!array && Object.getPrototypeOf(value) !== Object.prototype && Object.getPrototypeOf(value) !== null) fail('unsupported object prototype');
  active.add(value);
  for (const key of Reflect.ownKeys(value)) {
    if (array && key === 'length') continue;
    if (typeof key === 'symbol') fail('symbol property');
    if (array && (!/^(0|[1-9][0-9]*)$/.test(key) || Number(key) >= value.length)) fail('named array property');
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (descriptor.get || descriptor.set) fail(`accessor property ${key}`);
    if (!descriptor.enumerable) fail(`non-enumerable property ${key}`);
    validateJsonResult(descriptor.value, `${path}.${key}`, active, depth + 1);
  }
  if (array && Object.keys(value).length !== value.length) fail('sparse array');
  active.delete(value);
}
function respond(id, ok, result, error) {
  const normalized = result === undefined ? null : result;
  validateJsonResult(normalized);
  const message = { id, ok, result: normalized };
  if (error !== undefined) message.error = error;
  protocolWrite(JSON.stringify(message) + "\n");
}

async function register(id, name, entry, profile, options) {
  if (typeof entry !== 'string') throw new Error('plugin entry must be a filesystem path or file URL');
  let file = entry;
  if (!isAbsolute(entry) && /^[a-z][a-z0-9+.-]*:/i.test(entry)) {
    const url = new URL(entry);
    if (url.protocol !== 'file:' || url.search || url.hash) {
      throw new Error('plugin entry must be a local file URL without query or fragment; recreate the explicit host instead of cache-busting imports');
    }
    file = fileURLToPath(url);
  }
  // Import and registration use the same canonical file identity, including
  // symlink targets and percent-encoded file URLs.
  entry = pathToFileURL(realpathSync(file)).href;
  if (hookPlugins.has(name) || (profile === 'hooks' && registeredEntries.has(entry))) {
    throw new Error(`hook plugin ${name} is already registered or its entry was loaded; recreate the explicit Node host to change registrations`);
  }
  registeredEntries.add(entry);
  const module = await import(entry);
  let plugin;
  if (profile === 'hooks') {
    plugin = module.default ?? module;
    if (typeof plugin === 'function') plugin = await plugin(options);
    if (!plugin || typeof plugin !== 'object' || Array.isArray(plugin)) throw new Error(`plugin ${name} must return one hook object; arrays and conditional plugins are unsupported`);
    for (const key of dataKeys(plugin, name, plugin === module)) {
      if (key === 'name' || key === 'version') continue;
      if (!supportedHooks.has(key)) throw new Error(`unsupported foreign plugin property/hook ${name}.${key}; use the native plugin API`);
      hookHandler(plugin[key], `${name}.${key}`);
    }
  }
  assertHostState(`during registration of ${name}`);
  plugins.set(name, module);
  if (profile === 'hooks') hookPlugins.set(name, plugin);
  else hookPlugins.delete(name);
  respond(id, true, null);
}

async function hook(id, name, hook, input) {
  if (invalidHookPlugins.has(name)) throw new Error(`foreign plugin ${name} discovered dependencies during a hook; recreate the explicit host and import dependencies during registration`);
  const module = hookPlugins.get(name) || plugins.get(name);
  if (!module) {
    respond(id, false, null, `unknown plugin \`${name}\``);
    return;
  }
  if (!hookPlugins.has(name) && module.default != null && module[hook] == null) {
    throw new Error(`plugin ${name} exports a default factory/object; register it with register_hook_plugin and explicit factory options`);
  }
  const hookFn = hookHandler(module[hook], `${name}.${hook}`);
  if (hookFn == null) {
    respond(id, true, null);
    return;
  }
  let args;
  if (hook === "transform") {
    args = [input && input.code, input && input.id, input && input.options];
  } else if (hook === "resolveId") {
    args = [input && input.id !== undefined ? input.id : input, input && input.importer, input && input.options];
  } else if (hook === "load") {
    args = [input && input.id !== undefined ? input.id : input, input && input.options];
  } else {
    args = [input];
  }
  const fileCount = loadedFiles.size;
  let result;
  try {
    result = await hookFn.apply(hookContext, args);
  } finally {
    if (loadedFiles.size !== fileCount) {
      invalidHookPlugins.add(name);
      throw new Error(`foreign hook ${name}.${hook} loaded new file dependencies; recreate the explicit host and import them during registration before compiling`);
    }
    assertHostState(`during ${name}.${hook}`);
  }
  respond(id, true, result === undefined ? null : result);
}

protocolWrite(JSON.stringify({ ferrite: 3 }) + "\n");

// Serialize handling: `register` must settle before later lines run.
let tail = Promise.resolve();
const rl = createInterface({ input: process.stdin, terminal: false });
rl.on("line", (line) => {
  if (!line.trim()) return;
  let message;
  try {
    message = JSON.parse(line);
  } catch {
    return;
  }
  tail = tail
    .then(async () => {
      assertHostState(`before ${message.cmd}`);
      if (message.cmd === "dependencies") {
        respond(message.id, true, Object.fromEntries(loadedFiles));
      } else if (message.cmd === "profile") {
        respond(message.id, true, hostProfile);
      } else if (message.cmd === "register") {
        await register(message.id, message.name, message.entry, message.profile, message.options);
      } else if (message.cmd === "call") {
        const module = plugins.get(message.name);
        if (!module || typeof module[message.export] !== "function") throw new Error(`missing callable export ${message.name}.${message.export}`);
        const result = await module[message.export](message.input);
        assertHostState(`during ${message.name}.${message.export}`);
        respond(message.id, true, result);
      } else if (message.cmd === "hook") {
        await hook(message.id, message.name, message.hook, message.input);
      } else {
        respond(message.id, false, null, `unknown cmd \`${message.cmd}\``);
      }
    })
    .catch((error) => {
      respond(message.id, false, null, String((error && error.stack) || error));
    });
});
