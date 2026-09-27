export function Button({ active }: { active: boolean }) {
  return <button className={clsx("flex", active && "p-4")} />;
}
