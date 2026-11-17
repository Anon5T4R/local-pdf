//! The AI surface of the Craft App Suite: the connections form, the status strip and the chat panel.
//!
//! Every app that talks to a model shows the same three things to the same user: where the model is
//! (and the key, in the OS keychain), what is happening while it answers, and the conversation
//! itself. Those are this crate. What differs per app - the system prompt, what the document context
//! is, what "insert" means - stays in the app, passed in through the arguments and callbacks of each
//! component. The crate never touches the network: the app starts its own client and feeds the panel.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod chat;
pub mod connections;
pub mod policy;
pub mod strip;

pub use chat::{ChatAction, ChatLabels, ChatPanel};
pub use connections::{ConnectionDraft, ConnectionsIntent, ConnectionsPanel, Labels as ConnectionsLabels, Preset};
pub use policy::{ContextInfo, ContextKind, ContextSource, Line, Role, Side, Turns, WindowPolicy};
pub use strip::{StatusStrip, StripAction, StripLabels};
