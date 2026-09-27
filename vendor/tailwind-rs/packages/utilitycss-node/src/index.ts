import { createRequire } from "node:module";

/** A diagnostic returned by the Rust compiler. */
export interface Diagnostic {
  readonly severity?: "error" | "warning" | "note" | "help";
  readonly code: string;
  readonly message: string;
  readonly source?: string;
  readonly start?: number;
  readonly end?: number;
  readonly help?: string;
  readonly explanation?: string;
  readonly suggestions?: readonly string[];
}

/** Work counters returned by a compiler build. */
export interface CompileStats {
  readonly sourcesScanned: number;
  readonly bytesScanned: number;
  readonly candidatesFound: number;
  readonly uniqueCandidates: number;
  readonly candidatesParsed: number;
  readonly cacheHits: number;
  readonly rulesGenerated: number;
  readonly rulesRemoved: number;
}

/** A normalized result from one compiler build. */
export interface BuildResult {
  readonly css: string;
  readonly diagnostics: readonly Diagnostic[];
  readonly stats: CompileStats;
}

/** Result of transforming authored CSS through the native stylesheet layer. */
export interface StylesheetResult {
  readonly css: string;
  readonly diagnostics: readonly Diagnostic[];
}

/** Machine-readable result returned by candidate introspection methods. */
export type IntrospectionResult = Readonly<Record<string, unknown>>;

/** The native compiler surface consumed by this adapter. */
export interface NativeCompiler {
  updateSource(id: string, content: string, path?: string, candidates?: readonly CandidateInput[]): void;
  extractCandidates?(content: string, path?: string): readonly CandidateInput[];
  removeSource(id: string): boolean;
  build(): unknown;
  explain?(candidate: string): unknown;
  validate?(candidate: string): unknown;
  capabilities?(): unknown;
  transformStylesheet?(id: string, content: string, path?: string): unknown;
}

/** A statically extracted candidate accepted by the native update API. */
export interface CandidateInput {
  readonly raw: string;
  readonly start: number;
  readonly end: number;
  /** Extraction mode that produced this candidate. */
  readonly extractionMode?: "text" | "static" | "ast" | "hybrid";
}

/** A native compiler constructor, injectable for tests and alternate loaders. */
export type BrowserTarget = "modern" | "evergreen" | "safari-15" | "legacy";

/** A native compiler constructor, injectable for tests and alternate loaders. */
export type NativeCompilerFactory = new (
  pretty?: boolean,
  configSource?: string,
  browserTarget?: BrowserTarget
) => NativeCompiler;

/** Options for creating a Node adapter. */
export interface CompilerOptions {
  readonly pretty?: boolean;
  /** Declarative JSON or CSS-first configuration source. */
  readonly config?: string;
  /** Browser support target used for structured compatibility diagnostics. */
  readonly browserTarget?: BrowserTarget;
  readonly native?: NativeCompilerFactory;
}

/** A thin lifecycle adapter over the native Rust compiler. */
export class Compiler {
  private native: NativeCompiler | undefined;
  private disposed = false;

  /** Creates an adapter around one native compiler instance. */
  public constructor(native: NativeCompiler) {
    this.native = native;
  }

  /** Inserts or replaces source content. */
  public updateSource(
    id: string,
    content: string,
    path?: string,
    candidates?: readonly CandidateInput[]
  ): void {
    const native = this.activeNative();
    const extracted = candidates ?? native.extractCandidates?.(content, path);
    native.updateSource(id, content, path, extracted);
  }

  /** Removes source content and returns whether the source existed. */
  public removeSource(id: string): boolean {
    return this.activeNative().removeSource(id);
  }

  /** Extracts candidates through the native host-language extractor. */
  public extractCandidates(content: string, path?: string): readonly CandidateInput[] {
    const native = this.activeNative();
    if (!native.extractCandidates) {
      throw new Error("native compiler does not expose candidate extraction");
    }
    return native.extractCandidates(content, path).map(candidate => ({
      raw: candidate.raw,
      start: candidate.start,
      end: candidate.end,
      extractionMode: candidate.extractionMode
    }));
  }

  /** Builds CSS and normalizes the native result shape. */
  public build(): BuildResult {
    return normalizeBuildResult(this.activeNative().build());
  }

  /** Explains one candidate through the native registry-backed introspection API. */
  public explain(candidate: string): IntrospectionResult {
    const native = this.activeNative();
    if (!native.explain) {
      throw new Error("native compiler does not expose candidate explanation");
    }
    return normalizeIntrospectionResult(native.explain(candidate));
  }

  /** Validates one candidate through the native registry-backed introspection API. */
  public validate(candidate: string): IntrospectionResult {
    const native = this.activeNative();
    if (!native.validate) {
      throw new Error("native compiler does not expose candidate validation");
    }
    return normalizeIntrospectionResult(native.validate(candidate));
  }

  /** Returns the active machine-readable capability manifest. */
  public capabilities(): IntrospectionResult {
    const native = this.activeNative();
    if (!native.capabilities) {
      throw new Error("native compiler does not expose capability metadata");
    }
    return normalizeIntrospectionResult(native.capabilities());
  }

