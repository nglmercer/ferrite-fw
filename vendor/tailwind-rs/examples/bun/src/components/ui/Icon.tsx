import type { ComponentChildren } from "preact";

export type IconName =
  | "arrow"
  | "check"
  | "close"
  | "menu"
  | "spark"
  | "user"
  | "star"
  | "cart"
  | "heart"
  | "search"
  | "bell"
  | "info"
  | "warning"
  | "danger"
  | "chevron-down"
  | "chevron-up"
  | "chevron-right"
  | "dots"
  | "trash"
  | "edit"
  | "eye"
  | "plus"
  | "minus"
  | "calendar"
  | "clock"
  | "mail"
  | "phone"
  | "home"
  | "settings"
  | "sun"
  | "moon"
  | "send"
  | "copy"
  | "download"
  | "filter"
  | "heart-filled";

export function Icon({ name, className = "h-5 w-5" }: { name: IconName; className?: string }) {
  const paths: Record<IconName, ComponentChildren> = {
    arrow: <path d="M5 12h14m-6-6 6 6-6 6" />,
    check: <path d="m5 12 4 4L19 6" />,
    close: <path d="m6 6 12 12M18 6 6 18" />,
    menu: <path d="M4 7h16M4 12h16M4 17h16" />,
    spark: <path d="m12 3 1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9L12 3Zm6.5 11.5.7 1.8 1.8.7-1.8.7-.7 1.8-.7-1.8-1.8-.7 1.8-.7.7-1.8Z" />,
    user: <path d="M19 21a7 7 0 0 0-14 0m7-11a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z" />,
    star: <path d="m12 3 2.5 5 5.5.8-4 3.9.9 5.3L12 15.8 7.1 18l.9-5.3-4-3.9 5.5-.8L12 3Z" />,
    cart: <path d="M6 6h15l-1.5 9H7L6 6Zm4 13a1 1 0 1 0 0 2 1 1 0 0 0 0-2Zm6 0a1 1 0 1 0 0 2 1 1 0 0 0 0-2Z" />,
    heart: <path d="M12 21s-6.7-4.2-8.6-8.2A4.5 4.5 0 0 1 12 6c1.5 0 3 .8 3.9 2A4.5 4.5 0 0 1 20.6 13C18.7 16.8 12 21 12 21Z" />,
    search: <path d="m21 21-4.3-4.3M10 18a8 8 0 1 1 0-16 8 8 0 0 1 0 16Z" />,
    bell: <path d="M6 13V9a6 6 0 1 1 12 0v4l2 2v1H4v-1l2-2Z M10 20a2 2 0 0 0 4 0" />,
    info: <path d="M12 8h.01M12 12v6 M12 22a10 10 0 1 1 0-20 10 10 0 0 1 0 20Z" />,
    warning: <path d="M12 9v6m0 4h.01 M10.3 3 2.3 16a2 2 0 0 0 1.7 3h16a2 2 0 0 0 1.7-3L13.7 3a2 2 0 0 0-3.4 0Z" />,
    danger: <path d="M12 22a10 10 0 1 1 0-20 10 10 0 0 1 0 20Zm0-6h.01M12 8v6" />,
    "chevron-down": <path d="m6 9 6 6 6-6" />,
    "chevron-up": <path d="m6 15 6-6 6 6" />,
    "chevron-right": <path d="m9 6 6 6-6 6" />,
    dots: <path d="M12 13a1 1 0 1 1 0-2 1 1 0 0 1 0 2Zm6 0a1 1 0 1 1 0-2 1 1 0 0 1 0 2Zm-12 0a1 1 0 1 1 0-2 1 1 0 0 1 0 2Z" />,
    trash: <path d="M3 6h18M8 6V4h8v2M10 11v6M14 11v6M5 6l1 14h12l1-14" />,
    edit: <path d="M11 4H5a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2v-6M18 3a2.8 2.8 0 0 1 4 4L12 17l-4 1 1-4 9-11Z" />,
    eye: <path d="M1 12s4-7 11-7 11 7 11 7-4 7-11 7S1 12 1 12Zm11 3a3 3 0 1 1 0-6 3 3 0 0 1 0 6Z" />,
    plus: <path d="M12 5v14M5 12h14" />,
    minus: <path d="M5 12h14" />,
    calendar: <path d="M8 2v3M16 2v3M3 8h18M5 5h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2Z" />,
    clock: <path d="M12 6v6l4 2M12 22a10 10 0 1 1 0-20 10 10 0 0 1 0 20Z" />,
    mail: <path d="M4 6h16a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2Zm0 2 8 6 8-6" />,
    phone: <path d="M22 16.9v3a2 2 0 0 1-2.2 2A18 18 0 0 1 3.1 5.2 2 2 0 0 1 5 3h3a2 2 0 0 1 2 1.7l.4 2.8a2 2 0 0 1-.6 1.7l-1.5 1.5a16 16 0 0 0 6.2 6.2l1.5-1.5a2 2 0 0 1 1.7-.6l2.8.4A2 2 0 0 1 22 16.9Z" />,
    home: <path d="M3 9 12 2l9 7v11a2 2 0 0 1-2 2h-3v-7H8v7H5a2 2 0 0 1-2-2V9Z" />,
    settings: <path d="M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6V21a2 2 0 0 1-4 0v-.5a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.6-1H2a2 2 0 0 1 0-4h.5a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1A1.7 1.7 0 0 0 8.5 4a1.7 1.7 0 0 0 1-1.6V2a2 2 0 0 1 4 0v.5a1.7 1.7 0 0 0 1 1.6c.6.2 1.3.1 1.9-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1A1.7 1.7 0 0 0 19.4 9c.2.6.1 1.3-.3 1.9a1.7 1.7 0 0 0 1.6 1H21a2 2 0 0 1 0 4h-.5a1.7 1.7 0 0 0-1.6 1Z" />,
    sun: <><circle cx="12" cy="12" r="4" /><path d="M12 2v2m0 16v2M4.9 4.9l1.4 1.4m11.4 11.4 1.4 1.4M2 12h2m16 0h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" /></>,
    moon: <path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8Z" />,
    send: <path d="m22 2-7 20-4-9-9-4 20-7Z" />,
    copy: <><rect x="9" y="9" width="12" height="12" rx="2" /><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" /></>,
    download: <path d="M12 3v12m0 0 4-4m-4 4-4-4M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2" />,
    filter: <path d="M4 5h16l-6 7v5l-4 2v-7L4 5Z" />,
    "heart-filled": <path d="M12 21s-6.7-4.2-8.6-8.2A4.5 4.5 0 0 1 12 6c1.5 0 3 .8 3.9 2A4.5 4.5 0 0 1 20.6 13C18.7 16.8 12 21 12 21Z" fill="currentColor" stroke="none" />
  };
  return (
    <svg aria-hidden="true" className={className} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
      {paths[name]}
    </svg>
  );
}
