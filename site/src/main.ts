import "ferrite:tailwind.css";
import "./docs.css";

import type { DocsPage, DocsStrings, Locale } from "./docs-types";

import enOverview from "../pages/en/overview.md";
import enGettingStarted from "../pages/en/getting-started.md";
import enCli from "../pages/en/cli.md";
import enConfiguration from "../pages/en/configuration.md";
import enPlugins from "../pages/en/plugins.md";
import enProduction from "../pages/en/production.md";
import enTailwindVendor from "../pages/en/tailwind-vendor.md";
import esOverview from "../pages/es/overview.md";
import esGettingStarted from "../pages/es/getting-started.md";
import esCli from "../pages/es/cli.md";
import esConfiguration from "../pages/es/configuration.md";
import esPlugins from "../pages/es/plugins.md";
import esProduction from "../pages/es/production.md";
import esTailwindVendor from "../pages/es/tailwind-vendor.md";
import cnOverview from "../pages/cn/overview.md";
import cnGettingStarted from "../pages/cn/getting-started.md";
import cnCli from "../pages/cn/cli.md";
import cnConfiguration from "../pages/cn/configuration.md";
import cnPlugins from "../pages/cn/plugins.md";
import cnProduction from "../pages/cn/production.md";
import cnTailwindVendor from "../pages/cn/tailwind-vendor.md";

const LOCALES: Locale[] = ["en", "es", "cn"];
const LOCALE_NAMES: Record<Locale, string> = { en: "EN", es: "ES", cn: "中文" };
const DEFAULT_LOCALE: Locale = "en";

const STRINGS: Record<Locale, DocsStrings> = {
  en: {
    docs: "docs",
    tagline: "Rust-native SSR toolchain · built with itself",
    onThisPage: "On this page",
    pages: "Pages",
    renderedFrom: "rendered from Markdown at build time",
    theme: "Theme",
    light: "Light",
    dark: "Dark",
    language: "Language",
    copy: "Copy",
    copied: "Copied",
    previous: "Previous",
    next: "Next",
    menu: "Menu",
    close: "Close",
    title: "Ferrite Docs — Rust-native web toolchain",
  },
  es: {
    docs: "docs",
    tagline: "Toolchain SSR nativa de Rust · hecha consigo misma",
    onThisPage: "En esta página",
    pages: "Páginas",
    renderedFrom: "renderizado desde Markdown en build",
    theme: "Tema",
    light: "Claro",
    dark: "Oscuro",
    language: "Idioma",
    copy: "Copiar",
    copied: "Copiado",
    previous: "Anterior",
    next: "Siguiente",
    menu: "Menú",
    close: "Cerrar",
    title: "Docs Ferrite — toolchain web nativa de Rust",
  },
  cn: {
    docs: "文档",
    tagline: "Rust 原生 SSR 工具链 · 自举构建",
    onThisPage: "本页目录",
    pages: "页面",
    renderedFrom: "构建时由 Markdown 渲染",
    theme: "主题",
    light: "浅色",
    dark: "深色",
    language: "语言",
    copy: "复制",
    copied: "已复制",
    previous: "上一页",
    next: "下一页",
    menu: "菜单",
    close: "关闭",
    title: "Ferrite 文档 —— Rust 原生 Web 工具链",
  },
};

const PAGES: Record<Locale, DocsPage[]> = {
  en: [
    enOverview,
    enGettingStarted,
    enCli,
    enConfiguration,
    enPlugins,
    enProduction,
    enTailwindVendor,
  ],
  es: [
    esOverview,
    esGettingStarted,
    esCli,
    esConfiguration,
    esPlugins,
    esProduction,
    esTailwindVendor,
  ],
  cn: [
    cnOverview,
    cnGettingStarted,
    cnCli,
    cnConfiguration,
    cnPlugins,
    cnProduction,
    cnTailwindVendor,
  ],
};

for (const locale of LOCALES) {
  PAGES[locale].sort((a, b) => (a.order ?? 99) - (b.order ?? 99));
}

