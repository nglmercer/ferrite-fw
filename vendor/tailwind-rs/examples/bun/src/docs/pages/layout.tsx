import { useState } from "preact/hooks";

import { Button } from "../../components/actions/Button.tsx";
import { DividerDemo } from "../../components/layout/Divider.tsx";
import { DrawerDemo } from "../../components/layout/Drawer.tsx";
import { FooterDemo } from "../../components/layout/Footer.tsx";
import { JumbotronDemo } from "../../components/layout/Hero.tsx";
import { IndicatorDemo } from "../../components/layout/Indicator.tsx";
import { JoinDemo } from "../../components/layout/Join.tsx";
import { MaskDemo } from "../../components/layout/Mask.tsx";
import { StackDemo } from "../../components/layout/Stack.tsx";

export function DividerPage() { return <DividerDemo />; }

export function DrawerPage() {
  const [open, setOpen] = useState(false);
  return <><Button variant="outline" onClick={() => setOpen(true)}>Open drawer</Button><DrawerDemo open={open} onClose={() => setOpen(false)} /></>;
}

export function FooterPage() { return <FooterDemo />; }
export function JumbotronPage() { return <JumbotronDemo />; }
export function IndicatorPage() { return <IndicatorDemo />; }
export function JoinPage() { return <JoinDemo />; }
export function MaskPage() { return <MaskDemo />; }
export function StackPage() { return <StackDemo />; }
