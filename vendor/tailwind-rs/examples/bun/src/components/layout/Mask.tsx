const masks = [
  { label: "Circle", shape: "mask-circle" },
  { label: "Blob", shape: "mask-blob" },
  { label: "Hexagon", shape: "mask-hexagon" },
  { label: "Diamond", shape: "mask-diamond" }
] as const;

/** Images cropped into decorative shapes with pure CSS. */
export function MaskDemo() {
  return (
    <div className="mask-row">
      {masks.map(mask => (
        <figure key={mask.label} className="mask-figure">
          <span className={`mask-shape ${mask.shape}`} aria-hidden="true" />
          <figcaption>{mask.label}</figcaption>
        </figure>
      ))}
    </div>
  );
}
