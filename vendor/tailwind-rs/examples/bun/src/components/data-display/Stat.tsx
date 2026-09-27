interface StatFigure {
  readonly label: string;
  readonly value: string;
  readonly delta: string;
  readonly tone: "up" | "down" | "flat";
}

const figures: readonly StatFigure[] = [
  { label: "Builds this week", value: "1,284", delta: "+12.4%", tone: "up" },
  { label: "Median build time", value: "842ms", delta: "-9.1%", tone: "up" },
  { label: "Failed deploys", value: "3", delta: "+1", tone: "down" }
];

const toneClasses: Record<StatFigure["tone"], string> = {
  up: "stat-delta stat-delta-up",
  down: "stat-delta stat-delta-down",
  flat: "stat-delta"
};

/** Metric tiles with headline values and trend deltas. */
export function StatDemo() {
  return (
    <div className="grid gap-4 md:grid-cols-3">
      {figures.map(figure => (
        <div key={figure.label} className="stat-card">
          <span className="eyebrow">{figure.label}</span>
          <strong>{figure.value}</strong>
          <small className={toneClasses[figure.tone]}>{figure.delta} vs last week</small>
        </div>
      ))}
    </div>
  );
}
