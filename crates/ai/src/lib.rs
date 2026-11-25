//! BYOK AI for LocalPDF: the user's own keys, one OpenAI-compatible client for every provider.
//!
//! Thin wrapper over the shared [`local_ai`] crate with LocalPDF's identity (config folder,
//! keychain service, OpenRouter headers). Nothing here talks to a LocalPDF server: there is
//! none. A connection is a provider preset (base URL), a model name the picker fetches from the
//! provider's own `/models`, and a secret the OS keychain holds.

use std::path::PathBuf;

pub use local_ai::{Job, Msg, PROVIDERS, Provider, provider};

/// LocalPDF's identity in the shared `local-ai` crate: config folder and keychain service both
/// `"LocalPDF"`, and the OpenRouter referer points at this repository.
pub const APP: local_ai::App = local_ai::App { id: "LocalPDF", url: "https://github.com/Styx-Studios/Local-PDF" };

/// A saved connection: which provider, its base URL (editable for `custom` or when a gateway
/// moves) and the model the picker last chose. The API key is never in this file. Same shape
/// and file as the old in-tree crate had, so existing `ai-connections.json` files keep working.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Connection {
    pub id: String,
    pub provider: String,
    pub label: String,
    pub base_url: String,
    pub model: String,
}

impl Connection {
    fn shared(&self) -> local_ai::Connection {
        local_ai::Connection {
            id: self.id.clone(),
            provider: self.provider.clone(),
            label: self.label.clone(),
            base_url: self.base_url.clone(),
            model: self.model.clone(),
        }
    }

    fn wrap(all: Vec<local_ai::Connection>) -> Vec<Connection> {
        all.into_iter().map(|c| Connection { id: c.id, provider: c.provider, label: c.label, base_url: c.base_url, model: c.model }).collect()
    }

    /// Where the connections file lives: `<data dir>/LocalPDF/ai-connections.json`.
    pub fn store_path() -> Option<PathBuf> {
        local_ai::Connection::store_path(&APP)
    }

    /// Load every saved connection. A missing file is no connections, not an error; a broken
    /// one is an error, so a silent wipe can never pretend the setup is gone.
    pub fn load_all() -> Result<Vec<Connection>, String> {
        local_ai::Connection::load_all(&APP).map(Self::wrap)
    }

    /// Save every connection: write a sibling then rename over the real file.
    pub fn save_all(all: &[Connection]) -> Result<(), String> {
        let all: Vec<local_ai::Connection> = all.iter().map(|c| c.shared()).collect();
        local_ai::Connection::save_all(&APP, &all)
    }

    pub fn set_key(&self, key: &str) -> Result<(), String> {
        self.shared().set_key(&APP, key)
    }

    pub fn key(&self) -> Result<String, String> {
        self.shared().key(&APP)
    }

    pub fn has_key(&self) -> bool {
        self.shared().has_key(&APP)
    }

    pub fn clear_key(&self) -> Result<(), String> {
        self.shared().clear_key(&APP)
    }
}

/// A chat request: the whole conversation, the doc context already folded into the turns by
/// the caller.
pub fn chat(conn: &Connection, history: &[Msg]) -> Result<String, String> {
    local_ai::chat(&APP, &conn.shared(), history)
}

/// A vision request: one page picture plus the instruction, as a one-turn conversation.
pub fn vision(conn: &Connection, instruction: &str, png: &[u8]) -> Result<String, String> {
    local_ai::vision(&APP, &conn.shared(), instruction, png)
}

/// The provider's model list, sorted for the picker.
pub fn list_models(conn: &Connection) -> Result<Vec<String>, String> {
    local_ai::list_models(&APP, &conn.shared())
}

/// One streamed answer: `on_delta` receives each piece as it arrives and the whole text comes back.
pub fn chat_stream(conn: &Connection, history: &[Msg], on_delta: &mut dyn FnMut(&str)) -> Result<String, String> {
    local_ai::chat_stream(&APP, &conn.shared(), history, on_delta)
}

/// [`chat_stream`] with the key handed in instead of read from the OS keychain: for the tests that
/// exercise the transport end to end (a loopback provider, a real provider) without the keychain.
pub fn chat_stream_with_key(conn: &Connection, key: &str, history: &[Msg], on_delta: &mut dyn FnMut(&str)) -> Result<String, String> {
    local_ai::chat_stream_with_key(&APP, &conn.shared(), key, history, on_delta)
}

