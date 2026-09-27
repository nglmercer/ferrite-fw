import { useEffect, useRef, useState } from "preact/hooks";

import type { BrowserTarget } from "../../../../packages/utilitycss-node/src/index.ts";
import { Button } from "../components/actions/Button.tsx";
import { Badge } from "../components/data-display/Badge.tsx";
import { Tabs } from "../components/ui/Tabs.tsx";
import { cn } from "../lib/cn.ts";
import type { LabCompileResult, LabMode } from "./compile.ts";

interface LabPreset {
  readonly name: string;
  readonly mode: LabMode;
  readonly browserTarget: BrowserTarget;
  readonly source: string;
  readonly note: string;
}

const PRESETS: readonly LabPreset[] = [
  {
    name: "Responsive",
    mode: "markup",
    browserTarget: "modern",
    source: `<div class="mx-auto grid max-w-[24rem] gap-6 md:grid-cols-2 2xl:grid-cols-4">\n  <button class="bg-brand-600 px-4 py-2 text-white hover:bg-brand-700">Continue</button>\n</div>`,
    note: "Custom xs/2xl breakpoints from utilitycss.config.css sort by min-width value."
  },
  {
    name: "Arbitrary values",
    mode: "markup",
    browserTarget: "modern",
    source: `<div class="w-[70%] max-w-[24rem]">\n  <p class="text-sm text-gray-600">Bracket values compile to exact declarations.</p>\n</div>`,
    note: "Arbitrary values bypass the scale and emit the bracketed declaration."
  },
  {
    name: "Variants",
    mode: "markup",
    browserTarget: "modern",
    source: `<button class="bg-gray-100 px-4 py-2 hover:bg-gray-200 focus:outline-none xs:tracking-wide">Filter</button>`,
    note: "State and responsive variants stack on one candidate."
  },
  {
    name: "Transitions",
    mode: "markup",
    browserTarget: "modern",
    source: `<button class="bg-brand-600 px-4 py-2 text-white transition-all duration-300 ease-out hover:bg-brand-700 hover:shadow-lg">Continue</button>`,
    note: "Timing utilities and hover variants compose on one candidate."
  },
  {
    name: "@apply recipe",
    mode: "stylesheet",
    browserTarget: "modern",
    source: `.lab-card {\n  @apply rounded-lg border border-gray-200 bg-white p-6 shadow-sm;\n}`,
    note: "Stylesheet mode lowers @apply recipes; unknown utilities error here."
  },
  {
    name: "Compat warning",
    mode: "markup",
    browserTarget: "safari-15",
    source: `<div class="content-auto flex">\n  <p>Capped by the active browser target.</p>\n</div>`,
    note: "safari-15 warns on content-visibility while still emitting the rule."
  },
  {
    name: "Typo recovery",
    mode: "markup",
    browserTarget: "modern",
    source: `<div class="flx items-center">Typo intentional.</div>`,
    note: "Validation reports list the closest known utilities."
  }
];

const BROWSER_TARGETS: readonly BrowserTarget[] = ["modern", "evergreen", "safari-15", "legacy"];

async function postCompile(source: string, browserTarget: BrowserTarget, mode: LabMode): Promise<LabCompileResult> {
  const response = await fetch("/api/compile", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ source, browserTarget, mode })
  });
  const body: unknown = await response.json();
  if (!response.ok) {
    const message = typeof body === "object" && body !== null && "error" in body ? String((body as Record<string, unknown>).error) : `compile failed with status ${response.status}`;
    throw new Error(message);
  }
  return body as LabCompileResult;
}

type PlaygroundTab = "preview" | "css" | "diagnostics" | "candidates" | "api";

/** Static reference for the compile endpoint that powers this page. */
function ApiDoc() {
  return (
    <div className="space-y-4 text-sm leading-relaxed text-gray-700">
      <p>Every tab on this page is served by one endpoint, <code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs">POST /api/compile</code>, running the demo config through the native compiler. It accepts:</p>
      <pre className="demo-source"><code>{`POST /api/compile
Content-Type: application/json

{
  "source": "<div class=\\"flex\\">hi</div>",
  "browserTarget": "modern",
  "mode": "markup"
}`}</code></pre>
      <ul className="m-0 list-none space-y-2 p-0">
        <li><code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs">source</code> — markup or stylesheet text, 1–32768 characters.</li>
        <li><code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs">browserTarget</code> — one of <span className="font-mono text-xs">modern · evergreen · safari-15 · legacy</span>.</li>
        <li><code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs">mode</code> — <span className="font-mono text-xs">markup</span> extracts utilities, <span className="font-mono text-xs">stylesheet</span> lowers <span className="font-mono text-xs">@apply</span>.</li>
      </ul>
      <p>Responses are <code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs">200</code> with <code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs">css</code>, <code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs">diagnostics</code>, <code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs">stats</code>, <code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs">candidates</code>, and <code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs">reports</code>; invalid input returns <code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs">400</code> and other methods return <code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs">405</code>. The contract is exercised end to end by <code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs">bun run verify</code>.</p>
    </div>
  );
}

