# react example (needs network for `ferrite add`)

```bash
cd examples/react
ferrite add react react-dom
ferrite dev
```

No Node.js required: packages install into `.ferrite/npm/` and are served
through the native ESM pipeline (`/@npm/react@...`). TSX uses the automatic
JSX runtime by default (`[react] runtime = "automatic"`).
