export function SkeletonDemo() {
  return (
    <div className="animate-pulse-soft space-y-3">
      <div className="h-4 w-3/4 rounded bg-gray-200" />
      <div className="h-4 rounded bg-gray-200" />
      <div className="h-4 w-5/6 rounded bg-gray-200" />
      <div className="flex items-center gap-3 pt-2">
        <div className="h-10 w-10 rounded-full bg-gray-200" />
        <div className="flex-1 space-y-2"><div className="h-3 rounded bg-gray-200" /><div className="h-3 w-3/4 rounded bg-gray-200" /></div>
      </div>
    </div>
  );
}
