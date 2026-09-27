interface FooterLink {
  readonly label: string;
  readonly href: string;
}

const columns: readonly { title: string; links: readonly FooterLink[] }[] = [
  {
    title: "Components",
    links: [
      { label: "Button", href: "#/actions/button" },
      { label: "Card", href: "#/data-display/card" },
      { label: "Table", href: "#/data-display/table" }
    ]
  },
  {
    title: "Patterns",
    links: [
      { label: "Forms", href: "#/data-input/input" },
      { label: "Navigation", href: "#/navigation/navbar" },
      { label: "Feedback", href: "#/feedback/alert" }
    ]
  },
  {
    title: "Tools",
    links: [
      { label: "Playground", href: "#/tools/compiler-lab" },
      { label: "Motion", href: "#/tools/motion" },
      { label: "Themes", href: "#/actions/theme-controller" }
    ]
  }
];

/** Multi-column page footer with brand block and link groups. */
export function FooterDemo() {
  return (
    <footer className="footer-demo">
      <div className="footer-brand">
        <span className="brand-mark">u</span>
        <div>
          <strong>utilitycss</strong>
          <p>Deterministic utility CSS, compiled in Rust.</p>
        </div>
      </div>
      {columns.map(column => (
        <nav key={column.title} aria-label={column.title}>
          <p className="footer-title">{column.title}</p>
          <ul className="footer-links">
            {column.links.map(link => <li key={link.label}><a href={link.href}>{link.label}</a></li>)}
          </ul>
        </nav>
      ))}
    </footer>
  );
}
