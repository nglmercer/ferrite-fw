import "ferrite:tailwind.css";
import "./docs.css";

import type { DocsPage, DocsStrings, Locale } from "./docs-types";
import { el, getElement, setTrustedMarkdown } from "./dom";
import {
  enhanceCodeBlocks,
  Header,
  isLocale,
  MobileDrawer,
  PageNav,
  rewriteRelativeLinks,
  SidebarLinks,
  slugOf,
  Toc,
} from "./components";

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
  const slug =
    maybeLocale !== undefined && bySlug[locale].has(maybeLocale) ? maybeLocale : "overview";
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

function closeDrawer(): void {
  document.body.classList.remove("drawer-open");
}

function render(): void {
  const { locale, slug } = currentRoute();
  const strings = STRINGS[locale];
  const page = bySlug[locale].get(slug);
  if (!page) return;
  document.documentElement.lang = locale === "cn" ? "zh-CN" : locale;
  document.title = `${page.title || slug} · ${strings.title}`;
  const dark = theme() === "dark";
  const pages = PAGES[locale];
  const article = el("article", { class: "docs-body" });
  setTrustedMarkdown(article, page.html);
  const [backdrop, drawer] = MobileDrawer({
    locale,
    slug,
    pages,
    strings,
    onClose: closeDrawer,
  });
  const app = getElement("app");
  app.replaceChildren(
    Header({
      locale,
      locales: LOCALES,
      localeNames: LOCALE_NAMES,
      strings,
      dark,
      onLocaleChange: setLocale,
      onThemeToggle: () => setTheme(theme() === "dark" ? "light" : "dark"),
      onOpenDrawer: () => document.body.classList.add("drawer-open"),
    }),
    el(
      "div",
      { class: "mx-auto flex w-full max-w-6xl flex-1 gap-8 px-6 py-8" },
      el(
        "aside",
        { class: "hidden w-56 shrink-0 md:block" },
        el(
          "nav",
          { "aria-label": strings.pages, class: "sticky top-8 space-y-0.5" },
          ...SidebarLinks({ locale, slug, pages }),
        ),
      ),
      el(
        "main",
        { class: "min-w-0 flex-1" },
        article,
        PageNav({ locale, slug, pages, strings }),
        el(
          "footer",
          { class: "theme-footer theme-border mt-12 border-t pt-4 text-xs" },
          `Ferrite ${slug} · ${strings.renderedFrom}`,
        ),
      ),
      el(
        "aside",
        { class: "hidden w-52 shrink-0 lg:block" },
        el("div", { class: "sticky top-8" }, Toc({ page, strings })),
      ),
    ),
    backdrop,
    drawer,
  );
  rewriteRelativeLinks(
    app,
    (target) => bySlug[locale].has(target),
    locale,
  );
  enhanceCodeBlocks(article, strings);
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
