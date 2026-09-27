import { Icon } from "../ui/Icon.tsx";

export function ToastDemo() {
  return (
    <div className="flex items-center gap-4 rounded-lg border border-gray-200 bg-white p-4 shadow-sm" role="status">
      <span className="toast-icon"><Icon name="check" className="h-4 w-4" /></span>
      <div className="text-sm"><strong className="block text-gray-900">Item moved successfully.</strong><span className="text-gray-500">You can undo this action.</span></div>
      <button type="button" className="ml-auto text-sm font-semibold text-brand-600 hover:text-brand-700">Undo</button>
    </div>
  );
}
