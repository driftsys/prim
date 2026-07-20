//! `Content-Length`-framed JSON-RPC message transport over byte streams
//! (the LSP base protocol). prim's `lsp` server speaks this over stdin/stdout.

use std::io::{BufRead, Write};

use serde_json::Value;

/// Read one `Content-Length`-framed JSON-RPC message from `reader`, returning
/// its parsed body. `Ok(None)` means a clean end of stream (the client closed
/// the connection). A malformed frame or non-JSON body is an [`io::Error`].
pub fn read_message(reader: &mut impl BufRead) -> std::io::Result<Option<Value>> {
    let mut content_length: Option<usize> = None;
    let mut header = String::new();
    loop {
        header.clear();
        let read = reader.read_line(&mut header)?;
        if read == 0 {
            // EOF before any header of this message: clean shutdown.
            return Ok(None);
        }
        let line = header.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            // Blank line terminates the header block.
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            content_length = value.trim().parse().ok();
        }
    }

    let length = content_length.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "LSP message header missing Content-Length",
        )
    })?;

    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    let value = serde_json::from_slice(&body)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    Ok(Some(value))
}

/// Write `message` to `writer` as a `Content-Length`-framed JSON-RPC message,
/// flushing so the client sees it immediately.
pub fn write_message(writer: &mut impl Write, message: &Value) -> std::io::Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use serde_json::json;

    use super::*;

    #[test]
    fn round_trips_a_framed_message() {
        let message = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"});
        let mut buffer = Vec::new();
        write_message(&mut buffer, &message).unwrap();

        let framed = String::from_utf8(buffer.clone()).unwrap();
        assert!(framed.starts_with("Content-Length: "));
        assert!(framed.contains("\r\n\r\n"));

        let mut reader = Cursor::new(buffer);
        let read = read_message(&mut reader).unwrap().unwrap();
        assert_eq!(read, message);
    }

    #[test]
    fn reads_two_messages_from_one_stream() {
        let mut buffer = Vec::new();
        write_message(&mut buffer, &json!({"id": 1})).unwrap();
        write_message(&mut buffer, &json!({"id": 2})).unwrap();

        let mut reader = Cursor::new(buffer);
        assert_eq!(
            read_message(&mut reader).unwrap().unwrap(),
            json!({"id": 1})
        );
        assert_eq!(
            read_message(&mut reader).unwrap().unwrap(),
            json!({"id": 2})
        );
        assert!(read_message(&mut reader).unwrap().is_none());
    }

    #[test]
    fn empty_stream_is_a_clean_end() {
        let mut reader = Cursor::new(Vec::new());
        assert!(read_message(&mut reader).unwrap().is_none());
    }

    #[test]
    fn missing_content_length_is_an_error() {
        let mut reader = Cursor::new(b"X-Other: 1\r\n\r\n{}".to_vec());
        assert!(read_message(&mut reader).is_err());
    }
}
