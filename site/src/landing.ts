/// Landing page: localized strings + DOM renderer (no Markdown).
/// Route `#/{locale}/home`; also the default when the hash is empty.

import { el } from "./dom";
import { slugOf } from "./components";
import type { DocsPage, Locale } from "./docs-types";

export interface LandingFeature {
  title: string;
  body: string;
  slug: string;
}

export interface LandingStep {
  title: string;
  code: string;
}

export interface LandingStrings {
  kicker: string;
  headline: string;
  lede: string;
  primary: string;
  secondary: string;
  terminal: string;
  terminalCode: string;
  specs: Array<{ label: string; value: string }>;
  featuresTitle: string;
  features: LandingFeature[];
  quickstartTitle: string;
  steps: LandingStep[];
  tracksTitle: string;
  trackNames: string[];
  ctaText: string;
  footer: string;
}

/// Track slug groups (locale-independent); names come from strings.
const TRACKS: string[][] = [
  ["dev-server", "css", "assets"],
  ["npm"],
  ["ssr", "runtime", "frameworks", "wasm"],
  ["cli", "configuration", "pipeline"],
  ["plugins", "api"],
  ["production", "troubleshooting"],
  ["tailwind-vendor"],
];

const TERMINAL_CODE =
  "$ cargo install --path crates/ferrite-cli\n" +
  "$ ferrite create my-app && cd my-app\n" +
  "$ ferrite dev\n" +
  "  ➜ Local: http://127.0.0.1:5173/";

const STEP_CODE = [
  "cargo install --path crates/ferrite-cli",
  "ferrite create my-app\ncd my-app\nferrite dev",
  "ferrite build --standalone",
];

