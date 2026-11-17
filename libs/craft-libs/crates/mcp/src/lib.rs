//! MCP plumbing shared by the Craft apps.
//!
//! The apps are agent-native: each one exposes what it can do as commands, and an agent drives it
//! through a [Model Context Protocol](https://modelcontextprotocol.io) server. This crate is the
//! part of that which is the same in every app, so a second app adopts it without rewriting a
//! protocol:
//!
//! - [`jsonrpc`]: newline-delimited JSON-RPC 2.0 framing, the protocol revisions and the bounded
//!   line reader.
//! - [`server`]: an MCP server over stdio in front of a [`Handler`] the app implements (its tools,
//!   resources, prompts and the calls that run them).
//! - [`script`]: the job/step file a headless run reads (`run --script job.json`), the batch steps
//!   an agent sends, and the `"$N.path"` references between steps.
//! - [`automation`]: the `--automation-read-root` / `--automation-write-root` flags and the
//!   containment check they promise. The same flags in every app mean one MCP client config for
//!   the whole suite.
//! - [`remote`]: the bridge from the MCP server to a running app over its loopback JSON control
//!   channel (`mcp --connect PORT`).
//!
//! The app still owns its commands, its document model, its renders and its control channel. This
//! crate never draws anything and holds no state beyond one server's connection.
//!
//! Provenance: adapted from `storytold/vectorcraft` (`crates/mcp`, `docs/mcp.md`,
//! `docs/control-protocol.md`) and `storytold/wordcraft` (`--connect`, `run --script`), both
//! `MIT OR Apache-2.0`. See the README for what changed and what stayed.
#![forbid(unsafe_code)]

pub mod automation;
pub mod jsonrpc;
pub mod remote;
pub mod script;
pub mod server;

pub use jsonrpc::{PROTOCOL_VERSION, SUPPORTED_VERSIONS};
pub use server::{Handler, ReadError, ResourceContent, Server, ServerInfo, ToolResult};
