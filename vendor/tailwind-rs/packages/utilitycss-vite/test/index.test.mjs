import assert from "node:assert/strict";
import test from "node:test";

import { utilitycss } from "../dist/index.js";

test("keeps one compiler across transforms and invalidates the virtual module", async () => {
  const updates = [];
  let invalidated = false;
  class FakeNativeCompiler {
    updateSource(id, content, path) {
      updates.push([id, content, path]);
    }

    removeSource() {
      return false;
    }

    transformStylesheet(id, content, path) {
      assert.equal(id, "src/app.css");
      assert.equal(path, "src/app.css");
      return { css: content.replace("@apply p-4;", "padding: 1rem;"), diagnostics: [] };
    }

    build() {
      return {
        css: ".p-4{padding:1rem;}",
        diagnostics: [],
        stats: {
          sourcesScanned: 0,
          bytesScanned: 0,
          candidatesFound: 0,
          uniqueCandidates: 1,
          candidatesParsed: 0,
          cacheHits: 1,
          rulesGenerated: 1,
          rulesRemoved: 0
        }
      };
    }
  }

  const plugin = utilitycss({ native: FakeNativeCompiler });
  await plugin.transform("<div class=\"p-4\" />", "src/app.tsx");
  const modules = [{ id: "virtual" }];
  const returned = await plugin.handleHotUpdate({
    file: "src/app.tsx",
    modules,
    read: async () => "<div class=\"p-4\" />",
    server: {
      moduleGraph: {
        getModuleById: async () => ({ id: "\u0000virtual:utilitycss.css" }),
        invalidateModule: () => {
          invalidated = true;
        }
      }
    }
  });

  assert.deepEqual(returned, modules);
  assert.equal(plugin.load("\u0000virtual:utilitycss.css"), ".p-4{padding:1rem;}");
  assert.equal(updates.length, 2);
  assert.equal(invalidated, true);
});

test("transforms CSS files without sending them through source extraction", async () => {
  let updated = false;
  class CssNativeCompiler {
    updateSource() {
      updated = true;
    }
    removeSource() {
      return false;
    }
    transformStylesheet(id, content, path) {
      assert.equal(id, "src/app.css");
      assert.equal(path, "src/app.css");
      return { css: content.replace("@apply p-4;", "padding: 1rem;"), diagnostics: [] };
    }
    build() {
      return { css: "", diagnostics: [], stats: {
        sourcesScanned: 0,
        bytesScanned: 0,
        candidatesFound: 0,
        uniqueCandidates: 0,
        candidatesParsed: 0,
        cacheHits: 0,
        rulesGenerated: 0,
        rulesRemoved: 0
      }};
    }
  }

  const plugin = utilitycss({ native: CssNativeCompiler });
  const result = await plugin.transform.call(
    { warn: () => {}, error: () => { throw new Error("unexpected error"); } },
    ".button { @apply p-4; }",
    "src/app.css"
  );
  assert.deepEqual(result, { code: ".button { padding: 1rem; }", map: null });
  assert.equal(updated, false);
});

test("retransforms CSS on HMR updates and accepts query-string module IDs", async () => {
  const transformed = [];
  class CssNativeCompiler {
    updateSource() {}
    removeSource() {
      return false;
    }
    transformStylesheet(id, content, path) {
      transformed.push([id, content, path]);
      return { css: content, diagnostics: [] };
    }
    build() {
      return { css: "", diagnostics: [], stats: {
        sourcesScanned: 0,
        bytesScanned: 0,
        candidatesFound: 0,
        uniqueCandidates: 0,
        candidatesParsed: 0,
        cacheHits: 0,
        rulesGenerated: 0,
        rulesRemoved: 0
      }};
    }
  }
  const plugin = utilitycss({ native: CssNativeCompiler });
  const modules = [{ id: "src/app.css" }];
  await plugin.handleHotUpdate({
    file: "src/app.css?direct",
    event: { type: "update" },
    modules,
    read: async () => ".button { @apply p-8; }",
    server: { moduleGraph: {
      getModuleById: async () => undefined,
      invalidateModule: () => {}
    }}
  });
  assert.deepEqual(transformed, [["src/app.css", ".button { @apply p-8; }", "src/app.css"]]);
});

