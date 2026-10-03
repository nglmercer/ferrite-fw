// Ferrite HMR client (`/@ferrite/client`).
// Implements `import.meta.hot`, style updates, and the error overlay hook.
const socketProtocol = location.protocol === "https:" ? "wss:" : "ws:";
const socketHost = `${socketProtocol}//${location.host}/@ferrite/hmr`;

const hotModules = new Map(); // id -> { callbacks, disposeCallbacks, data }
const customHandlers = new Map(); // event -> Set<cb>
const styleSheets = new Map(); // id -> HTMLStyleElement

function clearCustomHandlers(entry) {
  for (const [event, callbacks] of entry.customHandlers ?? []) {
    const handlers = customHandlers.get(event);
    for (const wrapper of callbacks.values()) handlers?.delete(wrapper);
    if (handlers?.size === 0) customHandlers.delete(event);
  }
  entry.customHandlers = new Map();
}

function createHotContext(id) {
  if (!hotModules.has(id)) {
    hotModules.set(id, { callbacks: [], disposeCallbacks: [], pruneCallbacks: [], data: {} });
  }
  const entry = hotModules.get(id);
  clearCustomHandlers(entry);
  const generation = {};
  entry.generation = generation;
  entry.callbacks = [];
  entry.disposeCallbacks = [];
  entry.pruneCallbacks = [];
  entry.selfAccept = false;
  return {
    data: entry.data,
    accept(dep, cb) {
      if (typeof dep === "undefined") entry.selfAccept = true;
      else if (typeof dep === "function") entry.callbacks.push({ deps: [id], cb: dep, array: false });
      else {
        const deps = Array.isArray(dep) ? dep : [dep];
        entry.callbacks.push({ deps, cb, array: Array.isArray(dep) });
      }
    },
    dispose(cb) {
      entry.disposeCallbacks.push(cb);
    },
    prune(cb) {
      entry.pruneCallbacks.push(cb);
    },
    invalidate(message) {
      console.log(`[ferrite] invalidate ${id}: ${message ?? ""}`);
      location.reload();
    },
    on(event, cb) {
      if (hotModules.get(id) !== entry || entry.generation !== generation) return;
      if (!customHandlers.has(event)) customHandlers.set(event, new Set());
      if (!entry.customHandlers.has(event)) entry.customHandlers.set(event, new Map());
      const owned = entry.customHandlers.get(event);
      if (!owned.has(cb)) {
        const wrapper = (data) => cb(data);
        owned.set(cb, wrapper);
        customHandlers.get(event).add(wrapper);
      }
    },
    off(event, cb) {
      if (hotModules.get(id) !== entry || entry.generation !== generation) return;
      const owned = entry.customHandlers.get(event);
      const wrapper = owned?.get(cb);
      if (wrapper) customHandlers.get(event)?.delete(wrapper);
      owned?.delete(cb);
      if (owned?.size === 0) entry.customHandlers.delete(event);
      if (customHandlers.get(event)?.size === 0) customHandlers.delete(event);
    },
    send(event, data) {
      socket?.send(JSON.stringify({ type: "custom", event, data }));
    },
  };
}

globalThis.__ferrite_create_hot__ = createHotContext;

export function updateStyle(id, css) {
  let el = styleSheets.get(id);
  if (!el) {
    el = document.createElement("style");
    el.setAttribute("data-ferrite-id", id);
    document.head.appendChild(el);
    styleSheets.set(id, el);
  }
  el.textContent = css;
}

export function removeStyle(id) {
  styleSheets.get(id)?.remove();
  styleSheets.delete(id);
}

function showError(err) {
  let overlay = document.getElementById("ferrite-error-overlay");
  if (!overlay) {
    overlay = document.createElement("div");
    overlay.id = "ferrite-error-overlay";
    overlay.style.cssText =
      "position:fixed;inset:0;z-index:99999;background:rgba(15,15,20,0.92);color:#f5f5f5;" +
      "font-family:monospace;font-size:13px;padding:32px;overflow:auto;white-space:pre-wrap;";
    document.body.appendChild(overlay);
  }
  const where = err.id ? `\n  at ${err.id}` : "";
  const frame = err.frame ? `\n\n${err.frame}` : "";
  overlay.textContent = `[ferrite] ${err.code ?? "error"}: ${err.message}${where}${frame}`;
  overlay.onclick = () => overlay.remove();
}

function clearError() {
  document.getElementById("ferrite-error-overlay")?.remove();
}

async function fetchUpdate(path, timestamp) {
  const url = `${path}${path.includes("?") ? "&" : "?"}t=${timestamp}`;
  return await import(/* @vite-ignore */ url);
}

let socket = null;
let retries = 0;
let messageQueue = Promise.resolve();

function connect() {
  socket = new WebSocket(socketHost);
  socket.addEventListener("open", () => {
    retries = 0;
    console.log("[ferrite] connected");
  });
  socket.addEventListener("message", (event) => {
    const pending = messageQueue.then(async () => {
      let payload;
      try {
        payload = JSON.parse(event.data);
      } catch {
        return;
      }
      switch (payload.type) {
        case "connected":
          clearError();
          break;
        case "update": {
          clearError();
          // Snapshot owners before any import re-registers self boundaries.
          const pendingCallbacks = new Set();
          for (const update of payload.updates) {
            if (update.type !== "css-update") {
              for (const registration of hotModules.get(update.path)?.callbacks ?? []) {
                if (registration.deps.includes(update.acceptedPath)) pendingCallbacks.add(registration);
              }
            }
          }
          const loaded = new Map();
          try {
            for (const update of payload.updates) {
              if (update.type === "css-update") {
                await fetchUpdate(update.path, update.timestamp);
              } else if (!loaded.has(update.acceptedPath)) {
                console.log(`[ferrite] hmr update ${update.acceptedPath}`);
                const accepted = hotModules.get(update.acceptedPath);
                const disposers = accepted?.disposeCallbacks.slice() ?? [];
                for (const dispose of disposers) await dispose(accepted.data);
                loaded.set(update.acceptedPath, await fetchUpdate(update.acceptedPath, update.timestamp));
              }
            }
            for (const { deps, cb, array } of pendingCallbacks) {
              if (typeof cb === "function") {
                await cb(array ? deps.map(dep => loaded.get(dep)) : loaded.get(deps[0]));
              }
            }
          } catch (err) {
            console.error("[ferrite] update failed, reloading", err);
            location.reload();
          }
          break;
        }
        case "full-reload":
          location.reload();
          break;
        case "custom": {
          const handlers = customHandlers.get(payload.event);
          for (const cb of [...(handlers ?? [])]) await cb(payload.data);
          break;
        }
        case "error":
          showError(payload.err);
          break;
        case "prune": {
          for (const path of payload.paths ?? []) {
            const entry = hotModules.get(path);
            if (!entry) continue;
            try {
              for (const callback of [...entry.disposeCallbacks, ...entry.pruneCallbacks]) {
                try { await callback(entry.data); }
                catch (error) { console.error(`[ferrite] cleanup failed ${path}`, error); }
              }
            } finally {
              clearCustomHandlers(entry);
              hotModules.delete(path);
              removeStyle(path);
            }
          }
          break;
        }
      }
    });
    messageQueue = pending.catch((error) => {
      console.error("[ferrite] message handler failed", error);
    });
    return pending;
  });
  socket.addEventListener("close", () => {
    const delay = Math.min(1000 * 2 ** retries, 30000);
    retries += 1;
    setTimeout(connect, delay);
  });
}

connect();
