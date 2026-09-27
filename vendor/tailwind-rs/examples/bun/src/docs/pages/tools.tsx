import { MotionKeyframesDemo, MotionLoadingDemo, MotionReducedDemo, MotionTransitionsDemo } from "../../components/tools/Motion.tsx";
import { ExampleTabs } from "../ExampleTabs.tsx";

export function MotionPage() {
  return <ExampleTabs id="motion-examples" items={[
    { value: "keyframes", label: "Keyframes", content: <MotionKeyframesDemo /> },
    { value: "transitions", label: "Transitions", content: <MotionTransitionsDemo /> },
    { value: "loading", label: "Loading", content: <MotionLoadingDemo /> },
    { value: "reduced", label: "Reduced motion", content: <MotionReducedDemo /> }
  ]} />;
}
