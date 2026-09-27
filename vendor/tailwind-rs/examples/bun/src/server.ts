import homepage from "./index.html";
import { handleCompileRequest } from "./lab/route.ts";

const port = Number(process.env.PORT ?? 3000);
const development = process.env.NODE_ENV !== "production";

const server = Bun.serve({
  port,
  routes: {
    "/": homepage,
    "/api/compile": handleCompileRequest
  },
  development: development
    ? {
        hmr: true,
        console: true
      }
    : false,
  fetch() {
    return new Response("Not found", { status: 404 });
  }
});

console.log(`Bun server running at ${server.url}`);