  /** Transforms authored CSS and normalizes native stylesheet diagnostics. */
  public transformStylesheet(id: string, content: string, path?: string): StylesheetResult {
    const native = this.activeNative();
    if (!native.transformStylesheet) {
      throw new Error("native compiler does not expose stylesheet transformation");
    }
    return normalizeStylesheetResult(native.transformStylesheet(id, content, path));
  }

  /**
   * Releases this adapter's native compiler handle. The native reference is
   * dropped so the runtime can reclaim it; every later call throws.
   */
  public dispose(): void {
    this.native = undefined;
    this.disposed = true;
  }

  private activeNative(): NativeCompiler {
    const native = this.disposed ? undefined : this.native;
    if (!native) {
      throw new Error("utilitycss compiler has been disposed");
    }
    return native;
  }
}

/** Creates a Node adapter using the installed native binding or an injected factory. */
export function createCompiler(options: CompilerOptions = {}): Compiler {
  const factory = options.native ?? loadNativeFactory();
  return new Compiler(new factory(options.pretty, options.config, options.browserTarget));
}

function loadNativeFactory(): NativeCompilerFactory {
  const require = createRequire(import.meta.url);
  const moduleValue: unknown = require("@utilitycss/napi");
  if (!isRecord(moduleValue) || typeof moduleValue.Compiler !== "function") {
    throw new Error("@utilitycss/napi does not export a Compiler constructor");
  }
  return moduleValue.Compiler as NativeCompilerFactory;
}

function normalizeBuildResult(value: unknown): BuildResult {
  if (!isRecord(value) || typeof value.css !== "string" || !Array.isArray(value.diagnostics)) {
    throw new Error("native compiler returned an invalid build result");
  }
  const stats = normalizeStats(value.stats);
  const diagnostics = value.diagnostics.map(normalizeDiagnostic);
  return { css: value.css, diagnostics, stats };
}

function normalizeStylesheetResult(value: unknown): StylesheetResult {
  if (!isRecord(value) || typeof value.css !== "string" || !Array.isArray(value.diagnostics)) {
    throw new Error("native compiler returned an invalid stylesheet result");
  }
  return {
    css: value.css,
    diagnostics: value.diagnostics.map(normalizeDiagnostic)
  };
}

function normalizeIntrospectionResult(value: unknown): IntrospectionResult {
  const parsed = typeof value === "string" ? parseJson(value) : value;
  if (!isRecord(parsed)) {
    throw new Error("native compiler returned an invalid introspection result");
  }
  return parsed;
}

function parseJson(value: string): unknown {
  try {
    return JSON.parse(value) as unknown;
  } catch {
    throw new Error("native compiler returned invalid introspection JSON");
  }
}

function normalizeDiagnostic(value: unknown): Diagnostic {
  if (!isRecord(value) || typeof value.code !== "string" || typeof value.message !== "string") {
    throw new Error("native compiler returned an invalid diagnostic");
  }
  const diagnostic: {
    severity?: Diagnostic["severity"];
    code: string;
    message: string;
    source?: string;
    start?: number;
    end?: number;
    help?: string;
    explanation?: string;
    suggestions?: string[];
  } = {
    severity: optionalSeverity(value.severity),
    code: value.code,
    message: value.message,
    source: optionalString(value.source),
    start: optionalNumber(value.start),
    end: optionalNumber(value.end),
    help: optionalString(value.help)
  };
  const explanation = optionalString(value.explanation);
  if (explanation !== undefined) {
    diagnostic.explanation = explanation;
  }
  if (value.suggestions !== undefined && value.suggestions !== null) {
    if (!Array.isArray(value.suggestions) || !value.suggestions.every(item => typeof item === "string")) {
      throw new Error("native compiler returned invalid diagnostic suggestions");
    }
    diagnostic.suggestions = value.suggestions as string[];
  }
  return diagnostic;
}

function normalizeStats(value: unknown): CompileStats {
  if (!isRecord(value)) {
    throw new Error("native compiler returned invalid build stats");
  }
  return {
    sourcesScanned: requiredNumber(value.sourcesScanned),
    bytesScanned: requiredNumber(value.bytesScanned),
    candidatesFound: requiredNumber(value.candidatesFound),
    uniqueCandidates: requiredNumber(value.uniqueCandidates),
    candidatesParsed: requiredNumber(value.candidatesParsed),
    cacheHits: requiredNumber(value.cacheHits),
    rulesGenerated: requiredNumber(value.rulesGenerated),
    rulesRemoved: requiredNumber(value.rulesRemoved)
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function requiredNumber(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error("native compiler returned a non-numeric stat");
  }
  return value;
}

function optionalNumber(value: unknown): number | undefined {
  return value === undefined || value === null ? undefined : requiredNumber(value);
}

function optionalString(value: unknown): string | undefined {
  if (value === undefined || value === null) {
    return undefined;
  }
  if (typeof value !== "string") {
    throw new Error("native compiler returned a non-string diagnostic field");
  }
  return value;
}

function optionalSeverity(value: unknown): Diagnostic["severity"] {
  if (value === undefined || value === null) {
    return undefined;
  }
  if (value === "error" || value === "warning" || value === "note" || value === "help") {
    return value;
  }
  throw new Error("native compiler returned an invalid diagnostic severity");
}