const bySlug: Record<Locale, Map<string, DocsPage>> = {
  en: new Map(),
  es: new Map(),
  cn: new Map(),
};
for (const locale of LOCALES) {
  bySlug[locale] = new Map(
    PAGES[locale].map((page): [string, DocsPage] => [slugOf(page.path), page]),
  );
}

function slugOf(path: string): string {
  const base = path.split("/").pop() || "overview";
  return base.replace(/\.md$/, "");
}

function isLocale(value: unknown): value is Locale {
  return value === "en" || value === "es" || value === "cn";
}

function storedLocale(): Locale {
  try {
    const saved: unknown = window.localStorage.getItem("ferrite-docs-locale");
    if (isLocale(saved)) return saved;
    const nav = (window.navigator.language || "en").toLowerCase();
    if (nav.startsWith("es")) return "es";
    if (nav.startsWith("zh")) return "cn";
  } catch {
    /* private mode: fall through to default */
  }
  return DEFAULT_LOCALE;
}

interface Route {
  locale: Locale;
  slug: string;
}

function currentRoute(): Route {
  const hash = window.location.hash.replace(/^#\/?/, "");
  const [maybeLocale, maybeSlug] = hash.split("/");
  if (isLocale(maybeLocale)) {
    const pages = bySlug[maybeLocale];
    const slug = maybeSlug !== undefined && pages.has(maybeSlug) ? maybeSlug : "overview";
    return { locale: maybeLocale, slug };
  }
  // Legacy `#/slug` links resolve against the stored locale.
  const locale = storedLocale();
  const slug = maybeLocale !== undefined && bySlug[locale].has(maybeLocale) ? maybeLocale : "overview";
  return { locale, slug };
}

function setLocale(locale: Locale): void {
  try {
    window.localStorage.setItem("ferrite-docs-locale", locale);
  } catch {
    /* ignore */
  }
  const { slug } = currentRoute();
  window.location.hash = `#/${locale}/${slug}`;
}

type Theme = "dark" | "light";

function theme(): Theme {
  return document.documentElement.classList.contains("dark") ? "dark" : "light";
}

function setTheme(next: Theme): void {
  document.documentElement.classList.toggle("dark", next === "dark");
  try {
    window.localStorage.setItem("ferrite-docs-theme", next);
  } catch {
    /* ignore */
  }
  const button = document.getElementById("theme-toggle");
  if (button) {
    button.setAttribute("aria-pressed", String(next === "dark"));
    const label = button.querySelector(".theme-btn-label");
    if (label) label.textContent = themeLabel(next);
  }
}

function themeLabel(next: Theme): string {
  const { locale } = currentRoute();
  return next === "dark" ? STRINGS[locale].dark : STRINGS[locale].light;
}

const ICON_SUN =
  '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>';
const ICON_MOON =
  '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M21 12.8A9 9 0 1 1 11.2 3 7 7 0 0 0 21 12.8z"/></svg>';
const ICON_COPY =
  '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>';
const ICON_CHECK =
  '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M20 6 9 17l-5-5"/></svg>';
const ICON_BURGER =
  '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M3 6h18M3 12h18M3 18h18"/></svg>';
const ICON_CLOSE =
  '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M18 6 6 18M6 6l12 12"/></svg>';

function sidebar(locale: Locale, active: string): string {
  return PAGES[locale]
    .map(
      (page) => `
      <a href="#/${locale}/${slugOf(page.path)}"
         class="nav-link block rounded-md px-3 py-2 text-sm leading-6 no-underline ${
           slugOf(page.path) === active ? "nav-link-active font-semibold" : ""
         }">
        ${escapeHtml(page.title || slugOf(page.path))}
      </a>`,
    )
    .join("");
}

function toc(page: DocsPage, strings: DocsStrings): string {
  const items = (page.headings || []).filter((h) => h.level <= 3);
  if (items.length === 0) return "";
  return `
    <nav aria-label="${escapeHtml(strings.onThisPage)}" class="text-sm">
      <p class="toc-kicker px-3 text-xs font-semibold uppercase tracking-wider">${escapeHtml(strings.onThisPage)}</p>
      <ul class="toc-list mt-2 space-y-1">
        ${items
          .map(
            (h) => `
          <li>
            <a href="#${h.id}" data-toc
               class="toc-link block rounded px-3 py-1 no-underline ${h.level === 3 ? "pl-6" : ""}">
              ${escapeHtml(h.text)}
            </a>
          </li>`,
          )
          .join("")}
      </ul>
    </nav>`;
}

function escapeHtml(text: unknown): string {
  return String(text)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function rewriteRelativeLinks(root: Element, locale: Locale): void {
  for (const anchor of root.querySelectorAll("a[href]")) {
    const href = anchor.getAttribute("href");
    if (!href || href.startsWith("#") || /^[a-z]+:/i.test(href)) continue;
    const target = href.replace(/\.md$/, "").replace(/^\.\//, "");
    if (bySlug[locale].has(target)) anchor.setAttribute("href", `#/${locale}/${target}`);
  }
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

function addCopyButtons(article: Element, strings: DocsStrings): void {
  for (const pre of article.querySelectorAll("pre")) {
    const wrapper = document.createElement("div");
    wrapper.className = "codeblock";
    pre.replaceWith(wrapper);
    wrapper.appendChild(pre);
    const button = document.createElement("button");
    button.type = "button";
    button.className = "copy-btn";
    button.innerHTML = ICON_COPY;
    button.setAttribute("data-tip", strings.copy);
    button.setAttribute("aria-label", strings.copy);
    let timer: number | null = null;
    button.addEventListener("click", async () => {
      const code = pre.querySelector("code");
      const ok = await copyText((code || pre).textContent);
      button.innerHTML = ok ? ICON_CHECK : ICON_COPY;
      button.setAttribute("data-tip", ok ? strings.copied : strings.copy);
      button.classList.toggle("copied", ok);
      if (timer) window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        button.innerHTML = ICON_COPY;
        button.setAttribute("data-tip", strings.copy);
        button.classList.remove("copied");
      }, 1600);
    });
    wrapper.appendChild(button);
  }
}

interface PageNeighbors {
  prev: DocsPage | null;
  next: DocsPage | null;
}

function pageNeighbors(locale: Locale, slug: string): PageNeighbors {
  const pages = PAGES[locale];
  const index = pages.findIndex((page) => slugOf(page.path) === slug);
  return {
    prev: index > 0 ? (pages[index - 1] ?? null) : null,
    next: index >= 0 && index < pages.length - 1 ? (pages[index + 1] ?? null) : null,
  };
}

function pageNav(locale: Locale, slug: string, strings: DocsStrings): string {
  const { prev, next } = pageNeighbors(locale, slug);
  if (!prev && !next) return "";
  const link = (page: DocsPage, cls: string, kicker: string, arrow: string): string => `
    <a href="#/${locale}/${slugOf(page.path)}" class="${cls}">
      <span class="page-nav-kicker">${arrow} ${escapeHtml(kicker)}</span>
      <span class="page-nav-title">${escapeHtml(page.title || slugOf(page.path))}</span>
    </a>`;
  // Empty span keeps a lone next-link pinned right.
  return `
    <nav class="page-nav" aria-label="${escapeHtml(slug)}">
      ${prev ? link(prev, "page-prev", strings.previous, "←") : "<span></span>"}
      ${next ? link(next, "page-next", strings.next, "→") : ""}
    </nav>`;
}

function closeDrawer(): void {
  document.body.classList.remove("drawer-open");
}

function getElement(id: string): HTMLElement {
  const node = document.getElementById(id);
  if (!node) throw new Error(`missing #${id}`);
  return node;
}

function render(): void {
  const { locale, slug } = currentRoute();
  const strings = STRINGS[locale];
  const page = bySlug[locale].get(slug);
  if (!page) return;
  document.documentElement.lang = locale === "cn" ? "zh-CN" : locale;
  document.title = `${page.title || slug} · ${strings.title}`;
  const app = getElement("app");
  const dark = theme() === "dark";
  app.innerHTML = `
    <header class="theme-header">
      <div class="mx-auto flex max-w-6xl items-center gap-3 px-6 py-3">
        <button type="button" id="nav-toggle" class="theme-btn md:hidden" aria-label="${escapeHtml(strings.menu)}">${ICON_BURGER}</button>
        <span class="theme-badge inline-flex h-8 w-8 items-center justify-center rounded-md text-lg font-bold">F</span>
        <a href="#/${locale}/overview" class="theme-title text-base font-semibold tracking-tight no-underline">Ferrite</a>
        <span class="theme-pill rounded px-2 py-0.5 text-xs">${escapeHtml(strings.docs)}</span>
        <span class="theme-tagline ml-auto hidden text-xs sm:inline">${escapeHtml(strings.tagline)}</span>
        <select id="locale-select" class="theme-select" aria-label="${escapeHtml(strings.language)}">
          ${LOCALES.map(
            (code) =>
              `<option value="${code}"${code === locale ? " selected" : ""}>${LOCALE_NAMES[code]}</option>`,
          ).join("")}
        </select>
        <button type="button" id="theme-toggle" class="theme-btn" aria-pressed="${String(dark)}" title="${escapeHtml(strings.theme)}">
          ${dark ? ICON_MOON : ICON_SUN}<span class="theme-btn-label">${dark ? escapeHtml(strings.dark) : escapeHtml(strings.light)}</span>
        </button>
      </div>
    </header>
    <div class="mx-auto flex w-full max-w-6xl flex-1 gap-8 px-6 py-8">
      <aside class="hidden w-56 shrink-0 md:block">
        <nav aria-label="${escapeHtml(strings.pages)}" class="sticky top-8 space-y-0.5">${sidebar(locale, slug)}</nav>
      </aside>
      <main class="min-w-0 flex-1">
        <article class="docs-body">${page.html}</article>
        ${pageNav(locale, slug, strings)}
        <footer class="theme-footer theme-border mt-12 border-t pt-4 text-xs">
          Ferrite ${escapeHtml(slug)} · ${escapeHtml(strings.renderedFrom)}
        </footer>
      </main>
      <aside class="hidden w-52 shrink-0 lg:block">
        <div class="sticky top-8">${toc(page, strings)}</div>
      </aside>
    </div>
    <div class="drawer-backdrop md:hidden" id="drawer-backdrop"></div>
    <div class="drawer md:hidden" id="mobile-drawer" role="dialog" aria-label="${escapeHtml(strings.pages)}">
      <div class="flex items-center justify-between pb-3">
        <span class="text-sm font-semibold">${escapeHtml(strings.pages)}</span>
        <button type="button" id="drawer-close" class="theme-btn" aria-label="${escapeHtml(strings.close)}">${ICON_CLOSE}</button>
      </div>
      <nav class="space-y-0.5">${sidebar(locale, slug)}</nav>
    </div>`;
  rewriteRelativeLinks(app, locale);
  const article = app.querySelector("article");
  if (article) addCopyButtons(article, strings);
  getElement("locale-select").addEventListener("change", (event) => {
    const target = event.target;
    if (target instanceof HTMLSelectElement && isLocale(target.value)) {
      setLocale(target.value);
    }
  });
  getElement("theme-toggle").addEventListener("click", () => {
    setTheme(theme() === "dark" ? "light" : "dark");
  });
  getElement("nav-toggle").addEventListener("click", () => {
    document.body.classList.add("drawer-open");
  });
  getElement("drawer-close").addEventListener("click", closeDrawer);
  getElement("drawer-backdrop").addEventListener("click", closeDrawer);
  getElement("mobile-drawer")
    .querySelectorAll("a[href]")
    .forEach((link) => link.addEventListener("click", closeDrawer));
}

window.addEventListener("keydown", (event) => {
  if (event.key === "Escape") closeDrawer();
});

window.addEventListener("hashchange", () => {
  // Anchor jumps inside a page (TOC) must not re-render.
  if (window.location.hash.startsWith("#/") || window.location.hash === "") {
    closeDrawer();
    render();
    window.scrollTo(0, 0);
  }
});

if (!window.location.hash) {
  window.location.hash = `#/${storedLocale()}/overview`;
}
render();
