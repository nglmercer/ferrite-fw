import { useState } from "preact/hooks";
import { cn } from "../../lib/cn.ts";

type FilterValue = "all" | "open" | "closed";

const options: readonly { value: FilterValue; label: string }[] = [
  { value: "all", label: "All" },
  { value: "open", label: "Open" },
  { value: "closed", label: "Closed" }
];

const issues = [
  { title: "Tabs lose focus order", status: "open" as const },
  { title: "Drawer traps scroll", status: "open" as const },
  { title: "Banner dismiss restores", status: "closed" as const },
  { title: "Stat delta tone", status: "closed" as const }
];

/** Chip-style radio filter driving a small result list. */
export function FilterDemo() {
  const [filter, setFilter] = useState<FilterValue>("all");
  const visible = issues.filter(issue => filter === "all" || issue.status === filter);
  return (
    <div className="space-y-4">
      <div className="filter-chips" role="radiogroup" aria-label="Issue status">
        {options.map(option => (
          <label key={option.value} className={cn("filter-chip", filter === option.value && "filter-chip-checked")}>
            <input
              className="sr-only"
              type="radio"
              name="issue-filter"
              value={option.value}
              checked={filter === option.value}
              onChange={() => setFilter(option.value)}
            />
            {option.label}
          </label>
        ))}
      </div>
      <ul className="filter-results">
        {visible.map(issue => (
          <li key={issue.title}>
            <span>{issue.title}</span>
            <span className={cn("filter-pill", issue.status === "open" ? "filter-pill-open" : "filter-pill-closed")}>{issue.status}</span>
          </li>
        ))}
      </ul>
      <p className="text-sm text-gray-600" aria-live="polite">{visible.length} of {issues.length} issues shown.</p>
    </div>
  );
}
