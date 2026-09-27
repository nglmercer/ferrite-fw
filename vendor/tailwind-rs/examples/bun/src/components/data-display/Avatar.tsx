import { Icon } from "../ui/Icon.tsx";

export function AvatarDemo() {
  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center gap-4">
        <img className="h-8 w-8 rounded-full object-cover" alt="Avatar small" src="https://i.pravatar.cc/100?img=11" />
        <img className="h-10 w-10 rounded-full object-cover" alt="Avatar" src="https://i.pravatar.cc/100?img=12" />
        <img className="h-14 w-14 rounded-full object-cover" alt="Avatar large" src="https://i.pravatar.cc/100?img=13" />
        <span className="avatar">JD</span>
        <span className="avatar avatar-small">AL</span>
        <span className="inline-flex h-10 w-10 items-center justify-center rounded-full bg-gray-200 text-gray-600"><Icon name="user" className="h-5 w-5" /></span>
      </div>
      <div className="flex items-center gap-2">
        <div className="avatar-stack">
          <img className="avatar avatar-stacked" alt="a1" src="https://i.pravatar.cc/100?img=14" />
          <img className="avatar avatar-stacked" alt="a2" src="https://i.pravatar.cc/100?img=15" />
          <img className="avatar avatar-stacked" alt="a3" src="https://i.pravatar.cc/100?img=16" />
          <span className="avatar avatar-stacked avatar-more">+3</span>
        </div>
        <span className="ml-2 text-sm text-gray-600">Stacked avatars</span>
      </div>
      <div className="flex items-center gap-4">
        <span className="relative inline-flex"><img className="h-10 w-10 rounded-full" alt="online" src="https://i.pravatar.cc/100?img=20" /><span className="absolute bottom-0 right-0 h-3 w-3 rounded-full border-2 border-white bg-green-500" /></span>
        <span className="text-sm text-gray-600">Dot indicator (online)</span>
      </div>
    </div>
  );
}
