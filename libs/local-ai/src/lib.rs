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
//!
//! Requests go out through one shared `ureq::Agent` whose TLS provider is `rustls-rustcrypto`
//! (pure Rust): no C/asm crypto backend is compiled into the BYOK path.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

pub mod extract;

/// How long a request may spend resolving DNS, and then connecting. A provider that is down or a
/// name that does not resolve must fail fast, not leave a worker thread waiting.
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the request may spend sending and waiting for the response headers.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);
/// Total budget for reading the body. An SSE stream that never sends `[DONE]` and never closes
/// would otherwise block the reader forever; two minutes is long enough for a slow model and short
/// enough that a stuck stream ends with a visible error instead of a silent wait.
const BODY_TIMEOUT: Duration = Duration::from_secs(120);

/// The one crypto provider this crate uses: `rustls-rustcrypto`, pure Rust, so no C/asm TLS
/// backend (ring, aws-lc-rs, CNG) is ever compiled or loaded through the BYOK path.
fn pure_provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls_rustcrypto::provider())
}

/// The one HTTP agent every request shares: ureq is built without a default provider, so this
/// agent carries the pure one explicitly. A handshake it cannot complete surfaces as an error to
/// the caller; nothing falls back to another provider.
///
/// Every phase has a deadline: without them a provider that goes quiet mid-stream leaves the
/// caller blocked until the TCP connection times out (which may be never), and the UI shows a
/// spinner forever. The deadlines turn that into an ordinary error the app can show.
fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let tls = ureq::tls::TlsConfig::builder()
            .unversioned_rustls_crypto_provider(pure_provider())
            .build();
        ureq::Agent::config_builder()
            .tls_config(tls)
            // A non-2xx is a response, not a transport failure: the body carries the provider's
            // own message ("invalid api key", "model not found"), which is the only useful thing
            // to show. Without this, `send` errors with "http status: 401" and the body is lost.
            .http_status_as_error(false)
            .timeout_resolve(Some(RESOLVE_TIMEOUT))
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_send_request(Some(REQUEST_TIMEOUT))
            .timeout_recv_response(Some(RESPONSE_TIMEOUT))
            .timeout_recv_body(Some(BODY_TIMEOUT))
            .build()
            .new_agent()
    })
}

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
    Provider {
        id: "gemini",
        label: "Google Gemini",
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        vision: true,
    },
    Provider {
        id: "openai",
        label: "OpenAI (GPT)",
        base_url: "https://api.openai.com/v1",
        vision: true,
    },
    Provider {
        id: "groq",
        label: "Groq",
        base_url: "https://api.groq.com/openai/v1",
        vision: true,
    },
    Provider {
        id: "openrouter",
        label: "OpenRouter",
        base_url: "https://openrouter.ai/api/v1",
        vision: true,
    },
    Provider {
        id: "opencode-zen",
        label: "OpenCode Zen",
        base_url: "https://opencode.ai/zen/v1",
        vision: false,
    },
    Provider {
        id: "opencode-go",
        label: "OpenCode GO",
        // The Go subscription is served from its own base, not the Zen pay-as-you-go one: pointing
        // it at Zen makes a Go key bill against Zen credits (or fail). Requests also need the
        // per-conversation session header, added in `authorize`.
        base_url: "https://opencode.ai/inference/go/openai/v1",
        vision: false,
    },
    Provider {
        id: "custom",
        label: "Custom (OpenAI-compatible)",
        base_url: "",
        vision: false,
    },
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
        let Some(path) = Self::store_path(app) else {
            return Ok(Vec::new());
        };
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let mut all: Vec<Connection> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
                for connection in &mut all {
                    normalize_connection(connection);
                }
                Ok(all)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Save every connection: write a sibling then rename over the real file.
    pub fn save_all(app: &App, all: &[Connection]) -> Result<(), String> {
        let Some(path) = Self::store_path(app) else {
            return Err("no data directory".into());
        };
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
        keyring::Entry::new(app.id, &format!("ai-connection-{}", self.id))
            .map_err(|e| e.to_string())
    }

    pub fn set_key(&self, app: &App, key: &str) -> Result<(), String> {
        self.keyring_entry(app)?
            .set_password(key)
            .map_err(|e| e.to_string())
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
        if base.is_empty() {
            provider(&self.provider)
                .map(|p| p.base_url.trim_end_matches('/').to_owned())
                .unwrap_or_default()
        } else {
            base
        }
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
    if conn.provider == "opencode-go" {
        // The Go gateway routes by a session id; a missing/invalid one turns the request into a
        // pay-as-you-go Models request. The validated recipe (the user's working extension and the
        // opencode harnesses) is a fresh UUID per request: `crypto.randomUUID()`.
        headers.push(("x-opencode-session".to_owned(), uuid_v4()));
        headers.push(("x-opencode-client".to_owned(), app.id.to_owned()));
        headers.push(("User-Agent".to_owned(), format!("{}/1.0", app.id)));
    }
    headers
}

