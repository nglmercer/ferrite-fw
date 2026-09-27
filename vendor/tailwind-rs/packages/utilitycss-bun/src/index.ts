import { realpathSync } from "node:fs";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import {
  createCompiler,
  type BuildResult,
  type Compiler,
  type CompilerOptions,
  type Diagnostic,
  type StylesheetResult
} from "@utilitycss/node";
import type { BunPlugin } from "bun";

const DEFAULT_SPECIFIER = "utilitycss";
const VIRTUAL_NAMESPACE = "utilitycss";
const VIRTUAL_PATH = "generated.css";
const SOURCE_FILTER = /\.(?:html?|(?:m|c)?jsx?|(?:m|c)?tsx?|vue|svelte|astro)$/i;
const CSS_FILTER = /\.css$/i;

/** Options for the Bun bundler and fullstack adapter. */
export interface UtilityCssBunOptions extends CompilerOptions {
  /** The module specifier used by `<link rel="stylesheet" href="...">`. */
  readonly specifier?: string;
  /** Print build lifecycle and HMR recovery details to the Bun console. */
  readonly debug?: boolean;
}

/**
 * Creates a Bun plugin that extracts the current build's source graph and
 * exposes the generated stylesheet as a virtual CSS module.
 *
 * The compiler is recreated for each Bun build cycle and rehydrated from the latest source
 * snapshot. Bun may omit unchanged modules during incremental rebuilds, so source contents are
 * retained independently of callback order.
 */
export function utilitycss(options: UtilityCssBunOptions = {}): BunPlugin {
  const specifier = options.specifier ?? DEFAULT_SPECIFIER;
  const specifierFilter = new RegExp(`^${escapeRegExp(specifier)}$`);
  let compiler: Compiler | undefined;
  let buildNumber = 0;
  let lastSuccessfulBuild = 0;
  const moduleSources = new Map<string, string>();
  const moduleDependencies = new Map<string, Set<string>>();
  const moduleDependencyCompleteness = new Map<string, boolean>();

  const createBuildCompiler = (): void => {
    buildNumber += 1;
    compiler?.dispose();
    compiler = createCompiler({
      pretty: options.pretty,
      config: options.config,
      browserTarget: options.browserTarget,
      native: options.native
    });
    for (const [id, source] of moduleSources) {
      requireCompiler().updateSource(id, source, id);
    }
    if (options.debug) {
      const moduleCount = moduleSources.size;
      const moduleLabel = moduleCount === 1 ? "source module" : "source modules";
      console.info(`[utilitycss] build #${buildNumber} started (${moduleCount} retained ${moduleLabel})`);
    }
  };

  const requireCompiler = (): Compiler => {
    if (!compiler) {
      createBuildCompiler();
    }
    const current = compiler;
    if (!current) {
      throw new Error("utilitycss Bun plugin failed to create a compiler");
    }
    return current;
  };

  const pruneSourceGraph = (entrypoints: readonly string[] | undefined): void => {
    // Bun's serve/static lifecycle may omit bundler entrypoints. In that mode the live
    // graph is authoritative and there is no safe root set from which to prune.
    if (!entrypoints) {
      return;
    }
    // A failed resolution means Bun may know about a module that this adapter cannot identify.
    // Retaining all observed sources is safer than pruning a live module and silently dropping
    // its CSS. This also covers aliases and package-style imports handled by Bun's resolver.
    if ([...moduleDependencyCompleteness.values()].some((complete) => !complete)) {
      return;
    }
    const roots = entrypoints
      .map((entrypoint) => {
        try {
          return normalizeModuleId(Bun.resolveSync(entrypoint, process.cwd()));
        } catch {
          return normalizeModuleId(entrypoint);
        }
      })
      .filter((entrypoint) => moduleSources.has(entrypoint));
    const reachable = reachableModules(moduleSources, moduleDependencies, roots);
    for (const id of moduleSources.keys()) {
      if (!reachable.has(id)) {
        moduleSources.delete(id);
        moduleDependencies.delete(id);
        moduleDependencyCompleteness.delete(id);
        compiler?.removeSource(id);
      }
    }
  };

  return {
    name: "utilitycss-bun",
    setup(build): void {
      build.onStart(createBuildCompiler);

      build.onResolve({ filter: specifierFilter }, () => ({
        namespace: VIRTUAL_NAMESPACE,
        path: VIRTUAL_PATH
      }));

      build.onLoad({ filter: SOURCE_FILTER, namespace: "file" }, async ({ path }) => {
        const sourceId = normalizeModuleId(path);
        const source = await Bun.file(path).text();
        moduleSources.set(sourceId, source);
        const resolution = resolveDependencies(sourceId, source, specifier);
        moduleDependencies.set(sourceId, resolution.dependencies);
        moduleDependencyCompleteness.set(sourceId, resolution.complete);
        requireCompiler().updateSource(sourceId, source, sourceId);

        // Returning undefined preserves Bun's native loader and parser for the
        // module. The adapter only observes source content for extraction.
        return undefined;
      });

      build.onLoad({ filter: CSS_FILTER, namespace: "file" }, async ({ path }) => {
        const sourceId = normalizeModuleId(path);
        const source = await Bun.file(path).text();
        const result: StylesheetResult = requireCompiler().transformStylesheet(sourceId, source, path);
        reportDiagnostics(result, (diagnosticSource) => {
          return diagnosticSource && normalizeModuleId(diagnosticSource) === sourceId ? source : undefined;
        });
        return { contents: result.css, loader: "css" };
      });

      build.onEnd((result) => {
        pruneSourceGraph(build.config.entrypoints);
        if (!options.debug) {
          return;
        }
        if (result.success) {
          lastSuccessfulBuild = buildNumber;
          console.info(`[utilitycss] build #${buildNumber} succeeded`);
          return;
        }
        const lastBuild = lastSuccessfulBuild === 0 ? "none" : `#${lastSuccessfulBuild}`;
        console.error(
          `[utilitycss] build #${buildNumber} failed; Bun HMR keeps the last successful bundle (${lastBuild}) active. ` +
          "Fix the diagnostic below and save again."
        );
      });

      build.onLoad({ filter: /^generated\.css$/, namespace: VIRTUAL_NAMESPACE }, async ({ defer }) => {
        await defer();
        pruneSourceGraph(build.config.entrypoints);
        const result = requireCompiler().build();
        reportDiagnostics(result, (diagnosticSource) => {
          return diagnosticSource ? moduleSources.get(normalizeModuleId(diagnosticSource)) : undefined;
        });
        return { contents: result.css, loader: "css" };
      });
    }
  };
}

