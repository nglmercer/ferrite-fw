import { Icon } from "../ui/Icon.tsx";
import { cn } from "../../lib/cn.ts";

export function StepperDemo() {
  return (
    <ol className="m-0 flex list-none items-center gap-4 p-0">
      {[
        { label: "Cart", done: true },
        { label: "Checkout", done: true },
        { label: "Payment", active: true },
        { label: "Review", done: false }
      ].map(s => (
        <li key={s.label} className="flex items-center gap-2">
          <span className={`step-dot ${s.done ? "step-dot-done" : s.active ? "step-dot-active" : "step-dot-idle"}`}>{s.done ? <Icon name="check" className="h-3 w-3" /> : s.label[0]}</span>
          <span className={cn("text-sm font-medium", s.active ? "text-brand-700" : "text-gray-500")}>{s.label}</span>
          <span className="hidden h-0.5 w-8 bg-gray-200 md:block" />
        </li>
      ))}
    </ol>
  );
}
