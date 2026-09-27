import { useState } from "preact/hooks";
import { Icon } from "../ui/Icon.tsx";

/** File picker with a preview of the chosen file name. */
export function FileInputDemo() {
  const [fileName, setFileName] = useState<string | null>(null);
  return (
    <div className="space-y-4">
      <label className="form-label">Upload file
        <input
          className="form-input form-file"
          type="file"
          onChange={event => setFileName(event.currentTarget.files?.[0]?.name ?? null)}
        />
        <span className="form-help">PNG, JPG, or PDF up to 10 MB.</span>
      </label>
      <p className="file-chosen" aria-live="polite">
        <Icon name="copy" className="h-4 w-4" />
        {fileName ?? "No file chosen yet."}
      </p>
    </div>
  );
}
