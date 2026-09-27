import { useState } from "preact/hooks";
import { Icon, type IconName } from "../ui/Icon.tsx";
import { cn } from "../../lib/cn.ts";

interface FabAction {
  readonly label: string;
  readonly icon: IconName;
}

const speedDialActions: readonly FabAction[] = [
  { label: "New document", icon: "plus" },
  { label: "Send message", icon: "send" },
  { label: "Download report", icon: "download" }
];

/** Floating action button with an expandable speed-dial, kept inside a demo stage. */
export function FabDemo() {
  const [open, setOpen] = useState(false);
  const [notice, setNotice] = useState("Pick an action.");
  return (
    <div>
      <div className="fab-stage" aria-label="Floating action button demo stage">
        {open ? (
          <div className="fab-actions" role="menu" aria-label="Quick actions">
            {speedDialActions.map(action => (
              <button
                key={action.label}
                type="button"
                role="menuitem"
                className="fab-action"
                onClick={() => { setNotice(`${action.label} selected.`); setOpen(false); }}
              >
                <span className="fab-action-label">{action.label}</span>
                <span className="fab-action-icon"><Icon name={action.icon} className="h-4 w-4" /></span>
              </button>
            ))}
          </div>
        ) : null}
        <button
          type="button"
          className={cn("fab", open && "fab-open")}
          aria-expanded={open}
          aria-label={open ? "Close quick actions" : "Open quick actions"}
          onClick={() => setOpen(value => !value)}
        >
          <Icon name={open ? "close" : "plus"} className="h-5 w-5" />
        </button>
      </div>
      <p className="mt-3 text-sm text-gray-600" aria-live="polite">{notice}</p>
    </div>
  );
}
