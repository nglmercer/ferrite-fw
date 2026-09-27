import { useState } from "preact/hooks";

import { Button } from "../../components/actions/Button.tsx";
import { AlertDemo } from "../../components/feedback/Alert.tsx";
import { BannerDemo } from "../../components/feedback/Banner.tsx";
import { ProgressDemo } from "../../components/feedback/Progress.tsx";
import { RadialProgressDemo } from "../../components/feedback/RadialProgress.tsx";
import { SkeletonDemo } from "../../components/feedback/Skeleton.tsx";
import { SpinnerDemo } from "../../components/feedback/Spinner.tsx";
import { ToastDemo } from "../../components/feedback/Toast.tsx";
import { TooltipDemo } from "../../components/feedback/Tooltip.tsx";

export function AlertPage() { return <AlertDemo />; }

export function BannerPage() {
  const [visible, setVisible] = useState(true);
  return visible ? <BannerDemo onDismiss={() => setVisible(false)} /> : <Button variant="outline" onClick={() => setVisible(true)}>Show banner</Button>;
}

export function ProgressPage() { return <ProgressDemo />; }
export function RadialProgressPage() { return <RadialProgressDemo />; }
export function SkeletonPage() { return <SkeletonDemo />; }
export function SpinnerPage() { return <SpinnerDemo />; }
export function ToastPage() { return <ToastDemo />; }
export function TooltipPage() { return <TooltipDemo />; }
