import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { utilitycss } from "../../../packages/utilitycss-bun/src/index.ts";
import { buildProduction } from "./production-build.ts";
import { catalog, categoryDefinitions } from "./catalog.ts";
import { formatHashRoute, parseHashRoute } from "./hooks/useHashRoute.ts";
import { compileLabSource, parseLabInput, type LabCompileResult } from "./lab/compile.ts";
import { handleCompileRequest } from "./lab/route.ts";

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

const categoryValues = new Set(categoryDefinitions.map(category => category.value));
const slugs = catalog.map(item => item.slug);
assert(catalog.length >= 60, "catalog should cover the full component library (60+ focused pages)");
for (const category of categoryDefinitions) {
  const count = catalog.filter(item => item.category === category.value).length;
  assert(count > 0, `catalog category ${category.value} has no component pages`);
}
assert(catalog.filter(item => item.category === "data-input").length >= 10, "data-input should list each control on its own page");
assert(catalog.filter(item => item.category === "mockup").length >= 4, "mockup should cover browser, code, phone, and window frames");
assert(catalog.some(item => item.slug === "motion" && item.category === "tools"), "tools should include the motion page");

// Accessibility guard: demo anchors must point at real gallery routes. A bare
// `href="#"` rewrites the hash and drops the user on the first catalog page,
// so actions must use buttons and links must carry `#/category/slug` targets.
function sourceFiles(dir: string): string[] {
  const files: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...sourceFiles(full));
    } else if (/\.(tsx?|html)$/.test(entry.name)) {
      files.push(full);
    }
  }
  return files;
}
const deadLinks = sourceFiles(import.meta.dir).filter(file => !file.endsWith("verify.ts") && readFileSync(file, "utf8").includes('href="#"'));
assert(deadLinks.length === 0, `dead href="#" links found: ${deadLinks.join(", ")}`);
assert(new Set(slugs).size === slugs.length, "catalog contains duplicate component slugs");
for (const item of catalog) {
  assert(categoryValues.has(item.category), `catalog item ${item.slug} references an unknown category`);
  assert(item.description.trim().length > 0, `catalog item ${item.slug} is missing a description`);
  assert(typeof item.demo === "function", `catalog item ${item.slug} has no reachable demo`);
  const parsed = parseHashRoute(formatHashRoute(item.category, item.slug));
  assert(parsed.category === item.category && parsed.slug === item.slug, `catalog route failed to round-trip for ${item.slug}`);
}

const result = await buildProduction({ write: false });
assert(result.success, `Bun production build failed: ${result.logs.map(log => log.message).join("\n")}`);

const cssOutputs = await Promise.all(
  result.outputs
    .filter(output => output.path.endsWith(".css"))
    .map(async output => ({ path: output.path, contents: await output.text() }))
);
const css = cssOutputs.map(output => output.contents).join("\n");
assert(css.length > 0, "Bun production build did not emit a CSS asset");

