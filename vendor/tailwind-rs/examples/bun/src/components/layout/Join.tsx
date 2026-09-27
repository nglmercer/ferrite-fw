import { Button } from "../actions/Button.tsx";
import { Icon } from "../ui/Icon.tsx";

/** Controls fused into one continuous row or column. */
export function JoinDemo() {
  return (
    <div className="space-y-6">
      <div className="join" role="search">
        <input className="join-input" placeholder="Search components…" aria-label="Search components" />
        <Button>Search</Button>
      </div>
      <div className="join" role="group" aria-label="Text alignment">
        <button type="button" className="join-item join-item-active" aria-pressed="true">Left</button>
        <button type="button" className="join-item" aria-pressed="false">Center</button>
        <button type="button" className="join-item" aria-pressed="false">Right</button>
      </div>
      <div className="join join-vertical" role="group" aria-label="Share actions">
        <button type="button" className="join-item"><Icon name="send" className="h-4 w-4" />Send</button>
        <button type="button" className="join-item"><Icon name="copy" className="h-4 w-4" />Copy link</button>
        <button type="button" className="join-item"><Icon name="download" className="h-4 w-4" />Export</button>
      </div>
    </div>
  );
}
