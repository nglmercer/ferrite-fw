import { useState } from "preact/hooks";

/** Before/after comparison with a draggable reveal position. */
export function DiffDemo() {
  const [position, setPosition] = useState(50);
  return (
    <div>
      <div className="diff-frame" aria-label="Before and after comparison">
        <div className="diff-pane diff-pane-after">
          <span className="eyebrow">After</span>
          <strong>Deterministic CSS</strong>
          <p>Same bytes on every machine, every build.</p>
        </div>
        <div className="diff-pane diff-pane-before" style={{ width: `${position}%` }}>
          <span className="eyebrow">Before</span>
          <strong>Runtime utilities</strong>
          <p>CSS assembled in the browser, on every visit.</p>
        </div>
        <span className="diff-handle" style={{ left: `${position}%` }} aria-hidden="true" />
      </div>
      <label className="form-label mt-4">
        Reveal position: {position}%
        <input
          className="range-input mt-2 w-full"
          type="range"
          min={5}
          max={95}
          value={position}
          onInput={event => setPosition(Number(event.currentTarget.value))}
        />
      </label>
    </div>
  );
}
