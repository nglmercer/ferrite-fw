import { Badge } from "./Badge.tsx";
import { Button } from "../actions/Button.tsx";
import { Icon } from "../ui/Icon.tsx";
import { cn } from "../../lib/cn.ts";

export function CardDemo() {
  return (
    <div className="grid gap-6 lg:grid-cols-3">
      <article className="overflow-hidden rounded-lg border border-gray-200 bg-white shadow-sm">
        <div className="product-art flex aspect-video items-center justify-center bg-brand-600 text-white"><Icon name="spark" className="h-12 w-12" /></div>
        <div className="p-5">
          <div className="flex items-start justify-between gap-3"><div><span className="text-xs font-semibold uppercase tracking-wider text-brand-700">Starter kit</span><h3 className="mt-2 text-lg font-semibold text-gray-900">Launch faster</h3></div><Badge variant="success">New</Badge></div>
          <p className="mt-3 text-sm leading-relaxed text-gray-600">A simple card pattern with media, metadata, and one clear action.</p>
          <Button variant="primary" className="mt-5 w-full">Get started <Icon name="arrow" className="h-4 w-4" /></Button>
        </div>
      </article>

      <article className="component-card">
        <div className="flex items-start justify-between gap-4"><div><span className="eyebrow">Profile</span><h3 className="mt-2 text-lg font-semibold text-gray-900">Team member</h3></div><button aria-label="More" className="rounded-lg p-2 text-gray-500 hover:bg-gray-100" type="button">•••</button></div>
        <div className="mt-6 flex items-center gap-4"><span className="avatar">JD</span><div><strong className="block text-base text-gray-900">Jordan Diaz</strong><span className="text-sm text-gray-500">Product designer</span></div></div>
        <div className="mt-6 grid grid-cols-2 gap-3"><div className="profile-stat"><strong>18</strong><small>projects</small></div><div className="profile-stat"><strong>4.9</strong><small>rating</small></div></div>
        <Button variant="outline" className="mt-5 w-full"><Icon name="user" className="h-4 w-4" />View profile</Button>
      </article>

      <article className="component-card">
        <div className="component-heading"><div><span className="eyebrow">Activity</span><h3>Recent updates</h3></div><Badge variant="neutral">Today</Badge></div>
        <div className="mt-5 space-y-4">
          <div className="activity-row"><span className="activity-icon activity-icon-green"><Icon name="check" className="h-4 w-4" /></span><div><strong>Build completed</strong><small>utilitycss-bun · 4 min ago</small></div></div>
          <div className="activity-row"><span className="activity-icon activity-icon-brand"><Icon name="spark" className="h-4 w-4" /></span><div><strong>Theme updated</strong><small>brand palette · 18 min ago</small></div></div>
          <div className="activity-row"><span className="activity-icon activity-icon-gray"><Icon name="user" className="h-4 w-4" /></span><div><strong>New teammate</strong><small>Jordan joined · 1 hr ago</small></div></div>
        </div>
      </article>
    </div>
  );
}

export function PricingCardDemo() {
  return (
    <div className="grid gap-6 md:grid-cols-3">
      {[
        { title: "Starter", price: "$19", features: ["3 projects", "5 GB storage", "Community support"], cta: "Choose Starter", featured: false },
        { title: "Pro", price: "$49", features: ["Unlimited projects", "50 GB storage", "Priority support"], cta: "Choose Pro", featured: true },
        { title: "Enterprise", price: "$99", features: ["SAML SSO", "Unlimited storage", "Dedicated support"], cta: "Contact sales", featured: false }
      ].map(card => (
        <div key={card.title} className={cn("pricing-card", card.featured && "pricing-card-featured")}>
          <h3 className="text-lg font-semibold text-gray-900">{card.title}</h3>
          <p className="mt-2 text-3xl font-bold tracking-tight text-gray-900">{card.price}<span className="text-sm font-normal text-gray-500">/month</span></p>
          <ul className="mb-0 mt-6 list-none space-y-2 p-0 text-sm text-gray-600">
            {card.features.map(f => <li key={f} className="flex items-center gap-2"><Icon name="check" className="h-4 w-4 text-green-600" />{f}</li>)}
          </ul>
          <Button variant={card.featured ? "primary" : "outline"} className="mt-6 w-full">{card.cta}</Button>
        </div>
      ))}
    </div>
  );
}

export function HorizontalCardDemo() {
  return (
    <div className="flex flex-col overflow-hidden rounded-lg border border-gray-200 bg-white shadow-sm md:flex-row">
      <div className="flex w-full items-center justify-center bg-gray-100 p-8 md:w-56"><Icon name="spark" className="h-10 w-10 text-gray-400" /></div>
      <div className="flex-1 p-6">
        <span className="text-xs font-semibold uppercase tracking-wider text-brand-700">Flowbite card</span>
        <h3 className="mt-2 text-lg font-semibold text-gray-900">Noteworthy technology acquisitions 2024</h3>
        <p className="mt-2 text-sm leading-relaxed text-gray-600">Here are the biggest enterprise acquisitions this year so far, in chronological order.</p>
        <a className="mt-4 inline-flex items-center gap-2 text-sm font-semibold text-brand-700 hover:text-brand-800" href="#/data-display/card">Read more <Icon name="arrow" className="h-4 w-4" /></a>
      </div>
    </div>
  );
}
