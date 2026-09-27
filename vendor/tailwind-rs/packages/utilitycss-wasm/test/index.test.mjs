import assert from "node:assert/strict";
import test from "node:test";

import { createWasmCompiler } from "../dist/index.js";

test("forwards the WASM compiler surface", () => {
  const stats = {
    sourcesScanned: 1,
    bytesScanned: 16,
    candidatesFound: 1,
    uniqueCandidates: 1,
    candidatesParsed: 1,
    cacheHits: 0,
    rulesGenerated: 1,
    rulesRemoved: 0
  };
  const constructed = [];
  class FakeCompiler {
    constructor(...args) {
      constructed.push(args);
    }
    updateSource(...args) {
      constructed.push(["updateSource", ...args]);
    }
    updateSourceWithCandidates(...args) {
      constructed.push(["updateSourceWithCandidates", ...args]);
    }
    extractCandidates(content, path) {
      return [{ raw: "flex", start: 0, end: 4, extractionMode: content && path ? "ast" : "static" }];
    }
    explain(candidate) {
      return JSON.stringify({ candidate, status: "Valid" });
    }
    validate(candidate) {
      return JSON.stringify({ candidate, valid: true, status: "Valid" });
    }
    capabilities() {
      return JSON.stringify({ version: 1 });
    }
    removeSource() {
      return true;
    }
    build() {
      return {
        css: ".flex{display:flex;}",
        diagnostics: [
          {
            severity: "warning",
            code: "browser.unsupported",
            message: "field-sizing is not supported by the active target",
            source: "src/app.html",
            start: 0,
            end: 4
          }
        ],
        stats
      };
    }
    transformStylesheet() {
      return { css: ".button{display:flex;}", diagnostics: [] };
    }
    free() {}
  }

  const compiler = createWasmCompiler({ WasmCompiler: FakeCompiler }, true, "{}", "safari-15");
  assert.deepEqual(constructed[0], [true, "{}", "safari-15"]);
  compiler.updateSource("src/app.html", "flex", "src/app.html");
  compiler.updateSourceWithCandidates("src/app.tsx", "p-4", "src/app.tsx", [
    { raw: "p-4", start: 0, end: 3, extractionMode: "ast" }
  ]);
  assert.deepEqual(constructed[1], ["updateSource", "src/app.html", "flex", "src/app.html"]);
  assert.deepEqual(constructed[2][0], "updateSourceWithCandidates");
  assert.deepEqual(compiler.extractCandidates("<div />", "src/app.tsx"), [
    { raw: "flex", start: 0, end: 4, extractionMode: "ast" }
  ]);
  assert.equal(JSON.parse(compiler.explain("flex")).status, "Valid");
  assert.equal(JSON.parse(compiler.validate("flex")).valid, true);
  assert.equal(JSON.parse(compiler.capabilities()).version, 1);
  assert.equal(compiler.removeSource("src/app.html"), true);
  const result = compiler.build();
  assert.equal(result.css, ".flex{display:flex;}");
  assert.deepEqual(result.diagnostics, [
    {
      severity: "warning",
      code: "browser.unsupported",
      message: "field-sizing is not supported by the active target",
      source: "src/app.html",
      start: 0,
      end: 4
    }
  ]);
  assert.equal(result.stats.sourcesScanned, 1);
  assert.deepEqual(
    compiler.transformStylesheet("styles.css", ".button { @apply flex; }"),
    { css: ".button{display:flex;}", diagnostics: [] }
  );
  compiler.free();
});
