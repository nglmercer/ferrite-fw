# Glossary

## Adapter

Host-specific integration around the compiler, such as Vite or Bun plugin code.

## Arbitrary value

A value embedded directly in a candidate, usually bracketed.

Example:

```text
w-[37px]
```

## Binding

FFI or serialization layer exposing Rust functionality to another runtime.

## Candidate

A token extracted from source that may represent a utility class.

## Candidate AST

Structured syntactic representation of a candidate.

## Compiled config

Validated, normalized, immutable configuration used by hot compiler paths.

## Compiler facade

High-level API coordinating scanner/parser/resolver/cache/output.

## CSS IR

Internal CSS representation before serialization.

## Extractor

A component that discovers candidates. The default extractor is text-based; optional extractors may use ASTs.

## Incremental compile

A compile/update that reuses previous state and processes only affected inputs.

## Registry

Mapping from utility/variant names or patterns to semantic handlers.

## Resolver

Component that maps syntactic candidate structures to semantic meaning.

## Scanner

Fast source-text pass that extracts potential candidates.

## Source ID

Stable identifier for an input source unit.

## Utility

Base class behavior such as `flex`, `p-4`, or `bg-red-500`.

## Variant

Transformation prefix such as `hover:`, `md:`, or `dark:`.
