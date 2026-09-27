import { readFileSync } from "node:fs";
import { resolve } from "node:path";

/**
 * Reads the demo CSS-first config. Consumed as a Bun macro so production
 * bundles embed the content at build time; resolving a source-relative
 * config path from the emitted `dist/server.js` at runtime would break.
 */
export function loadLabConfig(): string {
  return readFileSync(resolve(import.meta.dir, "..", "..", "utilitycss.config.css"), "utf8");
}
