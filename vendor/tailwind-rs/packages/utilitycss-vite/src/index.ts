import {
  createCompiler,
  type Compiler,
  type CompilerOptions,
  type Diagnostic,
  type StylesheetResult
} from "@utilitycss/node";

/** Minimal Vite module shape needed for virtual CSS invalidation. */
export interface ViteModule {
  readonly id: string;
}

/** Minimal Vite module graph shape consumed by this adapter. */
export interface ViteModuleGraph {
  getModuleById(id: string): Promise<ViteModule | undefined>;
  invalidateModule(module: ViteModule): void;
}

/** Minimal HMR context shape used by the adapter. */
export interface ViteHotUpdateContext {
  readonly file: string;
  readonly modules: readonly ViteModule[];
  readonly read: () => Promise<string>;
  readonly event?: { readonly type: "create" | "update" | "delete" };
  readonly server: {
    readonly moduleGraph: ViteModuleGraph;
    readonly config?: {
      readonly logger?: {
        warn(message: string): void;
        error(message: string): void;
      };
    };
  };
}

/** The Rollup watcher change notification used for source deletion. */
export interface ViteWatchChange {
  readonly event: "create" | "update" | "delete";
}

/** The diagnostic methods supplied as `this` by Vite plugin hooks. */
export interface VitePluginContext {
  warn(message: string): void;
  error(message: string): never;
}

/** Result returned by Vite's CSS transform hook. */
export interface ViteTransformResult {
  readonly code: string;
  readonly map: null;
}

/** The subset of the Vite plugin contract implemented here. */
export interface UtilityCssVitePlugin {
  readonly name: "utilitycss";
  resolveId(id: string): string | undefined;
  load(id: string): string | undefined;
  transform(this: VitePluginContext, code: string, id: string): Promise<ViteTransformResult | null>;
  handleHotUpdate(context: ViteHotUpdateContext): Promise<readonly ViteModule[]>;
  watchChange(id: string, change: ViteWatchChange): void;
}

/** Options for the Vite adapter. */
export interface ViteOptions extends CompilerOptions {
  readonly virtualModuleId?: string;
  readonly include?: readonly RegExp[];
}

