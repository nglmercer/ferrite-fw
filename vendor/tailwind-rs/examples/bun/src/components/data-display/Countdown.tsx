import { useEffect, useState } from "preact/hooks";

interface CountdownParts {
  readonly days: string;
  readonly hours: string;
  readonly minutes: string;
  readonly seconds: string;
}

function pad(value: number): string {
  return String(Math.max(0, value)).padStart(2, "0");
}

function partsUntil(target: number, now: number): CountdownParts {
  const remaining = Math.max(0, Math.floor((target - now) / 1000));
  return {
    days: pad(Math.floor(remaining / 86_400)),
    hours: pad(Math.floor(remaining / 3600) % 24),
    minutes: pad(Math.floor(remaining / 60) % 60),
    seconds: pad(remaining % 60)
  };
}

/** Live countdown toward a near-future launch moment. */
export function CountdownDemo() {
  const [target] = useState(() => Date.now() + 1000 * 60 * 60 * 26 + 1000 * 60 * 14);
  const [parts, setParts] = useState<CountdownParts>(() => partsUntil(target, Date.now()));
  useEffect(() => {
    const timer = window.setInterval(() => setParts(partsUntil(target, Date.now())), 1000);
    return () => window.clearInterval(timer);
  }, [target]);
  const cells: readonly (readonly [string, string])[] = [
    [parts.days, "days"],
    [parts.hours, "hours"],
    [parts.minutes, "min"],
    [parts.seconds, "sec"]
  ];
  return (
    <div className="countdown" role="timer" aria-label="Time until launch">
      {cells.map(([value, label]) => (
        <span key={label} className="countdown-cell">
          <strong>{value}</strong>
          <small>{label}</small>
        </span>
      ))}
    </div>
  );
}
