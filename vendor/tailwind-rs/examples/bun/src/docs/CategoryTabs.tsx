import type { Category } from "../catalog.ts";
import { categoryDefinitions } from "../catalog.ts";
import { Tabs } from "../components/ui/Tabs.tsx";

interface CategoryTabsProps {
  readonly activeCategory: Category;
  readonly onChange: (category: Category) => void;
}

export function CategoryTabs({ activeCategory, onChange }: CategoryTabsProps) {
  return (
    <div className="category-tabs-wrap">
      <Tabs
        idPrefix="category"
        value={activeCategory}
        onChange={value => onChange(value as Category)}
        items={categoryDefinitions.map(category => ({ value: category.value, label: category.label }))}
        variant="line"
        size="sm"
        ariaLabel="Component categories"
      />
    </div>
  );
}