const expectedOutput = [
  ".button-primary",
  ".button-danger",
  ".button-success",
  ".button-group",
  ".component-card",
  ".form-input",
  ".badge-success",
  ".badge-brand",
  ".alert",
  ".banner",
  ".pagination-link",
  ".sidebar-link",
  ".progress-bar",
  ".spinner",
  ".timeline",
  ".dropdown-menu",
  ".popover",
  ".tooltip",
  ".carousel-dot",
  ".drawer-panel",
  ".avatar-stack",
  ".fab",
  ".swap",
  ".theme-controller",
  ".chat-bubble",
  ".fold-panel",
  ".countdown",
  ".diff-frame",
  ".kbd",
  ".status-list",
  ".dock",
  ".link",
  ".menu",
  ".megamenu-panel",
  ".radial-progress",
  ".tooltip-bubble",
  ".calendar-day",
  ".checkbox-input",
  ".radio-card",
  ".otp-box",
  ".validator-card",
  ".filter-chip",
  ".label-floating",
  ".divider",
  ".footer-demo",
  ".indicator-badge",
  ".join",
  ".mask-shape",
  ".stack-card",
  ".mockup-browser",
  ".mockup-code",
  ".mockup-phone",
  ".mockup-window",
  ".animate-fade-in",
  ".animate-pulse-soft",
  ".loading-dots",
  ".motion-stage",
  ".playground-preview",
  ".playground-empty",
  ".tabs",
  ".tab",
  ".tabs-box",
  ".bg-brand-600",
  ".bg-blue-600",
  ".bg-green-600",
  ".hover\\:bg-gray-100:hover",
  ".md\\:grid-cols-3",
  ".aspect-video",
  ".border-gray-200",
  ".sr-only",
  "@media"
];
const missing = expectedOutput.filter(fragment => !css.includes(fragment));
assert(missing.length === 0, `utilitycss output is missing: ${missing.join(", ")}`);
// Visual-regression proofs: recipes emit border width+color but no style, so a
// preflight reset is required; the timeline must not show list numerals; the
// carousel slide carries its own layout (template-literal classes are never
// extracted); the featured pricing card keeps its brand border via recipes.
assert(css.includes("border:0 solid"), "border preflight reset is missing from the output");
assert(css.includes(".timeline{list-style:none"), "timeline list-style reset was not emitted");
assert(css.includes(".carousel-slide{display:flex"), "carousel slide layout did not move into the recipe");
assert(css.includes(".pricing-card-featured{border-color:#4f46e5"), "featured pricing border was not emitted");
assert(css.includes(".list-none{list-style-type:none}"), "list-none utility was not emitted");
// Motion proofs: keyframe entrances ship with their keyframes, and every
// animation is disabled under prefers-reduced-motion.
assert(css.includes(".animate-fade-in{animation:.5s ease-out both fade-in}"), "fade-in entrance was not emitted");
assert(css.includes("@keyframes pulse-soft"), "pulse-soft keyframes are missing from the output");
assert(css.includes("prefers-reduced-motion"), "reduced-motion handling is missing from the output");
assert(css.includes(".skip-link:focus-visible"), "skip-link reveal styles are missing from the output");
assert(css.includes("background-color:#4f46e5"), "CSS-first brand token was not resolved");
assert(css.includes("border-radius"), "common radius utilities were not emitted");
assert(css.includes("box-shadow"), "common shadow utilities were not emitted");
assert(css.includes("clip:rect(0,0,0,0)"), "screen-reader-only labels are not visually clipped");
assert(css.includes(".button{") && css.includes("border:0"), "button recipe did not reset the native border");
assert(css.includes(".toggle{") && css.includes("padding:0"), "toggle recipe did not reset native button padding");
assert(
  css.includes("--spacing:.25rem") || css.includes("--spacing:0.25rem"),
  "demo CSS does not define the spacing scale used by numeric utilities"
);
// Showcase proofs: utilities extracted from the HTML entrypoint, from a cn()
// helper call in AppShell, and from CSS-first custom breakpoints.
assert(css.includes(".min-h-screen{min-height:100vh"), "HTML entrypoint utility was not extracted");
assert(css.includes(".content-auto{content-visibility:auto"), "cn() helper utility was not extracted");
assert(css.includes(".xs\\:tracking-wide"), "custom xs breakpoint variant was not emitted");
assert(css.includes("xl\\:text-base"), "custom 2xl breakpoint variant was not emitted");
assert(css.includes("30rem"), "custom xs breakpoint value is missing from the media query");
assert(css.includes("96rem"), "custom 2xl breakpoint value is missing from the media query");
assert(css.includes(".max-w-\\[24rem\\]{max-width:24rem}"), "arbitrary max-w-[24rem] value was not emitted");
assert(css.includes(".progress-bar-45{width:45%}"), "@apply w-[45%] arbitrary value was not emitted");

