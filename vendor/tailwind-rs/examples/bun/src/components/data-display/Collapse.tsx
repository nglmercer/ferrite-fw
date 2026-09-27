import { useState } from "preact/hooks";
import { Icon } from "../ui/Icon.tsx";
import { cn } from "../../lib/cn.ts";

/** Single collapsible panel with an explicit open/close affordance. */
export function CollapseDemo() {
  const [open, setOpen] = useState(true);
  return (
    <div className="fold-panel">
      <button
        type="button"
        id="fold-toggle"
        aria-expanded={open}
        aria-controls="fold-panel"
        onClick={() => setOpen(value => !value)}
        className="fold-toggle"
      >
        <Icon name="info" className="h-5 w-5 text-brand-600" />
        <span>When should I use collapse instead of accordion?</span>
        <Icon name="chevron-down" className={cn("fold-chevron", open && "fold-chevron-open")} />
      </button>
      {open ? (
        <div id="fold-panel" role="region" aria-labelledby="fold-toggle" className="fold-body">
          <p>Use collapse for a single standalone disclosure — shipping details, advanced settings, or one FAQ answer. Reach for accordion when several related panels share one container.</p>
        </div>
      ) : null}
    </div>
  );
}