/** The zero-options plugin used by Bun's `bunfig.toml` loader. */
const defaultPlugin = utilitycss();

export default defaultPlugin;

function reportDiagnostics(
  result: Pick<BuildResult | StylesheetResult, "diagnostics">,
  sourceLookup?: (source: string | undefined) => string | undefined
): void {
  const errors: string[] = [];
  const nonErrors: string[] = [];

  for (const diagnostic of result.diagnostics) {
    const formatted = formatDiagnostic(diagnostic, sourceLookup?.(diagnostic.source));
    if (diagnostic.severity === "error" || diagnostic.severity === undefined) {
      errors.push(formatted);
    } else {
      nonErrors.push(formatted);
    }
  }

  for (const diagnostic of nonErrors) {
    console.warn(diagnostic);
  }

  if (errors.length > 0) {
    const count = errors.length === 1 ? "1 error" : `${errors.length} errors`;
    throw new Error([
      `utilitycss compilation failed (${count})`,
      ...errors.map((error, index) => `${index + 1}. ${error}`),
      "",
      "Bun HMR keeps the last successful bundle active while this rebuild is invalid.",
      "Fix the diagnostic and save again; a successful rebuild will replace it."
    ].join("\n"));
  }
}

function formatDiagnostic(diagnostic: Diagnostic, sourceText?: string): string {
  const source = diagnostic.source ?? "<utilitycss>";
  const span = diagnostic.start === undefined
    ? ""
    : `:${diagnostic.start}-${diagnostic.end ?? diagnostic.start}`;
  const location = sourceText !== undefined && diagnostic.start !== undefined
    ? formatSourceLocation(sourceText, diagnostic.start, diagnostic.end)
    : "";
  const help = diagnostic.help ? `\n  help: ${diagnostic.help}` : "";
  const explanation = diagnostic.explanation ? `\n  explanation: ${diagnostic.explanation}` : "";
  const suggestions = diagnostic.suggestions && diagnostic.suggestions.length > 0
    ? `\n  suggestions: ${diagnostic.suggestions.join(", ")}`
    : "";
  return `${source}${span}${location}: ${diagnostic.message} [${diagnostic.code}]${help}${explanation}${suggestions}`;
}

