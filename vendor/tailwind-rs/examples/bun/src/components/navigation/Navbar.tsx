export function NavbarDemo() {
  return (
    <nav className="flex items-center justify-between rounded-lg border border-gray-200 bg-white px-4 py-3" aria-label="Demo site">
      <a className="flex items-center gap-2 font-bold text-gray-900" href="#/actions/button"><span className="brand-mark h-8 w-8 text-sm">u</span>utilitycss</a>
      <div className="hidden items-center gap-6 md:flex">
        <a className="nav-link" href="#/data-display/card">Components</a><a className="nav-link" href="#/tools/motion">Motion</a><a className="nav-link" href="#/actions/theme-controller">Themes</a><a className="nav-link" href="#/tools/compiler-lab">Playground</a>
      </div>
      <a className="rounded-lg bg-brand-600 px-4 py-2 text-sm font-semibold text-white hover:bg-brand-700" href="#/actions/button">Get started</a>
    </nav>
  );
}
