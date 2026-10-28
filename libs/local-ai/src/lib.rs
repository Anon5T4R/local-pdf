//! BYOK AI for the Local suite: the user's own keys, one OpenAI-compatible client for every
//! provider.
//!
//! Nothing here talks to a server: there is none. A connection is a provider preset (base URL),
//! a model name the picker fetches from the provider's own `/models`, and a secret the OS
//! keychain holds. Chat, vision and the models call all speak the OpenAI wire format, which
//! Gemini, Groq, OpenRouter and the OpenCode gateways also serve.
//!
//! Each suite app carries its own identity ([`App`]): the config folder its connections file
//! lives in, the keychain service its keys are grouped under, and the project URL the OpenRouter
//! headers advertise. Nothing else differs between apps.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The identity of the app consuming this crate: where its connections file lives, where its
/// keys are grouped in the OS keychain, and what OpenRouter should see as referer.
pub struct App {
    /// Folder under the user's data dir and keychain service name, e.g. `"LocalPDF"`.
    pub id: &'static str,
    /// Project URL advertised to OpenRouter (`HTTP-Referer`), e.g.
    /// `"https://github.com/Anon5T4R/local-pdf"`.
    pub url: &'static str,
}

/// The providers the connections panel offers. All serve the OpenAI wire format; the base URL
/// is editable, so a preset is only a default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Provider {
    pub id: &'static str,
    pub label: &'static str,
    pub base_url: &'static str,
    /// Whether the provider is known to serve vision (`image_url`) requests; only a hint for
    /// an OCR button's tooltip. Any provider may work: the model decides.
    pub vision: bool,
}

pub const PROVIDERS: &[Provider] = &[
    Provider { id: "gemini", label: "Google Gemini", base_url: "https://generativelanguage.googleapis.com/v1beta/openai", vision: true },
    Provider { id: "openai", label: "OpenAI (GPT)", base_url: "https://api.openai.com/v1", vision: true },
    Provider { id: "groq", label: "Groq", base_url: "https://api.groq.com/openai/v1", vision: true },
    Provider { id: "openrouter", label: "OpenRouter", base_url: "https://openrouter.ai/api/v1", vision: true },
    Provider { id: "opencode-zen", label: "OpenCode Zen", base_url: "https://opencode.ai/zen/v1", vision: false },
    Provider { id: "opencode-go", label: "OpenCode GO", base_url: "https://opencode.ai/zen/v1", vision: false },
    Provider { id: "custom", label: "Custom (OpenAI-compatible)", base_url: "", vision: false },
];

pub fn provider(id: &str) -> Option<&'static Provider> {
    PROVIDERS.iter().find(|p| p.id == id)
}

/// A saved connection: which provider, its base URL (editable for `custom` or when a gateway
/// moves) and the model the picker last chose. The API key is never in this file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub provider: String,
    pub label: String,
    pub base_url: String,
    pub model: String,
}

/// One chat turn.
#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    System(String),
    User(String),
    Assistant(String),
}

impl Connection {
    /// Where the connections file lives: `<data dir>/<app id>/ai-connections.json`.
    pub fn store_path(app: &App) -> Option<PathBuf> {
        dirs::data_dir().map(|d| d.join(app.id).join("ai-connections.json"))
    }

    /// Load every saved connection. A missing file is no connections, not an error; a broken
    /// one is an error, so a silent wipe can never pretend the setup is gone.
    pub fn load_all(app: &App) -> Result<Vec<Connection>, String> {
        let Some(path) = Self::store_path(app) else { return Ok(Vec::new()) };
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Save every connection: write a sibling then rename over the real file.
    pub fn save_all(app: &App, all: &[Connection]) -> Result<(), String> {
        let Some(path) = Self::store_path(app) else { return Err("no data directory".into()) };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(all).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }

    /// The keychain entry for this connection's key. The service name groups them under the app.
    fn keyring_entry(&self, app: &App) -> Result<keyring::Entry, String> {
        keyring::Entry::new(app.id, &format!("ai-connection-{}", self.id)).map_err(|e| e.to_string())
    }

    pub fn set_key(&self, app: &App, key: &str) -> Result<(), String> {
        self.keyring_entry(app)?.set_password(key).map_err(|e| e.to_string())
    }

    pub fn key(&self, app: &App) -> Result<String, String> {
        match self.keyring_entry(app)?.get_password() {
            Ok(k) => Ok(k),
            Err(keyring::Error::NoEntry) => Ok(String::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn has_key(&self, app: &App) -> bool {
        self.key(app).map(|k| !k.is_empty()).unwrap_or(false)
    }

    pub fn clear_key(&self, app: &App) -> Result<(), String> {
        match self.keyring_entry(app)?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn effective_base(&self) -> String {
        let base = self.base_url.trim().trim_end_matches('/').to_owned();
        if base.is_empty() { provider(&self.provider).map(|p| p.base_url.trim_end_matches('/').to_owned()).unwrap_or_default() } else { base }
    }
}

/// A result arriving on a background thread. The UI polls `poll` every frame.
pub struct Job<T: Send + 'static> {
    rx: std::sync::mpsc::Receiver<Result<T, String>>,
}

impl<T: Send + 'static> Job<T> {
    pub fn spawn(run: impl FnOnce() -> Result<T, String> + Send + 'static) -> Job<T> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(run());
        });
        Job { rx }
    }

    pub fn poll(&self) -> Option<Result<T, String>> {
        self.rx.try_recv().ok()
    }
}

/// One `Authorization: Bearer` header when a key is set; OpenRouter also takes a title, which
/// keeps its dashboard readable.
fn authorize(app: &App, conn: &Connection, key: &str) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    if !key.is_empty() {
        headers.push(("Authorization".to_owned(), format!("Bearer {key}")));
    }
    if conn.provider == "openrouter" {
        headers.push(("HTTP-Referer".to_owned(), app.url.to_owned()));
        headers.push(("X-Title".to_owned(), app.id.to_owned()));
    }
    headers
}

