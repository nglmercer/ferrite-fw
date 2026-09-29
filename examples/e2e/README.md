# e2e demo

Minimal consumer of `ferrite e2e`: a counter app plus a Rust test suite.

```bash
cd examples/e2e
ferrite e2e --check   # verify Chromium launches
ferrite e2e           # boot dev server + run tests/e2e.rs
```

`ferrite e2e` boots an in-process dev server on the `[e2e.web_server]` URL
(or reuses a running one), sets `FERRITE_E2E_BASE_URL`, and runs
`cargo test --test e2e`. Artifacts land in `test-results/`.
