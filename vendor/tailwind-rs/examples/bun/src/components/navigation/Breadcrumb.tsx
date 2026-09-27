import { Icon } from "../ui/Icon.tsx";

export function BreadcrumbDemo() {
  return (
    <nav aria-label="Breadcrumb" className="rounded-lg bg-gray-50 px-4 py-3">
      <ol className="m-0 flex list-none items-center gap-2 p-0 text-sm">
        <li><a className="inline-flex items-center gap-1 font-medium text-gray-700 hover:text-brand-600" href="#/actions/button"><Icon name="home" className="h-4 w-4" />Home</a></li>
        <li className="flex items-center gap-2"><Icon name="chevron-right" className="h-3 w-3 text-gray-400" /><a className="font-medium text-gray-700 hover:text-brand-600" href="#/data-display/table">Projects</a></li>
        <li className="flex items-center gap-2"><Icon name="chevron-right" className="h-3 w-3 text-gray-400" /><span className="font-medium text-gray-500" aria-current="page">utilitycss</span></li>
      </ol>
    </nav>
  );
}
