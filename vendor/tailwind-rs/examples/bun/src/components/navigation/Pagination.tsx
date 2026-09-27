import { useState } from "preact/hooks";
import { cn } from "../../lib/cn.ts";

const PER_PAGE = 10;
const TOTAL_ITEMS = 45;

export function PaginationDemo() {
  const last = Math.ceil(TOTAL_ITEMS / PER_PAGE);
  const [page, setPage] = useState(1);
  const start = (page - 1) * PER_PAGE + 1;
  const end = Math.min(page * PER_PAGE, TOTAL_ITEMS);
  return (
    <nav aria-label="Page navigation" className="flex items-center justify-between">
      <span className="text-sm text-gray-700" aria-live="polite">Showing <strong>{start}–{end}</strong> of <strong>{TOTAL_ITEMS}</strong></span>
      <div className="inline-flex -space-x-px rounded-lg border border-gray-200 bg-white">
        <button type="button" disabled={page === 1} onClick={() => setPage(current => Math.max(1, current - 1))} className="pagination-link pagination-link-first">Previous</button>
        {Array.from({ length: last }, (_, index) => index + 1).map(number => (
          <button key={number} type="button" aria-current={number === page ? "page" : undefined} onClick={() => setPage(number)} className={cn("pagination-link", number === page && "pagination-link-active")}>{number}</button>
        ))}
        <button type="button" disabled={page === last} onClick={() => setPage(current => Math.min(last, current + 1))} className="pagination-link pagination-link-last">Next</button>
      </div>
    </nav>
  );
}
