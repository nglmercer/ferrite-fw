interface HotContext {
  data: Record<string, any>;
  accept(): void;
  accept(cb: (mod: any) => void): void;
  accept(dep: string, cb?: (mod: any) => void): void;
  accept(deps: string[], cb?: (mods: any[]) => void): void;
  dispose(cb: (data: Record<string, any>) => void): void;
  prune(cb: () => void): void;
  invalidate(message?: string): void;
  on(event: string, cb: (...args: any[]) => void): void;
  off(event: string, cb: (...args: any[]) => void): void;
  send(event: string, data?: unknown): void;
}

interface ImportMeta {
  hot?: HotContext;
  env: Record<string, string | boolean> & {
    MODE: string;
    BASE_URL: string;
    PROD: boolean;
    DEV: boolean;
    SSR: boolean;
  };
}

declare global {
  interface Window {
    __ferrite_create_hot__: (id: string) => HotContext;
  }
  // eslint-disable-next-line no-var
  var __ferrite_create_hot__: (id: string) => HotContext;
}

export {};
