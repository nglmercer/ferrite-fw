# Generated-app package fixtures

`react.lock`/`react-registry/` and `vue.lock`/`vue-registry/` contain the
pinned React and Vue acceptance closures used by the offline generated-app
tests. The tarballs are unmodified npm artifacts for React 19.2.0, React DOM
19.2.0, React Refresh 0.17.0, Vue 3.5.22, TypeScript 5.9.3 and their locked
transitive/type dependencies. Tests serve them from a
loopback-only fixture server and rewrite the lock's tarball origin to that
server, so the frozen Ferrite installer verifies the recorded integrity without
consulting the public npm registry.

These fixtures prove operational Rust-only execution (no external Node, npm,
bun or pnpm executable). They are not a general package mirror and must not be
used to add unreviewed dependencies.
