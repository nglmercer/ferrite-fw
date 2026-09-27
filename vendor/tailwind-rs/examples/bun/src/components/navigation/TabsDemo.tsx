import { useState } from "preact/hooks";
import { Tabs } from "../ui/Tabs.tsx";

export function TabsDemo() {
  const tabs = [
    { value: "profile", label: "Profile" },
    { value: "dashboard", label: "Dashboard" },
    { value: "settings", label: "Settings" },
    { value: "contacts", label: "Contacts" }
  ];
  const [active, setActive] = useState("dashboard");
  return (
    <div className="w-full">
      <Tabs idPrefix="navigation-demo" value={active} onChange={setActive} items={tabs} variant="border" ariaLabel="Navigation demo tabs" />
      <div className="rounded-b-lg border border-gray-200 bg-white p-4 text-sm text-gray-600" id={`navigation-demo-panel-${active}`} role="tabpanel" aria-labelledby={`navigation-demo-tab-${active}`}>Content for <strong>{tabs.find(tab => tab.value === active)?.label}</strong> — Flowbite tabs replicated with utilitycss utilities + tiny state.</div>
    </div>
  );
}