/// A streaming answer on a worker thread: the pieces arrive on `deltas` as the provider sends them,
/// and the joined text (or the error) on `done`. Both channels close when the request ends.
pub struct Stream {
    pub deltas: std::sync::mpsc::Receiver<String>,
    pub done: std::sync::mpsc::Receiver<Result<String, String>>,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Stream {
    /// Starts the request on a worker thread. The UI polls [`Stream::poll`] every frame.
    pub fn start(conn: Connection, history: Vec<Msg>) -> Self {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::mpsc::channel;
        let (deltas_tx, deltas) = channel();
        let (done_tx, done) = channel();
        let cancelled = std::sync::Arc::new(AtomicBool::new(false));
        let stop = cancelled.clone();
        std::thread::Builder::new()
            .name("ai-chat".into())
            .spawn(move || {
                let result = local_ai::chat_stream(&APP, &conn.shared(), &history, |delta| {
                    // A cancelled request stops forwarding: closing the panel must not keep
                    // spending the user's tokens.
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let _ = deltas_tx.send(delta.to_owned());
                });
                let _ = done_tx.send(result);
            })
            .ok();
        Self { deltas, done, cancelled }
    }

    /// What arrived since the last frame, and the final result once there is one.
    pub fn poll(&self) -> (Vec<String>, Option<Result<String, String>>) {
        let mut pieces = Vec::new();
        while let Ok(delta) = self.deltas.try_recv() {
            pieces.push(delta);
        }
        (pieces, self.done.try_recv().ok())
    }

    /// Tells the worker to stop forwarding (the answer in flight is discarded by the caller).
    pub fn cancel(&self) {
        self.cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_old_store_path() {
        // Path separators differ per OS; the identity folder and file name are what matter.
        let path = Connection::store_path().unwrap();
        assert!(path.to_string_lossy().contains("LocalPDF"));
        assert!(path.ends_with("ai-connections.json"));
    }

    /// The saved shape is the suite's file format (the shared crate's `Connection`), so an existing
    /// `ai-connections.json` keeps working. The round trip is in memory: a test never writes the
    /// user's real store.
    #[test]
    fn round_trips_like_the_old_file_format() {
        let saved = vec![Connection { id: "a".into(), provider: "gemini".into(), label: "g".into(), base_url: String::new(), model: "m".into() }];
        let json = serde_json::to_string(&saved).expect("the file's JSON");
        let back: Vec<Connection> = serde_json::from_str(&json).expect("read back");
        assert_eq!(back, saved);
        // The shared crate reads the same file: its own shape has the same five fields.
        let shared: Vec<local_ai::Connection> = serde_json::from_str(&json).expect("the shared crate reads the same file");
        assert_eq!(shared.iter().map(|c| c.model.as_str()).collect::<Vec<_>>(), vec!["m"]);
    }

    /// A provider on the loopback: it answers the first POST with `body` as an SSE response and
    /// sends back the request (head and body). The CI gate for the streaming path, no network and
    /// no keychain.
    fn serve_sse(body: String) -> (String, std::sync::mpsc::Receiver<String>) {
        use std::io::{BufRead, BufReader, Read, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let port = listener.local_addr().expect("the bound address").port();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else { return };
            let mut reader = BufReader::new(stream.try_clone().expect("a reading handle"));
            let mut head = String::new();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                head.push_str(&line);
            }
            let length = head
                .lines()
                .find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length:").map(str::trim).map(str::to_owned))
                .and_then(|length| length.parse::<usize>().ok())
                .unwrap_or(0);
            let mut posted = vec![0u8; length];
            let _ = reader.read_exact(&mut posted);
            let _ = tx.send(format!("{head}\n{}", String::from_utf8_lossy(&posted)));
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{body}");
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        });
        (format!("http://127.0.0.1:{port}/v1"), rx)
    }

    /// The streaming path end to end over SSE: the pieces arrive in order and the answer joins.
    #[test]
    fn a_streamed_answer_arrives_in_pieces() {
        let answer = "O documento fala de **sepse**;\n- triagem\n- antibiótico";
        let cut = answer.char_indices().nth(answer.chars().count() / 2).map(|(i, _)| i).unwrap_or(answer.len());
        let chunk = |text: &str| format!("data: {{\"choices\":[{{\"delta\":{{\"content\":{}}}}}]}}\n\n", serde_json::to_string(text).unwrap());
        let body = format!("{}{}data: [DONE]\n\n", chunk(&answer[..cut]), chunk(&answer[cut..]));
        let (base_url, received) = serve_sse(body);
        let conn = Connection { id: "loopback".into(), provider: "custom".into(), label: "Loopback".into(), base_url, model: "mock".into() };
        let history = vec![Msg::System("sistema".into()), Msg::User("resuma".into())];
        let mut pieces = Vec::new();
        let whole = chat_stream_with_key(&conn, "test-key", &history, &mut |piece: &str| pieces.push(piece.to_owned())).expect("the streamed answer");
        assert_eq!(whole, answer, "the pieces join into the answer");
        assert_eq!(pieces, vec![answer[..cut].to_owned(), answer[cut..].to_owned()], "the pieces arrived in order");
        let request = received.recv().expect("the request");
        assert!(request.contains("POST /v1/chat/completions"), "{request}");
        assert!(request.contains("\"stream\":true"), "{request}");
    }
}
