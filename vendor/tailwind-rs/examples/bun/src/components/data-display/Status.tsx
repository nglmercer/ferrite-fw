import { cn } from "../../lib/cn.ts";

type StatusTone = "success" | "warning" | "danger" | "info" | "neutral";

const toneClasses: Record<StatusTone, string> = {
  success: "status-dot-live status-success",
  warning: "status-dot-live status-warning",
  danger: "status-dot-live status-danger",
  info: "status-dot-live status-info",
  neutral: "status-dot-live status-neutral"
};

const statuses: readonly { tone: StatusTone; label: string; pulse?: boolean }[] = [
  { tone: "success", label: "All systems operational", pulse: true },
  { tone: "warning", label: "Degraded performance" },
  { tone: "danger", label: "Major outage" },
  { tone: "info", label: "Scheduled maintenance" },
  { tone: "neutral", label: "Unknown" }
];

/** Presence-style status dots with optional pulse for live states. */
export function StatusDemo() {
  return (
    <ul className="status-list">
      {statuses.map(status => (
        <li key={status.label}>
          <span className={cn(toneClasses[status.tone], status.pulse && "status-pulse")} aria-hidden="true" />
          <span>{status.label}</span>
        </li>
      ))}
    </ul>
  );
}
