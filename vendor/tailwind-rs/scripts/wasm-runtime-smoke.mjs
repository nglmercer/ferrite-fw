import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const bindingsDirectory = process.env.UTILITYCSS_WASM_BINDINGS
  ? process.env.UTILITYCSS_WASM_BINDINGS
  : fileURLToPath(new URL("../target/wasm-bindgen", import.meta.url));
const { WasmCompiler } = await import(pathToFileURL(join(bindingsDirectory, "utilitycss_wasm.js")).href);
const compiler = new WasmCompiler(false);
compiler.updateSource("src/app.html", '<div class="p-4"></div>', "src/app.html");
const first = compiler.build();
if (!first.css.includes(".p-4{padding:1rem;}")) {
  throw new Error(`WASM compiler emitted unexpected initial CSS: ${JSON.stringify(first)}`);
}
if (!Array.isArray(first.diagnostics) || first.diagnostics.length !== 0) {
  throw new Error(`WASM compiler emitted unexpected initial diagnostics: ${JSON.stringify(first)}`);
}
if (typeof first.stats?.sourcesScanned !== "number") {
  throw new Error(`WASM compiler emitted unexpected build stats: ${JSON.stringify(first)}`);
}
const stylesheet = compiler.transformStylesheet("src/app.css", ".button { @apply flex p-4; }");
if (stylesheet.css !== ".button{display:flex;padding:1rem;}" || stylesheet.diagnostics.length !== 0) {
  throw new Error(`WASM stylesheet transform emitted unexpected result: ${JSON.stringify(stylesheet)}`);
}
if (!compiler.removeSource("src/app.html")) {
  throw new Error("WASM compiler did not report the source removal");
}
const second = compiler.build();
if (second.css !== "") {
  throw new Error(`WASM compiler retained CSS after removal: ${JSON.stringify(second)}`);
}

// N-API parity: constructor config/target, candidate extraction, and JSON-string introspection.
const parity = new WasmCompiler(false, "{}", "safari-15");
const extracted = parity.extractCandidates('<div className="p-4" />', "src/app.tsx");
if (!Array.isArray(extracted) || extracted[0]?.raw !== "p-4" || extracted[0]?.extractionMode !== "ast") {
  throw new Error(`WASM extractCandidates diverged from N-API shape: ${JSON.stringify(extracted)}`);
}
const explained = JSON.parse(parity.explain("p-4"));
if (explained.candidate !== "p-4" || explained.status !== "Valid" || explained.browser_target !== "safari-15") {
  throw new Error(`WASM explain payload diverged from N-API encoding: ${JSON.stringify(explained)}`);
}
const validated = JSON.parse(parity.validate("flx"));
if (validated.valid !== false || !Array.isArray(validated.alternatives) || validated.alternatives.length === 0) {
  throw new Error(`WASM validate payload diverged from N-API encoding: ${JSON.stringify(validated)}`);
}
if (typeof parity.capabilities() !== "string") {
  throw new Error("WASM capabilities payload is not a JSON string");
}
let rejected = false;
try {
  new WasmCompiler(false, null, "ie11");
} catch {
  rejected = true;
}
if (!rejected) {
  throw new Error("WASM compiler accepted an unknown browser target");
}
console.log("WASM runtime smoke passed");
