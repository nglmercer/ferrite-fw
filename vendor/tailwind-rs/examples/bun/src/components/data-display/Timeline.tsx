import { Icon } from "../ui/Icon.tsx";

export function TimelineDemo() {
  return (
    <ol className="timeline">
      <li className="timeline-item"><span className="timeline-dot"><Icon name="calendar" className="h-4 w-4" /></span><div className="timeline-content"><time className="text-xs text-gray-500">Jan 13, 2024</time><h4 className="text-sm font-semibold text-gray-900">Flowbite Application UI v2.0.0</h4><p className="text-sm text-gray-600">Get access to over 20+ pages with Figma and code.</p></div></li>
      <li className="timeline-item"><span className="timeline-dot"><Icon name="check" className="h-4 w-4" /></span><div className="timeline-content"><time className="text-xs text-gray-500">Dec 7, 2023</time><h4 className="text-sm font-semibold text-gray-900">Marketing UI code in Flowbite</h4></div></li>
      <li className="timeline-item"><span className="timeline-dot timeline-dot-last"><Icon name="clock" className="h-4 w-4" /></span><div className="timeline-content"><time className="text-xs text-gray-500">Dec 2, 2023</time><h4 className="text-sm font-semibold text-gray-900">utilitycss parity added</h4></div></li>
    </ol>
  );
}