test("reports CSS diagnostics during HMR and recovers after the edit is fixed", async () => {
  const errors = [];
  class CssNativeCompiler {
    updateSource() {}
    removeSource() {
      return false;
    }
    transformStylesheet(id, content, path) {
      assert.equal(id, "src/app.css");
      assert.equal(path, "src/app.css");
      if (content.includes("invalid")) {
        return {
          css: content,
          diagnostics: [{
            severity: "error",
            code: "apply.unknown-utility",
            message: "unknown utility `invalid`",
            source: "src/app.css",
            start: 17,
            end: 24,
            help: "remove the utility"
          }]
        };
      }
      return { css: content.replace("@apply p-8;", "padding: 2rem;"), diagnostics: [] };
    }
    build() {
      return { css: "", diagnostics: [], stats: {
        sourcesScanned: 0,
        bytesScanned: 0,
        candidatesFound: 0,
        uniqueCandidates: 0,
        candidatesParsed: 0,
        cacheHits: 0,
        rulesGenerated: 0,
        rulesRemoved: 0
      }};
    }
  }
  const plugin = utilitycss({ native: CssNativeCompiler });
  const server = {
    moduleGraph: { getModuleById: async () => undefined, invalidateModule: () => {} },
    config: { logger: { warn: () => {}, error: (message) => errors.push(message) } }
  };
  await assert.rejects(
    () => plugin.handleHotUpdate({
      file: "src/app.css",
      event: { type: "update" },
      modules: [],
      read: async () => ".button { @apply invalid; }",
      server
    }),
    /unknown utility `invalid`/
  );
  assert.match(errors[0], /apply\.unknown-utility/);

  await plugin.handleHotUpdate({
    file: "src/app.css",
    event: { type: "update" },
    modules: [],
    read: async () => ".button { @apply p-8; }",
    server
  });
});

test("normalizes module IDs and removes deleted source state", async () => {
  const removed = [];
  class FakeNativeCompiler {
    updateSource() {}

    removeSource(id) {
      removed.push(id);
      return true;
    }

    build() {
      return {
        css: "",
        diagnostics: [],
        stats: {
          sourcesScanned: 0,
          bytesScanned: 0,
          candidatesFound: 0,
          uniqueCandidates: 0,
          candidatesParsed: 0,
          cacheHits: 0,
          rulesGenerated: 0,
          rulesRemoved: 0
        }
      };
    }
  }

  const plugin = utilitycss({ native: FakeNativeCompiler });
  await plugin.handleHotUpdate({
    file: "/@fs/C:\\project\\App.tsx?vue&type=template",
    event: { type: "delete" },
    modules: [],
    read: async () => {
      throw new Error("deleted files must not be read");
    },
    server: {
      moduleGraph: {
        getModuleById: async () => undefined,
        invalidateModule: () => {}
      }
    }
  });

  assert.deepEqual(removed, ["C:/project/App.tsx"]);
});

test("removes deleted sources from Rollup watcher notifications", () => {
  const removed = [];
  class FakeNativeCompiler {
    updateSource() {}
    removeSource(id) {
      removed.push(id);
      return true;
    }
    build() {
      return {
        css: "",
        diagnostics: [],
        stats: {
          sourcesScanned: 0,
          bytesScanned: 0,
          candidatesFound: 0,
          uniqueCandidates: 0,
          candidatesParsed: 0,
          cacheHits: 0,
          rulesGenerated: 0,
          rulesRemoved: 0
        }
      };
    }
  }

  const plugin = utilitycss({ native: FakeNativeCompiler });
  plugin.watchChange("src/removed.html?direct", { event: "delete" });
  assert.deepEqual(removed, ["src/removed.html"]);
});

