# Class DSL Specification

## Purpose and versioning

Utility class names are a versioned language, not opaque strings. The native implementation is
grammar version `1`; its normative production list is in [`CLASS_DSL.ebnf`](./CLASS_DSL.ebnf), and
the exported `utilitycss-syntax::GRAMMAR_VERSION` constant is the machine-readable value.
Compatibility profiles MAY accept additional spellings, but native semantics MUST remain explicit.

## Candidate structure

```ebnf
candidate       = [important], variant_chain, utility, [important] ;
variant_chain   = { variant, ":" } ;
variant         = identifier, ["-", value]
                | "[", arbitrary_selector, "]"
                | "[", "@", at_rule_name, at_rule_prelude, "]" ;
utility         = arbitrary_property | [negative], utility_name, [value_part], [modifier] ;
value_part      = "-", value ;
modifier        = "/", value ;
value           = identifier | fraction | arbitrary_value | typed_arbitrary_value ;
fraction        = digits, "/", digits ;
arbitrary_value = "[", arbitrary_content, "]" ;
typed_arbitrary_value = "[", type_hint, ":", arbitrary_content, "]" ;
important       = "!" ;
negative        = "-" ;
```

The canonical native important form is trailing `!`, for example `hover:bg-red-500!`. A leading
`!` remains accepted for migration and is normalized by introspection to the trailing form.
Separators are recognized only at top level. Escaped characters, quotes, parentheses, and nested
brackets remain inside the current token.

## Examples

```text
flex
p-4
-px-2
bg-red-500
bg-red-500/50
hover:bg-red-500
md:hover:bg-red-500/50
w-[37px]
grid-cols-[1fr_2fr]
data-[state=open]:opacity-100
[content-visibility:auto]
[&>*]:p-4
[@supports(display:grid)]:grid
hover:bg-red-500/50!
w-1/2
[color:light-dark(#000,#fff)]
```

## AST and semantic obligations

The parser exposes a borrowed `CandidateAst` and an owned transport form. The AST preserves
important state, author-ordered variants, negative state, values, modifiers, typed arbitrary
values, fractions, and arbitrary properties. Arbitrary selector variants and arbitrary at-rule
variants are distinct structural kinds.

Every parsed field MUST be consumed by semantic resolution. A utility that does not support a
modifier or negative marker MUST return a typed diagnostic; it MUST NOT silently discard the field.

## Escaping and arbitrary values

The shared decoder handles `:`, `/`, `[`, `]`, whitespace, backslash, and quotes. Unescaped
underscores become CSS-token whitespace where appropriate:

```text
[calc(100%_-_2rem)]       -> calc(100% - 2rem)
[url('/what_a_rush.png')] -> url('/what_a_rush.png')
['hello\_world']          -> 'hello_world'
```

Arbitrary properties are a validated escape hatch:

```text
[content-visibility:auto]
[grid-template-columns:subgrid]
[--app-gap:1rem]
```

Property names, declaration delimiters, control characters, comments, and style-termination
fragments are rejected by semantic validation. The parser accepts structure; the resolver owns CSS
safety and type checking.

## Dynamic strings and extraction

The default compiler is static and MUST NOT execute application code. A string such as
`` `bg-${color}-500` `` is not a complete candidate. Hosts MAY use a separate AST extractor or a
hybrid extraction mode, but extractor output MUST retain byte-accurate spans.

## Variants and ordering

The parser preserves author order. The variant registry assigns deterministic ordering and applies
variants from the inside out. For example, `md:hover:bg-red-500` is not normalized by reordering
the author input. Equal semantic order keys are resolved by a stable candidate hash.

## Unknown syntax and diagnostics

The compiler distinguishes scanner false positives, invalid syntax, unresolved utility families,
invalid theme values, unsupported modifiers, unsafe CSS fragments, and unsupported variants.
`explain` and `validate` expose stable diagnostic codes, explanations, and ranked alternatives.

## Stability

The grammar is unstable before `1.0`. Any externally visible syntax change MUST update the EBNF,
fixtures, migration notes, and compatibility report.
