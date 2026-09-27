# Bun Example — Component Gallery Redesign Plan

## Goal

Redesign `examples/bun` into a documentation-style component explorer inspired by DaisyUI rather than displaying every example in one large page.

Main goals:

* Avoid rendering every component demo on the same page.
* Make components easier to discover.
* Add reusable tabs instead of a tabs-only demo.
* Give every component its own focused view.
* Group examples using DaisyUI-like categories.
* Keep Bun + Preact lightweight.
* Keep `utilitycss` classes statically discoverable.
* Eventually cover the full DaisyUI-style component catalog.
* Preserve `bun run verify`, `typecheck`, and production builds.

---

## 1. Change the page architecture

### Current

```text
app.tsx
 ├── Hero
 ├── Actions
 │   ├── Buttons
 │   ├── Dropdown
 │   └── Popover
 ├── Surfaces
 │   ├── Cards
 │   ├── Pricing
 │   ├── Carousel
 │   └── Jumbotron
 ├── Display
 ├── Navigation
 ├── Forms
 ├── Feedback
 └── Data
```

Everything is rendered together.

### Proposed

```text
AppShell
├── Header
├── CategoryTabs
├── Sidebar / ComponentMenu
└── ComponentPage
    ├── ComponentHeader
    ├── ExampleTabs
    │   ├── Default
    │   ├── Variants
    │   ├── Sizes
    │   └── States
    └── DemoFrame
        ├── Preview
        └── Code
```

Only the selected component should be rendered in the main content area.

---

## 2. Navigation design

Use **two navigation levels**.

### Level 1 — category tabs

```text
Actions
Data display
Navigation
Feedback
Data input
Layout
Mockup
```

These categories align closely with DaisyUI's current documentation structure. ([daisyui.com][1])

Desktop example:

```text
┌───────────────────────────────────────────────────────────┐
│ utilitycss                    GitHub       Theme           │
├───────────────────────────────────────────────────────────┤
│ Actions | Data display | Navigation | Feedback | Forms   │
├───────────────┬───────────────────────────────────────────┤
│ Components    │                                           │
│               │  Button                                   │
│ Button        │  Buttons allow users to take actions.     │
│ Dropdown      │                                           │
│ Modal         │  [ Default | Variants | Sizes | Icons ]   │
│ Swap          │  ┌─────────────────────────────────────┐  │
│ FAB           │  │              Preview                │  │
│               │  └─────────────────────────────────────┘  │
└───────────────┴───────────────────────────────────────────┘
```

### Level 2 — component sidebar

When `Actions` is active:

```text
Button
Dropdown
FAB
Modal
Swap
Theme Controller
```

Mobile can use a select/menu instead of a permanent sidebar.

---

## 3. Add simple URL routing

Don't add a full router dependency yet.

Use hash routing:

```text
#/components/button
#/components/card
#/components/tabs
#/components/modal
#/components/rating
```

Optionally preserve category:

```text
#/actions/button
#/data-display/card
#/navigation/tabs
```

Benefits:

* refresh-safe;
* component URLs can be shared;
* back/forward works;
* no Preact Router dependency;
* very small implementation.

Add:

```text
src/
  hooks/
    useHashRoute.ts
```

---

## 4. Create a component registry

Do not manually construct all navigation inside `app.tsx`.

Add:

```text
src/catalog.ts
```

Conceptually:

```ts
export interface CatalogItem {
  slug: string;
  name: string;
  category: Category;
  description: string;
  demo: ComponentType;
}
```

Example:

```ts
export const catalog = [
  {
    slug: "button",
    name: "Button",
    category: "actions",
    description: "Buttons allow users to perform actions.",
    demo: ButtonPage,
  },
  {
    slug: "dropdown",
    name: "Dropdown",
    category: "actions",
    description: "Displays contextual actions.",
    demo: DropdownPage,
  },
];
```

Then the sidebar, search, category tabs and component count all come from the same registry.

This removes duplicated navigation metadata.

---

## 5. Turn Tabs into a real reusable component

Currently `TabsDemo` owns its own four labels and local `useState`.

Create:

```text
src/components/ui/Tabs.tsx
```

API:

```tsx
<Tabs
  value={active}
  onChange={setActive}
  items={[
    { value: "variants", label: "Variants" },
    { value: "sizes", label: "Sizes" },
    { value: "icons", label: "Icons" },
  ]}
/>
```

Support:

* `role="tablist"`
* `role="tab"`
* `role="tabpanel"`
* `aria-selected`
* keyboard arrows
* disabled tabs
* `tabs-box`
* `tabs-border`
* `tabs-lift`
* sizes

