import { useState } from "preact/hooks";

import { Button, ButtonGroupDemo, ButtonSizesDemo, ButtonWithIconsDemo } from "../../components/actions/Button.tsx";
import { DropdownDemo } from "../../components/actions/Dropdown.tsx";
import { FabDemo } from "../../components/actions/Fab.tsx";
import { ModalDemo } from "../../components/actions/Modal.tsx";
import { PopoverDemo } from "../../components/actions/Popover.tsx";
import { SwapDemo } from "../../components/actions/Swap.tsx";
import { ThemeControllerDemo } from "../../components/actions/ThemeController.tsx";
import { Icon } from "../../components/ui/Icon.tsx";
import { ExampleTabs } from "../ExampleTabs.tsx";

export function ButtonPage() {
  return (
    <ExampleTabs
      id="button-examples"
      items={[
        { value: "variants", label: "Variants", content: <div className="flex flex-wrap items-center gap-3"><Button><Icon name="check" className="h-4 w-4" />Primary</Button><Button variant="secondary">Secondary</Button><Button variant="outline">Outline</Button><Button variant="ghost">Ghost</Button><Button variant="danger">Delete</Button><Button variant="success">Success</Button></div> },
        { value: "sizes", label: "Sizes", content: <ButtonSizesDemo /> },
        { value: "icons", label: "Icons", content: <ButtonWithIconsDemo /> },
        { value: "groups", label: "Groups", content: <ButtonGroupDemo /> },
        { value: "states", label: "States", content: <div className="flex flex-wrap items-center gap-3"><Button disabled>Disabled</Button><Button aria-busy="true"><span className="spinner spinner-sm" />Saving…</Button></div> }
      ]}
    />
  );
}

export function DropdownPage() { return <DropdownDemo />; }

export function ModalPage() {
  const [open, setOpen] = useState(false);
  return <div><Button onClick={() => setOpen(true)}>Open modal <Icon name="arrow" className="h-4 w-4" /></Button><ModalDemo open={open} onClose={() => setOpen(false)} /></div>;
}

export function PopoverPage() { return <PopoverDemo />; }

export function FabPage() { return <FabDemo />; }

export function SwapPage() { return <SwapDemo />; }

export function ThemeControllerPage() { return <ThemeControllerDemo />; }
