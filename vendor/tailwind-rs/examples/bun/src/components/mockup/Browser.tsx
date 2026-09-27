/** Browser chrome with tabs, address bar, and page body. */
export function BrowserMockupDemo() {
  return (
    <div className="mockup-browser" role="img" aria-label="Browser mockup showing the component gallery">
      <div className="mockup-browser-bar">
        <span className="mockup-dots" aria-hidden="true"><i /><i /><i /></span>
        <span className="mockup-tab mockup-tab-active">Gallery</span>
        <span className="mockup-tab">Compiler Lab</span>
      </div>
      <div className="mockup-browser-url"><span>utilitycss.dev/gallery</span></div>
      <div className="mockup-browser-body">
        <span className="mockup-line mockup-line-title" />
        <span className="mockup-line" />
        <span className="mockup-line mockup-line-short" />
        <span className="mockup-grid" aria-hidden="true"><i /><i /><i /></span>
      </div>
    </div>
  );
}
