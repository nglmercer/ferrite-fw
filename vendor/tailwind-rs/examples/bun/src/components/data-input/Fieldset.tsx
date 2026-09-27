import { Button } from "../actions/Button.tsx";

/** Grouped controls under one legend with a shared action row. */
export function FieldsetDemo() {
  return (
    <form className="fieldset-card" onSubmit={event => event.preventDefault()}>
      <fieldset>
        <legend>Notification preferences</legend>
        <div className="grid gap-4 md:grid-cols-2">
          <label className="form-label">Display name<input className="form-input" placeholder="Jordan Diaz" /></label>
          <label className="form-label">Email<input className="form-input" type="email" placeholder="jordan@example.com" /></label>
        </div>
        <div className="mt-4 space-y-2">
          <label className="checkbox-row"><input className="checkbox-input" type="checkbox" defaultChecked /><span>Build notifications</span></label>
          <label className="checkbox-row"><input className="checkbox-input" type="checkbox" /><span>Weekly digest</span></label>
        </div>
      </fieldset>
      <div className="mt-5 flex justify-end gap-3">
        <Button variant="ghost" type="reset">Reset</Button>
        <Button type="submit">Save preferences</Button>
      </div>
    </form>
  );
}