/// The `/models` URL and its headers.
fn models_url(app: &App, conn: &Connection, key: &str) -> Result<(String, Vec<(String, String)>), String> {
    let base = conn.effective_base();
    if base.is_empty() {
        return Err("the connection has no base URL".into());
    }
    Ok((format!("{base}/models"), authorize(app, conn, key)))
}

/// The provider's model list, sorted for the picker.
pub fn list_models(app: &App, conn: &Connection) -> Result<Vec<String>, String> {
    let key = conn.key(app)?;
    let (url, headers) = models_url(app, conn, &key)?;
    let mut req = ureq::get(&url);
    for (k, v) in headers {
        req = req.header(&k, &v);
    }
    let body = req.call().map_err(|e| format!("{e}"))?.body_mut().read_to_string().map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let mut out: Vec<String> = value
        .get("data")
        .and_then(|d| d.as_array())
        .map(|a| a.iter().filter_map(|m| m.get("id").and_then(|i| i.as_str()).map(str::to_owned)).collect())
        .unwrap_or_default();
    out.sort();
    Ok(out)
}

/// A chat request: the whole conversation, the document context already folded into the turns by
/// the caller.
pub fn chat(app: &App, conn: &Connection, history: &[Msg]) -> Result<String, String> {
    let base = conn.effective_base();
    if base.is_empty() {
        return Err("the connection has no base URL".into());
    }
    if conn.model.is_empty() {
        return Err("no model chosen".into());
    }
    let key = conn.key(app)?;
    let messages: Vec<serde_json::Value> = history
        .iter()
        .map(|m| match m {
            Msg::System(t) => serde_json::json!({ "role": "system", "content": t }),
            Msg::User(t) => serde_json::json!({ "role": "user", "content": t }),
            Msg::Assistant(t) => serde_json::json!({ "role": "assistant", "content": t }),
        })
        .collect();
    let body = serde_json::json!({ "model": conn.model, "messages": messages }).to_string();
    let mut builder = ureq::post(&format!("{base}/chat/completions"));
    for (k, v) in authorize(app, conn, &key) {
        builder = builder.header(&k, &v);
    }
    let mut response = builder.send(body.as_bytes()).map_err(|e| e.to_string())?;
    let text = response.body_mut().read_to_string().map_err(|e| e.to_string())?;
    parse_chat(&text)
}

/// A vision request: one picture plus the instruction, as a one-turn conversation.
pub fn vision(app: &App, conn: &Connection, instruction: &str, png: &[u8]) -> Result<String, String> {
    let base = conn.effective_base();
    if base.is_empty() {
        return Err("the connection has no base URL".into());
    }
    if conn.model.is_empty() {
        return Err("no model chosen".into());
    }
    let key = conn.key(app)?;
    let data = format!("data:image/png;base64,{}", base64(png));
    let body = serde_json::json!({
        "model": conn.model,
        "messages": [{ "role": "user", "content": [
            { "type": "text", "text": instruction },
            { "type": "image_url", "image_url": { "url": data } },
        ]}]
    })
    .to_string();
    let mut builder = ureq::post(&format!("{base}/chat/completions"));
    for (k, v) in authorize(app, conn, &key) {
        builder = builder.header(&k, &v);
    }
    let mut response = builder.send(body.as_bytes()).map_err(|e| e.to_string())?;
    let text = response.body_mut().read_to_string().map_err(|e| e.to_string())?;
    parse_chat(&text)
}

/// The one shape every OpenAI-compatible chat answer takes. A provider error body has no
/// choices: surface its text so the panel can show why.
fn parse_chat(body: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|e| format!("unreadable answer: {e}"))?;
    if let Some(err) = value.get("error") {
        let message = err.get("message").and_then(|m| m.as_str()).unwrap_or("unknown provider error");
        return Err(message.to_owned());
    }
    value.pointer("/choices/0/message/content").and_then(|c| c.as_str()).map(str::to_owned).ok_or_else(|| format!("the answer has no text: {body}"))
}

/// Standard base64 (RFC 4648), padding included: data URIs want it.
fn base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_chat_answer() {
        let body = serde_json::json!({ "choices": [{ "message": { "role": "assistant", "content": "olá" } }] }).to_string();
        assert_eq!(parse_chat(&body).unwrap(), "olá");
    }

    #[test]
    fn surfaces_a_provider_error() {
        let body = serde_json::json!({ "error": { "message": "bad key" } }).to_string();
        assert_eq!(parse_chat(&body).unwrap_err(), "bad key");
    }

    #[test]
    fn base64_matches_the_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn the_store_path_lives_under_the_apps_folder() {
        let app = App { id: "TestApp", url: "" };
        // Path separators differ per OS; the identity folder and file name are what matter.
        let path = Connection::store_path(&app).unwrap();
        assert!(path.to_string_lossy().contains("TestApp"));
        assert!(path.ends_with("ai-connections.json"));
    }
}
