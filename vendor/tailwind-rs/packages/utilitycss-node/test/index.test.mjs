import assert from "node:assert/strict";
import test from "node:test";

import { createCompiler } from "../dist/index.js";

test("forwards lifecycle calls and normalizes a native result", () => {
  const calls = [];
  class FakeNativeCompiler {
    updateSource(id, content, path) {
      calls.push(["updateSource", id, content, path]);
    }

    removeSource(id) {
      calls.push(["removeSource", id]);
      return true;
    }

    build() {
      return {
        css: ".flex{display:flex;}",
        diagnostics: [],
        stats: {
          sourcesScanned: 1,
          bytesScanned: 5,
          candidatesFound: 1,
          uniqueCandidates: 1,
          candidatesParsed: 1,
          cacheHits: 0,
          rulesGenerated: 1,
          rulesRemoved: 0
        }
      };
    }
  }

  const compiler = createCompiler({ native: FakeNativeCompiler });
  compiler.updateSource("src/app.html", "flex", "src/app.html");
  assert.equal(compiler.removeSource("src/old.html"), true);
  assert.equal(compiler.build().css, ".flex{display:flex;}");
  assert.deepEqual(calls, [
    ["updateSource", "src/app.html", "flex", "src/app.html"],
    ["removeSource", "src/old.html"]
  ]);
});

test("normalizes the native stylesheet transformation result", () => {
  class StylesheetNativeCompiler {
    updateSource() {}
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
    transformStylesheet(id, content, path) {
      assert.equal(id, "styles.css");
      assert.equal(path, "styles.css");
      assert.match(content, /@apply/);
      return {
        css: ".button{display:flex;}",
        diagnostics: [{
          severity: "warning",
          code: "apply.fixture",
          message: "fixture warning",
          source: id,
          start: 10,
          end: 16,
          help: "fixture help"
        }]
      };
    }
  }

  const compiler = createCompiler({ native: StylesheetNativeCompiler });
  const result = compiler.transformStylesheet("styles.css", ".button { @apply flex; }", "styles.css");
  assert.equal(result.css, ".button{display:flex;}");
  assert.deepEqual(result.diagnostics, [{
    severity: "warning",
    code: "apply.fixture",
    message: "fixture warning",
    source: "styles.css",
    start: 10,
    end: 16,
    help: "fixture help"
  }]);
});

test("dispose releases the native handle and blocks further use", () => {
  const calls = [];
  class FakeNativeCompiler {
    updateSource() {
      calls.push("updateSource");
    }
    extractCandidates() {
      calls.push("extractCandidates");
      return [];
    }
    removeSource() {
      calls.push("removeSource");
      return true;
    }
    build() {
      calls.push("build");
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
    explain() {
      calls.push("explain");
      return {};
    }
    validate() {
      calls.push("validate");
      return {};
    }
    capabilities() {
      calls.push("capabilities");
      return {};
    }
    transformStylesheet() {
      calls.push("transformStylesheet");
      return { css: "", diagnostics: [] };
    }
  }

  const compiler = createCompiler({ native: FakeNativeCompiler });
  compiler.updateSource("src/app.html", "flex", "src/app.html", []);
  compiler.dispose();
  compiler.dispose();

  assert.equal(compiler.native, undefined);
  assert.throws(() => compiler.updateSource("src/app.html", "flex"), /has been disposed/);
  assert.throws(() => compiler.removeSource("src/app.html"), /has been disposed/);
  assert.throws(() => compiler.extractCandidates("flex"), /has been disposed/);
  assert.throws(() => compiler.build(), /has been disposed/);
  assert.throws(() => compiler.explain("p-4"), /has been disposed/);
  assert.throws(() => compiler.validate("p-4"), /has been disposed/);
  assert.throws(() => compiler.capabilities(), /has been disposed/);
  assert.throws(() => compiler.transformStylesheet("a.css", "b"), /has been disposed/);
  assert.deepEqual(calls, ["updateSource"]);
});
