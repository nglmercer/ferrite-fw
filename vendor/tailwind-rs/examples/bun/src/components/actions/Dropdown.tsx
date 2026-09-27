import { useEffect, useRef, useState } from "preact/hooks";
import { useEscape } from "../../hooks/useDialog.ts";
import { Icon } from "../ui/Icon.tsx";
import { Button } from "./Button.tsx";

export function DropdownDemo() {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement | null>(null);
  useEscape(open, () => setOpen(false));
  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [open ]);
  const close = () => setOpen(false);
  return (
    <div className="relative inline-block" ref={root}>
      <Button variant="outline" aria-expanded={open} aria-haspopup="true" onClick={() => setOpen(v => !v)}>Dropdown <Icon name="chevron-down" className="h-4 w-4" /></Button>
      {open ? (
        <div className="dropdown-menu">
          <button type="button" className="dropdown-item" onClick={close}>Dashboard</button>
          <button type="button" className="dropdown-item" onClick={close}>Settings</button>
          <button type="button" className="dropdown-item" onClick={close}>Earnings</button>
          <div className="my-1 border border-gray-100" />
          <button type="button" className="dropdown-item dropdown-item-danger" onClick={close}>Sign out</button>
        </div>
      ) : null}
    </div>
  );
}
