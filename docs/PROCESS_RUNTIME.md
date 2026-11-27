# Process runtime removal

The Linux `napi-vm-process` backend was removed together with napi-vm. Select
`runtime.backend = "rquickjs"` and build Ferrite with `--features rquickjs`.
rquickjs provides cooperative interruption, request deadlines, bounded queues
and payload limits in an embedded worker thread, but it is not a process
sandbox and does not load native `.node` addons.

Use operating-system supervision, containers, or an external service boundary
when hard filesystem/network/resource isolation is required.
