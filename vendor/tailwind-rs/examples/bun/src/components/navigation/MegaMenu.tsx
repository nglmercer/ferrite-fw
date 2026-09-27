import { useState } from "preact/hooks";
import { Icon } from "../ui/Icon.tsx";
import { cn } from "../../lib/cn.ts";

interface MegaLink {
  readonly label: string;
  readonly href: string;
}

const columns: readonly { title: string; links: readonly MegaLink[] }[] = [
  {
    title: "Components",
    links: [
      { label: "Button", href: "#/actions/button" },
      { label: "Modal", href: "#/actions/modal" },
      { label: "Card", href: "#/data-display/card" }
    ]
  },
  {
    title: "Patterns",
    links: [
      { label: "Forms", href: "#/data-input/input" },
      { label: "Menus", href: "#/navigation/menu" },
      { label: "Tables", href: "#/data-display/table" }
    ]
  },
  {
    title: "Tools",
    links: [
      { label: "Playground", href: "#/tools/compiler-lab" },
      { label: "Motion", href: "#/tools/motion" },
      { label: "Dock", href: "#/navigation/dock" }
    ]
  }
];

/** Navbar entry that opens a full-width multi-column panel. */
export function MegaMenuDemo() {
  const [open, setOpen] = useState(true);
  return (
    <div className="megamenu-frame">
      <nav className="megamenu-bar" aria-label="Site">
        <a className="megamenu-brand" href="#/actions/button"><span className="brand-mark h-8 w-8 text-sm">u</span>utilitycss</a>
        <button
          type="button"
          aria-expanded={open}
          onClick={() => setOpen(value => !value)}
          className={cn("megamenu-trigger", open && "megamenu-trigger-open")}
        >
          Browse library
          <Icon name="chevron-down" className={cn("h-4 w-4", open && "rotate-180")} />
        </button>
        <a className="nav-link" href="#/data-display/timeline">Changelog</a>
      </nav>
      {open ? (
        <div className="megamenu-panel">
          {columns.map(column => (
            <div key={column.title}>
              <p className="megamenu-title">{column.title}</p>
              <ul className="megamenu-links">
                {column.links.map(link => (
                  <li key={link.label}><a href={link.href}>{link.label}</a></li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      ) : null}
    </div>
  );
}
