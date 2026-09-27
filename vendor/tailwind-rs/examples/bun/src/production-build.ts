import { rm } from "node:fs/promises";
import { resolve } from "node:path";
import { utilitycss } from "../../../packages/utilitycss-bun/src/index.ts";

const projectRoot = resolve(import.meta.dir, "..");
const outputDirectory = resolve(projectRoot, "dist");

export interface ProductionBuildOptions {
  readonly write?: boolean;
}

/** Builds the fullstack example with Bun's bundler and the utilitycss plugin. */
export async function buildProduction(options: ProductionBuildOptions = {}) {
  // Bun emits content-hashed chunks without cleaning the outdir, so stale CSS
  // from previous builds would linger and could be mistaken for fresh output.
  // In-memory verification builds leave any existing output untouched.
  if (options.write ?? true) {
    await rm(outputDirectory, { recursive: true, force: true });
  }
  const config = await Bun.file(resolve(projectRoot, "utilitycss.config.css")).text();
  const buildOptions = {
    entrypoints: [resolve(projectRoot, "src", "server.ts")],
    outdir: outputDirectory,
    target: "bun",
    minify: true,
    plugins: [utilitycss({ browserTarget: "modern", config, pretty: false })],
    // The Lab API route needs the native binding at runtime; bundling a
    // `.node` binary is unsupported, so it stays external and resolves from
    // the example's node_modules next to the emitted server bundle.
    external: ["@utilitycss/napi"],
    write: options.write ?? true
  } as Bun.BuildConfig & { write?: boolean };

  return Bun.build(buildOptions);
}

if (import.meta.main) {
  const result = await buildProduction();
  if (!result.success) {
    throw new Error(`Bun production build failed:\n${result.logs.map((log) => log.message).join("\n")}`);
  }

  const cssOutputs = await Promise.all(
    result.outputs.filter((output) => output.path.endsWith(".css")).map((output) => output.text())
  );
  const css = cssOutputs.join("\n");
  if (!css.includes(".button-primary") || !css.includes(".component-card") || !css.includes(".form-input") || !css.includes(".badge-success") || !css.includes(".alert") || !css.includes(".spinner")) {
    throw new Error("Bun production build did not include generated utilitycss CSS");
  }
  console.log(`Bun production build passed with ${result.outputs.length} output assets.`);
}
