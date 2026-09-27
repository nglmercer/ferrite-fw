# ferrite-client

Browser side of Ferrite HMR (spec §32–§35).

The dev server embeds the canonical client from
`crates/ferrite-hmr/src/client.js` and serves it at `/@ferrite/client`.
This package is the typed reference implementation: `src/client.ts` mirrors
that file with TypeScript types, and `src/hot.d.ts` declares the
`import.meta.hot` / `import.meta.env` surface (§33, §42).

Application code never imports this package directly; the dev pipeline
injects `import "/@ferrite/client"` automatically.
