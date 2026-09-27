import { Icon } from "../components/ui/Icon.tsx";
import { useEffect, useRef } from "preact/hooks";

interface ComponentSearchProps {
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly resultCount?: number;
}

export function ComponentSearch({ value, onChange, resultCount }: ComponentSearchProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <label className="component-search">
      <Icon name="search" className="h-4 w-4" />
      <span className="sr-only">Search components</span>
      <input
        ref={inputRef}
        value={value}
        onInput={event => onChange(event.currentTarget.value)}
        placeholder="Search components…"
        aria-label="Search components"
        type="search"
      />
      {value ? <span className="component-search-count" aria-live="polite">{resultCount ?? 0}</span> : <kbd>⌘K</kbd>}
    </label>
  );
}
