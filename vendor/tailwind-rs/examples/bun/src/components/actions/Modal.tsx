import { Icon } from "../ui/Icon.tsx";
import { useTrappedDialog } from "../../hooks/useDialog.ts";
import { Button } from "./Button.tsx";

export function ModalDemo({ open, onClose }: { open: boolean; onClose: () => void }) {
  const dialogRef = useTrappedDialog<HTMLDivElement>(open, onClose);
  if (!open) return null;
  return (
    <div className="modal-backdrop" role="presentation" onClick={onClose}>
      <div ref={dialogRef} aria-labelledby="modal-title" aria-modal="true" className="modal-card" onClick={e => e.stopPropagation()} role="dialog">
        <button aria-label="Close modal" className="modal-close" onClick={onClose} type="button"><Icon name="close" className="h-5 w-5" /></button>
        <span className="eyebrow text-brand-700">Modal dialog</span>
        <h2 className="mt-3 text-2xl font-bold text-gray-900" id="modal-title">Create a deployment</h2>
        <p className="mt-3 text-sm leading-relaxed text-gray-600">This focused surface is useful for confirmations, short forms, and important decisions.</p>
        <div className="mt-6 flex justify-end gap-3"><Button variant="ghost" onClick={onClose}>Cancel</Button><Button onClick={onClose}>Continue <Icon name="arrow" className="h-4 w-4" /></Button></div>
      </div>
    </div>
  );
}
