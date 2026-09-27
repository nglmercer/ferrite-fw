/** Inline link styles for body copy, navigation, and hover-only emphasis. */
export function LinkDemo() {
  return (
    <div className="space-y-4 text-sm leading-relaxed text-gray-700">
      <p>
        A <a className="link" href="#/navigation/link">default link</a> inherits the surrounding text style and only
        shows its affordance <a className="link link-hover" href="#/navigation/link">on hover</a>.
      </p>
      <p className="flex flex-wrap items-center gap-4">
        <a className="link link-primary" href="#/actions/button">Primary link</a>
        <a className="link link-neutral" href="#/data-display/card">Neutral link</a>
        <a className="link link-accent" href="#/tools/motion">Accent link</a>
      </p>
      <p>
        Links stay readable inside prose: read the <a className="link link-primary" href="#/tools/compiler-lab">compiler pipeline</a> guide
        before changing adapter behavior.
      </p>
    </div>
  );
}