/** Interactive playground: live preview, generated CSS, diagnostics, candidates, and API docs. */
export function CompilerLabPage() {
  const [presetName, setPresetName] = useState(PRESETS[0].name);
  const [source, setSource] = useState(PRESETS[0].source);
  const [mode, setMode] = useState<LabMode>(PRESETS[0].mode);
  const [browserTarget, setBrowserTarget] = useState<BrowserTarget>(PRESETS[0].browserTarget);
  const [result, setResult] = useState<LabCompileResult | null>(null);
  const [compiledSource, setCompiledSource] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [compiling, setCompiling] = useState(false);
  const [auto, setAuto] = useState(true);
  const [tab, setTab] = useState<PlaygroundTab>("preview");
  const mounted = useRef(false);

  const compile = async (next?: Partial<Pick<LabPreset, "source" | "mode" | "browserTarget">>): Promise<void> => {
    const request = { source, mode, browserTarget, ...next };
    setCompiling(true);
    setError(null);
    try {
      setResult(await postCompile(request.source, request.browserTarget, request.mode));
      setCompiledSource(request.source);
    } catch (failure) {
      setResult(null);
      setCompiledSource(null);
      setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setCompiling(false);
    }
  };

  useEffect(() => {
    void compile();
    // Compile once on mount; later edits compile on demand.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!mounted.current) {
      mounted.current = true;
      return;
    }
    if (!auto) return;
    const timer = window.setTimeout(() => void compile(), 600);
    return () => window.clearTimeout(timer);
    // Debounced recompile follows the editor; compile identity is intentionally skipped.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [source, mode, browserTarget, auto]);

  const applyPreset = (preset: LabPreset): void => {
    setPresetName(preset.name);
    setSource(preset.source);
    setMode(preset.mode);
    setBrowserTarget(preset.browserTarget);
    void compile(preset);
  };

  const activePreset = PRESETS.find(preset => preset.name === presetName);
  const diagnostics = result?.diagnostics ?? [];
  const reports = result?.reports ?? [];
  const stale = compiledSource === null || compiledSource !== source;
  const previewDoc = result && mode === "markup" && compiledSource !== null
    ? `<!doctype html><html><head><meta charset="utf-8"><style>${result.css}</style></head><body>${compiledSource}</body></html>`
    : null;

  return (
    <div className="lab-page">
      <div className="playground-toolbar" role="group" aria-label="Playground presets">
        {PRESETS.map(preset => (
          <Button
            key={preset.name}
            type="button"
            variant={preset.name === presetName ? "primary" : "secondary"}
            size="sm"
            onClick={() => applyPreset(preset)}
          >
            {preset.name}
          </Button>
        ))}
      </div>
      {activePreset ? <p className="form-help">{activePreset.note}</p> : null}

      <div className="playground-toolbar mt-4">
        <Tabs
          idPrefix="playground-mode"
          value={mode}
          onChange={value => setMode(value as LabMode)}
          items={[{ value: "markup", label: "Markup" }, { value: "stylesheet", label: "Stylesheet" }]}
          variant="box"
          size="sm"
          ariaLabel="Compile mode"
        />
        <label className="label-inline" htmlFor="lab-target">
          <span>Target</span>
          <select
            id="lab-target"
            className="form-input form-select form-input-sm"
            value={browserTarget}
            onChange={event => setBrowserTarget(event.currentTarget.value as BrowserTarget)}
          >
            {BROWSER_TARGETS.map(target => (
              <option key={target} value={target}>{target}</option>
            ))}
          </select>
        </label>
        <span className="playground-auto">
          <button
            type="button"
            role="switch"
            aria-checked={auto}
            aria-label="Auto-compile on edit"
            onClick={() => setAuto(value => !value)}
            className={cn("toggle", auto && "toggle-checked")}
          >
            <span className="toggle-knob" />
          </button>
          Auto-compile
        </span>
        <Button type="button" variant="primary" size="sm" disabled={compiling} onClick={() => void compile()}>
          {compiling ? "Compiling…" : "Compile"}
        </Button>
        {stale && !compiling ? <Badge variant="warning">Stale</Badge> : null}
      </div>

      <div className="mt-4 grid gap-4 lg:grid-cols-2">
        <div>
          <label className="form-label" htmlFor="lab-source">Source</label>
          <textarea
            id="lab-source"
            className="form-input font-mono text-xs"
            rows={16}
            spellcheck={false}
            value={source}
            onInput={event => setSource(event.currentTarget.value)}
          />
        </div>

        <div>
          <Tabs
            idPrefix="playground-result"
            value={tab}
            onChange={value => setTab(value as PlaygroundTab)}
            items={[
              { value: "preview", label: "Preview" },
              { value: "css", label: "CSS" },
              { value: "diagnostics", label: `Diagnostics (${diagnostics.length})` },
              { value: "candidates", label: `Candidates (${reports.length})` },
              { value: "api", label: "API" }
            ]}
            variant="line"
            size="sm"
            ariaLabel="Compile results"
          />
          <div className="mt-3">
            {tab === "preview" ? (
              previewDoc
                ? <><iframe title="Compiled preview" sandbox="" srcDoc={previewDoc} className="playground-preview" />{stale ? <p className="form-help">Preview shows the last compiled source — press Compile to refresh.</p> : null}</>
                : <p className="playground-empty">{mode === "stylesheet" ? "Stylesheet mode has no markup preview — see the CSS tab." : "Compile to render a live preview."}</p>
            ) : null}
            {tab === "css" ? (
              <div>
                <pre className="demo-source" aria-live="polite">
                  <code>{result ? result.css || "(no rules generated)" : error ?? "Compiling…"}</code>
                </pre>
                {result?.stats ? (
                  <p className="form-help">
                    {result.stats.candidatesFound} candidates · {result.stats.rulesGenerated} rules · {result.stats.sourcesScanned} source
                  </p>
                ) : null}
              </div>
            ) : null}
            {tab === "diagnostics" ? (
              diagnostics.length > 0 ? (
                <div className="grid gap-2">
                  {diagnostics.map((diagnostic, index) => (
                    <div key={`${diagnostic.code}-${index}`} className={diagnostic.severity === "error" ? "alert alert-danger" : "alert alert-warning"} role={diagnostic.severity === "error" ? "alert" : "status"}>
                      <div className="alert-content">
                        <strong>{diagnostic.code}</strong> · {diagnostic.message}
                        {diagnostic.help ? <span className="form-help"> {diagnostic.help}</span> : null}
                      </div>
                    </div>
                  ))}
                </div>
              ) : <p className="playground-empty">No diagnostics — clean compile.</p>
            ) : null}
            {tab === "candidates" ? (
              reports.length > 0 ? (
                <div className="grid gap-2">
                  {reports.map(report => {
                    const modes = result?.candidates.filter(candidate => candidate.raw === report.candidate).map(candidate => candidate.mode) ?? [];
                    const extraction = modes.length > 0 ? [...new Set(modes)].join(", ") : "text";
                    return (
                      <div key={report.candidate} className="component-card flex flex-wrap items-center gap-2 p-3">
                        <code className="font-mono text-xs">{report.candidate}</code>
                        <span className={report.valid ? "badge badge-success" : "badge badge-danger"}>{report.valid ? "valid" : report.status}</span>
                        <span className="badge badge-neutral">extracted: {extraction}</span>
                        {!report.valid && report.alternatives.length > 0 ? (
                          <span className="form-help">Did you mean {report.alternatives.slice(0, 3).join(", ")}?</span>
                        ) : null}
                      </div>
                    );
                  })}
                </div>
              ) : <p className="playground-empty">Compile markup to see per-candidate validation.</p>
            ) : null}
            {tab === "api" ? <ApiDoc /> : null}
          </div>
        </div>
      </div>

      {error ? (
        <div className="alert alert-danger mt-4" role="alert">
          <div className="alert-content"><strong>Compile request failed.</strong> {error}</div>
        </div>
      ) : null}
    </div>
  );
}
