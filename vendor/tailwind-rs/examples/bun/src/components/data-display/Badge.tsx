import type { ComponentChildren } from "preact";
import { Icon } from "../ui/Icon.tsx";
import { cn } from "../../lib/cn.ts";

type BadgeVariant = "success" | "warning" | "danger" | "neutral" | "brand" | "info";
type BadgeSize = "sm" | "md" | "lg";

const badgeStyles: Record<BadgeVariant, string> = {
  success: "badge badge-success",
  warning: "badge badge-warning",
  danger: "badge badge-danger",
  neutral: "badge badge-neutral",
  brand: "badge badge-brand",
  info: "badge badge-info"
};

export function Badge({ variant, children, size = "md", withDot = false, dismissible = false, onDismiss }: { variant: BadgeVariant; children: ComponentChildren; size?: BadgeSize; withDot?: boolean; dismissible?: boolean; onDismiss?: () => void }) {
  const sizeClass = size === "sm" ? "badge-sm" : size === "lg" ? "badge-lg" : "";
  return (
    <span className={cn(badgeStyles[variant], sizeClass)}>
      {withDot ? <span className={`badge-dot badge-dot-${variant}`} aria-hidden="true" /> : null}
      {children}
      {dismissible ? <button type="button" aria-label="Dismiss" className="badge-dismiss" onClick={onDismiss}><Icon name="close" className="h-3 w-3" /></button> : null}
    </span>
  );
}

export function BadgeDemo() {
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant="brand">Default</Badge>
        <Badge variant="success">Published</Badge>
        <Badge variant="warning">Review</Badge>
        <Badge variant="danger">Failed</Badge>
        <Badge variant="neutral">Draft</Badge>
        <Badge variant="info">New</Badge>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant="success" withDot>Active</Badge>
        <Badge variant="warning" withDot>Pending</Badge>
        <Badge variant="danger" size="sm">Small</Badge>
        <Badge variant="brand" size="lg">Large</Badge>
        <Badge variant="neutral" dismissible>Dismissible</Badge>
      </div>
      <div className="flex items-center gap-3 rounded-lg bg-gray-50 p-4">
        <span className="avatar avatar-small">AL</span>
        <div><strong className="block text-sm text-gray-900">Alex Lee</strong><span className="text-xs text-gray-500">Maintainer · 2 minutes ago</span></div>
        <Badge variant="success">Online</Badge>
      </div>
    </div>
  );
}
