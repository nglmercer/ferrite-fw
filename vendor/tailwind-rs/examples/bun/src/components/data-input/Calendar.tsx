import { useState } from "preact/hooks";
import { Icon } from "../ui/Icon.tsx";
import { cn } from "../../lib/cn.ts";

const WEEKDAYS = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"] as const;

function monthCells(year: number, month: number): readonly (number | null)[] {
  const first = new Date(year, month, 1);
  const lead = (first.getDay() + 6) % 7;
  const days = new Date(year, month + 1, 0).getDate();
  const cells: (number | null)[] = [];
  for (let blank = 0; blank < lead; blank += 1) cells.push(null);
  for (let day = 1; day <= days; day += 1) cells.push(day);
  return cells;
}

/** Month picker with keyboard-focusable day buttons. */
export function CalendarDemo() {
  const today = new Date();
  const [cursor, setCursor] = useState({ year: today.getFullYear(), month: today.getMonth() });
  const [selected, setSelected] = useState<number | null>(today.getDate());
  const label = new Date(cursor.year, cursor.month, 1).toLocaleString("en-US", { month: "long", year: "numeric" });
  function shift(delta: number) {
    setCursor(current => {
      const next = new Date(current.year, current.month + delta, 1);
      return { year: next.getFullYear(), month: next.getMonth() };
    });
    setSelected(null);
  }
  return (
    <div className="calendar max-w-[24rem]">
      <div className="calendar-header">
        <button type="button" aria-label="Previous month" className="calendar-nav" onClick={() => shift(-1)}><Icon name="chevron-right" className="h-4 w-4 rotate-180" /></button>
        <strong aria-live="polite">{label}</strong>
        <button type="button" aria-label="Next month" className="calendar-nav" onClick={() => shift(1)}><Icon name="chevron-right" className="h-4 w-4" /></button>
      </div>
      <div className="calendar-grid" role="grid" aria-label={label}>
        {WEEKDAYS.map(day => <span key={day} className="calendar-weekday">{day}</span>)}
        {monthCells(cursor.year, cursor.month).map((day, index) => day === null ? (
          <span key={`blank-${index}`} />
        ) : (
          <button
            key={day}
            type="button"
            role="gridcell"
            aria-selected={selected === day}
            onClick={() => setSelected(day)}
            className={cn("calendar-day", selected === day && "calendar-day-selected")}
          >
            {day}
          </button>
        ))}
      </div>
      <p className="calendar-note" aria-live="polite">{selected === null ? "Pick a day." : `Selected ${label} ${selected}.`}</p>
    </div>
  );
}
