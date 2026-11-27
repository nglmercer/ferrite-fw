# JavaScript SSR streaming (rquickjs)

Build Ferrite with `--features rquickjs`, then select streaming explicitly:

```toml
[runtime]
backend = "rquickjs"

[ssr]
streaming = true
```

Export `render(url, request)` from the configured SSR entry. Return a
`ReadableStream`, an asynchronous iterator, or `{ body, status, headers }` where
`body` is either of those. Headers use the SSR array-of-pairs contract. The HTML
shell must contain exactly one `<!--ssr-outlet-->`; Ferrite emits the prefix,
pulls renderer bytes on demand, and emits the suffix only after successful
completion.

The embedded stream implementation intentionally supports the default-reader,
pull-source subset needed by server renderers: `start`, `pull`, `enqueue`,
`close`, `error`, cancellation and lock release. BYOB readers, `tee` and piping
fail explicitly. Output chunks are limited to 64 KiB; strings are UTF-8 encoded.
The configured request timeout bounds the whole stream and the configured
payload bound applies to the compiled graph plus arguments.

Streaming leases a reusable worker from a pool bounded by `queue_capacity`; when all pool workers hold open responses, new streaming renders fail explicitly instead of creating unbounded threads; completing or dropping a response resets its JavaScript context before the worker is reused. The old `napi-vm-process`
Linux worker protocol and native `.node` addon support were removed with the
napi-vm backend. Framework-specific streaming acceptance remains narrower than
buffered SSR acceptance; validate a framework renderer before advertising its
streaming profile.
