import type { ComponentType } from "preact";

import {
  AccordionPage,
  AlertPage,
  AvatarPage,
  BadgePage,
  BannerPage,
  BreadcrumbsPage,
  BrowserMockupPage,
  ButtonPage,
  CalendarPage,
  CardPage,
  CarouselPage,
  ChatBubblePage,
  CheckboxPage,
  CodeMockupPage,
  CollapsePage,
  CountdownPage,
  DiffPage,
  DividerPage,
  DockPage,
  DrawerPage,
  DropdownPage,
  FabPage,
  FieldsetPage,
  FileInputPage,
  FilterPage,
  FooterPage,
  IndicatorPage,
  InputPage,
  JoinPage,
  JumbotronPage,
  KbdPage,
  LabelPage,
  LinkPage,
  ListPage,
  MaskPage,
  MegaMenuPage,
  MenuPage,
  ModalPage,
  MotionPage,
  NavbarPage,
  OtpPage,
  PaginationPage,
  PhoneMockupPage,
  PopoverPage,
  ProgressPage,
  RadialProgressPage,
  RadioPage,
  RangePage,
  RatingPage,
  SelectPage,
  SidebarPage,
  SkeletonPage,
  SpinnerPage,
  StackPage,
  StatPage,
  StatusPage,
  StepsPage,
  SwapPage,
  TablePage,
  TabsPage,
  TextareaPage,
  ThemeControllerPage,
  TimelinePage,
  ToastPage,
  TogglePage,
  TooltipPage,
  ValidatorPage,
  WindowMockupPage
} from "./docs/DemoPages.tsx";
import { CompilerLabPage } from "./lab/CompilerLab.tsx";

export const categoryDefinitions = [
  { value: "actions", label: "Actions" },
  { value: "data-display", label: "Data display" },
  { value: "navigation", label: "Navigation" },
  { value: "feedback", label: "Feedback" },
  { value: "data-input", label: "Data input" },
  { value: "layout", label: "Layout" },
  { value: "mockup", label: "Mockup" },
  { value: "tools", label: "Tools" }
] as const;

export type Category = typeof categoryDefinitions[number]["value"];

export interface CatalogItem {
  readonly slug: string;
  readonly name: string;
  readonly category: Category;
  readonly description: string;
  readonly demo: ComponentType;
  readonly code?: string;
  readonly css?: string;
}