/// A UUID v4-shaped id (8-4-4-4-12 hex, version 4, variant 10xx). The Go gateway only needs a unique
/// session id per request; this is the shape `crypto.randomUUID()` produces, without a `uuid` crate.
fn uuid_v4() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_nanos()).unwrap_or(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let bits = (nanos as u128) ^ ((counter as u128) << 64) ^ 0x9E37_79B9_7F4A_7C15_1234_5678_9ABC_DEF0;
    let mut bytes = bits.to_be_bytes();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

/// Fix the base URL of a connection saved before a preset moved. The `opencode-go` preset pointed at
/// the Zen (`/zen/v1`) base in earlier versions; a connection created then keeps billing the Zen
/// pay-as-you-go account even though the Go plan is paid. A saved base equal to the *other* preset's
/// default is the stale copy, so it is replaced by the provider's own base. A hand-edited base that
/// is neither default is left alone.
fn normalize_connection(conn: &mut Connection) {
    let Some(own) = provider(&conn.provider) else { return };
    let base = conn.base_url.trim().trim_end_matches('/');
    if own.base_url.is_empty() || base.is_empty() || base == own.base_url.trim_end_matches('/') {
        return;
    }
    let stale = PROVIDERS
        .iter()
        .any(|other| other.id != own.id && !other.base_url.is_empty() && other.base_url.trim_end_matches('/') == base);
    if stale {
        conn.base_url = own.base_url.to_owned();
    }
}

/// The `/models` URL and its headers for the OpenAI-compatible providers.
fn models_url(
    app: &App,
    conn: &Connection,
    key: &str,
) -> Result<(String, Vec<(String, String)>), String> {
    let base = conn.effective_base();
    if base.is_empty() {
        return Err("the connection has no base URL".into());
    }
    Ok((format!("{base}/models"), authorize(app, conn, key)))
}

/// Gemini's OpenAI-compatible base (`…/v1beta/openai`) has **no** `/models` list: that route is 404
/// (verified against the live service). The native list lives one level up (`…/v1beta/models`),
/// authenticates with `x-goog-api-key`, and answers `{ "models": [{ "name": "models/…" }] }`.
fn gemini_models_base(conn: &Connection) -> String {
    let base = conn.effective_base();
    base.strip_suffix("/openai").map(str::to_owned).unwrap_or(base)
}

/// The ids from Gemini's native model list, keeping only the chat models (`generateContent`) and
/// dropping the `models/` prefix the OpenAI-compatible chat endpoint does not expect.
fn parse_gemini_models(body: &str) -> Result<Vec<String>, String> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let models = value.get("models").and_then(|m| m.as_array()).ok_or("the answer has no models")?;
    let mut out: Vec<String> = models
        .iter()
        .filter(|model| {
            model
                .get("supportedGenerationMethods")
                .and_then(|methods| methods.as_array())
                .map_or(true, |methods| methods.iter().any(|method| method.as_str() == Some("generateContent")))
        })
        .filter_map(|model| model.get("name").and_then(|name| name.as_str()).map(|name| name.strip_prefix("models/").unwrap_or(name).to_owned()))
        .collect();
    out.sort();
    Ok(out)
}

