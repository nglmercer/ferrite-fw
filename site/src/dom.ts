/// Minimal typed DOM helpers: UI is built with `createElement` and text nodes
/// instead of HTML strings, so rendering is injection-safe by construction.

/// A child of an element: nodes are appended, strings become text nodes,
/// nullish/false values are skipped (for conditional children).
export type Child = Node | string | null | undefined | false;

/// Attribute values: `true` sets an empty attribute, nullish/false omits it.
export type Attrs = Record<string, string | boolean | null | undefined>;

/// Create an element, set attributes, and append children.
export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Attrs = {},
  ...children: Child[]
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [name, value] of Object.entries(attrs)) {
    if (value === null || value === undefined || value === false) continue;
    if (name === "class" && typeof value === "string") {
      node.className = value;
    } else if (value === true) {
      node.setAttribute(name, "");
    } else {
      node.setAttribute(name, value);
    }
  }
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue;
    node.append(child);
  }
  return node;
}

/// Icon names provided by the SVG sprite in `index.html` (`#icon-*`).
export type IconName = "sun" | "moon" | "copy" | "check" | "burger" | "close";

const SVG_NS = "http://www.w3.org/2000/svg";

/// Inline icon referencing the sprite; no SVG strings are parsed at runtime.
export function icon(name: IconName, size = 14): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("width", String(size));
  svg.setAttribute("height", String(size));
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "2");
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS(SVG_NS, "use");
  use.setAttribute("href", `#icon-${name}`);
  svg.append(use);
  return svg;
}

/**
 * Insert build-time-rendered Markdown HTML. This is the only place raw HTML
 * becomes DOM: the markup is produced at build time from local `.md` files
 * (never user input), and `DOMParser` documents never execute scripts.
 */
export function setTrustedMarkdown(container: Element, html: string): void {
  container.replaceChildren();
  const doc = new DOMParser().parseFromString(html, "text/html");
  container.append(...doc.body.childNodes);
}

/// Fetch an element that must exist (throws a clear error otherwise).
export function getElement(id: string): HTMLElement {
  const node = document.getElementById(id);
  if (!node) throw new Error(`missing #${id}`);
  return node;
}
