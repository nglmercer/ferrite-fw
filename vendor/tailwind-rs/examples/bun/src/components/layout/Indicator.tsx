import { Icon } from "../ui/Icon.tsx";

/** Corner badges pinned to avatars, buttons, and cards. */
export function IndicatorDemo() {
  return (
    <div className="flex flex-wrap items-center gap-8">
      <span className="indicator">
        <span className="avatar">JD</span>
        <span className="indicator-badge" aria-hidden="true">3</span>
        <span className="sr-only">3 unread notifications</span>
      </span>
      <span className="indicator">
        <button type="button" className="rounded-lg border border-gray-200 bg-white px-4 py-2 text-sm font-medium text-gray-700">
          <Icon name="bell" className="h-4 w-4" /> Notifications
        </button>
        <span className="indicator-dot" aria-label="Unread notifications" />
      </span>
      <span className="indicator">
        <span className="indicator-card">Deploy preview</span>
        <span className="indicator-tag">New</span>
      </span>
    </div>
  );
}
