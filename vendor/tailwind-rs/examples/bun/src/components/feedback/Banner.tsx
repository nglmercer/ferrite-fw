import { Icon } from "../ui/Icon.tsx";

export function BannerDemo({ onDismiss }: { onDismiss?: () => void }) {
  return (
    <div className="banner" role="status">
      <div className="flex items-center gap-3">
        <Icon name="info" className="h-5 w-5 text-brand-600" />
        <p className="text-sm text-gray-700"><strong>Flowbite parity:</strong> Every section below uses utilitycss utilities + <code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs">@apply</code> — no Tailwind runtime.</p>
      </div>
      <button type="button" aria-label="Dismiss banner" className="banner-close" onClick={onDismiss}><Icon name="close" className="h-4 w-4" /></button>
    </div>
  );
}
