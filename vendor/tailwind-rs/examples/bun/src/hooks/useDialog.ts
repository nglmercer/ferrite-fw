import { useEffect, useRef } from "preact/hooks";

/** Runs the handler when Escape is pressed while active. */
export function useEscape(active: boolean, onEscape: () => void): void {
  const saved = useRef(onEscape);
  saved.current = onEscape;
  useEffect(() => {
    if (!active) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") saved.current();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [active]);
}

const FOCUSABLE = "a[href], button:not([disabled]), textarea, input, select, [tabindex]:not([tabindex='-1'])";

function trapTab(container: HTMLElement, event: KeyboardEvent): void {
  if (event.key !== "Tab") return;
  const items = [...container.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(el => el.getAttribute("aria-hidden") !== "true");
  if (items.length === 0) return;
  const first = items[0];
  const last = items[items.length - 1];
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}

/**
 * Modal dialog behavior: Escape closes, Tab cycles inside, opening moves
 * focus to the first control and closing returns it to the trigger.
 */
export function useTrappedDialog<T extends HTMLElement>(open: boolean, onClose: () => void) {
  const ref = useRef<T | null>(null);
  const savedClose = useRef(onClose);
  savedClose.current = onClose;
  useEscape(open, () => savedClose.current());
  useEffect(() => {
    if (!open) return;
    const container = ref.current;
    const previouslyFocused = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    container?.querySelector<HTMLElement>(FOCUSABLE)?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (container) trapTab(container, event);
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      previouslyFocused?.focus();
    };
  }, [open ]);
  return ref;
}
