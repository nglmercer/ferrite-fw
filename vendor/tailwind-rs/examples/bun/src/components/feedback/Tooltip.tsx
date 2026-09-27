import type { ComponentChildren } from "preact";

type TooltipPosition = "top" | "bottom" | "left" | "right";

const positionClasses: Record<TooltipPosition, string> = {
  top: "tooltip-bubble tooltip-top",
  bottom: "tooltip-bubble tooltip-bottom",
  left: "tooltip-bubble tooltip-left",
  right: "tooltip-bubble tooltip-right"
};

/** CSS-only tooltip revealed on hover and keyboard focus. */
export function Tooltip({ tip, position = "top", children }: { tip: string; position?: TooltipPosition; children: ComponentChildren }) {
  return (
    <span className="tooltip-anchor" tabIndex={0} aria-label={tip}>
      {children}
      <span className={positionClasses[position]} role="tooltip">{tip}</span>
    </span>
  );
}

export function TooltipDemo() {
  return (
    <div className="flex flex-wrap items-center gap-8 p-6">
      <Tooltip tip="Shown above the trigger"><button type="button" className="rounded-lg border border-gray-200 bg-white px-4 py-2 text-sm font-medium text-gray-700">Top tooltip</button></Tooltip>
      <Tooltip tip="Shown below the trigger" position="bottom"><button type="button" className="rounded-lg border border-gray-200 bg-white px-4 py-2 text-sm font-medium text-gray-700">Bottom tooltip</button></Tooltip>
      <span className="tooltip">Static tooltip<span className="tooltip-arrow" /></span>
    </div>
  );
}
