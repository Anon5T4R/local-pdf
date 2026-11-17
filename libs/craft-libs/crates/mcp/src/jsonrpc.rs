//! JSON-RPC 2.0 framing and the MCP protocol revisions this crate speaks.
//!
//! One message per line (newline-delimited), no `Content-Length` headers: the same framing the
//! upstream Craft apps use, and what MCP clients on stdio expect.

use std::io::{BufRead, Write};

use serde_json::{Value, json};

/// The MCP revision this crate implements when a client asks for nothing newer.
pub const PROTOCOL_VERSION: &str = "2025-06-18";
/// The revision that adds the cache hints (`resultType`, `ttlMs`, `cacheScope`).
pub const MODERN_VERSION: &str = "2026-07-28";
/// Every revision the server answers, newest first.
pub const SUPPORTED_VERSIONS: &[&str] = &[MODERN_VERSION, PROTOCOL_VERSION, "2025-03-26", "2024-11-05"];

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;
pub const RESOURCE_NOT_FOUND: i64 = -32002;

/// Longest accepted request line (bytes, without the newline). Requests are small JSON objects;
/// anything longer is answered with an error rather than buffered, so one huge line can't make the
/// server allocate without bound.
pub const MAX_LINE_BYTES: usize = 16 * 1024 * 1024;
/// Most messages one batch may hold. Batches were removed from the protocol in 2025-06-18; this
/// only bounds what older clients can make the server parse and answer at once.
pub const MAX_BATCH_MESSAGES: usize = 64;

/// A success envelope.
pub fn response(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// An error envelope.
pub fn error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message.into()}})
}

/// The revision to answer an `initialize` asking for `asked` with: the same one when this server
/// knows it, else this crate's own.
pub fn negotiate(asked: &str) -> &'static str {
    SUPPORTED_VERSIONS.iter().copied().find(|v| *v == asked).unwrap_or(PROTOCOL_VERSION)
}

/// Whether `version` is the revision with the cache hints.
pub fn is_modern(version: &str) -> bool {
    version == MODERN_VERSION
}

/// The cache hints the modern revision adds to list and read results. `modern` is whether the
/// session negotiated that revision; a request can also name it in its `_meta`.
pub fn complete_result(method: &str, params: &Value, modern: bool, mut r: Value) -> Value {
    let modern = params
        .get("_meta")
        .and_then(|m| m.get("io.modelcontextprotocol/protocolVersion"))
        .and_then(Value::as_str)
        .map_or(modern, |v| v == MODERN_VERSION);
    if modern && let Some(result) = r.as_object_mut() {
        let ttl = match method {
            "tools/list" | "resources/list" | "resources/templates/list" | "prompts/list" => Some(600_000),
            "resources/read" | "prompts/get" => Some(0),
            _ => None,
        };
        if let Some(ttl) = ttl {
            result.insert("resultType".into(), json!("complete"));
            result.insert("ttlMs".into(), json!(ttl));
            result.insert("cacheScope".into(), json!("private"));
        }
    }
    r
}

/// Read one `\n`-terminated line into `buf` (newline stripped). `Ok(false)` at end of stream.
///
/// `buf` holding more than `max` bytes means the line is overlong: no newline arrived within the
/// bound, so the bytes so far are only its start. Only `max + 1` bytes are ever buffered for one
/// line, in the reader's own bounded chunks, so a peer can't make us allocate without bound.
pub fn read_line_capped(reader: &mut impl BufRead, buf: &mut Vec<u8>, max: usize) -> std::io::Result<bool> {
    buf.clear();
    loop {
        let take = {
            let chunk = reader.fill_buf()?;
            if chunk.is_empty() {
                return Ok(!buf.is_empty());
            }
            let upto = chunk.iter().position(|&b| b == b'\n').map_or(chunk.len(), |i| i + 1);
            upto.min(max.saturating_add(1).saturating_sub(buf.len()))
        };
        if take == 0 {
            // Exactly `max` content bytes plus the `\r` of a CRLF ending fill the bound; the line
            // is still within the limit when the `\n` follows.
            if buf.last() == Some(&b'\r') && reader.fill_buf()?.first() == Some(&b'\n') {
                reader.consume(1);
                buf.pop();
                return Ok(true);
            }
            // Already past the bound: the caller sees `buf.len() > max`.
            return Ok(true);
        }
        // The chunk is still buffered (nothing was consumed), so the same bytes are there; the
        // `min` keeps even a misbehaving reader from panicking us.
        let chunk = reader.fill_buf()?;
        let take = take.min(chunk.len());
        buf.extend_from_slice(&chunk[..take]);
        reader.consume(take);
        if buf.last() == Some(&b'\n') {
            buf.pop();
            if buf.last() == Some(&b'\r') {
                buf.pop();
            }
            return Ok(true);
        }
    }
}

/// Write one reply line (newline added) and flush, so a client sees it before the next request.
pub fn write_line(output: &mut impl Write, line: &str) -> std::io::Result<()> {
    output.write_all(line.as_bytes())?;
    output.write_all(b"\n")?;
    output.flush()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn read(text: &str, max: usize) -> (bool, Vec<u8>) {
        let mut cursor = Cursor::new(text.as_bytes().to_vec());
        let mut buf = Vec::new();
        let more = read_line_capped(&mut cursor, &mut buf, max).unwrap();
        (more, buf)
    }

    #[test]
    fn lines_are_split_and_crlf_stripped() {
        let (more, line) = read("{\"a\":1}\r\nnext\n", 1024);
        assert!(more);
        assert_eq!(line, b"{\"a\":1}");
        let (more, line) = read("tail", 1024);
        assert!(more, "a last line without a newline is still a line");
        assert_eq!(line, b"tail");
        let (more, line) = read("", 1024);
        assert!(!more);
        assert!(line.is_empty());
    }

    #[test]
    fn an_overlong_line_is_reported_without_buffering_all_of_it() {
        let (more, line) = read(&"x".repeat(1000), 16);
        assert!(more);
        assert!(line.len() > 16, "the caller sees that the line passed the bound");
        assert!(line.len() <= 17, "only the bound plus one is ever buffered: {}", line.len());
    }

    #[test]
    fn a_crlf_ending_right_at_the_bound_is_within_it() {
        // 15 content bytes + CRLF: the `\r` fills the bound, the `\n` still belongs to this line.
        let text = format!("{}\r\n", "y".repeat(15));
        let (more, line) = read(&text, 16);
        assert!(more);
        assert_eq!(line.len(), 15);
    }

    #[test]
    fn versions_negotiate_to_a_revision_this_crate_knows() {
        assert_eq!(negotiate("2025-03-26"), "2025-03-26");
        assert_eq!(negotiate(MODERN_VERSION), MODERN_VERSION);
        assert_eq!(negotiate("1999-01-01"), PROTOCOL_VERSION);
        assert!(is_modern(MODERN_VERSION));
        assert!(!is_modern(PROTOCOL_VERSION));
    }
}
