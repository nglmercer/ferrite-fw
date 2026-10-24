# Roadmap implementation evidence

Host-specific results for the changes in this PR. This is not full release or
cross-platform certification; unchecked work remains in COMPLETION_PLAN.md.

| Input | Validated selection |
|---|---|
| Host | Linux x86_64 |
| Rust | 1.99.1 |
| Native compiler | Oxc 0.151.0 |
| Explicit component compiler host | Node 24.19.0 |
| Framework versions | React 19.2.0, Vue 3.5.22, Svelte 5.39.6 |
| Embedded core + timer/web host | napi-vm revision 7362971e7781c0c57b36c5a11813fff0e6bc3521 |
| Chromium | 153.0.8010.0 |
| Firefox | 157.0 |

## Executed checks

- 309 unit checks across facade, CLI, config, frameworks, plugin, runtime,
  server, SSR and transform passed together. The 19 ignored checks were not
  counted as passes; selected slow fixtures below ran explicitly.
- All six generated-project fixtures in framework_compilers passed together:
  JS/TS Vue, Svelte and React build/preview hydration, Vue development graph
  invalidation/recovery, and Vue development hydration in each browser.
- Each framework's production fixture exercised both Chromium and Firefox,
  retained the SSR DOM node, clicked the counter, checked CSS and rejected
  hydration/console errors. Production preview used a dotted application route.
- Official Vue and Svelte emitted-renderer fixtures independently hydrated in
  Chromium and Firefox.
- Real prerender build/HTTP acceptance passed: sorted deterministic route
  reports, base paths, nested static pages, private renderer removal, missing
  route 404 and rollback after renderer failure.
- Generated SSR standalone binary compiled offline and passed HTTP execution
  with empty PATH and an unrelated working directory. Checks include dotted
  routes, request methods, response headers, asset HEAD/405 behavior and private
  renderer 404.
- Indexed-map flattening and composition passed nested-offset, equal-boundary,
  unmapped-region, contents/name/ignore-list and malformed-map regressions.
- Parser preflight measurement passed: 20 checks of 91,780 bytes measured
  858.21 ms uncached versus 7.24 ms cached in the debug test profile. This is not
  an SSR throughput measurement or a release performance budget.
- Strict all-target Clippy, formatting/diff checks and four acceptance-runner
  regressions passed. Runner reports now identify OS, architecture and tool
  versions; exact filtered gates reject zero-test false successes.

## Reproduction

Set FERRITE_CHROMIUM_PATH and FERRITE_FIREFOX_PATH to working installed browsers,
and select the explicit Node compiler host for Vue/Svelte. Use
`scripts/acceptance.py --profile frameworks`, `--profile runtime` and
`--profile standalone` for the corresponding gates and logs. The runner neither
installs prerequisites nor substitutes another browser/runtime.

The runtime/web-host integration uses real text codecs and structured clone;
it does not install fetch/network capabilities. Large official renderer graphs
use a 16 MiB native worker stack, independently of guest execution budgets.
Hard process/heap isolation, full framework development/standalone matrices,
streaming, contextual/workspace/portable optional package graphs and platform
release execution remain open. No entire roadmap or framework parity claim is
made by these results.
