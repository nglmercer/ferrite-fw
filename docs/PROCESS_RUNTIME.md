# Linux process runtime

Select the explicit `napi-vm-process` backend in a Ferrite CLI compiled with
`--features napi-vm`:

```toml
[runtime]
backend = "napi-vm-process"
request_timeout_ms = 5000
process_memory_limit_bytes = 2147483648
process_concurrency = 4
max_request_bytes = 8388608
```

Each atomic compiled-graph invocation starts a fresh copy of the executable.
The CLI and generated SSR servers handle the private worker entry point before
normal argument parsing. Library embedders must do the same using
`process::run_worker_if_requested`, or supply a compatible executable through
`ProcessOptions::executable`. Opaque guest handles cannot cross processes.

The parent deadline includes admission, transport, execution and reaping. Zero
selects five seconds for this profile. On timeout, dropped invocation or explicit
shared-runtime shutdown, the parent kills the child's process group. The child
uses Linux `RLIMIT_AS` and `RLIMIT_CPU`, set before exec. Zero memory/concurrency
settings select 2 GiB and four children. The address-space ceiling covers mappings
and thread stacks, not only JavaScript allocations. Transport has a 64 MiB hard
frame ceiling; the default request bound is 8 MiB. Native stdout/stderr are
suppressed and cannot corrupt the private socket protocol.

This is resource isolation, not a filesystem/network sandbox. Descendants which
intentionally leave the process group are outside its containment contract.
Existing native-addon allowlisting and SSR restrictions still apply. Generated
standalone servers currently reject native addons. Windows/macOS process limits
are not implemented, and this backend is unavailable there. The embedded
`napi-vm` backend remains a separate cooperative profile.

Framework adapters may serialize graph replacement/rendering; the process
concurrency setting is a ceiling rather than a guarantee of parallel requests.
