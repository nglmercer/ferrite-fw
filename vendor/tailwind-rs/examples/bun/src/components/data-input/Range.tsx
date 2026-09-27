import { useState } from "preact/hooks";

/** Sliders for volume-like scalar settings. */
export function RangeDemo() {
  const [volume, setVolume] = useState(42);
  return (
    <div className="space-y-5">
      <label className="form-label">Volume: {volume}
        <input
          className="range-input range-brand mt-2 w-full"
          type="range"
          min={0}
          max={100}
          value={volume}
          onInput={event => setVolume(Number(event.currentTarget.value))}
        />
      </label>
      <label className="form-label">Quality steps: {Math.round(volume / 25)}
        <input className="range-input mt-2 w-full" type="range" min={0} max={100} step={25} defaultValue={50} />
      </label>
      <label className="form-label">Disabled
        <input className="range-input mt-2 w-full" type="range" min={0} max={100} defaultValue={30} disabled />
      </label>
    </div>
  );
}
