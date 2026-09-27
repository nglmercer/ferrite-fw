import { useState } from "preact/hooks";
import { Icon } from "../ui/Icon.tsx";

type Item = { title: string; content: string };

const defaultItems: Item[] = [
  { title: "What is utilitycss?", content: "A Rust utility CSS compiler with deterministic output, incremental builds, and thin Bun/Vite adapters. It owns scanning → parsing → theme → utilities → variants → CSS IR." },
  { title: "How is it different from Tailwind?", content: "It is not drop-in Tailwind-compatible. Compat is explicit via presets with documented grammar, tokens, and ordering. The compiler core is runtime-agnostic." },
  { title: "Can I use @apply?", content: "Yes — @apply is backed by the same Rust registry. Write recipes like .btn { @apply px-4 py-2 rounded bg-brand-600 text-white } and get deterministic CSS." }
];

export function Accordion({ items = defaultItems }: { items?: Item[] }) {
  const [open, setOpen] = useState(0);
  return (
    <div className="overflow-hidden rounded-lg border border-gray-200 bg-white">
      {items.map((item, idx) => (
        <div key={item.title}>
          <button type="button" id={`accordion-tab-${idx}`} aria-expanded={open === idx} aria-controls={`accordion-panel-${idx}`} className="flex w-full items-center justify-between p-5 text-left font-medium text-gray-700 hover:bg-gray-50" onClick={() => setOpen(open === idx ? -1 : idx)}>
            <span>{item.title}</span>
            <Icon name={open === idx ? "chevron-up" : "chevron-down"} className="h-4 w-4 text-gray-500" />
          </button>
          {open === idx ? <div id={`accordion-panel-${idx}`} role="region" aria-labelledby={`accordion-tab-${idx}`} className="px-5 pb-5 text-sm leading-relaxed text-gray-500">{item.content}</div> : null}
        </div>
      ))}
    </div>
  );
}

export function AccordionFlushDemo() {
  const [open, setOpen] = useState(1);
  return (
    <div className="rounded-lg border border-gray-200 bg-white p-1">
      <Accordion items={[
        { title: "Flowbite accordion", content: "Replicated with static utilities and Preact state. No JS bundle for styles." },
        { title: "Flush variant", content: "Flush removes outer borders — here shown as nested card style." },
        { title: "Always open", content: "This demo allows one open panel at a time for clarity." }
      ]} />
    </div>
  );
}