// Breakpoint proof: custom xs/2xl plus default md/lg sorted by min-width value.
const mediaPreludes = [...css.matchAll(/@media[^{]*\{/g)].map(match => match[0]);
const mediaOrder = ["30rem", "768px", "1024px", "96rem"].map(
  marker => mediaPreludes.findIndex(prelude => prelude.includes(marker))
);
assert(!mediaOrder.includes(-1), "a breakpoint media query is missing from the output");
assert(
  mediaOrder[0] < mediaOrder[1] && mediaOrder[1] < mediaOrder[2] && mediaOrder[2] < mediaOrder[3],
  `breakpoint media queries are not ordered by min-width: ${mediaPreludes.join(" ")}`
);

const outputKinds = result.outputs.map(output => output.path.split(".").pop()).filter(Boolean);
assert(outputKinds.includes("html"), "Bun production build did not emit the HTML entry asset");
assert(outputKinds.includes("js"), "Bun production build did not emit the Preact JavaScript asset");

// Rebuild + deletion exercise: one plugin instance across three in-memory
// builds proves edited sources refresh the stylesheet and deleted sources
// are pruned instead of lingering as stale CSS. The fixture mirrors the
// demo pipeline (entry imports HTML; the stylesheet enters via the HTML
// `utilitycss` link) and keeps classes in a helper call: plain string
// literals are not candidates by design, and the helper stays deliberately
// unresolved because only the build graph matters here, never execution.
const fixtureDir = mkdtempSync(join(tmpdir(), "utilitycss-verify-"));
try {
  const fixtureEntry = join(fixtureDir, "entry.ts");
  const fixturePage = join(fixtureDir, "page.html");
  const fixtureWidget = join(fixtureDir, "widget.ts");
  const writeWidget = (utility: string): void => {
    writeFileSync(fixtureWidget, `export const cls: string = cn("${utility}");\n`);
  };
  const writePage = (withScript: boolean): void => {
    const script = withScript ? `<script type="module" src="./widget.ts"></script>` : "";
    writeFileSync(
      fixturePage,
      `<!doctype html><html><head><link rel="stylesheet" href="utilitycss"></head><body>${script}</body></html>\n`
    );
  };
  writeWidget("flex");
  writePage(true);
  writeFileSync(fixtureEntry, `import homepage from "./page.html";\nconsole.log(homepage);\n`);
  const fixturePlugin = utilitycss({ pretty: false });
  const buildFixtureCss = async (): Promise<string> => {
    const fixture = await Bun.build({
      entrypoints: [fixtureEntry],
      target: "bun",
      plugins: [fixturePlugin],
      write: false
    } as Bun.BuildConfig & { write?: boolean });
    assert(fixture.success, `fixture rebuild failed: ${fixture.logs.map(log => log.message).join("\n")}`);
    const parts = await Promise.all(
      fixture.outputs.filter(output => output.path.endsWith(".css")).map(output => output.text())
    );
    return parts.join("\n");
  };
  const firstCss = await buildFixtureCss();
  assert(/display:\s*flex/.test(firstCss), "initial fixture build did not emit the widget utility");
  writeWidget("grid");
  const secondCss = await buildFixtureCss();
  assert(/display:\s*grid/.test(secondCss), "rebuild did not pick up the edited fixture source");
  assert(!/display:\s*flex/.test(secondCss), "rebuild kept CSS for a utility that no longer exists");
  writePage(false);
  rmSync(fixtureWidget);
  const thirdCss = await buildFixtureCss();
  assert(!/display:\s*grid/.test(thirdCss), "deleted fixture source survived pruning as stale CSS");
} finally {
  rmSync(fixtureDir, { recursive: true, force: true });
}

// Diagnostics + recovery exercise through the Compiler Lab backend: an
// invalid `@apply` errors with a code, and the fixed stylesheet is clean.
const badApply = compileLabSource(parseLabInput({
  source: ".oops { @apply not-a-real-utility; }",
  browserTarget: "modern",
  mode: "stylesheet"
}));
assert(
  badApply.diagnostics.some(diagnostic => diagnostic.code === "apply.unknown-utility"),
  "invalid @apply did not report an apply.unknown-utility diagnostic"
);
const fixedApply = compileLabSource(parseLabInput({
  source: ".oops { @apply flex; }",
  browserTarget: "modern",
  mode: "stylesheet"
}));
assert(fixedApply.diagnostics.length === 0, "fixed stylesheet still reports diagnostics");
assert(/display:\s*flex/.test(fixedApply.css), "fixed stylesheet did not recover the flex rule");

// Compat exercise: safari-15 warns on content-visibility but keeps the rule;
// modern stays silent for the same source.
const compatSource = '<div class="content-auto">compat fixture</div>';
const compat = compileLabSource(parseLabInput({ source: compatSource, browserTarget: "safari-15", mode: "markup" }));
assert(
  compat.diagnostics.some(diagnostic =>
    diagnostic.code === "browser.unsupported-feature" &&
    diagnostic.message.includes("safari-15") &&
    diagnostic.message.includes("content-visibility")
  ),
  "safari-15 build did not warn about content-visibility"
);
assert(compat.css.includes("content-visibility"), "compat warning dropped the generated rule");
const modernCompat = compileLabSource(parseLabInput({ source: compatSource, browserTarget: "modern", mode: "markup" }));
assert(modernCompat.diagnostics.length === 0, "modern build unexpectedly reported compat diagnostics");

// Compiler Lab API contract: valid compile, typo suggestions, and input errors.
const labOk = await handleCompileRequest(new Request("http://lab.invalid/api/compile", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ source: '<div class="flex">lab fixture</div>' })
}));
assert(labOk.status === 200, `lab compile request failed with status ${labOk.status}`);
const labBody = (await labOk.json()) as LabCompileResult;
assert(/display:\s*flex/.test(labBody.css), "lab compile did not emit the flex rule");
assert(labBody.reports.some(report => report.candidate === "flex" && report.valid), "lab compile did not validate the flex candidate");
const labTypo = await handleCompileRequest(new Request("http://lab.invalid/api/compile", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ source: '<div class="flx">typo fixture</div>' })
}));
const typoBody = (await labTypo.json()) as LabCompileResult;
const typoReport = typoBody.reports.find(report => report.candidate === "flx");
assert(typoReport !== undefined && !typoReport.valid, "lab compile did not flag the flx typo");
assert(typoReport.alternatives.includes("flex"), "lab typo report did not suggest flex");
const labMethod = await handleCompileRequest(new Request("http://lab.invalid/api/compile", { method: "GET" }));
assert(labMethod.status === 405, `lab route accepted GET with status ${labMethod.status}`);
const labJson = await handleCompileRequest(new Request("http://lab.invalid/api/compile", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: "not json"
}));
assert(labJson.status === 400, `lab route accepted invalid JSON with status ${labJson.status}`);
const labTarget = await handleCompileRequest(new Request("http://lab.invalid/api/compile", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ source: "x", browserTarget: "ie11" })
}));
assert(labTarget.status === 400, `lab route accepted browserTarget ie11 with status ${labTarget.status}`);

