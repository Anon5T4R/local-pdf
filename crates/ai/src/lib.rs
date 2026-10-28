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
pub const APP: local_ai::App = local_ai::App { id: "LocalPDF", url: "https://github.com/Anon5T4R/local-pdf" };

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_old_store_path() {
        assert!(Connection::store_path().unwrap().ends_with(r"LocalPDF\ai-connections.json"));
    }

    #[test]
    fn round_trips_like_the_old_file_format() {
        let saved = vec![Connection { id: "a".into(), provider: "gemini".into(), label: "g".into(), base_url: String::new(), model: "m".into() }];
        Connection::save_all(&saved).unwrap();
        assert_eq!(Connection::load_all().unwrap(), saved);
    }
}