Those variants are also similar to DaisyUI's tabs API. ([daisyui.com][2])

---

## 6. Use tabs inside component examples

This is where tabs solve the biggest current UI problem.

### Button page

Instead of displaying everything simultaneously:

```text
Button

[ Variants | Sizes | Icons | Groups | States ]
```

### Card page

```text
Card

[ Basic | Product | Profile | Horizontal | Pricing ]
```

### Forms

Instead of one enormous `FormsDemo`:

```text
Input

[ Default | Colors | Sizes | Validation | Disabled ]
```

Then separate pages for:

```text
Checkbox
Radio
Select
Textarea
Toggle
Range
File Input
```

This makes the example much closer to a real component library.

---

## 7. Add reusable `DemoFrame`

Create:

```text
src/components/docs/DemoFrame.tsx
```

Every example should use the same presentation:

```text
┌─────────────────────────────────────────┐
│ Variants                        Preview │
├─────────────────────────────────────────┤
│                                         │
│  Primary   Secondary   Outline          │
│                                         │
├─────────────────────────────────────────┤
│ Preview | TSX | CSS                     │
└─────────────────────────────────────────┘
```

Possible API:

```tsx
<DemoFrame
  title="Button variants"
  description="Semantic button variants."
  code={`<Button variant="primary">Button</Button>`}
>
  <ButtonDemo />
</DemoFrame>
```

This avoids repeating markup such as:

```tsx
<article className="component-card">
  ...
</article>
```

throughout `app.tsx`.

---

## 8. Refactor `app.tsx`

`app.tsx` is currently around 15 KB and acts as gallery, router, documentation layout, global state holder, section renderer, and overlay controller.

Reduce it to roughly:

```tsx
function App() {
  const route = useHashRoute();

  return (
    <AppShell>
      <CategoryTabs />
      <ComponentSidebar />
      <ComponentPage route={route} />
    </AppShell>
  );
}
```

Suggested structure:

```text
src/
├── app.tsx
├── catalog.ts
│
├── docs/
│   ├── AppShell.tsx
│   ├── CategoryTabs.tsx
│   ├── ComponentSidebar.tsx
│   ├── ComponentPage.tsx
│   ├── DemoFrame.tsx
│   └── ComponentSearch.tsx
│
├── hooks/
│   └── useHashRoute.ts
│
├── components/
│   └── ui/
│       ├── Tabs.tsx
│       ├── Button.tsx
│       ├── Badge.tsx
│       └── ...
│
└── demos/
    ├── button/
    ├── badge/
    ├── card/
    └── ...
```

---

## 9. Do not move every file immediately

Refactor incrementally.

### Phase 1

Keep:

```text
components/Button.tsx
components/Navigation.tsx
components/Feedback.tsx
components/Forms.tsx
...
```

Build the new navigation around them first.

### Phase 2

Gradually split large grouped files.

For example:

```text
Navigation.tsx
```

becomes:

```text
navigation/
  Breadcrumb.tsx
  Navbar.tsx
  Pagination.tsx
  Steps.tsx
  Tabs.tsx
  Menu.tsx
  Dock.tsx
```

This avoids a huge one-shot rewrite.

---

## 10. DaisyUI-style catalog expansion

The current example describes itself as having 38 components. DaisyUI's current catalog lists 68, although the counts are **not directly comparable** because this project groups and names several demos differently. ([daisyui.com][1])

Target these categories.

### Actions

Existing or partially existing:

* Button
* Dropdown
* Modal

Add:

* FAB / Speed Dial
* Swap
* Theme Controller

### Data display

Existing:

* Accordion
* Avatar
* Badge
* Card
* Carousel
* List
* Stat
* Table
* Timeline

Add:

* Aura
* Chat Bubble
* Collapse
* Countdown
* Diff
* Hover 3D Card
* Hover Gallery
* Kbd
* Status
* Text Rotate

### Navigation

Existing:

* Breadcrumbs
* Navbar
* Pagination
* Steps
* Tabs

Add:

* Dock
* Link
* Megamenu
* Menu

### Feedback

Existing:

* Alert
* Spinner/loading
* Progress
* Skeleton
* Toast
* Tooltip

Add:

* Radial Progress

### Data input

Existing coverage is already reasonably broad.

Make each one independently discoverable:

* Input
* Checkbox
* Radio
* Select
* Textarea
* Toggle
* Range
* Rating
* File Input

Add:

