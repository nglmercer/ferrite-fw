import type { ComponentChildren } from "preact";
import { useState } from "preact/hooks";

import { Tabs } from "../components/ui/Tabs.tsx";

interface DemoFrameProps {
  readonly title: string;
  readonly description?: string;
  readonly code: string;
  readonly css?: string;
  readonly children: ComponentChildren;
}

const frameTabs = [
  { value: "preview", label: "Preview" },
  { value: "tsx", label: "TSX" },
  { value: "css", label: "CSS" }
] as const;

function safeId(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9_-]+/g, "-");
}

/** Consistent preview/source presentation for every catalog item. */
export function DemoFrame({ title, description, code, css = ".component { @apply ... }", children }: DemoFrameProps) {
  const [active, setActive] = useState("preview");
  const frameId = safeId(title);

  return (
    <section className="demo-frame" aria-labelledby={`${frameId}-demo-title`}>
      <div className="demo-frame-header">
        <div>
          <span className="eyebrow">Example</span>
          <h2 id={`${frameId}-demo-title`}>{title}</h2>
          {description ? <p>{description}</p> : null}
        </div>
        <span className="demo-frame-label">utilitycss</span>
      </div>
      <Tabs idPrefix={`${frameId}-frame`} value={active} onChange={setActive} items={frameTabs} variant="line" size="sm" ariaLabel={`${title} views`} />
      <div className="demo-frame-body">
        {active === "preview" ? <div className="demo-preview" id={`${frameId}-frame-panel-preview`} role="tabpanel" aria-labelledby={`${frameId}-frame-tab-preview`}>{children}</div> : null}
        {active === "tsx" ? <pre className="demo-source" id={`${frameId}-frame-panel-tsx`} role="tabpanel" aria-labelledby={`${frameId}-frame-tab-tsx`}><code>{code}</code></pre> : null}
        {active === "css" ? <pre className="demo-source" id={`${frameId}-frame-panel-css`} role="tabpanel" aria-labelledby={`${frameId}-frame-tab-css`}><code>{css}</code></pre> : null}
      </div>
    </section>
  );
}
