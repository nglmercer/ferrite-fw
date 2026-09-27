/// Ambient declarations for non-TS imports (resolved by Ferrite itself).

declare module "*.md" {
  import type { DocsPage } from "./docs-types";

  const page: DocsPage;
  export default page;
}

declare module "*.css" {
  const css: string;
  export default css;
}

declare module "ferrite:*" {
  const id: string;
  export default id;
}
