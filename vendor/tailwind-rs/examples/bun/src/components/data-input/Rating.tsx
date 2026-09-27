import { useState } from "preact/hooks";
import { Icon } from "../ui/Icon.tsx";
import { cn } from "../../lib/cn.ts";

/** Interactive star rating with a read-only score variant. */
export function RatingDemo() {
  const [value, setValue] = useState(4);
  return (
    <div className="space-y-5">
      <div className="flex items-center gap-1" role="radiogroup" aria-label="Your rating">
        {[1, 2, 3, 4, 5].map(n => (
          <button
            key={n}
            type="button"
            role="radio"
            aria-checked={value === n}
            aria-label={`${n} star${n === 1 ? "" : "s"}`}
            onClick={() => setValue(n)}
            className="rating-star"
          >
            <Icon name="star" className={cn("h-6 w-6", n <= value ? "fill-yellow-400 text-yellow-400" : "text-gray-300")} />
          </button>
        ))}
        <span className="ml-2 text-sm font-medium text-gray-700">{value}.0 out of 5</span>
      </div>
      <div className="flex items-center gap-2 text-sm text-gray-600"><span>95 reviews</span><span className="rounded bg-blue-100 px-2 py-0.5 text-xs font-semibold text-blue-800">Excellent</span></div>
    </div>
  );
}