test("reports non-CSS HMR diagnostics and recovers after invalid HTML is fixed", async () => {
  const errors = [];
  const sources = new Map();
  let invalidated = 0;
  const stats = {
    sourcesScanned: 1,
    bytesScanned: 1,
    candidatesFound: 1,
    uniqueCandidates: 1,
    candidatesParsed: 1,
    cacheHits: 0,
    rulesGenerated: 1,
    rulesRemoved: 0
  };
  class FakeNativeCompiler {
    updateSource(id, content) {
      sources.set(id, content);
    }
    removeSource(id) {
      return sources.delete(id);
    }
    build() {
      const invalid = [...sources.entries()].find(([, content]) => content.includes("p-[]"));
      if (invalid) {
        return {
          css: ".p-4{padding:1rem;}",
          diagnostics: [{
            severity: "error",
            code: "utility.unknown",
            message: `unknown utility in ${invalid[0]}`,
            source: invalid[0],
            start: 12,
            end: 16
          }],
          stats
        };
      }
      return { css: ".p-4{padding:1rem;}", diagnostics: [], stats };
    }
  }

  const plugin = utilitycss({ native: FakeNativeCompiler });
  const server = {
    moduleGraph: {
      getModuleById: async () => ({ id: "\u0000virtual:utilitycss.css" }),
      invalidateModule: () => {
        invalidated += 1;
      }
    },
    config: { logger: { warn: () => {}, error: (message) => errors.push(message) } }
  };
  const update = (content) => plugin.handleHotUpdate({
    file: "src/app.html",
    event: { type: "update" },
    modules: [],
    read: async () => content,
    server
  });

  await assert.rejects(() => update('<div class="p-[]"></div>'), /unknown utility in src\/app\.html/);
  assert.equal(errors.length, 1);
  assert.match(errors[0], /utility\.unknown/);

  await update('<div class="p-4"></div>');
  assert.equal(invalidated, 1);
});

test("reports non-CSS HMR diagnostics and recovers after invalid TSX is fixed", async () => {
  const errors = [];
  const sources = new Map();
  let invalidated = 0;
  const stats = {
    sourcesScanned: 1,
    bytesScanned: 1,
    candidatesFound: 1,
    uniqueCandidates: 1,
    candidatesParsed: 1,
    cacheHits: 0,
    rulesGenerated: 1,
    rulesRemoved: 0
  };
  class FakeNativeCompiler {
    updateSource(id, content) {
      sources.set(id, content);
    }
    removeSource(id) {
      return sources.delete(id);
    }
    build() {
      const invalid = [...sources.entries()].find(([, content]) => content.includes("bg-reed-500"));
      if (invalid) {
        return {
          css: "",
          diagnostics: [{
            severity: "error",
            code: "utility.unknown",
            message: `unknown utility in ${invalid[0]}`,
            source: invalid[0],
            start: 0,
            end: 11
          }],
          stats
        };
      }
      return { css: ".p-4{padding:1rem;}", diagnostics: [], stats };
    }
  }

  const plugin = utilitycss({ native: FakeNativeCompiler });
  const server = {
    moduleGraph: {
      getModuleById: async () => ({ id: "\u0000virtual:utilitycss.css" }),
      invalidateModule: () => {
        invalidated += 1;
      }
    },
    config: { logger: { warn: () => {}, error: (message) => errors.push(message) } }
  };
  const update = (content) => plugin.handleHotUpdate({
    file: "src/app.tsx",
    event: { type: "update" },
    modules: [],
    read: async () => content,
    server
  });

  await assert.rejects(() => update('<div className="bg-reed-500" />'), /unknown utility in src\/app\.tsx/);
  assert.equal(errors.length, 1);

  await update('<div className="p-4" />');
  assert.equal(invalidated, 1);
});

