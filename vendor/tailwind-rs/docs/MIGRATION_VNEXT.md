# vNext migration notes

This release keeps the initial utilitycss behavior while making the candidate language explicit.

- Native important output is normalized to trailing `!`. Leading `!` remains accepted.
- Arbitrary selectors and arbitrary at-rules now have separate AST kinds.
- Arbitrary properties such as `[content-visibility:auto]` are parsed structurally and validated
  before emission.
- Unescaped underscores in arbitrary CSS become token whitespace; quoted strings, URL contents,
  and escaped underscores are preserved.
- Numeric slash values such as `w-1/2` are represented as fractions.
- Modifiers are either consumed by a utility or reported as unsupported; they are never ignored.
- `tailwind-v3-subset` and `tailwind-v4-subset` are explicit compatibility labels, not full
  compatibility promises.

Consumers that persist parsed candidates SHOULD persist the grammar version alongside the raw
candidate. Consumers that persist generated capability data SHOULD regenerate it from the active
compiler rather than hand-editing the artifact.