function formatSourceLocation(source: string, start: number, end?: number): string {
  const startIndex = utf8OffsetToStringIndex(source, start);
  const endIndex = utf8OffsetToStringIndex(source, end ?? start + 1);
  const lineStart = source.lastIndexOf("\n", Math.max(0, startIndex - 1)) + 1;
  const lineEnd = source.indexOf("\n", startIndex) === -1
    ? source.length
    : source.indexOf("\n", startIndex);
  const lineNumber = 1 + countNewlines(source, lineStart);
  const line = source.slice(lineStart, lineEnd);
  const maximumExcerptLength = 140;
  const excerptStart = line.length > maximumExcerptLength
    ? Math.max(0, Math.min(startIndex - lineStart - 60, line.length - maximumExcerptLength))
    : 0;
  const excerptEnd = Math.min(line.length, excerptStart + maximumExcerptLength);
  const prefix = excerptStart > 0 ? "…" : "";
  const suffix = excerptEnd < line.length ? "…" : "";
  const excerpt = `${prefix}${line.slice(excerptStart, excerptEnd)}${suffix}`;
  const caretStart = prefix.length + visibleLength(line.slice(excerptStart, startIndex));
  const highlightEnd = Math.max(startIndex + 1, Math.min(endIndex, lineEnd));
  const caretLength = Math.max(1, visibleLength(line.slice(startIndex, highlightEnd)));
  const gutter = String(lineNumber).length;
  return ` (line ${lineNumber}, column ${visibleLength(line.slice(lineStart, startIndex)) + 1})` +
    `\n${" ".repeat(gutter + 3)}| ${lineNumber} | ${excerpt}` +
    `\n${" ".repeat(gutter + 3)}| ${" ".repeat(caretStart)}${"^".repeat(caretLength)}`;
}

function utf8OffsetToStringIndex(source: string, offset: number): number {
  const target = Math.max(0, Math.trunc(offset));
  let bytes = 0;
  let index = 0;
  while (index < source.length) {
    const codePoint = source.codePointAt(index);
    if (codePoint === undefined) {
      break;
    }
    const character = String.fromCodePoint(codePoint);
    const characterBytes = Buffer.byteLength(character, "utf8");
    if (bytes + characterBytes > target) {
      break;
    }
    bytes += characterBytes;
    index += character.length;
  }
  return index;
}

function countNewlines(source: string, end: number): number {
  let count = 0;
  for (let index = 0; index < end; index += 1) {
    if (source[index] === "\n") {
      count += 1;
    }
  }
  return count;
}

function visibleLength(value: string): number {
  return [...value].length;
}

