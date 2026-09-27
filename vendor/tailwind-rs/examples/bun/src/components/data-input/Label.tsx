/** Label placements: stacked, inline, and floating. */
export function LabelDemo() {
  return (
    <div className="space-y-5">
      <label className="form-label">Stacked label<input className="form-input" placeholder="Label sits above the control" /></label>
      <label className="label-inline"><span>Inline label</span><input className="form-input" placeholder="Label sits beside the control" /></label>
      <label className="label-floating">
        <input className="form-input" placeholder=" " />
        <span>Floating label</span>
      </label>
      <p className="form-help">Floating labels use <code>:placeholder-shown</code>; no JavaScript required.</p>
    </div>
  );
}
