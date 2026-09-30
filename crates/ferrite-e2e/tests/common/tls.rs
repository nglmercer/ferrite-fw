//! Ephemeral local TLS fixture, always killed when its owner is dropped.
use std::time::Duration;

pub(crate) struct TlsFixture {
    pub(crate) url: String,
    pub(crate) child: tokio::process::Child,
    _directory: tempfile::TempDir,
}
pub(crate) async fn tls_fixture() -> TlsFixture {
    let directory = tempfile::tempdir().unwrap();
    let cert = directory.path().join("cert.pem");
    let key = directory.path().join("key.pem");
    let output = tokio::process::Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=localhost",
            "-addext",
            "subjectAltName=DNS:localhost,IP:127.0.0.1",
            "-out",
        ])
        .arg(&cert)
        .arg("-keyout")
        .arg(&key)
        .kill_on_drop(true)
        .output()
        .await
        .expect("HTTPS fixture requires openssl");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let mut child = tokio::process::Command::new("openssl")
        .args(["s_server", "-quiet", "-www", "-accept"])
        .arg(address.to_string())
        .arg("-cert")
        .arg(cert)
        .arg("-key")
        .arg(key)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            assert!(child.try_wait().unwrap().is_none(), "HTTPS fixture exited");
            if tokio::net::TcpStream::connect(address).await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("HTTPS fixture startup");
    TlsFixture {
        url: format!("https://{address}"),
        child,
        _directory: directory,
    }
}