function normalizeModuleId(id: string): string {
  let normalized = id.replaceAll("\\", "/");
  const queryStart = normalized.search(/[?#]/);
  if (queryStart !== -1) {
    normalized = normalized.slice(0, queryStart);
  }
  if (normalized.startsWith("file://")) {
    try {
      normalized = fileURLToPath(normalized).replaceAll("\\", "/");
    } catch {
      // Preserve a normalized non-file URL when Bun supplies a virtual ID.
    }
  }
  if (normalized.startsWith("/@fs/")) {
    normalized = normalized.slice(5);
  }
  if (!normalized.startsWith("\0") && !normalized.startsWith("bun:")) {
    try {
      normalized = realpathSync(normalized).replaceAll("\\", "/");
    } catch {
      // Bun may expose a virtual or just-created module before realpath works.
    }
  }
  return normalized;
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

interface DependencyResolution {
  readonly dependencies: Set<string>;
  readonly complete: boolean;
}

function resolveDependencies(
  id: string,
  source: string,
  ignoredSpecifier: string
): DependencyResolution {
  const dependencies = new Set<string>();
  let complete = true;
  const resolveModuleSpecifier = (specifier: string): void => {
    if (specifier === ignoredSpecifier || /^(?:node|bun):/i.test(specifier)) {
      return;
    }
    // URLs are resources, not Bun modules. Hash-prefixed specifiers remain eligible because they
    // are a common alias form in Bun/TypeScript projects.
    if (/^(?:[a-z][a-z\d+.-]*:)?\/\//i.test(specifier)) {
      return;
    }
    try {
      dependencies.add(normalizeModuleId(Bun.resolveSync(specifier, dirname(id))));
    } catch {
      complete = false;
      // Bun reports unresolved imports through its normal build diagnostics. Until then, retain
      // every observed source so a failed custom resolution cannot remove live CSS.
    }
  };
  const resolveAttributeReference = (specifier: string): void => {
    if (specifier === ignoredSpecifier) {
      return;
    }
    // HTML href/src values are usually fragments, external URLs, or static assets rather than
    // Bun modules. Only plausible module paths participate in the graph, so one unresolvable
    // resource reference cannot disable pruning for every observed source.
    if (isSkippableResourceReference(specifier)) {
      return;
    }
    try {
      dependencies.add(normalizeModuleId(Bun.resolveSync(specifier, dirname(id))));
    } catch {
      complete = false;
    }
  };
  for (
    const match of source.matchAll(
      /(?:import\s+(?:[^"'`]*?\s+from\s+|)|export\s+[^"'`]*?\s+from\s+|require\s*\(|import\s*\()\s*["']([^"']+)["']/g
    )
  ) {
    resolveModuleSpecifier(match[1]);
  }
  for (const match of source.matchAll(/(?:src|href)\s*=\s*["']([^"']+)["']/gi)) {
    resolveAttributeReference(match[1]);
  }
  return { dependencies, complete };
}

const STATIC_RESOURCE_PATTERN =
  /\.(?:avif|bmp|css|eot|flac|gif|ico|jpe?g|map|mp3|mp4|ogg|ogv|otf|pdf|png|svg|ttf|txt|wav|webm|woff2?|xml)$/i;

/**
 * Returns whether an HTML href/src value can never be a Bun module: fragments,
 * external or data URLs, and ordinary static-resource references.
 */
function isSkippableResourceReference(specifier: string): boolean {
  const reference = specifier.trim();
  if (reference === "" || reference.startsWith("#")) {
    return true;
  }
  const isWindowsPath = /^[a-zA-Z]:[\\/]/.test(reference);
  if (
    !isWindowsPath &&
    (/^[a-z][a-z\d+.-]*:/i.test(reference) || reference.startsWith("//"))
  ) {
    return true;
  }
  const path = reference.split(/[?#]/, 1)[0] ?? "";
  return STATIC_RESOURCE_PATTERN.test(path);
}

function reachableModules(
  sources: ReadonlyMap<string, string>,
  dependencies: ReadonlyMap<string, ReadonlySet<string>>,
  configuredRoots: readonly string[]
): Set<string> {
  const imported = new Set<string>();
  for (const dependencySet of dependencies.values()) {
    for (const dependency of dependencySet) {
      if (sources.has(dependency)) {
        imported.add(dependency);
      }
    }
  }
  const roots = configuredRoots.length > 0
    ? [...configuredRoots]
    : [...sources.keys()].filter((id) => !imported.has(id));
  if (roots.length === 0) {
    return new Set(sources.keys());
  }
  const reachable = new Set<string>();
  const pending = [...roots];
  while (pending.length > 0) {
    const current = pending.pop();
    if (!current || reachable.has(current)) {
      continue;
    }
    reachable.add(current);
    for (const dependency of dependencies.get(current) ?? []) {
      if (sources.has(dependency)) {
        pending.push(dependency);
      }
    }
  }
  return reachable;
}
