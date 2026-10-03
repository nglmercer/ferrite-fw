// Ferrite HMR client (`/@ferrite/client`).
// Implements `import.meta.hot`, style updates, and the error overlay hook.
const socketProtocol = location.protocol === "https:" ? "wss:" : "ws:";
const socketHost = `${socketProtocol}//${location.host}/@ferrite/hmr`;

const hotModules = new Map(); // id -> { callbacks, disposeCallbacks, data }
const customHandlers = new Map(); // event -> Set<cb>
const styleSheets = new Map(); // id -> HTMLStyleElement

function createHotContext(id) {
  if (!hotModules.has(id)) {
    hotModules.set(id, { callbacks: [], disposeCallbacks: [], pruneCallbacks: [], data: {} });
  }
  const entry = hotModules.get(id);
  entry.callbacks = [];
  entry.disposeCallbacks = [];
  entry.pruneCallbacks = [];
  entry.selfAccept = false;
  return {
    data: entry.data,
    accept(dep, cb) {
      if (typeof dep === "undefined") entry.selfAccept = true;
      else if (typeof dep === "function") entry.callbacks.push({ deps: [id], cb: dep });
      else {
        const deps = Array.isArray(dep) ? dep : [dep];
        entry.callbacks.push({ deps, cb });
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
      if (!customHandlers.has(event)) customHandlers.set(event, new Set());
      customHandlers.get(event).add(cb);
    },
    off(event, cb) {
      customHandlers.get(event)?.delete(cb);
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

function connect() {
  socket = new WebSocket(socketHost);
  socket.addEventListener("open", () => {
    retries = 0;
    console.log("[ferrite] connected");
  });
  socket.addEventListener("message", async (event) => {
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
        for (const update of payload.updates) {
          if (update.type === "css-update") {
            try {
              await fetchUpdate(update.path, update.timestamp);
            } catch (err) {
              console.error("[ferrite] css update failed, reloading", err);
              location.reload();
            }
          } else {
            console.log(`[ferrite] hmr update ${update.path}`);
            try {
              const boundary = hotModules.get(update.acceptedPath);
              const callbacks = boundary?.callbacks.slice() ?? [];
              const disposers = boundary?.disposeCallbacks.slice() ?? [];
              for (const dispose of disposers) await dispose(boundary.data);
              const next = await fetchUpdate(update.acceptedPath, update.timestamp);
              for (const { deps, cb } of callbacks) {
                if (typeof cb === "function" && deps.includes(update.acceptedPath)) await cb(next);
              }
            } catch (err) {
              console.error("[ferrite] js update failed, reloading", err);
              location.reload();
            }
          }
        }
        break;
      }
      case "full-reload":
        location.reload();
        break;
      case "custom": {
        const handlers = customHandlers.get(payload.event);
        handlers?.forEach((cb) => cb(payload.data));
        break;
      }
      case "error":
        showError(payload.err);
        break;
      case "prune": {
        for (const path of payload.paths ?? []) {
          hotModules.delete(path);
        }
        break;
      }
    }
  });
  socket.addEventListener("close", () => {
    const delay = Math.min(1000 * 2 ** retries, 30000);
    retries += 1;
    setTimeout(connect, delay);
  });
}

connect();
