/// Shared types for the Ferrite docs site.

/// Supported documentation locales.
export type Locale = "en" | "es" | "cn";

/// One Markdown heading (`{ level, text, id }`, see `ferrite-docs`).
export interface DocsHeading {
  level: number;
  id: string;
  text: string;
}

/// A rendered Markdown page module (default export of `*.md`).
export interface DocsPage {
  path: string;
  title: string | null;
  order: number | null;
  headings: DocsHeading[];
  html: string;
}

/// Localized UI strings.
export interface DocsStrings {
  docs: string;
  tagline: string;
  onThisPage: string;
  pages: string;
  renderedFrom: string;
  theme: string;
  light: string;
  dark: string;
  language: string;
  copy: string;
  copied: string;
  previous: string;
  next: string;
  menu: string;
  close: string;
  title: string;
  home: string;
  docsLink: string;
}
