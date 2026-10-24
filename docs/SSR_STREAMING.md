# JavaScript SSR streaming (experimental Linux profile)

Build Ferrite with `--features napi-vm`, then select streaming explicitly:

```toml
[runtime]
backend = "napi-vm-process"

[ssr]
streaming = true
```

Export `render(url, request)` from the configured SSR entry. Return a
`ReadableStream`, an asynchronous iterator, or `{ body, status, headers }` where
`body` is either of those. Headers use the existing SSR array-of-pairs contract.
For example:

```js
export function render() {
  return {
    status: 200,
    headers: [['content-type', 'text/html; charset=utf-8']],
    body: new ReadableStream({
      start(controller) {
        controller.enqueue('<h1>Hello</h1>');
        controller.close();
      }
    })
  };
}
```

React's `react-dom/server.edge` `renderToReadableStream` and Vue's
`vue/server-renderer` `renderToWebStream` execute through the production graph.
This bridge does not change a buffered framework renderer into a streaming one.
The HTML shell must contain exactly one `<!--ssr-outlet-->`. Ferrite sends its
prefix, pulls renderer bytes on demand, and sends the suffix only after successful
completion. Status and headers must be available before the response starts;
producer failures after that point terminate the body rather than replacing the
already-sent response with an error page.

Each response owns a separate worker and retains its concurrency slot until
completion, cancellation or deadline. The absolute deadline includes admission,
renderer initialization and body consumption, even when the consumer never polls
the body. Disconnect drops the body and kills/reaps the worker process group.
Guest `cancel` callbacks and iterator finalizers are not guaranteed on teardown.

The private protocol uses bounded length-delimited frames and one pull per output
chunk. Output chunks contain at most 64 KiB, preserving arbitrary byte values and
typed-array view offsets. Guest source chunks and the bridge queue are limited to
1 MiB. Strings are encoded as UTF-8; a string's conservative queue accounting can
reject it before its UTF-8 bytes reach that limit. A larger source chunk fails
explicitly. The request payload has the configured runtime payload bound.

The embedded default-reader stream subset supports `start`, `pull`, `enqueue`,
`close`, `error`, reader cancellation and lock release. Timers use wall-clock time
and bytes enqueued during an asynchronous `start` can arrive before it settles.
BYOB readers, `tee`, piping and custom queue-size functions are unsupported and
fail explicitly. This is not a complete WHATWG Streams implementation. The
process backend currently requires Linux; Windows/macOS isolation and broader
framework/version/hydration acceptance remain open in `COMPLETION_PLAN.md`.

Reproduce transport and lifecycle acceptance with:

```bash
python3 scripts/acceptance.py --profile runtime
```

The framework profile additionally runs the official renderer fixtures and real
browser development hydration/HMR acceptance; it requires Node, Chromium and
Firefox installed explicitly.