export const LANDING_STRINGS: Record<Locale, LandingStrings> = {
  en: {
    kicker: "rust-native · no node.js required",
    headline: "One binary. Dev to production.",
    lede: "Ferrite is a Vite-like toolchain written in Rust: a dev server with HMR, an Oxc-powered pipeline, SSR, and production builds that pack into a single binary.",
    primary: "Read the docs",
    secondary: "Quickstart",
    terminal: "terminal",
    terminalCode: TERMINAL_CODE,
    specs: [
      { label: "engine", value: "oxc" },
      { label: "dev", value: "native esm + hmr" },
      { label: "output", value: "single binary" },
      { label: "node.js", value: "not required" },
    ],
    featuresTitle: "What you get",
    features: [
      {
        title: "Instant dev loop",
        body: "Native ESM per module, HMR over WebSocket, and React Refresh in dev — no bundling between you and the browser.",
        slug: "dev-server",
      },
      {
        title: "Builds that pack small",
        body: "Hashed chunks or scope-hoisted output, statement-level tree-shaking, CSS extraction, and chained source maps.",
        slug: "pipeline",
      },
      {
        title: "SSR without the ceremony",
        body: "Adapters, streaming, islands, and typed RPC over JSON, MessagePack, or CBOR — pure Rust or embedded JS.",
        slug: "ssr",
      },
      {
        title: "Yours to extend",
        body: "A Rust plugin API with Vite-like hooks, plus tier-2 JS plugins and Markdown modules like this very site.",
        slug: "plugins",
      },
    ],
    quickstartTitle: "Quickstart",
    steps: [
      { title: "Install the CLI", code: STEP_CODE[0] ?? "" },
      { title: "Create and run", code: STEP_CODE[1] ?? "" },
      { title: "Ship one binary", code: STEP_CODE[2] ?? "" },
    ],
    tracksTitle: "Browse the docs",
    trackNames: ["Develop", "Depend", "Render", "Build", "Extend", "Ship", "Vendored"],
    ctaText: "New here? The overview takes five minutes.",
    footer: "Ferrite docs · built with itself",
  },
  es: {
    kicker: "nativo rust · sin node.js",
    headline: "Un binario. De dev a producción.",
    lede: "Ferrite es una toolchain estilo Vite escrita en Rust: servidor dev con HMR, pipeline con Oxc, SSR y builds de producción que se empaquetan en un solo binario.",
    primary: "Leer los docs",
    secondary: "Inicio rápido",
    terminal: "terminal",
    terminalCode: TERMINAL_CODE,
    specs: [
      { label: "motor", value: "oxc" },
      { label: "dev", value: "esm nativo + hmr" },
      { label: "salida", value: "binario único" },
      { label: "node.js", value: "no requerido" },
    ],
    featuresTitle: "Qué obtienes",
    features: [
      {
        title: "Ciclo dev instantáneo",
        body: "ESM nativo por módulo, HMR por WebSocket y React Refresh en dev — sin bundle entre tú y el navegador.",
        slug: "dev-server",
      },
      {
        title: "Builds compactos",
        body: "Chunks hasheados o salida scope-hoisted, tree-shaking por sentencia, extracción de CSS y source maps encadenados.",
        slug: "pipeline",
      },
      {
        title: "SSR sin ceremonia",
        body: "Adaptadores, streaming, islands y RPC tipado sobre JSON, MessagePack o CBOR — Rust puro o JS embebido.",
        slug: "ssr",
      },
      {
        title: "Hecho para extender",
        body: "API de plugins en Rust con hooks estilo Vite, más plugins JS tier-2 y módulos Markdown como este sitio.",
        slug: "plugins",
      },
    ],
    quickstartTitle: "Inicio rápido",
    steps: [
      { title: "Instala el CLI", code: STEP_CODE[0] ?? "" },
      { title: "Crea y corre", code: STEP_CODE[1] ?? "" },
      { title: "Distribuye un binario", code: STEP_CODE[2] ?? "" },
    ],
    tracksTitle: "Explora los docs",
    trackNames: ["Develop", "Depend", "Render", "Build", "Extender", "Ship", "Vendored"],
    ctaText: "¿Nuevo aquí? El resumen toma cinco minutos.",
    footer: "Docs de Ferrite · construidos con Ferrite",
  },
  cn: {
    kicker: "rust 原生 · 无需 node.js",
    headline: "一个二进制，从开发到生产。",
    lede: "Ferrite 是 Rust 写的 Vite 风格工具链：带 HMR 的开发服务器、Oxc 驱动的流水线、SSR，以及打包成单个二进制的生产构建。",
    primary: "阅读文档",
    secondary: "快速上手",
    terminal: "终端",
    terminalCode: TERMINAL_CODE,
    specs: [
      { label: "引擎", value: "oxc" },
      { label: "开发", value: "原生 esm + hmr" },
      { label: "产物", value: "单个二进制" },
      { label: "node.js", value: "不需要" },
    ],
    featuresTitle: "你能得到",
    features: [
      {
        title: "即时开发循环",
        body: "按模块原生 ESM、WebSocket HMR、开发环境 React Refresh——你与浏览器之间没有打包步骤。",
        slug: "dev-server",
      },
      {
        title: "小而美的构建",
        body: "哈希 chunk 或 scope-hoisted 输出、语句级 tree-shaking、CSS 抽取、串联的 source map。",
        slug: "pipeline",
      },
      {
        title: "SSR 毫不繁琐",
        body: "适配器、流式、islands，以及 JSON / MessagePack / CBOR 上的类型化 RPC——纯 Rust 或内嵌 JS。",
        slug: "ssr",
      },
      {
        title: "为扩展而生",
        body: "Rust 插件 API（Vite 风格 hooks），加 tier-2 JS 插件与 Markdown 模块——本站即用它构建。",
        slug: "plugins",
      },
    ],
    quickstartTitle: "快速上手",
    steps: [
      { title: "安装 CLI", code: STEP_CODE[0] ?? "" },
      { title: "创建并运行", code: STEP_CODE[1] ?? "" },
      { title: "发布单个二进制", code: STEP_CODE[2] ?? "" },
    ],
    tracksTitle: "浏览文档",
    trackNames: ["开发", "依赖", "渲染", "构建", "扩展", "发布", "Vendored"],
    ctaText: "初次接触？概述只需五分钟。",
    footer: "Ferrite 文档 · 自举构建",
  },
};

function codeBlock(code: string): HTMLElement {
  return el("pre", { class: "landing-pre" }, el("code", {}, code));
}