// Dev-server exercise: boot the real `bun --hot` server on an ephemeral port
// and prove it serves the gallery plus the Lab API.
const labPort = 31971;
const serverProcess = Bun.spawn(["bun", "--hot", "src/server.ts"], {
  cwd: resolve(import.meta.dir, ".."),
  env: { ...process.env, PORT: String(labPort) },
  stdout: "ignore",
  stderr: "ignore"
});
try {
  let served: Response | null = null;
  for (let attempt = 0; attempt < 100; attempt += 1) {
    try {
      served = await fetch(`http://127.0.0.1:${labPort}/`);
      break;
    } catch {
      await Bun.sleep(150);
    }
  }
  assert(served !== null && served.status === 200, "dev server did not serve the gallery route");
  const servedHtml = await served.text();
  assert(servedHtml.includes('<div id="app"'), "dev server HTML is missing the app mount");
  const servedApi = await fetch(`http://127.0.0.1:${labPort}/api/compile`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ source: '<div class="flex">served fixture</div>' })
  });
  assert(servedApi.status === 200, `dev server Lab API failed with status ${servedApi.status}`);
  const servedBody = (await servedApi.json()) as LabCompileResult;
  assert(/display:\s*flex/.test(servedBody.css), "dev server Lab API did not emit the flex rule");
} finally {
  serverProcess.kill();
  await serverProcess.exited;
}

console.log(`Bun + Preact component gallery verification passed (${catalog.length} catalog entries).`);
console.log(`Generated stylesheet asset (${css.length} bytes) from the Bun module graph.`);
console.log(`Production assets: ${outputKinds.join(", ")}.`);
console.log("Rebuild, deletion, diagnostics, compat, Lab API, and dev-server exercises passed.");
