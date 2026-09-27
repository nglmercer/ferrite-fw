import type { CatalogItem, Category } from "../catalog.ts";
import { categoryDefinitions } from "../catalog.ts";
import { formatHashRoute } from "../hooks/useHashRoute.ts";
import { Icon } from "../components/ui/Icon.tsx";
import { cn } from "../lib/cn.ts";

interface ComponentSidebarProps {
  readonly category: Category;
  readonly items: readonly CatalogItem[];
  readonly activeSlug?: string;
  readonly searchQuery: string;
  readonly onNavigate: (item: CatalogItem) => void;
}

function categoryLabel(category: Category): string {
  return categoryDefinitions.find(item => item.value === category)?.label ?? category;
}

export function ComponentSidebar({ category, items, activeSlug, searchQuery, onNavigate }: ComponentSidebarProps) {
  const title = searchQuery ? "Search results" : categoryLabel(category);
  return (
    <aside className="component-sidebar" aria-label="Component navigation">
      <div className="component-sidebar-heading">
        <div>
          <span className="eyebrow">Browse</span>
          <h2>Components</h2>
        </div>
        <span className="component-sidebar-count">{items.length}</span>
      </div>
      <label className="component-mobile-picker">
        <span className="sr-only">Choose a component</span>
        <select
          value={activeSlug ?? ""}
          onChange={event => {
            const item = items.find(candidate => candidate.slug === event.currentTarget.value);
            if (item) onNavigate(item);
          }}
        >
          {items.length === 0 ? <option value="">No components</option> : null}
          {items.map(item => <option key={item.slug} value={item.slug}>{item.name}</option>)}
        </select>
      </label>
      <p className="component-sidebar-context">{title}</p>
      {items.length > 0 ? (
        <nav>
          <ul className="component-sidebar-list">
            {items.map(item => (
              <li key={item.slug}>
                <a
                  className={cn("component-sidebar-link", item.slug === activeSlug && "component-sidebar-link-active")}
                  href={formatHashRoute(item.category, item.slug)}
                  aria-current={item.slug === activeSlug ? "page" : undefined}
                  onClick={event => {
                    event.preventDefault();
                    onNavigate(item);
                  }}
                >
                  <span>{item.name}</span>
                  <Icon name="chevron-right" className="h-3.5 w-3.5" />
                </a>
              </li>
            ))}
          </ul>
        </nav>
      ) : (
        <div className="component-sidebar-empty">
          <Icon name="spark" className="h-5 w-5" />
          <p>No matching components yet.</p>
        </div>
      )}
    </aside>
  );
}
