/** Rules that separate content, with optional labels and vertical flow. */
export function DividerDemo() {
  return (
    <div className="space-y-6">
      <div>
        <p className="text-sm text-gray-600">Content above the plain divider.</p>
        <hr className="divider" />
        <p className="text-sm text-gray-600">Content below the plain divider.</p>
      </div>
      <div className="divider-label" role="separator" aria-label="Or continue with"><span>or continue with</span></div>
      <div className="divider-stack">
        <span className="divider-chip">Sign in</span>
        <span className="divider-vertical" aria-hidden="true" />
        <span className="divider-chip">Create account</span>
      </div>
    </div>
  );
}
