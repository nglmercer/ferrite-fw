// Tier-3 Node adapter guest driver (embedded by `node_adapter.rs`).
// Protocol: stdin JSON-lines `{id, cmd, ...}` → stdout JSON-lines
// `{id, ok, result?, error?}`. Boot prints `{"ferrite":3}`.
//
// Guest plugins are ESM modules exporting hook functions with Vite-like
// signatures: `resolveId(id)`, `load(id)`, `transform(code, id)`.
// A hook returning `null`/`undefined` means "skip" (Rust keeps its default).
import { createInterface } from "node:readline";

// Protocol owns the original stdout writer. Ordinary guest output is logs.
const protocolWrite = process.stdout.write.bind(process.stdout);
process.stdout.write = process.stderr.write.bind(process.stderr);
const plugins = new Map();

function respond(id, ok, result, error) {
  const message = { id, ok, result: result === undefined ? null : result };
  if (error !== undefined) message.error = error;
  protocolWrite(JSON.stringify(message) + "\n");
}

async function register(id, name, entry) {
  const module = await import(entry);
  plugins.set(name, module);
  respond(id, true, null);
}

async function hook(id, name, hook, input) {
  const module = plugins.get(name);
  if (!module) {
    respond(id, false, null, `unknown plugin \`${name}\``);
    return;
  }
  const hookFn = module[hook];
  if (typeof hookFn !== "function") {
    respond(id, true, null);
    return;
  }
  let args;
  if (hook === "transform") {
    args = [input && input.code, input && input.id];
  } else if (hook === "resolveId" || hook === "load") {
    args = [input && input.id !== undefined ? input.id : input];
  } else {
    args = [input];
  }
  const result = await hookFn(...args);
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
      if (message.cmd === "register") {
        await register(message.id, message.name, message.entry);
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