test("refuses to load CSS while compiler errors are unresolved", () => {
  const stats = {
    sourcesScanned: 0,
    bytesScanned: 0,
    candidatesFound: 0,
    uniqueCandidates: 0,
    candidatesParsed: 0,
    cacheHits: 0,
    rulesGenerated: 0,
    rulesRemoved: 0
  };
  class ErrorNativeCompiler {
    updateSource() {}
    removeSource() {
      return false;
    }
    build() {
      return {
        css: ".p-4{padding:1rem;}",
        diagnostics: [{
          severity: "error",
          code: "utility.unknown",
          message: "unknown utility `nope`",
          source: "src/app.html",
          start: 12,
          end: 16
        }],
        stats
      };
    }
  }
  class WarningNativeCompiler extends ErrorNativeCompiler {
    build() {
      const result = super.build();
      return {
        ...result,
        diagnostics: result.diagnostics.map((diagnostic) => ({ ...diagnostic, severity: "warning" }))
      };
    }
  }

  assert.throws(
    () => utilitycss({ native: ErrorNativeCompiler }).load("\u0000virtual:utilitycss.css"),
    /unknown utility `nope`/
  );
  const warnings = [];
  const originalWarn = console.warn;
  console.warn = (message) => warnings.push(message);
  try {
    assert.equal(
      utilitycss({ native: WarningNativeCompiler }).load("\u0000virtual:utilitycss.css"),
      ".p-4{padding:1rem;}"
    );
  } finally {
    console.warn = originalWarn;
  }
  assert.equal(warnings.length, 1);
  assert.match(warnings[0], /utility\.unknown/);
});

test("includes .htm sources like .html", async () => {
  const updates = [];
  class FakeNativeCompiler {
    updateSource(id, content, path) {
      updates.push([id, content, path]);
    }
    removeSource() {
      return false;
    }
    build() {
      return {
        css: "",
        diagnostics: [],
        stats: {
          sourcesScanned: 0,
          bytesScanned: 0,
          candidatesFound: 0,
          uniqueCandidates: 0,
          candidatesParsed: 0,
          cacheHits: 0,
          rulesGenerated: 0,
          rulesRemoved: 0
        }
      };
    }
  }

  const plugin = utilitycss({ native: FakeNativeCompiler });
  await plugin.transform.call(
    { warn: () => {}, error: () => { throw new Error("unexpected error"); } },
    "<div />",
    "src/partial.htm"
  );
  await plugin.transform.call(
    { warn: () => {}, error: () => { throw new Error("unexpected error"); } },
    "<div />",
    "src/upper.HTM"
  );
  assert.deepEqual(updates, [
    ["src/partial.htm", "<div />", "src/partial.htm"],
    ["src/upper.HTM", "<div />", "src/upper.HTM"]
  ]);
});

test("forwards compiler diagnostics to the Vite hook context", async () => {
  const warnings = [];
  class FakeNativeCompiler {
    updateSource() {}
    removeSource() {
      return false;
    }
    build() {
      return {
        css: "",
        diagnostics: [{
          severity: "warning",
          code: "fixture.warning",
          message: "fixture warning",
          source: "src/app.html",
          start: 4,
          end: 8,
          help: "fix the fixture"
        }],
        stats: {
          sourcesScanned: 1,
          bytesScanned: 1,
          candidatesFound: 1,
          uniqueCandidates: 1,
          candidatesParsed: 1,
          cacheHits: 0,
          rulesGenerated: 0,
          rulesRemoved: 0
        }
      };
    }
  }

  const plugin = utilitycss({ native: FakeNativeCompiler });
  await plugin.transform.call(
    { warn: (message) => warnings.push(message), error: () => { throw new Error("unexpected error"); } },
    "<div />",
    "src/app.html"
  );

  assert.deepEqual(warnings, [
    "src/app.html:4-8: fixture warning [fixture.warning]; fix the fixture"
  ]);
});
