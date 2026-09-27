export function ProgressDemo() {
  return (
    <div className="space-y-4">
      <div className="w-full rounded-full bg-gray-200" role="progressbar" aria-valuenow={45} aria-valuemin={0} aria-valuemax={100} aria-label="Upload progress"><div className="progress-bar progress-bar-45">45%</div></div>
      <div className="w-full rounded-full bg-gray-200" role="progressbar" aria-valuenow={70} aria-valuemin={0} aria-valuemax={100} aria-label="Profile completion"><div className="progress-bar progress-bar-70 bg-green-600">70%</div></div>
      <div className="w-full rounded-full bg-gray-200" role="progressbar" aria-valuenow={55} aria-valuemin={0} aria-valuemax={100} aria-label="Processing"><div className="progress-bar progress-bar-striped">Striped</div></div>
      <div className="flex gap-2 text-xs font-medium text-gray-600"><span>Default</span><span>·</span><span>Sizes: sm / md / lg via height utilities</span></div>
    </div>
  );
}
