//! The bridge from an MCP server to a running app: its loopback JSON control channel.
//!
//! An app started with `--control PORT` listens on `127.0.0.1:PORT`, one JSON request per line
//! (`{"id": ., "method": ., "params": .}`) and one JSON reply per line
//! (`{"id": ., "ok": true, "result": .}` or `{"id": ., "ok": false, "error": .}`). [`Remote`] speaks
//! exactly that, reconnecting per call, so an agent that starts before the app catches up once the
//! app opens its port instead of losing the server for its whole session.
//!
//! [`normalize_addr`] is what `mcp --connect PORT` takes: a bare port becomes loopback, and a host
//! that isn't loopback is refused. A loopback port is not authentication, and this crate does not
//! pretend it is: an app that wants a key adds one to its own protocol, and the bridge carries what
//! the app's control channel asks for.

use std::io::{BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use serde_json::{Value, json};

use crate::jsonrpc::read_line_capped;

/// The control port the apps use when given none.
pub const DEFAULT_PORT: u16 = 7979;
/// The address that port lives at.
pub const DEFAULT_ADDR: &str = "127.0.0.1:7979";

/// Longest accepted reply line (bytes, without the newline). The apps answer small JSON objects
/// except for images, which reach tens of megabytes; anything longer is refused and the connection
/// dropped, so a faulty endpoint can't make the bridge buffer without bound.
const MAX_REPLY_BYTES: usize = 256 * 1024 * 1024;
/// How long one connection attempt waits before trying the next address.
const CONNECT_TIMEOUT: Duration = Duration::from_millis(800);
/// How long a reply may take. The apps answer within 60 s (their own timeout); leave headroom.
const REPLY_TIMEOUT: Duration = Duration::from_secs(90);

/// The address `--connect` names: `PORT` (loopback), `HOST:PORT`, or `localhost:PORT`. Anything
/// that isn't a loopback host is refused: this channel is a local one.
pub fn normalize_addr(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("--connect needs an address or a port".into());
    }
    if raw.bytes().all(|b| b.is_ascii_digit()) {
        let port: u16 = raw.parse().map_err(|_| format!("--connect {raw}: a port is 1 to 65535"))?;
        if port == 0 {
            return Err(format!("--connect {raw}: a port is 1 to 65535"));
        }
        return Ok(format!("127.0.0.1:{port}"));
    }
    let (host, port) = raw.rsplit_once(':').ok_or_else(|| format!("--connect {raw}: give PORT or HOST:PORT"))?;
    let port: u16 = port.parse().map_err(|_| format!("--connect {raw}: a port is 1 to 65535"))?;
    if port == 0 {
        return Err(format!("--connect {raw}: a port is 1 to 65535"));
    }
    let host = host.trim_start_matches('[').trim_end_matches(']');
    match host {
        "127.0.0.1" | "localhost" => Ok(format!("{host}:{port}")),
        "::1" => Ok(format!("[::1]:{port}")),
        "" => Err(format!("--connect {raw}: give PORT or HOST:PORT")),
        other => Err(format!("--connect {raw}: the control channel is loopback only, not {other}")),
    }
}

/// A running app, reached through its loopback control port.
pub struct Remote {
    addr: String,
    conn: Option<(BufReader<TcpStream>, TcpStream)>,
    next_id: u64,
}

impl Remote {
    /// Connect to `addr` now, failing when nothing is listening. `addr` should come from
    /// [`normalize_addr`].
    pub fn connect(addr: &str) -> std::io::Result<Self> {
        let mut r = Self::lazy(addr);
        r.reconnect()?;
        Ok(r)
    }

    /// An app that may not be running yet: the first call tries, and every call tries again, so an
    /// MCP client that starts before the app works once the app opens its port.
    pub fn lazy(addr: &str) -> Self {
        Self { addr: addr.to_string(), conn: None, next_id: 1 }
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }

