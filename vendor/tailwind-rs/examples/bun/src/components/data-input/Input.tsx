import { Icon } from "../ui/Icon.tsx";

/** Text-like inputs: flavors, leading icon, sizes, and disabled state. */
export function InputDemo() {
  return (
    <div className="grid gap-4 md:grid-cols-2">
      <label className="form-label">First name<input className="form-input" name="firstName" placeholder="Jordan" /></label>
      <label className="form-label">Email address<input className="form-input" name="email" placeholder="jordan@example.com" type="email" autoComplete="email" /><span className="form-help">We will only use this for account updates.</span></label>
      <label className="form-label">Password<input className="form-input" name="password" type="password" placeholder="••••••••" autoComplete="new-password" /></label>
      <div className="relative">
        <label className="form-label">Search<input className="form-input pl-10" placeholder="Search components…" /></label>
        <Icon name="search" className="pointer-events-none absolute left-3 top-9 h-4 w-4 text-gray-400" />
      </div>
      <label className="form-label">Small<input className="form-input form-input-sm" placeholder="Compact input" /></label>
      <label className="form-label">Disabled<input className="form-input" placeholder="Unavailable" disabled /></label>
    </div>
  );
}
