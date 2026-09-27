import { useState } from "preact/hooks";

function RadialRing({ value, label, tone }: { value: number; label: string; tone: string }) {
  const radius = 34;
  const circumference = 2 * Math.PI * radius;
  const offset = circumference - (Math.min(100, Math.max(0, value)) / 100) * circumference;
  return (
    <div className="radial-progress" role="progressbar" aria-valuenow={value} aria-valuemin={0} aria-valuemax={100} aria-label={label}>
      <svg viewBox="0 0 80 80" aria-hidden="true">
        <circle className="radial-track" cx="40" cy="40" r={radius} />
        <circle
          className={`radial-value ${tone}`}
          cx="40"
          cy="40"
          r={radius}
          strokeDasharray={`${circumference}`}
          strokeDashoffset={`${offset}`}
        />
      </svg>
      <strong>{value}%</strong>
    </div>
  );
}

/** Circular progress rings with an adjustable demo value. */
export function RadialProgressDemo() {
  const [value, setValue] = useState(68);
  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center gap-6">
        <RadialRing value={value} label="Upload progress" tone="radial-brand" />
        <RadialRing value={25} label="Back-up progress" tone="radial-green" />
        <RadialRing value={90} label="Migration progress" tone="radial-blue" />
      </div>
      <label className="form-label">
        Upload progress: {value}%
        <input
          className="range-input mt-2 w-full"
          type="range"
          min={0}
          max={100}
          value={value}
          onInput={event => setValue(Number(event.currentTarget.value))}
        />
      </label>
    </div>
  );
}
