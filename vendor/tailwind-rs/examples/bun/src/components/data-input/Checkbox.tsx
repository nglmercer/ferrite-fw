import { useState } from "preact/hooks";

const topics = ["Release notes", "Changelog digest", "Community events"];

/** Standalone and grouped checkboxes with a select-all row. */
export function CheckboxDemo() {
  const [selected, setSelected] = useState<readonly string[]>(["Release notes"]);
  const allSelected = selected.length === topics.length;
  function toggle(topic: string) {
    setSelected(current => current.includes(topic) ? current.filter(entry => entry !== topic) : [...current, topic]);
  }
  return (
    <div className="space-y-3">
      <label className="checkbox-row">
        <input
          className="checkbox-input"
          type="checkbox"
          checked={allSelected}
          onChange={() => setSelected(allSelected ? [] : topics)}
        />
        <span>Subscribe to everything</span>
      </label>
      <div className="checkbox-group" role="group" aria-label="Email topics">
        {topics.map(topic => (
          <label key={topic} className="checkbox-row">
            <input
              className="checkbox-input"
              type="checkbox"
              checked={selected.includes(topic)}
              onChange={() => toggle(topic)}
            />
            <span>{topic}</span>
          </label>
        ))}
      </div>
      <p className="text-sm text-gray-600" aria-live="polite">{selected.length} of {topics.length} topics selected.</p>
    </div>
  );
}
