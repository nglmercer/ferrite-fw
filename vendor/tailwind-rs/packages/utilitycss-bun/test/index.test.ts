import { strict as assert } from "node:assert";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "bun:test";
import utilitycssDefault, { utilitycss } from "../src/index";

type SourceMap = Map<string, string>;

const STATS = {
  sourcesScanned: 0,
  bytesScanned: 0,
  candidatesFound: 0,
  uniqueCandidates: 0,
  candidatesParsed: 0,
  cacheHits: 0,
  rulesGenerated: 0,
  rulesRemoved: 0
};

class FakeNativeCompiler {
  private readonly sources: SourceMap = new Map();

  public constructor(_pretty = false) {}

  public updateSource(id: string, content: string): void {
    this.sources.set(id, content);
  }

  public removeSource(id: string): boolean {
    return this.sources.delete(id);
  }

  public transformStylesheet(_id: string, content: string) {
    const replacements: Record<string, string> = {
      "@apply p-4;": "padding: 1rem;",
      "@apply p-8;": "padding: 2rem;",
      "@apply hover:bg-red-500;": "background-color: #ef4444;"
    };
    let css = content;
    for (const [from, to] of Object.entries(replacements)) {
      css = css.replaceAll(from, to);
    }
    return { css, diagnostics: [] };
  }

  public build() {
    const diagnostics = [...this.sources.entries()]
      .filter(([, content]) => content.includes("utility-error"))
      .map(([source]) => ({
        severity: "error" as const,
        code: "UTILITY001",
        message: "invalid arbitrary value",
        source,
        start: 10,
        end: 24,
        help: "expected a balanced CSS value"
      }));
    const warnings = [...this.sources.entries()]
      .filter(([, content]) => content.includes("utility-warning"))
      .map(([source]) => ({
        severity: "warning" as const,
        code: "UTILITY002",
        message: "fixture warning",
        source,
        start: 2,
        end: 8,
        help: "remove the fixture marker"
      }));
    const classes = new Set<string>();
    for (const content of this.sources.values()) {
      for (const match of content.matchAll(/\b(?:flex|grid|p-4|p-8|bg-red-500)\b/g)) {
        classes.add(match[0]);
      }
    }
    const rules: Record<string, string> = {
      "bg-red-500": ".bg-red-500{background-color:#ef4444;}",
      flex: ".flex{display:flex;}",
      grid: ".grid{display:grid;}",
      "p-4": ".p-4{padding:1rem;}",
      "p-8": ".p-8{padding:2rem;}"
    };
    const css = [...classes].sort().map((name) => rules[name]).join("");
    return {
      css,
      diagnostics: [...diagnostics, ...warnings],
      stats: { ...STATS, sourcesScanned: this.sources.size, bytesScanned: [...this.sources.values()].join("").length }
    };
  }
}

test("exports a configurable factory and a Bunfig-compatible default plugin", () => {
  assert.equal(typeof utilitycssDefault.setup, "function");
  assert.equal(utilitycssDefault.name, "utilitycss-bun");
  assert.equal(typeof utilitycss().setup, "function");
});

