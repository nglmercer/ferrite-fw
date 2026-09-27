import { useState } from "preact/hooks";

import { ModalDemo } from "../../components/actions/Modal.tsx";
import { Accordion, AccordionFlushDemo } from "../../components/data-display/Accordion.tsx";
import { AvatarDemo } from "../../components/data-display/Avatar.tsx";
import { BadgeDemo } from "../../components/data-display/Badge.tsx";
import { CardDemo, HorizontalCardDemo, PricingCardDemo } from "../../components/data-display/Card.tsx";
import { CarouselDemo } from "../../components/data-display/Carousel.tsx";
import { ChatBubbleDemo } from "../../components/data-display/ChatBubble.tsx";
import { CollapseDemo } from "../../components/data-display/Collapse.tsx";
import { CountdownDemo } from "../../components/data-display/Countdown.tsx";
import { DiffDemo } from "../../components/data-display/Diff.tsx";
import { KbdDemo } from "../../components/data-display/Kbd.tsx";
import { ListGroupDemo } from "../../components/data-display/ListGroup.tsx";
import { StatDemo } from "../../components/data-display/Stat.tsx";
import { StatusDemo } from "../../components/data-display/Status.tsx";
import { TableSection } from "../../components/data-display/Table.tsx";
import { TimelineDemo } from "../../components/data-display/Timeline.tsx";
import { ExampleTabs } from "../ExampleTabs.tsx";

export function AccordionPage() {
  return <ExampleTabs id="accordion-examples" items={[
    { value: "default", label: "Default", content: <Accordion /> },
    { value: "flush", label: "Flush", content: <AccordionFlushDemo /> }
  ]} />;
}

export function AvatarPage() { return <AvatarDemo />; }
export function BadgePage() { return <BadgeDemo />; }

export function CardPage() {
  return <ExampleTabs id="card-examples" items={[
    { value: "basic", label: "Basic", content: <CardDemo /> },
    { value: "pricing", label: "Pricing", content: <PricingCardDemo /> },
    { value: "horizontal", label: "Horizontal", content: <HorizontalCardDemo /> }
  ]} />;
}

export function CarouselPage() { return <CarouselDemo />; }
export function ChatBubblePage() { return <ChatBubbleDemo />; }
export function CollapsePage() { return <CollapseDemo />; }
export function CountdownPage() { return <CountdownDemo />; }
export function DiffPage() { return <DiffDemo />; }
export function KbdPage() { return <KbdDemo />; }
export function ListPage() { return <ListGroupDemo />; }
export function StatPage() { return <StatDemo />; }
export function StatusPage() { return <StatusDemo />; }

export function TablePage() {
  const [open, setOpen] = useState(false);
  return <><TableSection onNew={() => setOpen(true)} /><ModalDemo open={open} onClose={() => setOpen(false)} /></>;
}

export function TimelinePage() { return <TimelineDemo />; }
