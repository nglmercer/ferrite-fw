import type { ComponentChildren } from "preact";

/** Semantic keyboard hint rendered as a keycap. */
export function Kbd({ children }: { children: ComponentChildren }) {
  return <kbd className="kbd">{children}</kbd>;
}

export function KbdDemo() {
  return (
    <div className="space-y-4 text-sm text-gray-700">
      <p className="flex flex-wrap items-center gap-2">
        Press <Kbd>Ctrl</Kbd> + <Kbd>K</Kbd> to focus the component search.
      </p>
      <p className="flex flex-wrap items-center gap-2">
        <Kbd>Tab</Kbd> moves forward, <Kbd>Shift</Kbd> + <Kbd>Tab</Kbd> moves back.
      </p>
      <p className="flex flex-wrap items-center gap-2">
        Sizes: <Kbd>Esc</Kbd>
        <span className="kbd kbd-lg">Enter</span>
      </p>
    </div>
  );
}