/// The provider's model list, sorted for the picker.
pub fn list_models(app: &App, conn: &Connection) -> Result<Vec<String>, String> {
    let key = conn.key(app)?;
    list_models_with_key(app, conn, &key)
}

/// [`list_models`] with the key handed over by the caller (the keychain is skipped), so the transport
/// can be exercised end to end in tests.
pub fn list_models_with_key(app: &App, conn: &Connection, key: &str) -> Result<Vec<String>, String> {
    // Gemini's OpenAI-compatible route has no model list; use its native one.
    if conn.provider == "gemini" {
        let base = gemini_models_base(conn);
        if base.is_empty() {
            return Err("the connection has no base URL".into());
        }
        let url = format!("{base}/models");
        let mut response = agent().get(&url).header("x-goog-api-key", key).call().map_err(|e| format!("{e}"))?;
        let status = response.status();
        let body = response.body_mut().read_to_string().map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(provider_error(status, &body));
        }
        return parse_gemini_models(&body);
    }
    let (url, headers) = models_url(app, conn, key)?;
    let mut req = agent().get(&url);
    for (k, v) in headers {
        req = req.header(&k, &v);
    }
    let mut response = req.call().map_err(|e| format!("{e}"))?;
    let status = response.status();
    let body = response.body_mut().read_to_string().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(provider_error(status, &body));
    }
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
    let mut builder = agent().post(&format!("{base}/chat/completions")).header("Content-Type", "application/json");
    for (k, v) in authorize(app, conn, &key) {
        builder = builder.header(&k, &v);
    }
    let mut response = builder.send(body.as_bytes()).map_err(|e| e.to_string())?;
    let status = response.status();
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(provider_error(status, &text));
    }
    parse_chat(&text)
}

/// A vision request: one picture plus the instruction, as a one-turn conversation.
pub fn vision(
    app: &App,
    conn: &Connection,
    instruction: &str,
    png: &[u8],
) -> Result<String, String> {
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
    let mut builder = agent().post(&format!("{base}/chat/completions")).header("Content-Type", "application/json");
    for (k, v) in authorize(app, conn, &key) {
        builder = builder.header(&k, &v);
    }
    let mut response = builder.send(body.as_bytes()).map_err(|e| e.to_string())?;
    let status = response.status();
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(provider_error(status, &text));
    }
    parse_chat(&text)
}

/// A streaming chat: `on_delta` receives each piece of the answer as it arrives, and the whole text
/// comes back at the end.
///
/// The wire format is the OpenAI `stream: true` one (SSE): the provider sends `data:` lines, one
/// JSON object per event carrying `choices[0].delta.content`, and `data: [DONE]` closes it. Ollama,
/// llama-server and every OpenAI-compatible gateway speak it; a provider that ignores `stream`
/// answers with a plain body, which [`read_stream`] still reads (one event, whole text).
pub fn chat_stream(
    app: &App,
    conn: &Connection,
    history: &[Msg],
    on_delta: impl FnMut(&str),
) -> Result<String, String> {
    let key = conn.key(app)?;
    chat_stream_with_key(app, conn, &key, history, on_delta)
}

