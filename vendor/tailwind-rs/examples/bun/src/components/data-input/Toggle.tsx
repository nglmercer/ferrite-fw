import { useState } from "preact/hooks";
import { cn } from "../../lib/cn.ts";

interface ToggleRow {
  readonly label: string;
  readonly hint: string;
}

const rows: readonly ToggleRow[] = [
  { label: "Enable notifications", hint: "Build and deploy alerts" },
  { label: "Weekly digest", hint: "Every Monday morning" }
];

/** Switch toggles for on/off preferences. */
export function ToggleDemo() {
  const [enabled, setEnabled] = useState<readonly boolean[]>([true, false]);
  function flip(index: number) {
    setEnabled(current => current.map((value, position) => position === index ? !value : value));
  }
  return (
    <div className="space-y-3">
      {rows.map((row, index) => (
        <div key={row.label} className="toggle-row">
          <span>
            <strong>{row.label}</strong>
            <small>{row.hint}</small>
          </span>
          <button
            type="button"
            role="switch"
            aria-checked={enabled[index]}
            aria-label={row.label}
            onClick={() => flip(index)}
            className={cn("toggle", enabled[index] && "toggle-checked")}
          >
            <span className="toggle-knob" />
          </button>
        </div>
      ))}
    </div>
  );
}
