import { Button } from "../actions/Button.tsx";
import { Icon } from "../ui/Icon.tsx";

export function JumbotronDemo() {
  return (
    <div className="rounded-lg bg-gray-50 p-8 text-center md:p-12">
      <span className="eyebrow flex justify-center">Jumbotron</span>
      <h2 className="mt-4 text-4xl font-extrabold tracking-tight text-gray-900">We invest in the world’s potential</h2>
      <p className="mt-4 text-lg text-gray-600">Flowbite jumbotron replicated with utilitycss — deterministic CSS, no runtime.</p>
      <div className="mt-8 flex justify-center gap-3"><Button>Get started <Icon name="arrow" className="h-4 w-4" /></Button><Button variant="outline">Learn more</Button></div>
    </div>
  );
}
