/** Native selects with consistent chrome, sizes, and groupings. */
export function SelectDemo() {
  return (
    <div className="grid gap-4 md:grid-cols-2">
      <label className="form-label">Category
        <select className="form-input form-select" defaultValue="Engineering">
          <option>Design</option>
          <option>Engineering</option>
          <option>Marketing</option>
        </select>
      </label>
      <label className="form-label">Timezone
        <select className="form-input form-select">
          <optgroup label="Americas">
            <option>UTC−08:00 Pacific</option>
            <option>UTC−05:00 Eastern</option>
          </optgroup>
          <optgroup label="Europe">
            <option>UTC+00:00 London</option>
            <option>UTC+01:00 Berlin</option>
          </optgroup>
        </select>
      </label>
      <label className="form-label">Small<select className="form-input form-select form-input-sm" defaultValue="Draft"><option>Draft</option><option>Review</option><option>Published</option></select></label>
      <label className="form-label">Disabled<select className="form-input form-select" disabled><option>Unavailable</option></select></label>
    </div>
  );
}
