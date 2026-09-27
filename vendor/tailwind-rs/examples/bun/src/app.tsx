import { render } from "preact";
import { useState } from "preact/hooks";

import { catalog, categoryDefinitions, type CatalogItem, type Category } from "./catalog.ts";
import { AppShell } from "./docs/AppShell.tsx";
import { ComponentPage } from "./docs/ComponentPage.tsx";
import { ComponentSidebar } from "./docs/ComponentSidebar.tsx";
import { useHashRoute } from "./hooks/useHashRoute.ts";

function findItem(category: Category | undefined, slug: string | undefined): CatalogItem | undefined {
  if (slug) {
    const item = catalog.find(candidate => candidate.slug === slug);
    if (item) return item;
  }
  return category ? catalog.find(candidate => candidate.category === category) : catalog[0];
}

function App() {
  const { route, navigate } = useHashRoute({ category: "actions", slug: "button" });
  const [searchQuery, setSearchQuery] = useState("");
  const selectedItem = findItem(route.category, route.slug);
  const activeCategory = selectedItem?.category ?? route.category ?? categoryDefinitions[0].value;
  const normalizedQuery = searchQuery.trim().toLowerCase();
  const visibleItems = normalizedQuery
    ? catalog.filter(item => `${item.name} ${item.slug} ${item.description}`.toLowerCase().includes(normalizedQuery))
    : catalog.filter(item => item.category === activeCategory);

  function selectCategory(category: Category) {
    setSearchQuery("");
    navigate(category, catalog.find(item => item.category === category)?.slug);
  }

  function selectItem(item: CatalogItem) {
    setSearchQuery("");
    navigate(item.category, item.slug);
  }

  return (
    <AppShell
      activeCategory={activeCategory}
      searchQuery={searchQuery}
      searchResultCount={visibleItems.length}
      onCategoryChange={selectCategory}
      onSearchChange={setSearchQuery}
    >
      <main className="page-width docs-layout">
        <ComponentSidebar
          category={activeCategory}
          items={visibleItems}
          activeSlug={selectedItem?.slug}
          searchQuery={normalizedQuery}
          onNavigate={selectItem}
        />
        <section id="docs-main" className="docs-main" tabIndex={-1} aria-live="polite">
          <ComponentPage item={selectedItem} category={activeCategory} />
        </section>
      </main>
    </AppShell>
  );
}

const root = document.querySelector<HTMLElement>("#app");
if (!root) throw new Error("Bun example is missing the #app mount point.");
render(<App />, root);
