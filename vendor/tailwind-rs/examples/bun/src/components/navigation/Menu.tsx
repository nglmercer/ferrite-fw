import { Icon, type IconName } from "../ui/Icon.tsx";
import { cn } from "../../lib/cn.ts";

interface MenuEntry {
  readonly label: string;
  readonly icon: IconName;
  readonly href?: string;
  readonly active?: boolean;
  readonly disabled?: boolean;
  readonly badge?: string;
}

const sections: readonly { title: string; entries: readonly MenuEntry[] }[] = [
  {
    title: "Workspace",
    entries: [
      { label: "Dashboard", icon: "home", href: "#/data-display/stat", active: true },
      { label: "Projects", icon: "dots", href: "#/data-display/table", badge: "12" },
      { label: "Calendar", icon: "calendar", href: "#/data-input/calendar" }
    ]
  },
  {
    title: "Account",
    entries: [
      { label: "Profile", icon: "user", href: "#/data-display/avatar" },
      { label: "Billing", icon: "cart", disabled: true },
      { label: "Settings", icon: "settings", href: "#/actions/theme-controller" }
    ]
  }
];

/** Sectioned vertical menu with active, badge, and disabled states. */
export function MenuDemo() {
  return (
    <nav className="menu" aria-label="Workspace menu">
      {sections.map(section => (
        <div key={section.title}>
          <p className="menu-title">{section.title}</p>
          <ul className="menu-list">
            {section.entries.map(entry => (
              <li key={entry.label}>
                {entry.disabled || !entry.href ? (
                  <span aria-disabled="true" className={cn("menu-item", entry.active && "menu-item-active", "menu-item-disabled")}>
                    <Icon name={entry.icon} className="h-4 w-4" />
                    <span>{entry.label}</span>
                    {entry.badge ? <span className="menu-badge">{entry.badge}</span> : null}
                  </span>
                ) : (
                  <a
                    href={entry.href}
                    aria-current={entry.active ? "page" : undefined}
                    className={cn("menu-item", entry.active && "menu-item-active")}
                  >
                    <Icon name={entry.icon} className="h-4 w-4" />
                    <span>{entry.label}</span>
                    {entry.badge ? <span className="menu-badge">{entry.badge}</span> : null}
                  </a>
                )}
              </li>
            ))}
          </ul>
        </div>
      ))}
    </nav>
  );
}