* Calendar
* Fieldset
* Filter
* Label
* Validator
* OTP

### Layout

Add dedicated examples for:

* Divider
* Drawer
* Footer
* Hero
* Indicator
* Join
* Mask
* Stack

### Mockup

Add:

* Browser
* Code
* Phone
* Window

The current DaisyUI category/component organization can be used as a coverage checklist. ([daisyui.com][1])

---

## 11. Keep Flowbite-specific examples separate

I would stop calling the entire gallery:

```text
Flowbite + daisyUI parity
```

because that becomes confusing once the catalog grows.

Instead use:

```text
utilitycss Component Gallery
```

and metadata badges:

```text
DaisyUI pattern
Flowbite pattern
utilitycss native
```

Optional category:

```text
Compatibility
  Flowbite
  DaisyUI
```

The examples demonstrate **utilitycss capabilities**, not runtime compatibility with those libraries, which the current README already correctly explains.

---

## 12. Component search

Once there are 50–70 examples, add search.

Desktop header:

```text
Search components...  ⌘K
```

Filter the registry:

```ts
catalog.filter(component =>
  component.name.toLowerCase().includes(query)
);
```

No search library is necessary.

Later add keyboard navigation.

---

## 13. Theme switcher

A DaisyUI-style gallery benefits heavily from theme testing.

Start simply:

```text
Light
Dark
System
```

Use CSS variables rather than duplicating component definitions.

Later additional demo themes could be added to prove utilitycss token/theme behavior.

---

## 14. CSS strategy

Keep the existing separation initially:

```text
app.css
style.css
utilitycss.config.css
```

The current README says `app.css` contains `@apply` component recipes while `style.css` contains authored layout styles.

Do **not** restructure all CSS during the navigation refactor.

First make the component explorer work.

Then optionally move toward:

```text
styles/
  docs.css
  components.css
  themes.css
```

provided the Bun/utilitycss pipeline handles those imports correctly.

---

## 15. Static utilitycss scanning requirement

Keep components statically imported through the catalog.

Prefer:

```ts
import { ButtonPage } from "./demos/button/ButtonPage";
import { CardPage } from "./demos/card/CardPage";
```

over runtime-generated module paths.

The current verification checks the actual CSS generated from the Bun module graph for component recipes and utilities.

So avoid patterns such as:

```ts
const className = `bg-${color}-600`;
```

when the compiler needs a literal class.

Prefer explicit maps:

```ts
const variants = {
  primary: "bg-brand-600",
  success: "bg-green-600",
  danger: "bg-red-600",
};
```

---

## 16. Verification improvements

Keep:

```bash
bun run typecheck
bun run verify
bun run build
```

The existing package already exposes those commands.

Add catalog verification for:

```text
✓ every slug is unique
✓ every component has a category
✓ every category exists
✓ every component has a description
✓ every catalog demo is reachable
✓ generated CSS contains core component recipes
```

Also update the verification output from:

```text
38 components
```

to use the catalog count automatically instead of manually hardcoding the number in several places.

---

## Implementation order

* [x] **1.** Create `catalog.ts`
* [x] **2.** Create hash-based component routing
* [x] **3.** Create category tabs
* [x] **4.** Create component sidebar
* [x] **5.** Create reusable accessible `Tabs`
* [x] **6.** Create `DemoFrame`
* [x] **7.** Convert `Button` into the first new component page
* [x] **8.** Convert `Card`
* [x] **9.** Convert `Navigation`
* [x] **10.** Convert forms into individual component pages
* [x] **11.** Convert feedback components
* [x] **12.** Remove the old long-page sections
* [x] **13.** Add component search
* [ ] **14.** Add light/dark theme switcher (ThemeController page done; shell-wide toggle deferred)
* [x] **15.** Add missing DaisyUI-style components by category
* [x] **16.** Update `verify.ts`
* [x] **17.** Update Bun example README
* [x] **18.** Run `typecheck`, `verify`, and `build`

## Recommended first milestone

I would **not** implement all 68-style components first.

The first milestone should be:

```text
Component explorer
├── Category tabs
├── Sidebar
├── Hash URLs
├── Search
├── DemoFrame
└── 10 existing components migrated
```

Once that UX is solid, adding the remaining components becomes repetitive and much easier.

This will be substantially cleaner than the current single-page gallery and much closer to the way DaisyUI presents a large component library.

[1]: https://daisyui.com/components/ "Tailwind CSS Components"
[2]: https://daisyui.com/components/tab/?utm_source=chatgpt.com "Tailwind Tabs Component – daisyUI"