export const catalog: readonly CatalogItem[] = [
  { slug: "button", name: "Button", category: "actions", description: "Buttons let people take clear, immediate actions.", demo: ButtonPage, code: `<Button variant="primary">Continue</Button>`, css: `.button-primary { @apply bg-brand-600 text-white shadow-sm; }` },
  { slug: "dropdown", name: "Dropdown", category: "actions", description: "Expose contextual actions without taking the user away from the current view.", demo: DropdownPage, code: `<Dropdown items={["Dashboard", "Settings"]} />` },
  { slug: "fab", name: "FAB", category: "actions", description: "Float one primary action above the content with an expandable speed-dial.", demo: FabPage, code: `<Fab actions={actions} />`, css: `.fab { @apply absolute bottom-4 right-4 inline-flex h-14 w-14 items-center justify-center rounded-full bg-brand-600 text-white shadow-lg; }` },
  { slug: "modal", name: "Modal", category: "actions", description: "Focus attention on a confirmation or short, high-priority task.", demo: ModalPage, code: `<Modal open={open} onClose={close}>...</Modal>` },
  { slug: "popover", name: "Popover", category: "actions", description: "Show supporting content next to the control that triggered it.", demo: PopoverPage, code: `<Popover>Additional context</Popover>` },
  { slug: "swap", name: "Swap", category: "actions", description: "Toggle between two states with an animated icon transition.", demo: SwapPage, code: `<Swap active={saved} onToggle={toggle} />` },
  { slug: "theme-controller", name: "Theme Controller", category: "actions", description: "Switch between light, dark, and system color themes.", demo: ThemeControllerPage, code: `<ThemeController value={theme} onChange={setTheme} />` },

  { slug: "accordion", name: "Accordion", category: "data-display", description: "Progressively disclose related content in compact panels.", demo: AccordionPage, code: `<Accordion items={items} />` },
  { slug: "avatar", name: "Avatar", category: "data-display", description: "Represent people, teams, and online presence with compact identity cues.", demo: AvatarPage, code: `<Avatar src="/avatar.png" alt="Jordan Diaz" />` },
  { slug: "badge", name: "Badge", category: "data-display", description: "Label statuses, metadata, and small pieces of supporting information.", demo: BadgePage, code: `<Badge variant="success">Published</Badge>` },
  { slug: "card", name: "Card", category: "data-display", description: "Compose media, metadata, content, and actions into a contained surface.", demo: CardPage, code: `<Card><CardBody>...</CardBody></Card>` },
  { slug: "carousel", name: "Carousel", category: "data-display", description: "Let people move through a small, related set of visual panels.", demo: CarouselPage, code: `<Carousel slides={slides} />` },
  { slug: "chat-bubble", name: "Chat Bubble", category: "data-display", description: "Render conversation messages with per-participant alignment.", demo: ChatBubblePage, code: `<ChatBubble author="Maya" own>Morning!</ChatBubble>`, css: `.chat-bubble { @apply max-w-md rounded-2xl rounded-tl-sm bg-gray-100 px-4 py-2 text-sm text-gray-800; }` },
  { slug: "collapse", name: "Collapse", category: "data-display", description: "Disclose one standalone section without leaving the page.", demo: CollapsePage, code: `<Collapse title="Details">...</Collapse>` },
  { slug: "countdown", name: "Countdown", category: "data-display", description: "Count down toward a launch, deadline, or live event.", demo: CountdownPage, code: `<Countdown target={launchAt} />` },
  { slug: "diff", name: "Diff", category: "data-display", description: "Compare before and after states with a draggable reveal.", demo: DiffPage, code: `<Diff before={...} after={...} />` },
  { slug: "kbd", name: "Kbd", category: "data-display", description: "Document keyboard shortcuts with keycap styling.", demo: KbdPage, code: `<Kbd>Ctrl</Kbd> + <Kbd>K</Kbd>`, css: `.kbd { @apply inline-flex items-center rounded-md border border-gray-200 bg-white px-2 py-1 font-mono text-xs font-semibold text-gray-700 shadow-sm; }` },
  { slug: "list", name: "List group", category: "data-display", description: "Present related rows with active, secondary, and action states.", demo: ListPage, code: `<ListGroup items={items} active="profile" />` },
  { slug: "stat", name: "Stat", category: "data-display", description: "Surface headline metrics with trend deltas at a glance.", demo: StatPage, code: `<Stat label="Builds" value="1,284" delta="+12%" />`, css: `.stat-card { @apply rounded-lg border border-gray-200 bg-white p-4 shadow-sm; }` },
  { slug: "status", name: "Status", category: "data-display", description: "Signal live presence and system health with status dots.", demo: StatusPage, code: `<Status tone="success">Operational</Status>` },
  { slug: "table", name: "Table", category: "data-display", description: "Scan structured records with responsive columns and row actions.", demo: TablePage, code: `<Table rows={deployments} />` },
  { slug: "timeline", name: "Timeline", category: "data-display", description: "Show chronological activity, history, or a sequence of milestones.", demo: TimelinePage, code: `<Timeline items={events} />` },

  { slug: "breadcrumbs", name: "Breadcrumbs", category: "navigation", description: "Give users a compact map of their location in a hierarchy.", demo: BreadcrumbsPage, code: `<Breadcrumbs items={items} />` },
  { slug: "dock", name: "Dock", category: "navigation", description: "Anchor app-like navigation to the bottom of the viewport.", demo: DockPage, code: `<Dock active="home" items={items} />`, css: `.dock { @apply flex items-stretch justify-around rounded-2xl border border-gray-200 bg-white shadow-lg; }` },
  { slug: "link", name: "Link", category: "navigation", description: "Style inline links for prose, navigation, and hover emphasis.", demo: LinkPage, code: `<a className="link link-primary">Guide</a>` },
  { slug: "megamenu", name: "Megamenu", category: "navigation", description: "Open a full-width panel of grouped destination links.", demo: MegaMenuPage, code: `<MegaMenu columns={columns} />` },
  { slug: "menu", name: "Menu", category: "navigation", description: "List sectioned destinations with active and disabled states.", demo: MenuPage, code: `<Menu sections={sections} active="dashboard" />`, css: `.menu-item { @apply flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium text-gray-700 hover:bg-gray-100; }` },
  { slug: "navbar", name: "Navbar", category: "navigation", description: "Provide a consistent top-level route between major areas.", demo: NavbarPage, code: `<Navbar brand="utilitycss" links={links} />` },
  { slug: "pagination", name: "Pagination", category: "navigation", description: "Move through a long collection of records one page at a time.", demo: PaginationPage, code: `<Pagination page={1} totalPages={5} />` },
  { slug: "sidebar", name: "Sidebar", category: "navigation", description: "Keep secondary navigation visible beside the current content.", demo: SidebarPage, code: `<Sidebar items={items} />` },
  { slug: "steps", name: "Steps", category: "navigation", description: "Make progress through a multi-step flow visible at a glance.", demo: StepsPage, code: `<Steps current={2} items={items} />` },
  { slug: "tabs", name: "Tabs", category: "navigation", description: "Switch between related views while keeping the surrounding context.", demo: TabsPage, code: `<Tabs value={active} onChange={setActive} items={items} />` },

  { slug: "alert", name: "Alert", category: "feedback", description: "Communicate important status, warning, success, or error information.", demo: AlertPage, code: `<Alert variant="success">Profile updated.</Alert>` },
  { slug: "banner", name: "Banner", category: "feedback", description: "Place a persistent, dismissible message across the page layout.", demo: BannerPage, code: `<Banner onDismiss={dismiss}>Announcement</Banner>` },
  { slug: "progress", name: "Progress", category: "feedback", description: "Show determinate progress toward a task or upload completion.", demo: ProgressPage, code: `<Progress value={70} />` },
  { slug: "radial-progress", name: "Radial Progress", category: "feedback", description: "Show determinate progress as a compact circular ring.", demo: RadialProgressPage, code: `<RadialProgress value={68} />`, css: `.radial-progress { @apply relative inline-flex h-24 w-24 items-center justify-center; }` },
  { slug: "skeleton", name: "Skeleton", category: "feedback", description: "Reserve space for content while a page or request is loading.", demo: SkeletonPage, code: `<Skeleton lines={3} />` },
  { slug: "spinner", name: "Spinner", category: "feedback", description: "Signal an in-progress operation when its duration is not yet known.", demo: SpinnerPage, code: `<Spinner size="md" />` },
  { slug: "toast", name: "Toast", category: "feedback", description: "Offer lightweight confirmation without interrupting the current task.", demo: ToastPage, code: `<Toast>Item moved successfully.</Toast>` },
  { slug: "tooltip", name: "Tooltip", category: "feedback", description: "Reveal helper text on hover and keyboard focus.", demo: TooltipPage, code: `<Tooltip tip="Shown above">Trigger</Tooltip>`, css: `.tooltip-bubble { @apply pointer-events-none absolute rounded-lg bg-gray-900 px-3 py-2 text-xs font-medium text-white opacity-0 shadow-lg transition; }` },

  { slug: "calendar", name: "Calendar", category: "data-input", description: "Pick a day from a navigable month grid.", demo: CalendarPage, code: `<Calendar value={day} onChange={setDay} />`, css: `.calendar-day { @apply inline-flex h-9 w-9 items-center justify-center rounded-lg text-sm text-gray-700 hover:bg-gray-100; }` },
  { slug: "checkbox", name: "Checkbox", category: "data-input", description: "Toggle independent options on or off, alone or in groups.", demo: CheckboxPage, code: `<Checkbox checked={all} onChange={toggleAll}>Everything</Checkbox>` },
  { slug: "fieldset", name: "Fieldset", category: "data-input", description: "Group related controls under one legend and action row.", demo: FieldsetPage, code: `<form><fieldset><legend>Preferences</legend>...</fieldset></form>` },
  { slug: "file-input", name: "File Input", category: "data-input", description: "Let people choose files with a styled native picker.", demo: FileInputPage, code: `<input type="file" className="form-input form-file" />` },
  { slug: "filter", name: "Filter", category: "data-input", description: "Narrow a list through chip-style single-choice filters.", demo: FilterPage, code: `<Filter value={filter} onChange={setFilter} />` },
  { slug: "input", name: "Input", category: "data-input", description: "Capture short free-form text in text, email, and search flavors.", demo: InputPage, code: `<input className="form-input" placeholder="Jordan" />` },
  { slug: "label", name: "Label", category: "data-input", description: "Associate captions with controls in stacked, inline, and floating placements.", demo: LabelPage, code: `<label className="form-label">Name<input /></label>` },
  { slug: "otp", name: "OTP", category: "data-input", description: "Collect one-time passcodes across auto-advancing boxes.", demo: OtpPage, code: `<Otp length={6} onComplete={verify} />`, css: `.otp-box { @apply h-12 w-11 rounded-lg border border-gray-200 bg-white text-center text-lg font-semibold text-gray-900; }` },
  { slug: "radio", name: "Radio", category: "data-input", description: "Choose exactly one option from a small exclusive set.", demo: RadioPage, code: `<Radio name="plan" value="pro" />` },
  { slug: "range", name: "Range", category: "data-input", description: "Adjust a scalar setting along a bounded slider.", demo: RangePage, code: `<input type="range" className="range-input" />` },
  { slug: "rating", name: "Rating", category: "data-input", description: "Display a score or collect a small amount of qualitative feedback.", demo: RatingPage, code: `<Rating value={4} max={5} />` },
  { slug: "select", name: "Select", category: "data-input", description: "Pick one option from a collapsed native list.", demo: SelectPage, code: `<select className="form-input form-select">...</select>` },
  { slug: "textarea", name: "Textarea", category: "data-input", description: "Capture multi-line messages with a live character count.", demo: TextareaPage, code: `<textarea className="form-input" rows={4} />` },
  { slug: "toggle", name: "Toggle", category: "data-input", description: "Flip preferences on and off with switch semantics.", demo: TogglePage, code: `<Toggle checked={on} onChange={setOn} />`, css: `.toggle { @apply relative inline-flex h-6 w-11 items-center rounded-full bg-gray-200 transition; }` },
  { slug: "validator", name: "Validator", category: "data-input", description: "Validate fields live and explain how to fix them.", demo: ValidatorPage, code: `<form noValidate onSubmit={submit}>...</form>` },

  { slug: "divider", name: "Divider", category: "layout", description: "Separate content with rules, labels, and vertical flow.", demo: DividerPage, code: `<hr className="divider" />` },
  { slug: "drawer", name: "Drawer", category: "layout", description: "Reveal contextual content from the edge of the viewport.", demo: DrawerPage, code: `<Drawer open={open} onClose={close}>...</Drawer>` },
  { slug: "footer", name: "Footer", category: "layout", description: "Close the page with brand context and grouped links.", demo: FooterPage, code: `<Footer columns={columns} />` },
  { slug: "jumbotron", name: "Hero", category: "layout", description: "Create a prominent introductory block for a page or campaign.", demo: JumbotronPage, code: `<Hero title="Build with primitives" />` },
  { slug: "indicator", name: "Indicator", category: "layout", description: "Pin count badges and unread dots to any corner.", demo: IndicatorPage, code: `<Indicator count={3}><Avatar /></Indicator>`, css: `.indicator-badge { @apply absolute -right-2 -top-2 rounded-full bg-brand-600 px-2 py-0.5 text-xs font-bold text-white; }` },
  { slug: "join", name: "Join", category: "layout", description: "Fuse inputs and buttons into one continuous control.", demo: JoinPage, code: `<Join><Input /><Button /></Join>` },
  { slug: "mask", name: "Mask", category: "layout", description: "Crop visuals into circles, blobs, and polygons with pure CSS.", demo: MaskPage, code: `<Mask shape="hexagon"><img /></Mask>` },
  { slug: "stack", name: "Stack", category: "layout", description: "Layer cards and previews with a fanned offset.", demo: StackPage, code: `<Stack><Card /><Card /></Stack>` },

  { slug: "browser", name: "Browser", category: "mockup", description: "Present a page inside realistic browser chrome.", demo: BrowserMockupPage, code: `<BrowserMockup url="utilitycss.dev" />`, css: `.mockup-browser { @apply overflow-hidden rounded-xl border border-gray-200 bg-white shadow-sm; }` },
  { slug: "code", name: "Code", category: "mockup", description: "Showcase snippets in a code window with line numbers.", demo: CodeMockupPage, code: `<CodeMockup file="app.css" />` },
  { slug: "phone", name: "Phone", category: "mockup", description: "Frame mobile content inside a phone silhouette.", demo: PhoneMockupPage, code: `<PhoneMockup>...</PhoneMockup>` },
  { slug: "window", name: "Window", category: "mockup", description: "Frame desktop content inside an OS window.", demo: WindowMockupPage, code: `<WindowMockup title="main" />` },

  { slug: "compiler-lab", name: "Playground", category: "tools", description: "Experiment live against the demo config: rendered preview, generated CSS, diagnostics, candidate validation, and API docs.", demo: CompilerLabPage, code: `POST /api/compile\n{ "source": "<div class=\\"flex\\">…", "browserTarget": "safari-15", "mode": "markup" }`, css: `.flex {\n  display: flex;\n}` },
  { slug: "motion", name: "Motion", category: "tools", description: "Keyframe entrances, transition timing, loading states, and reduced-motion handling.", demo: MotionPage, code: `<div className="animate-fade-in">Hello</div>`, css: `.animate-fade-in { animation: fade-in 0.5s ease-out both; }` }
];

export function getCategoryLabel(category: Category): string {
  return categoryDefinitions.find(item => item.value === category)?.label ?? category;
}