    fn reconnect(&mut self) -> std::io::Result<()> {
        self.conn = None;
        let mut last = std::io::Error::new(std::io::ErrorKind::NotFound, format!("cannot resolve {}", self.addr));
        for sa in self.addr.to_socket_addrs()? {
            match TcpStream::connect_timeout(&sa, CONNECT_TIMEOUT) {
                Ok(s) => {
                    s.set_nodelay(true).ok();
                    s.set_read_timeout(Some(REPLY_TIMEOUT)).ok();
                    let read = s.try_clone()?;
                    self.conn = Some((BufReader::new(read), s));
                    return Ok(());
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    fn prepare_connection(&mut self) -> std::io::Result<()> {
        if let Some((_, stream)) = self.conn.as_ref() {
            stream.set_nonblocking(true)?;
            let probe = stream.peek(&mut [0]);
            stream.set_nonblocking(false)?;
            match probe {
                // Closed by the app, or holding bytes nobody asked for (an app that explains why it
                // closes): either way no reply to the next request can be read from it.
                Ok(_) => self.conn = None,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => self.conn = None,
            }
        }
        if self.conn.is_none() {
            self.reconnect()?;
        }
        Ok(())
    }

    fn roundtrip(&mut self, line: &str) -> std::io::Result<String> {
        let Some((reader, writer)) = self.conn.as_mut() else {
            return Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "not connected to the app"));
        };
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        let mut buf = Vec::new();
        if !read_line_capped(reader, &mut buf, MAX_REPLY_BYTES)? {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "control channel closed"));
        }
        if buf.len() > MAX_REPLY_BYTES {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("control reply longer than {MAX_REPLY_BYTES} bytes")));
        }
        String::from_utf8(buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Call one control method. `Ok` carries the `result`, `Err` the error message (with whether the
    /// operation may have completed, when the connection failed mid-call).
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.prepare_connection().map_err(|e| {
            self.conn = None;
            format!("the app at {} is not reachable: {e}; `{method}`: nothing was sent", self.addr)
        })?;
        let id = self.next_id;
        self.next_id += 1;
        let line = json!({"id": id, "method": method, "params": params}).to_string();
        let v: Value = self
            .roundtrip(&line)
            .and_then(|reply| {
                serde_json::from_str(reply.trim())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("bad reply from app: {e}")))
            })
            .map_err(|e| {
                self.conn = None;
                format!("the app at {}: `{method}` failed: {e}; operation may have completed; inspect state before retrying", self.addr)
            })?;
        if v.get("id").is_none() && v.get("ok").and_then(Value::as_bool) == Some(false) {
            // The app's refusals carry no id (too many connections, a line it would not read): it
            // ran nothing and closes the connection, so the next call starts a fresh one.
            self.conn = None;
            let why = v.get("error").and_then(Value::as_str).unwrap_or("unknown error");
            return Err(format!("the app at {} refused `{method}`: {why}; nothing was run", self.addr));
        }
        if v.get("id") != Some(&json!(id)) {
            // Not the answer to this call: the stream is desynchronised (or the endpoint faulty), so
            // the connection is dropped and the next call starts a fresh one.
            self.conn = None;
            return Err(format!(
                "the app at {}: reply id mismatch (expected {id}); connection dropped; operation may have completed; inspect state before retrying",
                self.addr
            ));
        }
        if v.get("ok").and_then(Value::as_bool) == Some(true) {
            Ok(v.get("result").cloned().unwrap_or(Value::Null))
        } else {
            Err(v.get("error").and_then(Value::as_str).unwrap_or("unknown error").to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    use super::*;

    #[test]
    fn a_bare_port_is_loopback() {
        assert_eq!(normalize_addr("7979").unwrap(), "127.0.0.1:7979");
        assert_eq!(normalize_addr(" 7981 ").unwrap(), "127.0.0.1:7981");
        assert_eq!(normalize_addr("127.0.0.1:7979").unwrap(), "127.0.0.1:7979");
        assert_eq!(normalize_addr("localhost:7979").unwrap(), "localhost:7979");
        assert_eq!(normalize_addr("[::1]:7979").unwrap(), "[::1]:7979");
    }

    #[test]
    fn a_non_loopback_host_or_bad_port_is_refused() {
        for bad in ["0", "99999", "192.168.1.5:7979", "example.com:80", "7979:80", ":7979", ""] {
            assert!(normalize_addr(bad).is_err(), "{bad}");
        }
    }

    /// A one-connection stand-in for the app: one request, one reply.
    fn one_reply(reply: impl FnOnce(Value) -> Value + Send + 'static) -> (String, std::thread::JoinHandle<Value>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let request: Value = serde_json::from_str(line.trim()).unwrap();
            let body = reply(request.clone());
            let mut writer = stream;
            writeln!(writer, "{body}").unwrap();
            request
        });
        (addr, handle)
    }

    #[test]
    fn a_call_round_trips_over_loopback() {
        let (addr, handle) = one_reply(|request| {
            let id = request.get("id").cloned().unwrap_or(Value::Null);
            json!({"id": id, "ok": true, "result": {"mine": request["method"]}})
        });
        let mut remote = Remote::connect(&addr).unwrap();
        let result = remote.call("engine.execute", json!({"command": "select.all"})).unwrap();
        assert_eq!(result["mine"], "engine.execute");
        assert_eq!(handle.join().unwrap()["params"]["command"], "select.all");
    }

    #[test]
    fn an_error_reply_is_the_call_error() {
        let (addr, handle) = one_reply(|request| {
            let id = request.get("id").cloned().unwrap_or(Value::Null);
            json!({"id": id, "ok": false, "error": "nothing selected"})
        });
        let mut remote = Remote::connect(&addr).unwrap();
        assert_eq!(remote.call("engine.execute", json!({})).unwrap_err(), "nothing selected");
        handle.join().unwrap();
    }

    #[test]
    fn a_lazy_bridge_that_finds_nothing_says_nothing_was_sent() {
        // A port bound without listening: connections are refused.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        drop(listener);
        let mut remote = Remote::lazy(&addr);
        let e = remote.call("ping", json!({})).unwrap_err();
        assert!(e.contains("not reachable") && e.contains("nothing was sent"), "{e}");
    }
}
