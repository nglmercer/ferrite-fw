import type { CatalogItem, Category } from "../catalog.ts";
import { categoryDefinitions } from "../catalog.ts";
import { DemoFrame } from "./DemoFrame.tsx";

interface ComponentPageProps {
  readonly item?: CatalogItem;
  readonly category: Category;
}

function categoryLabel(category: Category): string {
  return categoryDefinitions.find(candidate => candidate.value === category)?.label ?? category;
}

export function ComponentPage({ item, category }: ComponentPageProps) {
  if (!item) {
    return (
      <section className="empty-category component-card" aria-labelledby="empty-category-title">
        <span className="eyebrow">{categoryLabel(category)}</span>
        <h1 id="empty-category-title">More components are on the way.</h1>
        <p>This category is reserved for the next catalog expansion. The explorer is ready to add statically imported demos here.</p>
      </section>
    );
  }

  const Demo = item.demo;
  return (
    <div className="component-page">
      <header className="component-page-header">
        <span className="eyebrow">{categoryLabel(item.category)}</span>
        <h1>{item.name}</h1>
        <p>{item.description}</p>
      </header>
      <DemoFrame
        title={`${item.name} preview`}
        description="Interactive examples run in Preact; the styles are emitted by the Rust compiler."
        code={item.code ?? `<${item.name}>Example</${item.name}>`}
        css={item.css}
      >
        <Demo />
      </DemoFrame>
    </div>
  );
}
