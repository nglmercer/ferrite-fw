import { useState } from "preact/hooks";
import { Icon, type IconName } from "../ui/Icon.tsx";
import { cn } from "../../lib/cn.ts";

const dockItems: readonly { value: string; label: string; icon: IconName; badge?: string }[] = [
  { value: "home", label: "Home", icon: "home" },
  { value: "search", label: "Search", icon: "search" },
  { value: "messages", label: "Messages", icon: "mail", badge: "3" },
  { value: "settings", label: "Settings", icon: "settings" }
];

/** Bottom tab bar for app-like navigation with an active indicator. */
export function DockDemo() {
  const [active, setActive] = useState("home");
  return (
    <div className="dock-frame">
      <p className="dock-preview" aria-live="polite">Current section: <strong>{dockItems.find(item => item.value === active)?.label}</strong></p>
      <nav className="dock" aria-label="Primary">
        {dockItems.map(item => (
          <button
            key={item.value}
            type="button"
            aria-current={active === item.value ? "page" : undefined}
            onClick={() => setActive(item.value)}
            className={cn("dock-item", active === item.value && "dock-item-active")}
          >
            <span className="dock-icon">
              <Icon name={item.icon} className="h-5 w-5" />
              {item.badge ? <span className="dock-badge">{item.badge}</span> : null}
            </span>
            <span>{item.label}</span>
          </button>
        ))}
      </nav>
    </div>
  );
}
