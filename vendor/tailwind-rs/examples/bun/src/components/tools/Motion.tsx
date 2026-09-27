import type { ComponentChildren } from "preact";
import { useState } from "preact/hooks";

import { Button } from "../actions/Button.tsx";
import { Badge } from "../data-display/Badge.tsx";
import { SpinnerDemo } from "../feedback/Spinner.tsx";

/** Re-mounts its child so a keyframe entrance can be replayed on demand. */
export function ReplayStage({ title, animation, children }: { title: string; animation: string; children: ComponentChildren }) {
  const [run, setRun] = useState(0);
  return (
    <div className="motion-stage">
      <div key={run} className={animation}>{children}</div>
      <Button variant="outline" size="sm" onClick={() => setRun(value => value + 1)}>Replay {title}</Button>
    </div>
  );
}

export function MotionKeyframesDemo() {
  return (
    <div className="grid gap-4 md:grid-cols-2">
      <ReplayStage title="fade" animation="animate-fade-in"><Badge variant="brand">Fade in</Badge></ReplayStage>
      <ReplayStage title="slide" animation="animate-slide-up"><Badge variant="success">Slide up</Badge></ReplayStage>
      <ReplayStage title="scale" animation="animate-scale-in"><Badge variant="info">Scale in</Badge></ReplayStage>
      <ReplayStage title="pop" animation="animate-pop"><Badge variant="warning">Pop</Badge></ReplayStage>
    </div>
  );
}

export function MotionTransitionsDemo() {
  return (
    <div className="space-y-5">
      <div className="space-y-2">
        <p className="text-sm font-semibold text-gray-700">Duration ladder — hover each row</p>
        <div className="flex items-center justify-between rounded-lg border border-gray-200 bg-white px-4 py-3 text-sm transition-all duration-150 ease-out hover:border-brand-600 hover:bg-brand-50"><strong>150ms</strong><span className="text-gray-500">snappy</span></div>
        <div className="flex items-center justify-between rounded-lg border border-gray-200 bg-white px-4 py-3 text-sm transition-all duration-300 ease-out hover:border-brand-600 hover:bg-brand-50"><strong>300ms</strong><span className="text-gray-500">default feel</span></div>
        <div className="flex items-center justify-between rounded-lg border border-gray-200 bg-white px-4 py-3 text-sm transition-all duration-500 ease-out hover:border-brand-600 hover:bg-brand-50"><strong>500ms</strong><span className="text-gray-500">deliberate</span></div>
        <div className="flex items-center justify-between rounded-lg border border-gray-200 bg-white px-4 py-3 text-sm transition-all duration-700 ease-out hover:border-brand-600 hover:bg-brand-50"><strong>700ms</strong><span className="text-gray-500">slow</span></div>
        <div className="flex items-center justify-between rounded-lg border border-gray-200 bg-white px-4 py-3 text-sm transition-all duration-1000 ease-out hover:border-brand-600 hover:bg-brand-50"><strong>1000ms</strong><span className="text-gray-500">glacial</span></div>
      </div>
      <div className="space-y-2">
        <p className="text-sm font-semibold text-gray-700">Easing curves — hover each card</p>
        <div className="grid gap-3 md:grid-cols-4">
          <div className="rounded-lg border border-gray-200 bg-white p-4 text-center text-sm font-medium transition-all duration-500 ease-linear hover:-translate-y-1 hover:shadow-lg">Linear</div>
          <div className="rounded-lg border border-gray-200 bg-white p-4 text-center text-sm font-medium transition-all duration-500 ease-in hover:-translate-y-1 hover:shadow-lg">Ease in</div>
          <div className="rounded-lg border border-gray-200 bg-white p-4 text-center text-sm font-medium transition-all duration-500 ease-out hover:-translate-y-1 hover:shadow-lg">Ease out</div>
          <div className="rounded-lg border border-gray-200 bg-white p-4 text-center text-sm font-medium transition-all duration-500 ease-in-out hover:-translate-y-1 hover:shadow-lg">Ease in-out</div>
        </div>
      </div>
      <div className="space-y-2">
        <p className="text-sm font-semibold text-gray-700">Delayed entrance — hover and wait</p>
        <div className="rounded-lg border border-gray-200 bg-white px-4 py-3 text-sm transition-all delay-300 duration-300 ease-out hover:bg-green-50 hover:text-green-700">Starts 300ms after hover</div>
      </div>
    </div>
  );
}

export function MotionLoadingDemo() {
  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center gap-6">
        <SpinnerDemo />
        <span className="loading-dots" aria-label="Loading"><span /><span className="delay-150" /><span className="delay-300" /></span>
      </div>
      <div className="animate-pulse-soft space-y-3" aria-label="Loading content">
        <div className="h-4 w-3/4 rounded bg-gray-200" />
        <div className="h-4 rounded bg-gray-200" />
        <div className="h-4 w-5/6 rounded bg-gray-200" />
      </div>
    </div>
  );
}

export function MotionReducedDemo() {
  return (
    <div className="space-y-4 text-sm leading-relaxed text-gray-700">
      <p>Every animation in this gallery honors <code className="rounded bg-gray-100 px-1 py-0.5 font-mono text-xs">prefers-reduced-motion</code>. With reduced motion enabled, keyframe entrances render in their final state, loading loops go static, and decorative transitions are removed — content never depends on motion to be understood.</p>
      <pre className="demo-source"><code>{`@media (prefers-reduced-motion: reduce) {
  .animate-fade-in, .animate-slide-up,
  .animate-scale-in, .animate-pop,
  .animate-pulse-soft, .animate-spin-slow {
    animation: none;
  }
  .spinner, .status-pulse { animation: none; }
  .swap-icon, .radial-value { transition: none; }
}`}</code></pre>
    </div>
  );
}
