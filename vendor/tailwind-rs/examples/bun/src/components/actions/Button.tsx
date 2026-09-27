import type { ComponentChildren, JSX } from "preact";
import { Icon } from "../ui/Icon.tsx";
import { cn } from "../../lib/cn.ts";

type ButtonVariant = "primary" | "secondary" | "outline" | "ghost" | "danger" | "success";
type ButtonSize = "xs" | "sm" | "md" | "lg";

const variantClasses: Record<ButtonVariant, string> = {
  primary: "button button-primary",
  secondary: "button button-secondary",
  outline: "button button-outline",
  ghost: "button button-ghost",
  danger: "button button-danger",
  success: "button button-success"
};

const sizeClasses: Record<ButtonSize, string> = {
  xs: "button-small",
  sm: "button-sm",
  md: "",
  lg: "button-lg"
};

type ButtonProps = Omit<JSX.ButtonHTMLAttributes<HTMLButtonElement>, "className"> & {
  readonly variant?: ButtonVariant;
  readonly size?: ButtonSize;
  readonly className?: string;
  readonly children?: ComponentChildren;
};

export function Button({ variant = "primary", size = "md", className = "", children, ...props }: ButtonProps) {
  return (
    <button {...props} className={cn(variantClasses[variant], sizeClasses[size], className)}>
      {children}
    </button>
  );
}

export function ButtonGroupDemo() {
  return (
    <div className="flex flex-wrap gap-4">
      <div className="button-group" role="group" aria-label="Flowbite button group">
        <button type="button" className="button-group-item">Profile</button>
        <button type="button" className="button-group-item">Settings</button>
        <button type="button" className="button-group-item">Messages</button>
      </div>
      <div className="button-group" role="group" aria-label="Icon button group">
        <button type="button" className="button-group-item-icon" aria-label="Search"><Icon name="search" className="h-4 w-4" /></button>
        <button type="button" className="button-group-item-icon" aria-label="Notifications"><Icon name="bell" className="h-4 w-4" /></button>
        <button type="button" className="button-group-item-icon" aria-label="Settings"><Icon name="settings" className="h-4 w-4" /></button>
      </div>
    </div>
  );
}

export function ButtonSizesDemo() {
  return (
    <div className="flex flex-wrap items-center gap-3">
      <Button size="xs">Extra small</Button>
      <Button size="sm">Small</Button>
      <Button size="md">Base</Button>
      <Button size="lg">Large</Button>
      <Button disabled>Disabled</Button>
    </div>
  );
}

export function ButtonWithIconsDemo() {
  return (
    <div className="flex flex-wrap items-center gap-3">
      <Button variant="primary"><Icon name="cart" className="h-4 w-4" />Buy now</Button>
      <Button variant="outline"><Icon name="heart" className="h-4 w-4" />Add to cart</Button>
      <Button variant="ghost">View more<Icon name="arrow" className="h-4 w-4" /></Button>
    </div>
  );
}
