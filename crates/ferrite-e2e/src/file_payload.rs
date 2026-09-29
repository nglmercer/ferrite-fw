use crate::{E2eError, E2eResult};

/// An upload generated in memory. Bytes are transferred unchanged; MIME type
/// overrides filename inference. Empty payload lists clear the input.
#[derive(Debug, Clone)]
pub struct FilePayload {
    pub name: String,
    pub mime_type: String,
    pub bytes: Vec<u8>,
}
impl FilePayload {
    pub fn new(
        name: impl Into<String>,
        mime_type: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            name: name.into(),
            mime_type: mime_type.into(),
            bytes: bytes.into(),
        }
    }
}
pub(crate) const MAX_UPLOAD_BYTES: usize = 64 * 1024 * 1024;
pub(crate) fn encode_payloads(files: &[FilePayload]) -> E2eResult<String> {
    let mut total = 0usize;
    // Validate the complete batch before allocating base64 strings or mutating DOM.
    for file in files {
        if file.name.is_empty() || file.name.contains('\0') {
            return Err(E2eError::Config(
                "upload filename must be nonempty and contain no NUL".into(),
            ));
        }
        if !file.mime_type.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
            return Err(E2eError::Config(
                "upload MIME type must contain printable ASCII only".into(),
            ));
        }
        total = total
            .checked_add(file.bytes.len())
            .ok_or_else(|| E2eError::Config("upload size overflow".into()))?;
        if total > MAX_UPLOAD_BYTES {
            return Err(E2eError::Config(
                "set_input_files payload exceeds 64 MiB".into(),
            ));
        }
    }
    serde_json::to_string(&files.iter().map(|file| serde_json::json!({
        "name": file.name, "mime": file.mime_type, "data": crate::driver::base64_encode(&file.bytes),
    })).collect::<Vec<_>>()).map_err(E2eError::Json)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_batches_fail_before_encoding_and_empty_lists_clear() {
        for file in [
            FilePayload::new("", "text/plain", b"x"),
            FilePayload::new("bad\0name", "text/plain", b"x"),
            FilePayload::new("name", "text/é", b"x"),
        ] {
            assert!(encode_payloads(&[file]).is_err());
        }
        assert_eq!(encode_payloads(&[]).unwrap(), "[]");
        let oversized = FilePayload::new(
            "big.bin",
            "application/octet-stream",
            vec![0; MAX_UPLOAD_BYTES + 1],
        );
        assert!(encode_payloads(&[oversized])
            .unwrap_err()
            .to_string()
            .contains("64 MiB"));
    }
}
