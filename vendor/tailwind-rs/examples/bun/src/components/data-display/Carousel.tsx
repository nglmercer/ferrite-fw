import { useState } from "preact/hooks";
import { Icon } from "../ui/Icon.tsx";

const slides = [
  { bg: "bg-brand-600", label: "Slide 1 — utilitycss" },
  { bg: "bg-gray-800", label: "Slide 2 — deterministic CSS" },
  { bg: "bg-blue-600", label: "Slide 3 — Bun HMR" }
];

export function CarouselDemo() {
  const [idx, setIdx] = useState(0);
  return (
    <div className="relative overflow-hidden rounded-lg border border-gray-200 bg-white" role="region" aria-roledescription="carousel" aria-label="Highlights">
      <span className="hidden bg-blue-600 bg-brand-600 bg-gray-800" aria-hidden="true" />
      <div className={`carousel-slide ${slides[idx].bg}`} aria-live="polite">
        <span className="text-lg font-semibold">{slides[idx].label}</span>
      </div>
      <button type="button" aria-label="Previous" className="carousel-control carousel-control-prev" onClick={() => setIdx(i => (i - 1 + slides.length) % slides.length)}><Icon name="chevron-right" className="h-5 w-5 rotate-180" /></button>
      <button type="button" aria-label="Next" className="carousel-control carousel-control-next" onClick={() => setIdx(i => (i + 1) % slides.length)}><Icon name="chevron-right" className="h-5 w-5" /></button>
      <div className="absolute bottom-3 left-0 flex w-full justify-center gap-2">
        {slides.map((_, i) => <button key={i} aria-label={`Go to slide ${i + 1}`} className={`carousel-dot ${i === idx ? "carousel-dot-active" : ""}`} onClick={() => setIdx(i)} />)}
      </div>
    </div>
  );
}
