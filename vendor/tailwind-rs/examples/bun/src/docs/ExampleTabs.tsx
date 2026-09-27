import type { ComponentChildren } from "preact";
import { useState } from "preact/hooks";

import { Tabs } from "../components/ui/Tabs.tsx";

export interface ExampleTab {
  readonly value: string;
  readonly label: string;
  readonly content: ComponentChildren;
}

interface ExampleTabsProps {
  readonly id: string;
  readonly items: readonly ExampleTab[];
}

/** Small page-level tab set for variants, sizes, states, and related examples. */
export function ExampleTabs({ id, items }: ExampleTabsProps) {
  const [active, setActive] = useState(items[0]?.value ?? "");
  const activeItem = items.find(item => item.value === active) ?? items[0];

  if (!activeItem) return null;

  return (
    <div className="example-tabs">
      <Tabs
        idPrefix={id}
        value={activeItem.value}
        onChange={setActive}
        items={items}
        variant="border"
        ariaLabel={`${id} examples`}
      />
      <div
        className="example-tab-panel"
        id={`${id}-panel-${activeItem.value}`}
        role="tabpanel"
        aria-labelledby={`${id}-tab-${activeItem.value}`}
        tabIndex={0}
      >
        {activeItem.content}
      </div>
    </div>
  );
}
