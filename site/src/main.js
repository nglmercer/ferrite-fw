import "ferrite:tailwind.css";
import "./docs.css";

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

const LOCALES = ["en", "es", "cn"];
const LOCALE_NAMES = { en: "EN", es: "ES", cn: "中文" };
const DEFAULT_LOCALE = "en";

const STRINGS = {
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
    title: "Ferrite 文档 —— Rust 原生 Web 工具链",
  },
};

const PAGES = {
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

const bySlug = {};
for (const locale of LOCALES) {
  bySlug[locale] = new Map(PAGES[locale].map((page) => [slugOf(page.path), page]));
}

function slugOf(path) {
  const base = path.split("/").pop() || "overview";
  return base.replace(/\.md$/, "");
}

function storedLocale() {
  try {
    const saved = window.localStorage.getItem("ferrite-docs-locale");
    if (LOCALES.includes(saved)) return saved;
    const nav = (window.navigator.language || "en").toLowerCase();
    if (nav.startsWith("es")) return "es";
    if (nav.startsWith("zh")) return "cn";
  } catch {
    /* private mode: fall through to default */
  }
  return DEFAULT_LOCALE;
}

function currentRoute() {
  const hash = window.location.hash.replace(/^#\/?/, "");
  const [maybeLocale, maybeSlug] = hash.split("/");
  if (LOCALES.includes(maybeLocale)) {
    const pages = bySlug[maybeLocale];
    const slug = pages.has(maybeSlug) ? maybeSlug : "overview";
    return { locale: maybeLocale, slug };
  }
  // Legacy `#/slug` links resolve against the stored locale.
  const locale = storedLocale();
  const slug = bySlug[locale].has(maybeLocale) ? maybeLocale : "overview";
  return { locale, slug };
}

function setLocale(locale) {
  try {
    window.localStorage.setItem("ferrite-docs-locale", locale);
  } catch {
    /* ignore */
  }
  const { slug } = currentRoute();
  window.location.hash = `#/${locale}/${slug}`;
}

function theme() {
  return document.documentElement.classList.contains("dark") ? "dark" : "light";
}

function setTheme(next) {
  document.documentElement.classList.toggle("dark", next === "dark");
  try {
    window.localStorage.setItem("ferrite-docs-theme", next);
  } catch {
    /* ignore */
  }
  const button = document.getElementById("theme-toggle");
  if (button) {
    button.setAttribute("aria-pressed", String(next === "dark"));
    button.querySelector("span").textContent = themeLabel(next);
  }
}

function themeLabel(next) {
  const { locale } = currentRoute();
  return next === "dark" ? STRINGS[locale].dark : STRINGS[locale].light;
}

const ICON_SUN =
  '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>';
const ICON_MOON =
  '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M21 12.8A9 9 0 1 1 11.2 3 7 7 0 0 0 21 12.8z"/></svg>';

function sidebar(locale, active) {
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

function toc(page, strings) {
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

function escapeHtml(text) {
  return String(text)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function rewriteRelativeLinks(root, locale) {
  for (const anchor of root.querySelectorAll("a[href]")) {
    const href = anchor.getAttribute("href");
    if (!href || href.startsWith("#") || /^[a-z]+:/i.test(href)) continue;
    const target = href.replace(/\.md$/, "").replace(/^\.\//, "");
    if (bySlug[locale].has(target)) anchor.setAttribute("href", `#/${locale}/${target}`);
  }
}

async function copyText(text) {
  if (window.navigator.clipboard && window.isSecureContext !== false) {
    try {
      await window.navigator.clipboard.writeText(text);
      return true;
    } catch {
      /* fall through to the legacy path */
    }
  }
  const area = document.createElement("textarea");
  area.value = text;
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

function addCopyButtons(article, strings) {
  for (const pre of article.querySelectorAll("pre")) {
    const wrapper = document.createElement("div");
    wrapper.className = "codeblock";
    pre.replaceWith(wrapper);
    wrapper.appendChild(pre);
    const button = document.createElement("button");
    button.type = "button";
    button.className = "copy-btn";
    button.textContent = strings.copy;
    button.setAttribute("aria-label", strings.copy);
    let timer = null;
    button.addEventListener("click", async () => {
      const code = pre.querySelector("code");
      const ok = await copyText((code || pre).textContent);
      button.textContent = ok ? strings.copied : strings.copy;
      button.classList.toggle("copied", ok);
      if (timer) window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        button.textContent = strings.copy;
        button.classList.remove("copied");
      }, 1600);
    });
    wrapper.appendChild(button);
  }
}

function render() {
  const { locale, slug } = currentRoute();
  const strings = STRINGS[locale];
  const page = bySlug[locale].get(slug);
  document.documentElement.lang = locale === "cn" ? "zh-CN" : locale;
  document.title = `${page.title || slug} · ${strings.title}`;
  const app = document.getElementById("app");
  const dark = theme() === "dark";
  app.innerHTML = `
    <header class="theme-header">
      <div class="mx-auto flex max-w-6xl items-center gap-3 px-6 py-3">
        <span class="theme-badge inline-flex h-8 w-8 items-center justify-center rounded-md text-lg font-bold">F</span>
        <a href="#/${locale}/overview" class="theme-title text-base font-semibold tracking-tight no-underline">Ferrite</a>
        <span class="theme-pill rounded px-2 py-0.5 text-xs">${escapeHtml(strings.docs)}</span>
        <span class="theme-tagline ml-auto hidden text-xs sm:inline">${escapeHtml(strings.tagline)}</span>
        <div class="flex items-center gap-2" role="group" aria-label="${escapeHtml(strings.language)}">
          ${LOCALES.map(
            (code) =>
              `<button type="button" class="theme-btn" data-locale="${code}" aria-pressed="${String(code === locale)}">${LOCALE_NAMES[code]}</button>`,
          ).join("")}
        </div>
        <button type="button" id="theme-toggle" class="theme-btn" aria-pressed="${String(dark)}" title="${escapeHtml(strings.theme)}">
          ${dark ? ICON_MOON : ICON_SUN}<span>${dark ? escapeHtml(strings.dark) : escapeHtml(strings.light)}</span>
        </button>
      </div>
    </header>
    <div class="mx-auto flex w-full max-w-6xl flex-1 gap-8 px-6 py-8">
      <aside class="hidden w-56 shrink-0 md:block">
        <nav aria-label="${escapeHtml(strings.pages)}" class="sticky top-8 space-y-0.5">${sidebar(locale, slug)}</nav>
      </aside>
      <main class="min-w-0 flex-1">
        <details class="theme-border mb-6 rounded-lg border p-3 md:hidden">
          <summary class="cursor-pointer text-sm font-medium">${escapeHtml(strings.pages)}</summary>
          <nav class="mt-2 space-y-0.5">${sidebar(locale, slug)}</nav>
        </details>
        <article class="docs-body">${page.html}</article>
        <footer class="theme-footer theme-border mt-12 border-t pt-4 text-xs">
          Ferrite ${escapeHtml(slug)} · ${escapeHtml(strings.renderedFrom)}
        </footer>
      </main>
      <aside class="hidden w-52 shrink-0 lg:block">
        <div class="sticky top-8">${toc(page, strings)}</div>
      </aside>
    </div>`;
  rewriteRelativeLinks(app, locale);
  addCopyButtons(app.querySelector("article"), strings);
  for (const button of app.querySelectorAll("[data-locale]")) {
    button.addEventListener("click", () => setLocale(button.getAttribute("data-locale")));
  }
  document.getElementById("theme-toggle").addEventListener("click", () => {
    setTheme(theme() === "dark" ? "light" : "dark");
  });
}

window.addEventListener("hashchange", () => {
  // Anchor jumps inside a page (TOC) must not re-render.
  if (window.location.hash.startsWith("#/") || window.location.hash === "") {
    render();
    window.scrollTo(0, 0);
  }
});

if (!window.location.hash) {
  window.location.hash = `#/${storedLocale()}/overview`;
}
render();
