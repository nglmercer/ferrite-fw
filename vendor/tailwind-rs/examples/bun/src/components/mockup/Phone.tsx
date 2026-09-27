/** Phone frame with notch, app header, and content rows. */
export function PhoneMockupDemo() {
  return (
    <div className="mockup-phone" role="img" aria-label="Phone mockup showing a deployment app">
      <span className="mockup-phone-notch" aria-hidden="true" />
      <div className="mockup-phone-screen">
        <p className="mockup-phone-title">Deploys</p>
        <ul className="mockup-phone-rows">
          <li><strong>utilitycss-site</strong><span className="mockup-pill mockup-pill-ready">Ready</span></li>
          <li><strong>docs-preview</strong><span className="mockup-pill mockup-pill-building">Building</span></li>
          <li><strong>component-kit</strong><span className="mockup-pill mockup-pill-failed">Failed</span></li>
        </ul>
        <span className="mockup-phone-cta">New deployment</span>
      </div>
    </div>
  );
}