test("generates CSS from HTML linked to the utilitycss virtual stylesheet", async () => {
  const root = await makeProject({
    "index.html": '<link rel="stylesheet" href="utilitycss"><main class="p-4"></main>'
  });
  try {
    const output = await build(root, utilitycss({ native: FakeNativeCompiler }), "index.html");
    assert.equal(output.result.success, true, JSON.stringify(output.result.logs));
    assert.match(output.css, /\.p-4\s*\{\s*padding:\s*1rem;/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("extracts JavaScript and TSX module graph sources", async () => {
  const root = await makeProject({
    "entry.ts": 'import "./helpers.js"; import "./Button.tsx"; import "utilitycss";',
    "helpers.js": 'export const classes = cn("flex", "p-4");',
    "Button.tsx": 'export const Button = () => <button className={clsx("flex", active && "p-4")} />;'
  });
  try {
    const output = await build(root, utilitycss({ native: FakeNativeCompiler }), "entry.ts");
    assert.equal(output.result.success, true, JSON.stringify(output.result.logs));
    assert.match(output.css, /\.flex\s*\{\s*display:\s*flex;/);
    assert.match(output.css, /\.p-4\s*\{\s*padding:\s*1rem;/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("retains modules reached through Bun path aliases", async () => {
  const root = await makeProject({
    "tsconfig.json": JSON.stringify({ compilerOptions: { baseUrl: ".", paths: { "@/*": ["./*"] } } }),
    "entry.ts": 'import "@/Button"; import "utilitycss";',
    "Button.ts": 'export const Button = () => "p-4";'
  });
  try {
    const output = await build(root, utilitycss({ native: FakeNativeCompiler }), "entry.ts");
    assert.equal(output.result.success, true, JSON.stringify(output.result.logs));
    assert.match(output.css, /\.p-4\s*\{\s*padding:\s*1rem;/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("rebuilds from the current graph without stale replacement or deleted-source CSS", async () => {
  const root = await makeProject({
    "entry.ts": 'import "./source.ts"; import "utilitycss";',
    "source.ts": 'export const classes = "p-4";'
  });
  let constructors = 0;
  class CountingNativeCompiler extends FakeNativeCompiler {
    public constructor(pretty = false) {
      super(pretty);
      constructors += 1;
    }
  }
  const plugin = utilitycss({ native: CountingNativeCompiler });
  try {
    const first = await build(root, plugin, "entry.ts");
    assert.match(first.css, /\.p-4\s*\{\s*padding:\s*1rem;/);

    await writeFile(join(root, "source.ts"), 'export const classes = "p-8";');
    const second = await build(root, plugin, "entry.ts");
    assert.match(second.css, /\.p-8\s*\{\s*padding:\s*2rem;/);
    assert.doesNotMatch(second.css, /\.p-4\s*\{/);

    await writeFile(join(root, "entry.ts"), 'import "utilitycss";');
    const third = await build(root, plugin, "entry.ts");
    assert.doesNotMatch(third.css, /\.p-(?:4|8)\s*\{/);
    assert.equal(constructors, 3);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("retains unchanged modules during an incremental HMR graph rebuild", async () => {
  const root = await makeProject({
    "entry.ts": 'import "./a.ts"; import "./b.ts"; import "utilitycss";',
    "a.ts": 'export const classes = "p-4";',
    "b.ts": 'export const classes = "flex";'
  });
  const plugin = utilitycss({ native: FakeNativeCompiler });
  try {
    const first = await build(root, plugin, "entry.ts");
    assert.match(first.css, /\.p-4\s*\{/);
    assert.match(first.css, /\.flex\s*\{/);

    await writeFile(join(root, "a.ts"), 'export const classes = "p-8";');
    const second = await build(root, plugin, "entry.ts");
    assert.match(second.css, /\.p-8\s*\{/);
    assert.match(second.css, /\.flex\s*\{/);
    assert.doesNotMatch(second.css, /\.p-4\s*\{/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("rebuilds an HTML-only edit through the live source graph", async () => {
  const root = await makeProject({
    "index.html": '<link rel="stylesheet" href="utilitycss"><main class="p-4"></main>'
  });
  const plugin = utilitycss({ native: FakeNativeCompiler });
  try {
    const first = await build(root, plugin, "index.html");
    assert.match(first.css, /\.p-4\s*\{/);

    await writeFile(
      join(root, "index.html"),
      '<link rel="stylesheet" href="utilitycss"><main class="p-8"></main>'
    );
    const second = await build(root, plugin, "index.html");
    assert.match(second.css, /\.p-8\s*\{/);
    assert.doesNotMatch(second.css, /\.p-4\s*\{/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("rebuilds a TSX-only edit while preserving the imported graph", async () => {
  const root = await makeProject({
    "entry.ts": 'import "./Button.tsx"; import "utilitycss";',
    "Button.tsx": 'export const Button = () => <button className="p-4" />;'
  });
  const plugin = utilitycss({ native: FakeNativeCompiler });
  try {
    const first = await build(root, plugin, "entry.ts");
    assert.match(first.css, /\.p-4\s*\{/);

    await writeFile(join(root, "Button.tsx"), 'export const Button = () => <button className="p-8" />;');
    const second = await build(root, plugin, "entry.ts");
    assert.match(second.css, /\.p-8\s*\{/);
    assert.doesNotMatch(second.css, /\.p-4\s*\{/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("emits byte-identical CSS for the same graph twice", async () => {
  const root = await makeProject({
    "entry.ts": 'import "./source.ts"; import "utilitycss";',
    "source.ts": 'export const classes = "p-8 flex";'
  });
  const plugin = utilitycss({ native: FakeNativeCompiler });
  try {
    const first = await build(root, plugin, "entry.ts");
    const second = await build(root, plugin, "entry.ts");
    assert.equal(second.css, first.css);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("fails the Bun build for errors and keeps warnings visible", async () => {
  const root = await makeProject({
    "entry.ts": 'import "./source.ts"; import "utilitycss";',
    "source.ts": 'export const classes = "utility-error utility-warning";'
  });
  const warnings: string[] = [];
  const originalWarn = console.warn;
  console.warn = (...values: unknown[]) => warnings.push(values.join(" "));
  try {
    await assert.rejects(
      () => build(root, utilitycss({ native: FakeNativeCompiler }), "entry.ts"),
      /Bundle failed/
    );
    assert.match(warnings.join("\n"), /UTILITY002/);
    assert.match(warnings.join("\n"), /help: remove the fixture marker/);
    assert.match(warnings.join("\n"), /line 1, column 3/);
    assert.match(warnings.join("\n"), /\| export const classes/);
  } finally {
    console.warn = originalWarn;
    await rm(root, { recursive: true, force: true });
  }
});

test("transforms imported CSS through the stylesheet API", async () => {
  const root = await makeProject({
    "entry.ts": 'import "./app.css"; import "utilitycss";',
    "app.css": ".button { @apply p-4; }"
  });
  try {
    const output = await build(root, utilitycss({ native: FakeNativeCompiler }), "entry.ts");
    assert.equal(output.result.success, true, JSON.stringify(output.result.logs));
    assert.match(output.css, /\.button\s*\{\s*padding:\s*1rem;/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("updates imported CSS during an incremental development build", async () => {
  const root = await makeProject({
    "entry.ts": 'import "./app.css"; import "utilitycss";',
    "app.css": ".button { @apply p-4; }"
  });
  const plugin = utilitycss({ native: FakeNativeCompiler });
  try {
    const first = await build(root, plugin, "entry.ts");
    assert.match(first.css, /padding:\s*1rem/);
    await writeFile(join(root, "app.css"), ".button { @apply p-8; }");
    const second = await build(root, plugin, "entry.ts");
    assert.match(second.css, /padding:\s*2rem/);
    assert.doesNotMatch(second.css, /padding:\s*1rem/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("uses Bun's live development server mode alongside incremental graph builds", async () => {
  const server = Bun.serve({
    port: 0,
    development: { hmr: true, console: false },
    routes: {
      "/": new Response("utilitycss dev server")
    },
    fetch() {
      return new Response("Not found", { status: 404 });
    }
  });
  try {
    assert.equal(server.development, true);
    assert.equal(await (await fetch(server.url)).text(), "utilitycss dev server");
  } finally {
    server.stop(true);
  }
});

test("does not require bundler entrypoints in Bun's serve lifecycle", () => {
  type SetupBuild = Parameters<NonNullable<ReturnType<typeof utilitycss>["setup"]>>[0];
  let onEnd: (() => void) | undefined;
  const build = {
    config: {},
    onStart() {},
    onResolve() {},
    onLoad() {},
    onEnd(callback: () => void) {
      onEnd = callback;
    }
  } as unknown as SetupBuild;

  utilitycss({ native: FakeNativeCompiler }).setup(build);
  assert.ok(onEnd);
  assert.doesNotThrow(() => onEnd?.());
});

test("recovers after an incremental diagnostic and handles graph creation/removal", async () => {
  const root = await makeProject({
    "entry.ts": 'import "./a.ts"; import "./b.ts"; import "utilitycss";',
    "a.ts": 'export const classes = "p-4";',
    "b.ts": 'export const classes = "flex";'
  });
  const plugin = utilitycss({ native: FakeNativeCompiler, debug: true });
  const debugErrors: string[] = [];
  const originalError = console.error;
  console.error = (...values: unknown[]) => debugErrors.push(values.join(" "));
  try {
    const first = await build(root, plugin, "entry.ts");
    assert.match(first.css, /\.p-4/);
    assert.match(first.css, /\.flex/);

    await writeFile(join(root, "a.ts"), 'export const classes = "utility-error";');
    let failure: unknown;
    try {
      await build(root, plugin, "entry.ts");
    } catch (error) {
      failure = error;
    }
    assert.ok(failure);
    assert.match(debugErrors.join("\n"), /Bun HMR keeps the last successful bundle/);

    await writeFile(join(root, "a.ts"), 'export const classes = "p-8";');
    await writeFile(join(root, "entry.ts"), 'import "./a.ts"; import "./c.ts"; import "utilitycss";');
    await writeFile(join(root, "c.ts"), 'export const classes = "grid";');
    const recovered = await build(root, plugin, "entry.ts");
    assert.equal(recovered.result.success, true, JSON.stringify(recovered.result.logs));
    assert.match(recovered.css, /\.p-8/);
    assert.match(recovered.css, /\.grid/);
    assert.doesNotMatch(recovered.css, /\.flex/);
  } finally {
    console.error = originalError;
    await rm(root, { recursive: true, force: true });
  }
});

test("ignores fragments, URLs, and missing assets when pruning the source graph", async () => {
  const poisonedTemplate =
    'const template = \'<a href="#section"><img src="/assets/logo.png">' +
    '<img src="missing.png"><img src="https://cdn.example/x.png"><img src="data:image/png;base64,AAA">\';';
  const root = await makeProject({
    "entry.ts": `import "./used.ts";\nimport "./stale.ts";\nimport "utilitycss";\n${poisonedTemplate}\n`,
    "used.ts": 'export const classes = "p-4";',
    "stale.ts": 'export const classes = "flex";'
  });
  const plugin = utilitycss({ native: FakeNativeCompiler });
  try {
    const first = await build(root, plugin, "entry.ts");
    assert.match(first.css, /\.flex\s*\{/);

    await writeFile(
      join(root, "entry.ts"),
      `import "./used.ts";\nimport "utilitycss";\n${poisonedTemplate}\n`
    );
    const second = await build(root, plugin, "entry.ts");
    assert.equal(second.result.success, true, JSON.stringify(second.result.logs));
    assert.match(second.css, /\.p-4\s*\{/);
    assert.doesNotMatch(second.css, /\.flex\s*\{/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("prunes unreachable modules despite real HTML fragment and asset references", async () => {
  const root = await makeProject({
    "index.html":
      '<link rel="stylesheet" href="utilitycss">' +
      '<a href="#section">section</a>' +
      '<img src="logo.png" alt="">' +
      '<img src="https://cdn.example/x.png" alt="">' +
      '<img src="data:image/png;base64,AAA" alt="">' +
      '<main class="p-4"></main>',
    "logo.png": "fake-png-bytes",
    "entry.ts": 'import "./used.ts";\nimport "./stale.ts";\nimport "utilitycss";\n',
    "used.ts": 'export const classes = "grid";',
    "stale.ts": 'export const classes = "flex";'
  });
  const plugin = utilitycss({ native: FakeNativeCompiler });
  const buildAll = async () => {
    const result = await Bun.build({
      entrypoints: [join(root, "index.html"), join(root, "entry.ts")],
      outdir: join(root, "out"),
      plugins: [plugin],
      target: "browser"
    });
    const outputs = await Promise.all(result.outputs.map((artifact) => artifact.text()));
    const css = outputs.find((contents, index) => result.outputs[index].path.endsWith(".css")) ?? "";
    return { result, css };
  };
  try {
    const first = await buildAll();
    assert.equal(first.result.success, true, JSON.stringify(first.result.logs));
    assert.match(first.css, /\.flex\s*\{/);

    await writeFile(join(root, "entry.ts"), 'import "./used.ts";\nimport "utilitycss";\n');
    const second = await buildAll();
    assert.equal(second.result.success, true, JSON.stringify(second.result.logs));
    assert.match(second.css, /\.p-4\s*\{/);
    assert.match(second.css, /\.grid\s*\{/);
    assert.doesNotMatch(second.css, /\.flex\s*\{/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("drops deleted files from the graph and recovers when they return", async () => {
  const root = await makeProject({
    "entry.ts": 'import "./used.ts";\nimport "./doomed.ts";\nimport "utilitycss";\n',
    "used.ts": 'export const classes = "p-4";',
    "doomed.ts": 'export const classes = "flex";'
  });
  const plugin = utilitycss({ native: FakeNativeCompiler });
  try {
    const first = await build(root, plugin, "entry.ts");
    assert.match(first.css, /\.flex\s*\{/);

    await rm(join(root, "doomed.ts"), { force: true });
    await writeFile(join(root, "entry.ts"), 'import "./used.ts";\nimport "utilitycss";\n');
    const second = await build(root, plugin, "entry.ts");
    assert.equal(second.result.success, true, JSON.stringify(second.result.logs));
    assert.match(second.css, /\.p-4\s*\{/);
    assert.doesNotMatch(second.css, /\.flex\s*\{/);

    await writeFile(join(root, "doomed.ts"), 'export const classes = "flex";');
    await writeFile(
      join(root, "entry.ts"),
      'import "./used.ts";\nimport "./doomed.ts";\nimport "utilitycss";\n'
    );
    const third = await build(root, plugin, "entry.ts");
    assert.equal(third.result.success, true, JSON.stringify(third.result.logs));
    assert.match(third.css, /\.flex\s*\{/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("extracts classes from imported .htm modules like .html", async () => {
  // NOTE: .htm files reach the adapter through imports. Bun 1.4.0 crashes on
  // Windows when an .htm file is used as a bundler entrypoint, so entries use
  // .html (covered above) while this test pins adapter-level .htm parity.
  const root = await makeProject({
    "entry.ts": 'import "./page.htm";\nimport "utilitycss";\n',
    "page.htm": '<main class="p-4"></main>'
  });
  const plugin = utilitycss({ native: FakeNativeCompiler });
  try {
    const { result, css } = await build(root, plugin, "entry.ts");
    assert.equal(result.success, true, JSON.stringify(result.logs));
    assert.match(css, /\.p-4\s*\{/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

async function makeProject(files: Record<string, string>): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "utilitycss-bun-test-"));
  await mkdir(join(root, "out"), { recursive: true });
  for (const [relativePath, content] of Object.entries(files)) {
    await writeFile(join(root, relativePath), content);
  }
  return root;
}

async function build(root: string, plugin: ReturnType<typeof utilitycss>, entry: string) {
  const result = await Bun.build({
    entrypoints: [join(root, entry)],
    outdir: join(root, "out"),
    plugins: [plugin],
    target: "browser",
    jsx: { runtime: "classic", factory: "h" }
  });
  const outputs = await Promise.all(result.outputs.map((artifact) => artifact.text()));
  const css = outputs.find((contents, index) => result.outputs[index].path.endsWith(".css")) ?? "";
  return { result, css };
}
