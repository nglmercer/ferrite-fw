//! Drain launched process pipes without retaining a browser/process owner.
//! Readers live through process shutdown, then abort even if descendants keep
//! inherited descriptors open. Only the last 4 KiB of stderr are retained.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncReadExt};

const TAIL_BYTES: usize = 4096;

#[derive(Default)]
pub(crate) struct ProcessOutput {
    workers: Vec<tokio::task::JoinHandle<()>>,
    stderr: Arc<Mutex<VecDeque<u8>>>,
}

impl ProcessOutput {
    pub(crate) fn take(child: &mut tokio::process::Child) -> Self {
        let mut output = Self::default();
        if let Some(stdout) = child.stdout.take() {
            output.watch(stdout, false);
        }
        if let Some(stderr) = child.stderr.take() {
            output.watch(stderr, true);
        }
        output
    }

    fn watch(&mut self, mut reader: impl AsyncRead + Unpin + Send + 'static, stderr: bool) {
        let tail = stderr.then(|| self.stderr.clone());
        self.workers.push(tokio::spawn(async move {
            let mut buffer = [0; 8192];
            while let Ok(count) = reader.read(&mut buffer).await {
                if count == 0 {
                    break;
                }
                if let Some(tail) = &tail {
                    let mut tail = tail.lock().unwrap_or_else(|e| e.into_inner());
                    let bytes = &buffer[..count];
                    if bytes.len() >= TAIL_BYTES {
                        tail.clear();
                        tail.extend(&bytes[bytes.len() - TAIL_BYTES..]);
                    } else {
                        let excess = (tail.len() + bytes.len()).saturating_sub(TAIL_BYTES);
                        tail.drain(..excess);
                        tail.extend(bytes);
                    }
                }
            }
        }));
    }

    pub(crate) async fn diagnostic(&self) -> String {
        // A process exit can precede delivery of the last bytes/EOF. Keep the
        // old finite startup diagnostic wait, without retaining open pipes.
        let _ = tokio::time::timeout(Duration::from_millis(500), async {
            while self.workers.iter().any(|worker| !worker.is_finished()) {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await;
        let bytes: Vec<_> = self
            .stderr
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .copied()
            .collect();
        String::from_utf8_lossy(&bytes)
            .lines()
            .take(5)
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

impl Drop for ProcessOutput {
    fn drop(&mut self) {
        for worker in &self.workers {
            worker.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn concurrent_large_output_is_drained_and_stderr_retention_is_bounded() {
        let (stdout, mut stdout_writer) = tokio::io::duplex(64);
        let (stderr, mut stderr_writer) = tokio::io::duplex(64);
        let mut output = ProcessOutput::default();
        output.watch(stdout, false);
        output.watch(stderr, true);
        tokio::time::timeout(Duration::from_secs(5), async {
            tokio::join!(
                async {
                    stdout_writer
                        .write_all(&vec![b'o'; 256 * 1024])
                        .await
                        .unwrap();
                },
                async {
                    stderr_writer
                        .write_all(&vec![b'e'; 256 * 1024])
                        .await
                        .unwrap();
                    stderr_writer
                        .write_all(b"\nlast diagnostic\n")
                        .await
                        .unwrap();
                }
            );
        })
        .await
        .unwrap();
        drop(stdout_writer);
        drop(stderr_writer);
        assert!(output.diagnostic().await.ends_with("last diagnostic"));
        assert!(output.workers.iter().all(|worker| worker.is_finished()));
        assert!(output.stderr.lock().unwrap().len() <= TAIL_BYTES);
    }

    #[tokio::test]
    async fn dropping_output_releases_pending_readers_without_waiting_for_eof() {
        struct Reader {
            stream: tokio::io::DuplexStream,
            _owner: Arc<()>,
        }
        impl AsyncRead for Reader {
            fn poll_read(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
                buffer: &mut tokio::io::ReadBuf<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::pin::Pin::new(&mut self.stream).poll_read(cx, buffer)
            }
        }
        let (stream, _writer) = tokio::io::duplex(64);
        let owner = Arc::new(());
        let weak = Arc::downgrade(&owner);
        let mut output = ProcessOutput::default();
        output.watch(
            Reader {
                stream,
                _owner: owner,
            },
            true,
        );
        tokio::task::yield_now().await;
        assert!(weak.upgrade().is_some());
        drop(output);
        tokio::time::timeout(Duration::from_secs(1), async {
            while weak.upgrade().is_some() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
}
