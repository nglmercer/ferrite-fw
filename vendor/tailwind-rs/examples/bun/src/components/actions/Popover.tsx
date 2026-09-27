import { useEffect, useRef, useState } from "preact/hooks";
import { useEscape } from "../../hooks/useDialog.ts";

type PopoverId = "default" | "descriptive";

/** Click-to-toggle popovers; opening one closes the other so panels never overlap. */
export function PopoverDemo() {
  const [open, setOpen] = useState<PopoverId | null>(null);
  const root = useRef<HTMLDivElement | null>(null);

  useEscape(open !== null, () => setOpen(null));
  useEffect(() => {
    if (open === null) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(null);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [open ]);

  function toggle(id: PopoverId) {
    setOpen(current => current === id ? null : id);
  }

  return (
    <div className="flex flex-wrap gap-8" ref={root}>
      <div className="relative">
        <button type="button" aria-expanded={open === "default"} onClick={() => toggle("default")} className="rounded-lg border border-gray-200 bg-white px-4 py-2 text-sm font-medium text-gray-700">Default popover</button>
        {open === "default" ? <div className="popover"><div className="popover-header">Popover title</div><div className="popover-body">And here&apos;s some amazing content. It&apos;s very engaging.</div></div> : null}
      </div>
      <div className="relative">
        <button type="button" aria-expanded={open === "descriptive"} onClick={() => toggle("descriptive")} className="rounded-lg border border-gray-200 bg-white px-4 py-2 text-sm font-medium text-gray-700">Descriptive popover</button>
        {open === "descriptive" ? <div className="popover"><div className="popover-body">Popover bodies can also stand alone when no title is needed.</div></div> : null}
      </div>
    </div>
  );
}
