import { useState } from "preact/hooks";
import { Icon, type IconName } from "../ui/Icon.tsx";
import { cn } from "../../lib/cn.ts";

interface SwapProps {
  readonly active: boolean;
  readonly onToggle: () => void;
  readonly activeIcon: IconName;
  readonly inactiveIcon: IconName;
  readonly label: string;
}

/** Swaps between two icons with a rotation transition. */
export function Swap({ active, onToggle, activeIcon, inactiveIcon, label }: SwapProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={active}
      aria-label={label}
      onClick={onToggle}
      className={cn("swap", active && "swap-active")}
    >
      <span className="swap-icon swap-icon-off" aria-hidden="true"><Icon name={inactiveIcon} className="h-5 w-5" /></span>
      <span className="swap-icon swap-icon-on" aria-hidden="true"><Icon name={activeIcon} className="h-5 w-5" /></span>
    </button>
  );
}

export function SwapDemo() {
  const [favorite, setFavorite] = useState(false);
  const [muted, setMuted] = useState(true);
  return (
    <div className="flex flex-wrap items-center gap-6">
      <span className="inline-flex items-center gap-3">
        <Swap active={favorite} onToggle={() => setFavorite(value => !value)} activeIcon="heart-filled" inactiveIcon="heart" label="Mark as favorite" />
        <span className="text-sm text-gray-600">{favorite ? "Saved to favorites." : "Not saved yet."}</span>
      </span>
      <span className="inline-flex items-center gap-3">
        <Swap active={!muted} onToggle={() => setMuted(value => !value)} activeIcon="bell" inactiveIcon="close" label="Toggle notifications" />
        <span className="text-sm text-gray-600">{muted ? "Notifications muted." : "Notifications on."}</span>
      </span>
    </div>
  );
}
