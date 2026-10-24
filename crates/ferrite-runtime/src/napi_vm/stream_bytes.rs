//! Pure byte-window encoding for the private streamed-renderer bridge.
//! Read bounded typed-array views directly instead of interpreting an element
//! loop and serializing tens of thousands of guest Number objects per pull.
use napi_vm::{Interpreter, Value, VmErr};
pub(super) fn encode(_: &mut Interpreter, _: Value, args: Vec<Value>) -> Result<Value, VmErr> {
    let fail = |message: &str| VmErr::Msg(message.into());
    let offset = match args.get(1) {
        Some(Value::Number(number))
            if number.is_finite()
                && *number >= 0.0
                && number.fract() == 0.0
                && *number <= 1048576.0 =>
        {
            *number as usize
        }
        _ => return Err(fail("invalid stream byte offset")),
    };
    let (length, bytes) = match args.first() {
        Some(Value::String(text)) => {
            let bytes = text.to_string().into_bytes();
            if bytes.len() > 1048576 || offset > bytes.len() {
                return Err(fail(
                    "stream source chunk exceeds 1 MiB or has invalid offset",
                ));
            }
            (
                bytes.len(),
                bytes[offset..(offset + 65536).min(bytes.len())].to_vec(),
            )
        }
        Some(Value::TypedArray(view)) if matches!(view.kind, napi_vm::value::TypedKind::Uint8) => {
            let length = view.effective_length();
            if length > 1048576 || offset > length || view.buffer.is_detached() {
                return Err(fail(
                    "stream source chunk exceeds 1 MiB or has invalid offset/buffer",
                ));
            }
            let start = view
                .effective_byte_offset()
                .checked_add(offset)
                .ok_or_else(|| fail("stream byte offset overflow"))?;
            (
                length,
                view.buffer
                    .read(start, (length - offset).min(65536))
                    .ok_or_else(|| fail("stream byte view is out of bounds"))?,
            )
        }
        _ => return Err(fail("stream chunks must be strings or Uint8Array bytes")),
    };
    let mut hex = String::with_capacity(bytes.len() * 2);
    const DIGITS: &[u8] = b"0123456789abcdef";
    for byte in bytes {
        hex.push(DIGITS[(byte >> 4) as usize] as char);
        hex.push(DIGITS[(byte & 15) as usize] as char);
    }
    Ok(Value::object(vec![
        ("hex".into(), Value::checked_string(hex)?),
        ("length".into(), Value::Number(length as f64)),
    ]))
}
