import { LabInputError, compileLabSource, parseLabInput } from "./compile.ts";

function jsonError(message: string, status: number): Response {
  return Response.json({ error: message }, { status });
}

/** Handles `POST /api/compile` for the Compiler Lab page. Pure function of the request. */
export async function handleCompileRequest(request: Request): Promise<Response> {
  if (request.method !== "POST") {
    return jsonError("method not allowed; use POST", 405);
  }
  let body: unknown;
  try {
    body = await request.json();
  } catch {
    return jsonError("request body must be valid JSON", 400);
  }
  try {
    return Response.json(compileLabSource(parseLabInput(body)));
  } catch (error) {
    if (error instanceof LabInputError) {
      return jsonError(error.message, 400);
    }
    throw error;
  }
}
