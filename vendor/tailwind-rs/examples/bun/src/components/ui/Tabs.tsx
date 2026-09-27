import type { ComponentChildren } from "preact";
import { useRef } from "preact/hooks";
import { cn } from "../../lib/cn.ts";

export interface TabItem {
  readonly value: string;
  readonly label: ComponentChildren;
  readonly disabled?: boolean;
}

export type TabsVariant = "line" | "box" | "border" | "lift";
export type TabsSize = "sm" | "md" | "lg";

export interface TabsProps {
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly items: readonly TabItem[];
  readonly variant?: TabsVariant;
  readonly size?: TabsSize;
  readonly idPrefix?: string;
  readonly ariaLabel?: string;
}

const variantClasses: Record<TabsVariant, string> = {
  line: "tabs tabs-line",
  box: "tabs tabs-box",
  border: "tabs tabs-border",
  lift: "tabs tabs-lift"
};

const sizeClasses: Record<TabsSize, string> = {
  sm: "tabs-sm",
  md: "tabs-md",
  lg: "tabs-lg"
};

function safeId(value: string): string {
  return value.replace(/[^a-zA-Z0-9_-]/g, "-");
}

/** Accessible controlled tabs with arrow-key navigation and optional disabled items. */
export function Tabs({ value, onChange, items, variant = "line", size = "md", idPrefix = "tabs", ariaLabel = "Tabs" }: TabsProps) {
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const activeIndex = Math.max(0, items.findIndex(item => item.value === value));
  const prefix = safeId(idPrefix);

  function moveFocus(index: number) {
    const enabledIndexes = items.map((item, itemIndex) => item.disabled ? -1 : itemIndex).filter(itemIndex => itemIndex >= 0);
    if (enabledIndexes.length === 0) return;

    const currentPosition = Math.max(0, enabledIndexes.indexOf(activeIndex));
    const nextPosition = (currentPosition + index + enabledIndexes.length) % enabledIndexes.length;
    const nextIndex = enabledIndexes[nextPosition];
    const nextItem = items[nextIndex];
    onChange(nextItem.value);
    tabRefs.current[nextIndex]?.focus();
  }

  function focusBoundary(last: boolean) {
    let nextIndex = -1;
    if (last) {
      for (let index = items.length - 1; index >= 0; index -= 1) {
        if (!items[index].disabled) {
          nextIndex = index;
          break;
        }
      }
    } else {
      nextIndex = items.findIndex(item => !item.disabled);
    }
    if (nextIndex < 0) return;
    onChange(items[nextIndex].value);
    tabRefs.current[nextIndex]?.focus();
  }

  return (
    <div className={cn(variantClasses[variant], sizeClasses[size])} role="tablist" aria-label={ariaLabel}>
      {items.map((item, index) => {
        const selected = item.value === value;
        const id = `${prefix}-tab-${safeId(item.value)}`;
        const panelId = `${prefix}-panel-${safeId(item.value)}`;
        return (
          <button
            ref={element => { tabRefs.current[index] = element; }}
            key={item.value}
            id={id}
            type="button"
            role="tab"
            aria-selected={selected}
            aria-controls={panelId}
            aria-disabled={item.disabled || undefined}
            tabIndex={selected ? 0 : -1}
            disabled={item.disabled}
            className={cn("tab", selected && "tab-active")}
            onClick={() => onChange(item.value)}
            onKeyDown={event => {
              if (event.key === "ArrowRight" || event.key === "ArrowDown") {
                event.preventDefault();
                moveFocus(1);
              } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
                event.preventDefault();
                moveFocus(-1);
              } else if (event.key === "Home") {
                event.preventDefault();
                focusBoundary(false);
              } else if (event.key === "End") {
                event.preventDefault();
                focusBoundary(true);
              }
            }}
          >
            {item.label}
          </button>
        );
      })}
    </div>
  );
}
