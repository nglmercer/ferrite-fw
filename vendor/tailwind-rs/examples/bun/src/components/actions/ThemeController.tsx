import { useState } from "preact/hooks";
import { Icon, type IconName } from "../ui/Icon.tsx";
import { cn } from "../../lib/cn.ts";

type ThemeChoice = "light" | "dark" | "system";

const themeChoices: readonly { value: ThemeChoice; label: string; icon: IconName }[] = [
  { value: "light", label: "Light", icon: "sun" },
  { value: "dark", label: "Dark", icon: "moon" },
  { value: "system", label: "System", icon: "settings" }
];

function resolveSystemTheme(): "light" | "dark" {
  if (typeof window !== "undefined" && typeof window.matchMedia === "function" && window.matchMedia("(prefers-color-scheme: dark)").matches) {
    return "dark";
  }
  return "light";
}

/** Theme switcher scoped to a preview panel so the docs shell stays light. */
export function ThemeControllerDemo() {
  const [choice, setChoice] = useState<ThemeChoice>("light");
  const resolved = choice === "system" ? resolveSystemTheme() : choice;
  return (
    <div className="space-y-4">
      <div className="theme-controller" role="group" aria-label="Color theme">
        {themeChoices.map(theme => (
          <button
            key={theme.value}
            type="button"
            aria-pressed={choice === theme.value}
            onClick={() => setChoice(theme.value)}
            className={cn("theme-controller-option", choice === theme.value && "theme-controller-option-active")}
          >
            <Icon name={theme.icon} className="h-4 w-4" />
            {theme.label}
          </button>
        ))}
      </div>
      <div className="theme-preview" data-theme={resolved}>
        <span className="eyebrow">Preview · {resolved}</span>
        <p className="theme-preview-title">Every component, both themes.</p>
        <p className="theme-preview-body">The toggle drives a <code>data-theme</code> attribute; component tokens respond through plain CSS.</p>
        <span className="theme-preview-cta">Continue</span>
      </div>
    </div>
  );
}
