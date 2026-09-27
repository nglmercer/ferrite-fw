import type { ComponentChildren } from "preact";

import type { Category } from "../catalog.ts";
import { catalog } from "../catalog.ts";
import { cn } from "../lib/cn.ts";
import { CategoryTabs } from "./CategoryTabs.tsx";
import { ComponentSearch } from "./ComponentSearch.tsx";

interface AppShellProps {
  readonly activeCategory: Category;
  readonly searchQuery: string;
  readonly searchResultCount: number;
  readonly onCategoryChange: (category: Category) => void;
  readonly onSearchChange: (query: string) => void;
  readonly children: ComponentChildren;
}

export function AppShell({ activeCategory, searchQuery, searchResultCount, onCategoryChange, onSearchChange, children }: AppShellProps) {
  function skipToContent() {
    document.getElementById("docs-main")?.focus();
  }

  return (
    <div className="app-shell">
      <button type="button" className="skip-link" onClick={skipToContent}>Skip to content</button>
      <header className="site-header">
        <div className="page-width docs-header">
          <a className="brand" href="#/actions/button" aria-label="utilitycss component gallery">
            <span className="brand-mark">u</span>
            <span>
              <strong>utilitycss</strong>
              <small>Component gallery</small>
            </span>
          </a>
          <div className="docs-header-actions">
            <ComponentSearch value={searchQuery} onChange={onSearchChange} resultCount={searchResultCount} />
            <a className="docs-header-link" href="https://github.com/nglmercer/tailwind-rs" target="_blank" rel="noreferrer">GitHub</a>
            <span className="docs-header-count">{catalog.length} components</span>
          </div>
        </div>
        <div className="page-width category-nav">
          <CategoryTabs activeCategory={activeCategory} onChange={onCategoryChange} />
        </div>
      </header>
      {children}
      <footer className={cn("site-footer page-width px-4", "xs:tracking-wide 2xl:text-base content-auto")}>
        <span>utilitycss / bun example</span>
        <span>Deterministic CSS · Bun + Preact</span>
      </footer>
    </div>
  );
}