/// [`chat_stream`], with the key handed over by the caller instead of read from the OS keychain.
/// The suite keeps its key there; a test (or a caller that stores the secret elsewhere) supplies it
/// directly, so the transport can be exercised end to end without touching the keychain. The key is
/// never written anywhere by this crate.
pub fn chat_stream_with_key(
    app: &App,
    conn: &Connection,
    key: &str,
    history: &[Msg],
    mut on_delta: impl FnMut(&str),
) -> Result<String, String> {
    let base = conn.effective_base();
    if base.is_empty() {
        return Err("the connection has no base URL".into());
    }
    if conn.model.is_empty() {
        return Err("no model chosen".into());
    }
    let messages: Vec<serde_json::Value> = history
        .iter()
        .map(|m| match m {
            Msg::System(t) => serde_json::json!({ "role": "system", "content": t }),
            Msg::User(t) => serde_json::json!({ "role": "user", "content": t }),
            Msg::Assistant(t) => serde_json::json!({ "role": "assistant", "content": t }),
        })
        .collect();
    let body = serde_json::json!({ "model": conn.model, "messages": messages, "stream": true })
        .to_string();
    let mut builder = agent().post(&format!("{base}/chat/completions")).header("Content-Type", "application/json");
    for (k, v) in authorize(app, conn, key) {
        builder = builder.header(&k, &v);
    }
    let mut response = builder.send(body.as_bytes()).map_err(|e| e.to_string())?;
    let status = response.status();
    if !status.is_success() {
        // The provider's own message is the only useful thing here (a bad key, a moved model).
        let text = response.body_mut().read_to_string().unwrap_or_default();
        return Err(provider_error(status, &text));
    }
    read_stream(
        std::io::BufReader::new(response.body_mut().as_reader()),
        &mut on_delta,
    )
}

/// Reads an OpenAI-style SSE body. Split from the request so the contract is testable without a
/// network: the deltas in order, the joined answer, the provider's error, and a plain body.
fn read_stream(
    reader: impl std::io::BufRead,
    on_delta: &mut impl FnMut(&str),
) -> Result<String, String> {
    let mut answer = String::new();
    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                // A provider that never sends `[DONE]` and never closes the stream (keep-alive)
                // hits the body deadline. If the text already arrived, keep it; if nothing came,
                // the timeout is the error the user has to see.
                if answer.is_empty() {
                    return Err(format!("reading the answer failed: {error}"));
                }
                break;
            }
        };
        // SSE comments (`:`), the blank line between events, and `event:`/`id:` fields carry nothing
        // this client shows.
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() {
            continue;
        }
        if data == "[DONE]" {
            break;
        }
        let value: serde_json::Value = match serde_json::from_str(data) {
            Ok(value) => value,
            // A provider that ignored `stream` sends the plain chat JSON: read it whole.
            Err(_) => return parse_chat(data),
        };
        if let Some(error) = value.get("error") {
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown provider error");
            return Err(message.to_owned());
        }
        // A non-streaming answer inside a `data:` line: the whole chat JSON, done.
        if value.pointer("/choices/0/message/content").is_some() {
            return parse_chat(data);
        }
        if let Some(delta) = value
            .pointer("/choices/0/delta/content")
            .and_then(|c| c.as_str())
        {
            answer.push_str(delta);
            on_delta(delta);
        }
    }
    if answer.is_empty() {
        Err("the stream ended without text".into())
    } else {
        Ok(answer)
    }
}

/// The provider's reason for a non-2xx answer: the `error.message` every OpenAI-compatible gateway
/// sends, else the raw body, else the status. Never empty, so the caller always has something to
/// show the user.
fn provider_error(status: impl std::fmt::Display, body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        let error = value.get("error");
        // The most useful reason first: OpenRouter puts the upstream provider's own message in
        // `error.metadata.raw` (the plain `error.message` there is only "Provider returned error"),
        // then the usual `error.message`, then a top-level `message`.
        let message = error
            .and_then(|e| e.get("metadata"))
            .and_then(|m| m.get("raw"))
            .and_then(|r| r.as_str())
            .or_else(|| error.and_then(|e| e.get("message")).and_then(|m| m.as_str()))
            .or_else(|| value.get("message").and_then(|m| m.as_str()));
        if let Some(message) = message {
            // The provider name turns "rate-limited" into something actionable.
            let provider = error.and_then(|e| e.get("metadata")).and_then(|m| m.get("provider_name")).and_then(|p| p.as_str());
            return match provider {
                Some(provider) => format!("{provider}: {message}"),
                None => message.to_owned(),
            };
        }
    }
    let text = body.trim();
    if text.is_empty() {
        format!("HTTP {status}")
    } else {
        format!("HTTP {status}: {text}")
    }
}

