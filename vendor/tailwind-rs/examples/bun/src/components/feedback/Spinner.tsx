export function SpinnerDemo() {
  return (
    <div className="flex flex-wrap items-center gap-6">
      <span className="spinner spinner-sm" aria-label="Loading" />
      <span className="spinner" aria-label="Loading" />
      <span className="spinner spinner-lg" aria-label="Loading" />
      <span className="inline-flex items-center gap-2 text-sm text-gray-600"><span className="spinner spinner-sm" />Loading…</span>
    </div>
  );
}
