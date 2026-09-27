import { useState } from "preact/hooks";

import { BreadcrumbDemo } from "../../components/navigation/Breadcrumb.tsx";
import { DockDemo } from "../../components/navigation/Dock.tsx";
import { LinkDemo } from "../../components/navigation/Link.tsx";
import { MegaMenuDemo } from "../../components/navigation/MegaMenu.tsx";
import { MenuDemo } from "../../components/navigation/Menu.tsx";
import { NavbarDemo } from "../../components/navigation/Navbar.tsx";
import { PaginationDemo } from "../../components/navigation/Pagination.tsx";
import { SidebarDemo } from "../../components/navigation/Sidebar.tsx";
import { StepperDemo } from "../../components/navigation/Steps.tsx";
import { TabsDemo } from "../../components/navigation/TabsDemo.tsx";
import { Tabs } from "../../components/ui/Tabs.tsx";
import { ExampleTabs } from "../ExampleTabs.tsx";

const variantShowcase = [
  { value: "line", label: "Line" },
  { value: "box", label: "Box" },
  { value: "border", label: "Border" },
  { value: "lift", label: "Lift" }
] as const;

function TabsVariantsDemo() {
  const [active, setActive] = useState("box");
  return (
    <div className="space-y-5">
      <Tabs
        idPrefix="tabs-variants"
        value={active}
        onChange={setActive}
        items={[{ value: "line", label: "Line" }, { value: "box", label: "Box" }, { value: "border", label: "Border" }, { value: "lift", label: "Lift" }]}
        variant={active as "line" | "box" | "border" | "lift"}
        ariaLabel="Tabs style variants"
      />
      <p className="text-sm text-gray-600">The same primitive renders <strong>{variantShowcase.find(entry => entry.value === active)?.label}</strong> chrome; sizes <span className="font-mono text-xs">sm / md / lg</span> scale padding and type.</p>
      <Tabs idPrefix="tabs-sizes" value="md" onChange={() => undefined} items={[{ value: "sm", label: "Small" }, { value: "md", label: "Medium" }, { value: "lg", label: "Large" }]} variant="box" size="sm" ariaLabel="Tabs sizes" />
    </div>
  );
}

export function BreadcrumbsPage() { return <BreadcrumbDemo />; }
export function DockPage() { return <DockDemo />; }
export function LinkPage() { return <LinkDemo />; }
export function MegaMenuPage() { return <MegaMenuDemo />; }
export function MenuPage() { return <MenuDemo />; }
export function NavbarPage() { return <NavbarDemo />; }
export function PaginationPage() { return <PaginationDemo />; }
export function SidebarPage() { return <SidebarDemo />; }
export function StepsPage() { return <StepperDemo />; }

export function TabsPage() {
  return <ExampleTabs id="tabs-examples" items={[
    { value: "demo", label: "Demo", content: <TabsDemo /> },
    { value: "variants", label: "Variants", content: <TabsVariantsDemo /> }
  ]} />;
}