/** Creates a Vite lifecycle adapter around one persistent compiler instance. */
export function utilitycss(options: ViteOptions = {}): UtilityCssVitePlugin {
  const compiler: Compiler = createCompiler(options);
  const virtualModuleId = options.virtualModuleId ?? "virtual:utilitycss.css";
  const resolvedVirtualId = `\0${virtualModuleId}`;
  const include = options.include ?? [/\.(?:html?|astro|(?:m|c)?jsx?|(?:m|c)?tsx?|vue|svelte)$/i];

  const isCss = (id: string): boolean => /\.css(?:$|[?#])/i.test(id);

  const shouldInclude = (id: string): boolean => {
    const normalized = normalizeModuleId(id);
    return !normalized.startsWith("\0") && include.some((pattern) => {
      pattern.lastIndex = 0;
      return pattern.test(normalized);
    });
  };

  const reportDiagnostics = (
    diagnostics: readonly Diagnostic[],
    context: VitePluginContext | undefined
  ): void => {
    if (!context) {
      return;
    }
    for (const diagnostic of diagnostics) {
      const location = diagnostic.source ?? "";
      const span = diagnostic.start === undefined ? "" : `${diagnostic.start}-${diagnostic.end ?? diagnostic.start}`;
      const suffix = span ? `${location ? ":" : ""}${span}` : "";
      const message = `${location}${suffix}: ${diagnostic.message} [${diagnostic.code}]${diagnostic.help ? `; ${diagnostic.help}` : ""}`;
      if (diagnostic.severity === "error" || diagnostic.severity === undefined) {
        context.error(message);
      } else {
        context.warn(message);
      }
    }
  };

  return {
    name: "utilitycss",
    resolveId(id: string): string | undefined {
      return id === virtualModuleId ? resolvedVirtualId : undefined;
    },
    load(id: string): string | undefined {
      if (id !== resolvedVirtualId) {
        return undefined;
      }
      const result = compiler.build();
      const errors = result.diagnostics.filter(
        (diagnostic) => diagnostic.severity === "error" || diagnostic.severity === undefined
      );
      for (const diagnostic of result.diagnostics) {
        if (diagnostic.severity !== "error" && diagnostic.severity !== undefined) {
          console.warn(formatDiagnostic(diagnostic));
        }
      }
      if (errors.length > 0) {
        throw new Error(errors.map((diagnostic) => formatDiagnostic(diagnostic)).join("\n"));
      }
      return result.css;
    },
    async transform(this: VitePluginContext, code: string, id: string): Promise<ViteTransformResult | null> {
      const normalizedId = normalizeModuleId(id);
      if (isCss(normalizedId)) {
        const result: StylesheetResult = compiler.transformStylesheet(normalizedId, code, normalizedId);
        reportDiagnostics(result.diagnostics, this);
        return { code: result.css, map: null };
      }
      if (shouldInclude(normalizedId)) {
        compiler.updateSource(normalizedId, code, normalizedId);
        reportDiagnostics(compiler.build().diagnostics, this);
      }
      return null;
    },
    async handleHotUpdate(context: ViteHotUpdateContext): Promise<readonly ViteModule[]> {
      const normalizedId = normalizeModuleId(context.file);
      if (isCss(normalizedId)) {
        if (context.event?.type !== "delete") {
          const result = compiler.transformStylesheet(normalizedId, await context.read(), normalizedId);
          reportHotUpdateDiagnostics(result.diagnostics, context);
        }
        return context.modules;
      }
      if (shouldInclude(normalizedId)) {
        if (context.event?.type === "delete") {
          compiler.removeSource(normalizedId);
        } else {
          compiler.updateSource(normalizedId, await context.read(), normalizedId);
        }
        reportHotUpdateDiagnostics(compiler.build().diagnostics, context);
        const virtualModule = await context.server.moduleGraph.getModuleById(resolvedVirtualId);
        if (virtualModule) {
          context.server.moduleGraph.invalidateModule(virtualModule);
        }
      }
      return context.modules;
    },
    watchChange(id: string, change: ViteWatchChange): void {
      const normalizedId = normalizeModuleId(id);
      if (change.event === "delete" && shouldInclude(normalizedId)) {
        compiler.removeSource(normalizedId);
      }
    }
  };
}

function normalizeModuleId(id: string): string {
  let normalized = id.split("\\").join("/");
  if (normalized.startsWith("file://")) {
    try {
      const url = new URL(normalized);
      normalized = decodeURIComponent(url.pathname);
      if (/^\/[A-Za-z]:\//.test(normalized)) {
        normalized = normalized.slice(1);
      }
    } catch {
      // Keep the original normalized spelling when an adapter supplies a non-URL file ID.
    }
  }
  if (normalized.startsWith("/@fs/")) {
    normalized = normalized.slice(5);
  }
  const query = normalized.search(/[?#]/);
  return query === -1 ? normalized : normalized.slice(0, query);
}

function reportHotUpdateDiagnostics(
  diagnostics: readonly Diagnostic[],
  context: ViteHotUpdateContext
): void {
  for (const diagnostic of diagnostics) {
    const message = formatDiagnostic(diagnostic);
    if (diagnostic.severity === "error" || diagnostic.severity === undefined) {
      context.server.config?.logger?.error(message);
      throw new Error(message);
    }
    if (context.server.config?.logger) {
      context.server.config.logger.warn(message);
    } else {
      console.warn(message);
    }
  }
}

function formatDiagnostic(diagnostic: Diagnostic): string {
  const location = diagnostic.source ?? "";
  const span = diagnostic.start === undefined
    ? ""
    : `${location ? ":" : ""}${diagnostic.start}-${diagnostic.end ?? diagnostic.start}`;
  return `${location}${span}: ${diagnostic.message} [${diagnostic.code}]${diagnostic.help ? `; ${diagnostic.help}` : ""}`;
}
