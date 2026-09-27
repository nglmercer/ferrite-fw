/// Presentational components for the docs site. Every component builds real
/// DOM nodes with text content; no HTML strings are constructed here.

import { el, icon } from "./dom";
import type { DocsPage, DocsStrings, Locale } from "./docs-types";

/// `overview.md` → `overview`.
export function slugOf(path: string): string {
  const base = path.split("/").pop() || "overview";
  return base.replace(/\.md$/, "");
}

/// Narrow an unknown value to a supported locale.
export function isLocale(value: unknown): value is Locale {
  return value === "en" || value === "es" || value === "cn";
}

/// Landing link heading the nav lists (shared by sidebar and drawer).
export function homeLink(
  locale: Locale,
  slug: string,
  label: string,
  onNavigate?: () => void,
): HTMLAnchorElement {
  const link = el(
    "a",
    {
      href: `#/${locale}/home`,
      class: `nav-link block rounded-md px-3 py-2 text-sm leading-6 no-underline ${
        slug === "home" ? "nav-link-active font-semibold" : ""
      }`,
    },
    `← ${label}`,
  );
  if (onNavigate) link.addEventListener("click", onNavigate);
  return link;
}

interface SidebarProps {
  locale: Locale;
  slug: string;
  pages: DocsPage[];
  onNavigate?: () => void;
}

/// One nav link per page (shared by the desktop sidebar and the drawer).
export function SidebarLinks(props: SidebarProps): HTMLAnchorElement[] {
  return props.pages.map((page) => {
    const slug = slugOf(page.path);
    const link = el(
      "a",
      {
        href: `#/${props.locale}/${slug}`,
        class: `nav-link block rounded-md px-3 py-2 text-sm leading-6 no-underline ${
          slug === props.slug ? "nav-link-active font-semibold" : ""
        }`,
      },
      page.title || slug,
    );
    if (props.onNavigate) link.addEventListener("click", props.onNavigate);
    return link;
  });
}

interface TocProps {
  page: DocsPage;
  strings: DocsStrings;
}

/// Table of contents (`null` when the page has no headings).
export function Toc(props: TocProps): HTMLElement | null {
  const items = (props.page.headings || []).filter((h) => h.level <= 3);
  if (items.length === 0) return null;
  return el(
    "nav",
    { "aria-label": props.strings.onThisPage, class: "text-sm" },
    el(
      "p",
      { class: "toc-kicker px-3 text-xs font-semibold uppercase tracking-wider" },
      props.strings.onThisPage,
    ),
    el(
      "ul",
      { class: "toc-list mt-2 space-y-1" },
      ...items.map((h) =>
        el(
          "li",
          {},
          el(
            "a",
            {
              href: `#${h.id}`,
              "data-toc": true,
              class: `toc-link block rounded px-3 py-1 no-underline ${h.level === 3 ? "pl-6" : ""}`,
            },
            h.text,
          ),
        ),
      ),
    ),
  );
}

interface PageNavProps {
  locale: Locale;
  slug: string;
  pages: DocsPage[];
  strings: DocsStrings;
}

function neighbors(
  pages: DocsPage[],
  slug: string,
): { prev: DocsPage | null; next: DocsPage | null } {
  const index = pages.findIndex((page) => slugOf(page.path) === slug);
  return {
    prev: index > 0 ? (pages[index - 1] ?? null) : null,
    next: index >= 0 && index < pages.length - 1 ? (pages[index + 1] ?? null) : null,
  };
}

/// Previous/next page navigation (`null` for a lone page).
export function PageNav(props: PageNavProps): HTMLElement | null {
  const { prev, next } = neighbors(props.pages, props.slug);
  if (!prev && !next) return null;
  const link = (page: DocsPage, cls: string, kicker: string, arrow: string): HTMLElement =>
    el(
      "a",
      { href: `#/${props.locale}/${slugOf(page.path)}`, class: cls },
      el("span", { class: "page-nav-kicker" }, `${arrow} ${kicker}`),
      el("span", { class: "page-nav-title" }, page.title || slugOf(page.path)),
    );
  // Empty span keeps a lone next-link pinned right.
  return el(
    "nav",
    { class: "page-nav", "aria-label": props.slug },
    prev ? link(prev, "page-prev", props.strings.previous, "←") : el("span"),
    next ? link(next, "page-next", props.strings.next, "→") : null,
  );
}

interface HeaderProps {
  locale: Locale;
  locales: Locale[];
  localeNames: Record<Locale, string>;
  strings: DocsStrings;
  dark: boolean;
  onLocaleChange: (locale: Locale) => void;
  onThemeToggle: () => void;
  onOpenDrawer: () => void;
}