/// Full landing `<main>` for `locale`; page titles resolve track links.
export function renderLanding(locale: Locale, pages: DocsPage[]): HTMLElement {
  const s = LANDING_STRINGS[locale];
  const titles = new Map(pages.map((p): [string, string] => [slugOf(p.path), p.title || slugOf(p.path)]));
  const titleOf = (slug: string): string => titles.get(slug) ?? slug;
  const hrefOf = (slug: string): string => `#/${locale}/${slug}`;

  const hero = el(
    "section",
    { class: "landing-hero" },
    el(
      "div",
      { class: "landing-hero-text" },
      el("p", { class: "landing-kicker" }, s.kicker),
      el("h1", { class: "landing-headline" }, s.headline),
      el("p", { class: "landing-lede" }, s.lede),
      el(
        "p",
        { class: "landing-ctas" },
        el("a", { href: hrefOf("overview"), class: "landing-btn landing-btn-primary" }, s.primary),
        el("a", { href: "#quickstart", class: "landing-btn landing-btn-secondary" }, `${s.secondary} ↓`),
      ),
    ),
    el(
      "div",
      { class: "landing-terminal", role: "region", "aria-label": s.terminal },
      el(
        "div",
        { class: "landing-terminal-bar" },
        el("span", { class: "landing-terminal-dot" }),
        el("span", { class: "landing-terminal-dot" }),
        el("span", { class: "landing-terminal-dot" }),
        el("span", { class: "landing-terminal-title" }, s.terminal),
      ),
      codeBlock(s.terminalCode),
    ),
  );

  const specs = el(
    "dl",
    { class: "landing-specs" },
    ...s.specs.map((spec) =>
      el("div", { class: "landing-spec" }, el("dt", {}, spec.label), el("dd", {}, spec.value)),
    ),
  );

  const features = el(
    "section",
    { class: "landing-section", "aria-label": s.featuresTitle },
    el("h2", { class: "landing-h2" }, s.featuresTitle),
    el(
      "ol",
      { class: "landing-features" },
      ...s.features.map((feature, index) =>
        el(
          "li",
          { class: "landing-feature" },
          el("span", { class: "landing-feature-num" }, `0${index + 1}`),
          el("h3", { class: "landing-feature-title" }, feature.title),
          el("p", { class: "landing-feature-body" }, feature.body),
          el("a", { href: hrefOf(feature.slug), class: "landing-feature-link" }, `${titleOf(feature.slug)} →`),
        ),
      ),
    ),
  );

  const quickstart = el(
    "section",
    { class: "landing-section", id: "quickstart", "aria-label": s.quickstartTitle },
    el("h2", { class: "landing-h2" }, s.quickstartTitle),
    el(
      "ol",
      { class: "landing-steps" },
      ...s.steps.map((step, index) =>
        el(
          "li",
          { class: "landing-step" },
          el("span", { class: "landing-step-num" }, String(index + 1)),
          el(
            "div",
            { class: "landing-step-body" },
            el("h3", { class: "landing-step-title" }, step.title),
            codeBlock(step.code),
          ),
        ),
      ),
    ),
  );

  const tracks = el(
    "section",
    { class: "landing-section", "aria-label": s.tracksTitle },
    el("h2", { class: "landing-h2" }, s.tracksTitle),
    el(
      "div",
      { class: "landing-tracks" },
      ...TRACKS.map((slugs, index) =>
        el(
          "div",
          { class: "landing-track" },
          el("h3", { class: "landing-track-name" }, s.trackNames[index] ?? ""),
          el(
            "ul",
            { class: "landing-track-links" },
            ...slugs.map((slug) =>
              el("li", {}, el("a", { href: hrefOf(slug) }, titleOf(slug))),
            ),
          ),
        ),
      ),
    ),
  );

  const closing = el(
    "section",
    { class: "landing-closing" },
    el("p", { class: "landing-closing-text" }, s.ctaText),
    el("a", { href: hrefOf("overview"), class: "landing-btn landing-btn-primary" }, s.primary),
  );

  return el(
    "main",
    { class: "landing" },
    hero,
    specs,
    features,
    quickstart,
    tracks,
    closing,
    el("footer", { class: "landing-footer" }, s.footer),
  );
}
