// Tier-3 Node adapter guest driver (embedded by `node_adapter.rs`).
// Protocol: stdin JSON-lines `{id, cmd, ...}` → stdout JSON-lines
// `{id, ok, result?, error?}`. Boot prints `{"ferrite":3}`.
//
// Explicit hook profiles accept one factory/object or named functions:
// resolveId(id, importer, options), load(id, options), transform(code, id, options).
// A hook returning `null`/`undefined` means "skip" (Rust keeps its default).
import { createInterface } from "node:readline";
import { realpathSync } from "node:fs";
import { isAbsolute } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

// Protocol owns the original stdout writer. Ordinary guest output is logs.
const protocolWrite = process.stdout.write.bind(process.stdout);
process.stdout.write = process.stderr.write.bind(process.stderr);
const plugins = new Map();
const hookPlugins = new Map();
const registeredEntries = new Set();
const supportedHooks = new Set(['resolveId', 'load', 'transform']);
// Capture the running host before any guest is evaluated. This is capability
// identity, not a promise of compatibility for every Node version.
const hostProfile = {
  nodeVersion: process.version,
  executable: realpathSync(process.execPath),
  platform: process.platform,
  arch: process.arch,
  versions: {...process.versions},
};
function hookHandler(value, name) {
  if (value == null) return null;
  if (typeof value === 'function') return value;
  if (typeof value === 'object' && typeof value.handler === 'function') {
    for (const key of Object.keys(value)) {
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

function respond(id, ok, result, error) {
  const message = { id, ok, result: result === undefined ? null : result };
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
    for (const key of Object.keys(plugin)) {
      if (key === 'name' || key === 'version') continue;
      if (!supportedHooks.has(key)) throw new Error(`unsupported foreign plugin property/hook ${name}.${key}; use the native plugin API`);
      hookHandler(plugin[key], `${name}.${key}`);
    }
  }
  plugins.set(name, module);
  if (profile === 'hooks') hookPlugins.set(name, plugin);
  else hookPlugins.delete(name);
  respond(id, true, null);
}

async function hook(id, name, hook, input) {
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
  const result = await hookFn.apply(hookContext, args);
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
      if (message.cmd === "profile") {
        respond(message.id, true, hostProfile);
      } else if (message.cmd === "register") {
        await register(message.id, message.name, message.entry, message.profile, message.options);
      } else if (message.cmd === "call") {
        const module = plugins.get(message.name);
        if (!module || typeof module[message.export] !== "function") throw new Error(`missing callable export ${message.name}.${message.export}`);
        respond(message.id, true, await module[message.export](message.input));
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
