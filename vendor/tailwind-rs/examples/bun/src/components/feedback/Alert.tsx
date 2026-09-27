import { useState } from "preact/hooks";
import { Icon } from "../ui/Icon.tsx";

type AlertVariant = "info" | "success" | "warning" | "danger" | "brand";

export function Alert({ variant, title, children, dismissible }: { variant: AlertVariant; title?: string; children: string; dismissible?: boolean }) {
  const [visible, setVisible] = useState(true);
  if (!visible) return null;
  return (
    <div className={`alert alert-${variant}`} role="alert">
      <span className="alert-icon-wrap">
        {variant === "info" ? <Icon name="info" className="h-5 w-5" /> : variant === "success" ? <Icon name="check" className="h-5 w-5" /> : variant === "warning" ? <Icon name="warning" className="h-5 w-5" /> : variant === "danger" ? <Icon name="danger" className="h-5 w-5" /> : <Icon name="spark" className="h-5 w-5" />}
      </span>
      <div className="alert-content">
        {title ? <strong>{title}</strong> : null}
        <span>{children}</span>
      </div>
      {dismissible ? <button type="button" aria-label="Dismiss" className="alert-close" onClick={() => setVisible(false)}><Icon name="close" className="h-4 w-4" /></button> : null}
    </div>
  );
}

export function AlertDemo() {
  return (
    <div className="space-y-3">
      <Alert variant="info" title="Info — " dismissible>Change a few things up and try submitting again.</Alert>
      <Alert variant="success" title="Success! ">Your profile has been updated.</Alert>
      <Alert variant="warning" title="Warning! ">Your trial ends in 3 days.</Alert>
      <Alert variant="danger" title="Error! ">We could not connect to the server.</Alert>
      <Alert variant="brand" title="Update — ">New Flowbite 2.5 components are now available.</Alert>
    </div>
  );
}
