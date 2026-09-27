export function render(url: string): string {
  return `<h1>hello from ${url}</h1><p>rendered on the ${typeof window === "undefined" ? "server" : "client"}</p>`;
}
