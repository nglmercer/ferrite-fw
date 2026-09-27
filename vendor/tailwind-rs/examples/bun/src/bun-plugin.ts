import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { utilitycss } from "../../../packages/utilitycss-bun/src/index.ts";

const config = readFileSync(resolve(import.meta.dir, "..", "utilitycss.config.css"), "utf8");

/** Bunfig entry point that keeps dev/HMR on the same CSS-first config as production builds. */
export default utilitycss({ config, browserTarget: "modern", debug: true });