/// The one shape every OpenAI-compatible chat answer takes. A provider error body has no
/// choices: surface its text so the panel can show why.
fn parse_chat(body: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("unreadable answer: {e}"))?;
    if let Some(err) = value.get("error") {
        let message = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown provider error");
        return Err(message.to_owned());
    }
    value
        .pointer("/choices/0/message/content")
        .and_then(|c| c.as_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("the answer has no text: {body}"))
}

/// Standard base64 (RFC 4648), padding included: data URIs want it.
fn base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
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

    /// OpenRouter's 429 hides the useful reason in `error.metadata.raw`; the banner must show that
    /// and the provider name, not the generic "Provider returned error".
    #[test]
    fn a_provider_error_prefers_the_raw_metadata_reason() {
        let body = r#"{"error":{"message":"Provider returned error","code":429,"metadata":{"raw":"google/gemma-4-31b-it:free is temporarily rate-limited upstream.","provider_name":"Google AI Studio"}}}"#;
        let error = provider_error(429, body);
        assert!(error.contains("Google AI Studio"), "{error}");
        assert!(error.contains("temporarily rate-limited upstream"), "{error}");
        assert!(!error.contains("Provider returned error"), "{error}");
    }

    /// A plain provider error (no metadata) keeps its own message.
    #[test]
    fn a_plain_provider_error_keeps_its_message() {
        let body = r#"{"error":{"message":"invalid api key"}}"#;
        assert_eq!(provider_error(401, body), "invalid api key");
    }

    #[test]
    fn base64_matches_the_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    /// The contract of the streaming reader: the deltas arrive in order, the answer is their join,
    /// and the SSE scaffolding (`event:`, comments, blank lines) is not text.
    #[test]
    fn a_stream_delivers_the_deltas_in_order() {
        let body = "\
: keep-alive\n\
event: message\n\
data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"}}]}\n\
\n\
data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Se\"}}]}\n\
\n\
data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"pse\"}}]}\n\
\n\
data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\" é grave\"}}]}\n\
\n\
data: [DONE]\n";
        let mut deltas = Vec::new();
        let answer = read_stream(std::io::Cursor::new(body), &mut |delta| {
            deltas.push(delta.to_owned())
        })
        .unwrap();
        assert_eq!(deltas, ["Se", "pse", " é grave"]);
        assert_eq!(answer, "Sepse é grave");
    }

    #[test]
    fn a_stream_surfaces_a_provider_error() {
        let body = "data: {\"error\":{\"message\":\"bad key\"}}\n";
        let error = read_stream(std::io::Cursor::new(body), &mut |_| {}).unwrap_err();
        assert_eq!(error, "bad key");
    }

    #[test]
    fn a_provider_that_ignores_stream_sends_the_plain_answer() {
        // One `data:` line holding the whole chat JSON, which is what a gateway does when it does
        // not stream: the client still gets the text.
        let body = "data: {\"choices\":[{\"message\":{\"content\":\"pronto\"}}]}\n";
        let answer = read_stream(std::io::Cursor::new(body), &mut |_| {}).unwrap();
        assert_eq!(answer, "pronto");
    }

    #[test]
    fn an_empty_stream_is_an_error_not_an_empty_answer() {
        let error = read_stream(std::io::Cursor::new("data: [DONE]\n"), &mut |_| {}).unwrap_err();
        assert!(error.contains("without text"), "{error}");
    }

    /// The shared agent is wired to the pure provider: TLS 1.3 suites can only come from
    /// `rustls-rustcrypto`, never from a feature-enabled C backend.
    #[test]
    fn the_shared_agent_is_wired_to_the_pure_provider() {
        let provider = pure_provider();
        assert!(
            provider
                .cipher_suites
                .iter()
                .any(|s| s.suite() == rustls::CipherSuite::TLS13_AES_256_GCM_SHA384)
        );
    }

    /// Every phase of the shared agent has a deadline. Without them a provider that goes quiet
    /// mid-stream leaves the caller blocked until the TCP stack gives up (possibly never).
    #[test]
    fn the_shared_agent_has_deadlines() {
        let timeouts = agent().config().timeouts();
        assert_eq!(timeouts.resolve, Some(RESOLVE_TIMEOUT));
        assert_eq!(timeouts.connect, Some(CONNECT_TIMEOUT));
        assert_eq!(timeouts.send_request, Some(REQUEST_TIMEOUT));
        assert_eq!(timeouts.recv_response, Some(RESPONSE_TIMEOUT));
        assert_eq!(timeouts.recv_body, Some(BODY_TIMEOUT));
    }

    /// A provider that sends the headers and then holds the connection open without a byte of body
    /// must end as an error at the body deadline, not block forever.
    #[test]
    fn a_stream_with_no_body_times_out() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::{Duration, Instant};

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            if let Ok((mut socket, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = socket.read(&mut buf);
                let _ = socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n");
                let _ = socket.flush();
                std::thread::sleep(Duration::from_secs(3));
            }
        });

        let started = Instant::now();
        let mut response = short_agent(1)
            .post(&format!("http://127.0.0.1:{port}/v1/chat/completions"))
            .send(b"{}".as_slice())
            .expect("headers");
        let error = read_stream(std::io::BufReader::new(response.body_mut().as_reader()), &mut |_| {}).expect_err("no body must time out");
        assert!(started.elapsed() < Duration::from_secs(3), "the deadline stops the wait");
        assert!(!error.is_empty());
    }

    /// A stream that delivered text and then went quiet (no `[DONE]`, no close) keeps the text: the
    /// answer is not thrown away because the provider forgot to close.
    #[test]
    fn a_stream_that_goes_quiet_after_text_keeps_the_text() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::{Duration, Instant};

        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"pronto\"}}]}\n\n";
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            if let Ok((mut socket, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = socket.read(&mut buf);
                let head = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{}\r\n", body.len(), body);
                let _ = socket.write_all(head.as_bytes());
                let _ = socket.flush();
                std::thread::sleep(Duration::from_secs(3));
            }
        });

        let started = Instant::now();
        let mut response = short_agent(1)
            .post(&format!("http://127.0.0.1:{port}/v1/chat/completions"))
            .send(b"{}".as_slice())
            .expect("headers");
        let answer = read_stream(std::io::BufReader::new(response.body_mut().as_reader()), &mut |_| {}).expect("the text arrives before the deadline");
        assert_eq!(answer, "pronto");
        assert!(started.elapsed() < Duration::from_secs(3), "the deadline stops the wait");
    }

    /// An agent with a short body deadline, for the socket tests.
    fn short_agent(body_secs: u64) -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_recv_body(Some(Duration::from_secs(body_secs)))
            .build()
            .new_agent()
    }

    /// End to end: the configured agent completes a real TLS handshake. Ignored by default (it
    /// needs the network); run with `cargo test -- --ignored`.
    #[test]
    #[ignore = "network: real TLS handshake with api.github.com"]
    fn the_shared_agent_completes_a_real_handshake() -> Result<(), String> {
        let mut response = agent()
            .get("https://api.github.com/")
            .call()
            .map_err(|e| e.to_string())?;
        response
            .body_mut()
            .read_to_string()
            .map(|_body| ())
            .map_err(|e| e.to_string())
    }

    #[test]
    fn the_store_path_lives_under_the_apps_folder() {
        let app = App {
            id: "TestApp",
            url: "",
        };
        // Path separators differ per OS; the identity folder and file name are what matter.
        let path = Connection::store_path(&app).unwrap();
        assert!(path.to_string_lossy().contains("TestApp"));
        assert!(path.ends_with("ai-connections.json"));
    }

    /// The Go preset points at Go's own base (not Zen, the pay-as-you-go one) and identifies the
    /// app per conversation. Pointing Go at Zen is what makes a Go key bill the wrong account; the
    /// session header is what the Go gateway requires to route a request at all.
    #[test]
    fn the_opencode_go_preset_uses_the_go_base_and_session() {
        let go = provider("opencode-go").expect("the Go preset exists");
        assert_eq!(go.base_url, "https://opencode.ai/inference/go/openai/v1");

        let app = App { id: "LocalMind", url: "" };
        let conn = Connection { id: "conv-7".into(), provider: "opencode-go".into(), label: "Go".into(), base_url: String::new(), model: "deepseek-v4.1-flash".into() };
        let headers = authorize(&app, &conn, "k");
        let session = headers.iter().find(|(key, _)| key == "x-opencode-session").map(|(_, value)| value.clone()).expect("the session header");
        assert_eq!(session.len(), 36, "a UUID, like crypto.randomUUID(): {session}");
        let groups: Vec<&str> = session.split('-').collect();
        assert_eq!(groups.iter().map(|group| group.len()).collect::<Vec<_>>(), [8, 4, 4, 4, 12], "{session}");
        assert!(groups[2].starts_with('4'), "version 4: {session}");
        assert!(groups[3].starts_with(['8', '9', 'a', 'b']), "variant 10xx: {session}");
        assert!(session.chars().filter(|c| *c != '-').all(|c| c.is_ascii_hexdigit()), "{session}");
        assert!(headers.iter().any(|(key, value)| key == "x-opencode-client" && value == "LocalMind"));
        assert!(headers.iter().any(|(key, value)| key == "User-Agent" && value == "LocalMind/1.0"));

        // Two requests get different session ids (the recipe is one per request).
        let other = authorize(&app, &conn, "k");
        let other_session = other.iter().find(|(key, _)| key == "x-opencode-session").map(|(_, value)| value.clone()).expect("session");
        assert_ne!(session, other_session);

        let mut zen = conn.clone();
        zen.provider = "opencode-zen".into();
        assert!(!authorize(&app, &zen, "k").iter().any(|(key, _)| key == "x-opencode-session"));
    }

    /// A connection saved before the Go preset moved still points at the Zen base, which bills the
    /// pay-as-you-go account even though the user pays Go. Loading fixes that; a hand-edited base
    /// that is neither preset's default is left alone.
    #[test]
    fn a_stale_opencode_go_base_moves_to_the_go_base() {
        let mut stale = Connection { id: "go".into(), provider: "opencode-go".into(), label: "Go".into(), base_url: "https://opencode.ai/zen/v1".into(), model: "deepseek-v4.1-flash".into() };
        normalize_connection(&mut stale);
        assert_eq!(stale.base_url, "https://opencode.ai/inference/go/openai/v1");

        let mut edited = stale.clone();
        edited.base_url = "https://gateway.example.com/v1".into();
        normalize_connection(&mut edited);
        assert_eq!(edited.base_url, "https://gateway.example.com/v1", "a custom base is the user's choice");
    }

    /// Gemini's OpenAI-compatible base has no `/models` route (404 live); the native list is one
    /// level up, and its ids drop the `models/` prefix and the non-chat models.
    #[test]
    fn gemini_models_use_the_native_list_and_keep_only_chat_models() {
        let conn = Connection {
            id: "g".into(),
            provider: "gemini".into(),
            label: "Gemini".into(),
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai".into(),
            model: String::new(),
        };
        // The native list lives one level up from the OpenAI-compatible base; the caller appends
        // `/models` to this root.
        assert_eq!(gemini_models_base(&conn), "https://generativelanguage.googleapis.com/v1beta");
        let body = r#"{"models":[
            {"name":"models/gemini-2.5-flash","supportedGenerationMethods":["generateContent","countTokens"]},
            {"name":"models/gemini-embedding-001","supportedGenerationMethods":["embedContent"]},
            {"name":"models/gemini-2.5-pro","supportedGenerationMethods":["generateContent"]}
        ]}"#;
        assert_eq!(parse_gemini_models(body).unwrap(), ["gemini-2.5-flash", "gemini-2.5-pro"]);
    }
}
