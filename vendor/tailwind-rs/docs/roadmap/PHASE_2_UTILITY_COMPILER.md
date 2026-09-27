# Phase 2 — Utility Compiler

## Objective

Turn parsed candidates into useful deterministic CSS.

## Initial utility set

### Layout

- block
- inline
- flex
- grid
- hidden

### Spacing

- p-*
- px-*
- py-*
- pt/pr/pb/pl-*
- m-* family if negative semantics are defined
- gap-*

### Sizing

- w-*
- h-*
- min/max subset
- arbitrary values

### Color

- bg-*
- text-*
- border-* subset

### Radius

- rounded*
- rounded-*

### Flex/grid

- items-*
- justify-*
- grid-cols-*

## Theme

Implement typed theme/token lookup.

## CSS IR

CSS is generated through IR, not direct string concatenation in each utility.

## Ordering

Define initial stable utility order.

## Exit criteria

- fixture project compiles to expected CSS,
- output is byte-stable across runs,
- arbitrary-value escaping tested,
- duplicate candidates compile once,
- utility registry can be extended internally without editing parser,
- full rebuild benchmark recorded.

## Won't yet

- full Tailwind compatibility,
- arbitrary user plugins,
- all typography utilities.
