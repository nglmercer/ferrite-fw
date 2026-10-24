//! Versioned, length-delimited, one-pull/one-frame streaming protocol.
use super::*;
use tokio::time::{timeout_at, Instant};
pub(super) const WORKER_ARG: &str = "--ferrite-runtime-stream-worker";
const CHUNK_BYTES: usize = 64 * 1024;
struct Session {
    socket: tokio::net::UnixStream,
    deadline: Instant,
    // Closing this sender wakes the independent owner, including an unpolled
    // response body. The owner kills before reaping and releases admission.
    _stop: tokio::sync::oneshot::Sender<()>,
}
async fn write_json(
    socket: &mut tokio::net::UnixStream,
    value: &impl serde::Serialize,
    limit: usize,
) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(error)?;
    if bytes.len() > limit {
        return Err(error("stream frame exceeds payload limit"));
    }
    socket.write_u32(bytes.len() as u32).await.map_err(error)?;
    socket.write_all(&bytes).await.map_err(error)
}
async fn read_json<T: serde::de::DeserializeOwned>(
    socket: &mut tokio::net::UnixStream,
    limit: usize,
) -> Result<T> {
    let length = socket.read_u32().await.map_err(error)? as usize;
    if length > limit {
        return Err(error("stream frame exceeds payload limit"));
    }
    let mut bytes = vec![0; length];
    socket.read_exact(&mut bytes).await.map_err(error)?;
    serde_json::from_slice(&bytes).map_err(error)
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Chunk {
    done: bool,
    bytes: Vec<u8>,
    error: Option<String>,
}
pub(super) async fn invoke(
    runtime: &ProcessRuntime,
    graph: CompiledModuleGraph,
    export: &str,
    args: Vec<JsValue>,
    env: RuntimeEnvironment,
) -> Result<crate::JsStreamResponse> {
    graph.validate()?;
    for arg in &args {
        crate::js_value_to_json(arg)?;
    }
    let timeout = if runtime.options.engine.request_timeout_ms == 0 {
        5000
    } else {
        runtime.options.engine.request_timeout_ms
    };
    let deadline = Instant::now() + Duration::from_millis(timeout);
    timeout_at(deadline, async {
        let permit = runtime
            .state
            .slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| error("shut down"))?;
        let request = Request {
            version: 1,
            graph,
            export: export.into(),
            args,
            env,
            options: runtime.options.engine.clone(),
        };
        let limit = if runtime.options.engine.max_request_bytes == 0 {
            8 * 1024 * 1024
        } else {
            runtime.options.engine.max_request_bytes.min(MAX_FRAME)
        };
        // Validate the bound before creating the child or encoding a u32 frame.
        if serde_json::to_vec(&request).map_err(error)?.len() > limit {
            return Err(error("stream request exceeds payload limit"));
        }
        if Instant::now() >= deadline {
            return Err(error("stream request deadline exceeded in queue"));
        }
        let (socket, mut guard, memory, cpu) = runtime.spawn_worker(timeout, WORKER_ARG)?;
        let (stop, cancelled) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let _permit = permit;
            tokio::select! { _ = cancelled => (), _ = tokio::time::sleep_until(deadline) => () }
            guard.stop();
            let _ = guard.child.wait().await;
        });
        let mut session = Session {
            socket,
            deadline,
            _stop: stop,
        };
        write_json(&mut session.socket, &request, limit).await?;
        let response: Response = read_json(&mut session.socket, MAX_FRAME).await?;
        if response.address_space != memory || response.cpu_seconds != cpu {
            return Err(error("stream worker resource limits differ"));
        }
        let head = response.result.map_err(error)?;
        let body = futures::stream::try_unfold(session, |mut session| async move {
            timeout_at(session.deadline, async {
                session.socket.write_u8(b'N').await.map_err(error)?;
                let chunk: Chunk = read_json(&mut session.socket, CHUNK_BYTES * 8).await?;
                if let Some(message) = chunk.error {
                    return Err(error(message));
                }
                if chunk.bytes.len() > CHUNK_BYTES {
                    return Err(error("decoded stream chunk exceeds limit"));
                }
                if chunk.done {
                    if !chunk.bytes.is_empty() {
                        return Err(error("invalid terminal chunk"));
                    }
                    Ok(None)
                } else {
                    Ok(Some((chunk.bytes, session)))
                }
            })
            .await
            .map_err(|_| error("stream request deadline exceeded"))?
        });
        Ok(crate::JsStreamResponse {
            head,
            body: Box::pin(body),
        })
    })
    .await
    .map_err(|_| error("stream request deadline exceeded"))?
}
pub(super) async fn worker() -> Result<()> {
    let mut socket = worker_socket()?;
    let mut request: Request = read_json(&mut socket, MAX_FRAME).await?;
    if request.version != 1 {
        return Err(error("unsupported stream protocol"));
    }
    request.graph.validate()?;
    let wrapper_id = "/@ferrite/stream-session";
    if request
        .graph
        .modules
        .iter()
        .any(|module| module.id == wrapper_id)
    {
        return Err(error("reserved stream wrapper module identity"));
    }
    let code = include_str!("stream_wrapper.js")
        .replace(
            "__ENTRY__",
            &serde_json::to_string(&request.graph.entry).map_err(error)?,
        )
        .replace(
            "__EXPORT__",
            &serde_json::to_string(&request.export).map_err(error)?,
        );
    request.graph.modules.push(CompiledModule {
        id: wrapper_id.into(),
        code,
        url: None,
    });
    request.graph.entry = wrapper_id.into();
    request.options.stream_helpers = true;
    let runtime = crate::napi_vm::NapiVmRuntime::new(request.options);
    let start = async {
        let namespace = runtime
            .evaluate_module_graph(request.graph, request.env)
            .await?;
        let start = namespace.get_function("start")?.clone();
        let next = namespace.get_function("next")?.clone();
        let head = runtime.call(&start, request.args).await?;
        crate::js_value_to_json(&head)?;
        Ok::<_, FerriteError>((head, next))
    }
    .await;
    let mut address = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let mut cpu = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::getrlimit(libc::RLIMIT_AS, &mut address) } != 0
        || unsafe { libc::getrlimit(libc::RLIMIT_CPU, &mut cpu) } != 0
    {
        return Err(error("cannot verify stream limits"));
    }
    let response = Response {
        result: start
            .as_ref()
            .map(|(head, _)| head.clone())
            .map_err(|error| error.to_string()),
        address_space: address.rlim_cur,
        cpu_seconds: cpu.rlim_cur,
    };
    write_json(&mut socket, &response, MAX_FRAME).await?;
    let (_, next) = start?;
    loop {
        if socket.read_u8().await.map_err(error)? != b'N' {
            return Err(error("invalid stream pull command"));
        }
        let item = runtime
            .call(&next, vec![])
            .await
            .and_then(|value| crate::js_value_to_json(&value));
        let chunk = match item.and_then(chunk_from_js) {
            Ok(chunk) => chunk,
            Err(error) => Chunk {
                done: false,
                bytes: vec![],
                error: Some(error.to_string()),
            },
        };
        if chunk.bytes.len() > CHUNK_BYTES {
            return Err(error("worker chunk exceeds limit"));
        }
        let complete = chunk.done || chunk.error.is_some();
        write_json(&mut socket, &chunk, CHUNK_BYTES * 8).await?;
        if complete {
            break;
        }
    }
    Ok(())
}

fn chunk_from_js(value: serde_json::Value) -> Result<Chunk> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct EncodedChunk {
        done: bool,
        hex: String,
        error: Option<String>,
    }
    let chunk: EncodedChunk = serde_json::from_value(value).map_err(error)?;
    if chunk.hex.len() > CHUNK_BYTES * 2 || !chunk.hex.len().is_multiple_of(2) {
        return Err(error("invalid encoded stream chunk length"));
    }
    let digit = |byte: u8| match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(error("invalid encoded stream byte")),
    };
    let bytes = chunk
        .hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| Ok((digit(pair[0])? << 4) | digit(pair[1])?))
        .collect::<Result<Vec<_>>>()?;
    Ok(Chunk {
        done: chunk.done,
        bytes,
        error: chunk.error,
    })
}
