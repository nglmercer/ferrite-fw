import {
  createCompiler,
  type BrowserTarget,
  type CompileStats,
  type Diagnostic
} from "../../../../packages/utilitycss-node/src/index.ts";
import { loadLabConfig } from "./load-config.ts" with { type: "macro" };

/** Lab compilation modes: utility extraction from markup, or `@apply` stylesheets. */
export type LabMode = "markup" | "stylesheet";

/** Validated Compiler Lab request body. */
export interface LabCompileRequest {
  readonly source: string;
  readonly browserTarget: BrowserTarget;
  readonly mode: LabMode;
}

/** One extracted candidate plus the extraction mode that produced it. */
export interface LabCandidate {
  readonly raw: string;
  readonly mode: string;
}

/** Per-candidate validation report with fix suggestions. */
export interface LabCandidateReport {
  readonly candidate: string;
  readonly valid: boolean;
  readonly status: string;
  readonly alternatives: readonly string[];
  readonly diagnostics: readonly Diagnostic[];
}

/** Full Compiler Lab result served to the page and asserted by verify. */
export interface LabCompileResult {
  readonly css: string;
  readonly diagnostics: readonly Diagnostic[];
  readonly stats: CompileStats | null;
  readonly candidates: readonly LabCandidate[];
  readonly reports: readonly LabCandidateReport[];
}

const BROWSER_TARGETS: readonly BrowserTarget[] = ["modern", "evergreen", "safari-15", "legacy"];
const MAX_SOURCE_CHARS = 32_768;
const MAX_REPORTS = 32;

const config = loadLabConfig();

/** Thrown when a Lab request body fails validation; the route maps this to HTTP 400. */
export class LabInputError extends Error {
  public constructor(message: string) {
    super(message);
    this.name = "LabInputError";
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/** Validates an untrusted Lab request body. */
export function parseLabInput(value: unknown): LabCompileRequest {
  if (!isRecord(value)) {
    throw new LabInputError("request body must be a JSON object");
  }
  if (typeof value.source !== "string") {
    throw new LabInputError("field `source` must be a string");
  }
  if (value.source.length === 0 || value.source.length > MAX_SOURCE_CHARS) {
    throw new LabInputError(`field \`source\` must be 1..${MAX_SOURCE_CHARS} characters`);
  }
  const browserTarget = value.browserTarget ?? "modern";
  if (typeof browserTarget !== "string" || !BROWSER_TARGETS.includes(browserTarget as BrowserTarget)) {
    throw new LabInputError(`field \`browserTarget\` must be one of ${BROWSER_TARGETS.join(", ")}`);
  }
  const mode = value.mode ?? "markup";
  if (mode !== "markup" && mode !== "stylesheet") {
    throw new LabInputError("field `mode` must be `markup` or `stylesheet`");
  }
  return { source: value.source, browserTarget: browserTarget as BrowserTarget, mode };
}

function toAlternatives(value: unknown): readonly string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  const names: string[] = [];
  for (const entry of value) {
    if (isRecord(entry) && typeof entry.candidate === "string") {
      names.push(entry.candidate);
    }
  }
  return names;
}

function toReport(candidate: string, result: Readonly<Record<string, unknown>>): LabCandidateReport {
  const diagnostics = Array.isArray(result.diagnostics)
    ? (result.diagnostics.filter(isRecord) as unknown as readonly Diagnostic[])
    : [];
  return {
    candidate,
    valid: result.valid === true,
    status: typeof result.status === "string" ? result.status : "Unknown",
    alternatives: toAlternatives(result.alternatives),
    diagnostics
  };
}

/**
 * Compiles Lab source through a short-lived native compiler using the demo's
 * own CSS-first config. The compiler is always disposed before returning.
 */
export function compileLabSource(input: LabCompileRequest): LabCompileResult {
  const compiler = createCompiler({ config, browserTarget: input.browserTarget, pretty: true });
  try {
    if (input.mode === "stylesheet") {
      const transformed = compiler.transformStylesheet("lab.css", input.source, "lab.css");
      return { css: transformed.css, diagnostics: transformed.diagnostics, stats: null, candidates: [], reports: [] };
    }
    const extracted = compiler.extractCandidates(input.source, "lab.tsx");
    compiler.updateSource("lab", input.source, "lab.tsx");
    const built = compiler.build();
    const seen = new Set<string>();
    const uniques: string[] = [];
    for (const candidate of extracted) {
      if (!seen.has(candidate.raw)) {
        seen.add(candidate.raw);
        uniques.push(candidate.raw);
      }
    }
    const reports = uniques.slice(0, MAX_REPORTS).map(candidate => toReport(candidate, compiler.validate(candidate)));
    return {
      css: built.css,
      diagnostics: built.diagnostics,
      stats: built.stats,
      candidates: extracted.map(candidate => ({ raw: candidate.raw, mode: candidate.extractionMode ?? "text" })),
      reports
    };
  } finally {
    compiler.dispose();
  }
}
