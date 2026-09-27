type ClassValue = string | false | null | undefined;

/**
 * Minimal class-name joiner in the spirit of clsx/cn. Falsy values are
 * skipped and whitespace runs collapse, so conditional fragments never leak
 * stray spaces into the rendered `class` attribute.
 */
export function cn(...parts: ClassValue[]): string {
  const tokens: string[] = [];
  for (const part of parts) {
    if (typeof part !== "string" || part === "") {
      continue;
    }
    for (const token of part.split(/\s+/)) {
      if (token !== "") {
        tokens.push(token);
      }
    }
  }
  return tokens.join(" ");
}