/// Site header: nav toggle, brand, locale select, theme toggle.
export function Header(props: HeaderProps): HTMLElement {
  const select = el(
    "select",
    { id: "locale-select", class: "theme-select", "aria-label": props.strings.language },
    ...props.locales.map((code) =>
      el(
        "option",
        { value: code, selected: code === props.locale || undefined },
        props.localeNames[code],
      ),
    ),
  );
  select.addEventListener("change", () => {
    if (isLocale(select.value)) props.onLocaleChange(select.value);
  });
  const themeButton = el(
    "button",
    {
      type: "button",
      id: "theme-toggle",
      class: "theme-btn",
      "aria-pressed": String(props.dark),
      title: props.strings.theme,
    },
    props.dark ? icon("moon") : icon("sun"),
    el(
      "span",
      { class: "theme-btn-label" },
      props.dark ? props.strings.dark : props.strings.light,
    ),
  );
  themeButton.addEventListener("click", props.onThemeToggle);
  const navToggle = el(
    "button",
    {
      type: "button",
      id: "nav-toggle",
      class: "theme-btn md:hidden",
      "aria-label": props.strings.menu,
    },
    icon("burger", 16),
  );
  navToggle.addEventListener("click", props.onOpenDrawer);
  return el(
    "header",
    { class: "theme-header" },
    el(
      "div",
      { class: "mx-auto flex max-w-6xl items-center gap-3 px-6 py-3" },
      navToggle,
      el(
        "span",
        {
          class:
            "theme-badge inline-flex h-8 w-8 items-center justify-center rounded-md text-lg font-bold",
        },
        "F",
      ),
      el(
        "a",
        {
          href: `#/${props.locale}/home`,
          class: "theme-title text-base font-semibold tracking-tight no-underline",
        },
        "Ferrite",
      ),
      el("span", { class: "theme-pill rounded px-2 py-0.5 text-xs" }, props.strings.docs),
      el(
        "a",
        { href: `#/${props.locale}/overview`, class: "theme-doclink text-sm no-underline" },
        props.strings.docsLink,
      ),
      el(
        "span",
        { class: "theme-tagline ml-auto hidden text-xs sm:inline" },
        props.strings.tagline,
      ),
      select,
      themeButton,
    ),
  );
}

interface DrawerProps {
  locale: Locale;
  slug: string;
  pages: DocsPage[];
  strings: DocsStrings;
  onClose: () => void;
}

/// Mobile drawer + backdrop (returned as siblings, like the original markup).
export function MobileDrawer(props: DrawerProps): [HTMLDivElement, HTMLDivElement] {
  const backdrop = el("div", { class: "drawer-backdrop md:hidden", id: "drawer-backdrop" });
  backdrop.addEventListener("click", props.onClose);
  const closeButton = el(
    "button",
    {
      type: "button",
      id: "drawer-close",
      class: "theme-btn",
      "aria-label": props.strings.close,
    },
    icon("close", 16),
  );
  closeButton.addEventListener("click", props.onClose);
  const drawer = el(
    "div",
    {
      class: "drawer md:hidden",
      id: "mobile-drawer",
      role: "dialog",
      "aria-label": props.strings.pages,
    },
    el(
      "div",
      { class: "flex items-center justify-between pb-3" },
      el("span", { class: "text-sm font-semibold" }, props.strings.pages),
      closeButton,
    ),
    el(
      "nav",
      { class: "space-y-0.5" },
      homeLink(props.locale, props.slug, props.strings.home, props.onClose),
      ...SidebarLinks({
        locale: props.locale,
        slug: props.slug,
        pages: props.pages,
        onNavigate: props.onClose,
      }),
    ),
  );
  return [backdrop, drawer];
}

async function copyText(text: string | null): Promise<boolean> {
  const value = text ?? "";
  if (window.navigator.clipboard && window.isSecureContext !== false) {
    try {
      await window.navigator.clipboard.writeText(value);
      return true;
    } catch {
      /* fall through to the legacy path */
    }
  }
  const area = document.createElement("textarea");
  area.value = value;
  area.style.position = "fixed";
  area.style.opacity = "0";
  document.body.appendChild(area);
  area.select();
  let ok = false;
  try {
    ok = document.execCommand("copy");
  } catch {
    ok = false;
  }
  area.remove();
  return ok;
}

/// Wrap each `pre` with a copy button (icon swaps to a check on success).
export function enhanceCodeBlocks(article: Element, strings: DocsStrings): void {
  for (const pre of article.querySelectorAll("pre")) {
    const wrapper = el("div", { class: "codeblock" });
    pre.replaceWith(wrapper);
    wrapper.append(pre);
    const button = el("button", {
      type: "button",
      class: "copy-btn",
      "data-tip": strings.copy,
      "aria-label": strings.copy,
    });
    button.append(icon("copy"));
    let timer: number | null = null;
    button.addEventListener("click", async () => {
      const code = pre.querySelector("code");
      const ok = await copyText((code || pre).textContent);
      button.replaceChildren(icon(ok ? "check" : "copy"));
      button.setAttribute("data-tip", ok ? strings.copied : strings.copy);
      button.classList.toggle("copied", ok);
      if (timer) window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        button.replaceChildren(icon("copy"));
        button.setAttribute("data-tip", strings.copy);
        button.classList.remove("copied");
      }, 1600);
    });
    wrapper.append(button);
  }
}

/// Rewrite relative Markdown links (`./other.md`) to hash routes.
export function rewriteRelativeLinks(
  root: Element,
  hasSlug: (target: string) => boolean,
  locale: Locale,
): void {
  for (const anchor of root.querySelectorAll("a[href]")) {
    const href = anchor.getAttribute("href");
    if (!href || href.startsWith("#") || /^[a-z]+:/i.test(href)) continue;
    const target = href.replace(/\.md$/, "").replace(/^\.\//, "");
    if (hasSlug(target)) anchor.setAttribute("href", `#/${locale}/${target}`);
  }
}
