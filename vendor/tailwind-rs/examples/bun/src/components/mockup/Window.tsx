/** Desktop window with sidebar navigation and a preview pane. */
export function WindowMockupDemo() {
  return (
    <div className="mockup-window" role="img" aria-label="Window mockup showing project files">
      <div className="mockup-window-bar">
        <span className="mockup-dots" aria-hidden="true"><i /><i /><i /></span>
        <span className="mockup-window-title">utilitycss — main</span>
      </div>
      <div className="mockup-window-body">
        <ul className="mockup-window-side" aria-hidden="true">
          <li className="mockup-window-active">src</li>
          <li>crates</li>
          <li>packages</li>
          <li>docs</li>
        </ul>
        <div className="mockup-window-main">
          <span className="mockup-line mockup-line-title" />
          <span className="mockup-line" />
          <span className="mockup-line mockup-line-short" />
        </div>
      </div>
    </div>
  );
}
