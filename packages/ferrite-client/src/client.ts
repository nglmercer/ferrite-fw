// Typed reference of the Ferrite HMR client (see crates/ferrite-hmr/src/client.js).
import type {} from "./hot.d.ts";

type HotCallback = { deps: string[]; cb?: (mod: any) => void };
interface HotEntry {
  callbacks: HotCallback[];
  disposeCallbacks: Array<(data: Record<string, any>) => void>;
  pruneCallbacks: Array<() => void>;
  selfAccept?: boolean;
  data: Record<string, any>;
}

const socketProtocol = location.protocol === "https:" ? "wss:" : "ws:";
const socketHost = `${socketProtocol}//${location.host}/@ferrite/hmr`;

const hotModules = new Map<string, HotEntry>();
const customHandlers = new Map<string, Set<(...args: any[]) => void>>();
const styleSheets = new Map<string, HTMLStyleElement>();

function createHotContext(id: string): HotContext {
  if (!hotModules.has(id)) {
    hotModules.set(id, { callbacks: [], disposeCallbacks: [], pruneCallbacks: [], data: {} });
  }
  const entry = hotModules.get(id)!;
  return {
    data: entry.data,
    accept(dep?: any, cb?: any) {
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
    invalidate(message?: string) {
      console.log(`[ferrite] invalidate ${id}: ${message ?? ""}`);
      location.reload();
    },
    on(event, cb) {
      if (!customHandlers.has(event)) customHandlers.set(event, new Set());
      customHandlers.get(event)!.add(cb);
    },
    off(event, cb) {
      customHandlers.get(event)?.delete(cb);
    },
    send(event, data) {
      socket?.send(JSON.stringify({ type: "custom", event, data }));
    },
  };
}

(globalThis as any).__ferrite_create_hot__ = createHotContext;

export function updateStyle(id: string, css: string): void {
  let el = styleSheets.get(id);
  if (!el) {
    el = document.createElement("style");
    el.setAttribute("data-ferrite-id", id);
    document.head.appendChild(el);
    styleSheets.set(id, el);
  }
  el.textContent = css;
}

export function removeStyle(id: string): void {
  styleSheets.get(id)?.remove();
  styleSheets.delete(id);
}

let socket: WebSocket | null = null;
let retries = 0;

function connect(): void {
  socket = new WebSocket(socketHost);
  socket.addEventListener("open", () => {
    retries = 0;
    console.log("[ferrite] connected");
  });
  socket.addEventListener("message", async (event) => {
    const payload = JSON.parse(event.data);
    if (payload.type === "full-reload") location.reload();
  });
  socket.addEventListener("close", () => {
    const delay = Math.min(1000 * 2 ** retries, 30000);
    retries += 1;
    setTimeout(connect, delay);
  });
}

connect();
