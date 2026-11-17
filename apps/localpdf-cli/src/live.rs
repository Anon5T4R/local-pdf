//! The live MCP session: an agent drives the **running** LocalPDF app through its control channel.
//!
//! `localpdf-cli mcp --connect <control file>` serves MCP on stdio like the headless server, but
//! every tool call is forwarded to the app started with `localpdf --control <file>`: the user
//! watches the change live. The app's channel is loopback-only and authenticated (`auth` with the
//! token in the control file), and this module refuses anything else.
//!
//! The live tool set is the control channel's own (`ui_*`), plus the names the headless server
//! shares where the channel can honour them: `command_list`, `command_run` (a live command runs
//! exactly as the menu does, so parameters are not forwarded), `doc_open`, `doc_inspect` and
//! `render_preview`. Everything else (saving, exporting, OCR, forms.) needs the headless server.
//!
//! The protocol here is the suite's [`craft_mcp::Server`], so the framing, the lifecycle and the
//! error shapes are the same as every other Craft app's.

use std::io::{BufRead, BufReader, Write as _};
use std::net::TcpStream;
use std::time::Duration;

use craft_mcp::{Handler, Server, ServerInfo, ToolResult};
use serde_json::{Value, json};

/// How long a control call may take. The app answers within its own bounds; leave headroom.
const REPLY_TIMEOUT: Duration = Duration::from_secs(40);

/// A running app, reached through the control file it wrote.
pub struct Live {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    next_id: u64,
}

impl Live {
    /// Read `control_file` (the one `localpdf --control FILE` wrote) and authenticate.
    pub fn open(control_file: &str) -> Result<Self, String> {
        let info = crate::read_control_file(control_file)?;
        let raw = info["port"].as_u64().ok_or(format!("{control_file}: no port"))?;
        let port = u16::try_from(raw).map_err(|_| format!("{control_file}: {raw} is not a port number"))?;
        let token = info["token"].as_str().ok_or(format!("{control_file}: no token"))?.to_string();
        // The app listens on loopback only; this never reaches any other host.
        let stream =
            TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("can't reach the app on port {port}: {e} (is it still running?)"))?;
        stream.set_read_timeout(Some(REPLY_TIMEOUT)).map_err(|e| e.to_string())?;
        let reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        let mut live = Self { stream, reader, next_id: 1 };
        live.call("auth", json!({ "token": token }))?;
        Ok(live)
    }

    /// One request, one reply, on the authenticated connection.
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string();
        self.stream
            .write_all(line.as_bytes())
            .and_then(|()| self.stream.write_all(b"\n"))
            .and_then(|()| self.stream.flush())
            .map_err(|e| format!("the app at port {} stopped accepting requests: {e}", self.stream.peer_addr().map(|a| a.port()).unwrap_or(0)))?;
        let mut reply = String::new();
        if self.reader.read_line(&mut reply).map_err(|e| e.to_string())? == 0 {
            return Err("the app closed the control connection".into());
        }
        let v: Value = serde_json::from_str(reply.trim()).map_err(|e| format!("bad reply from the app: {e}"))?;
        if let Some(error) = v.get("error") {
            return Err(error["message"].as_str().unwrap_or("the app reported an error").to_string());
        }
        Ok(v.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Forward one control method and return its JSON.
    fn forward(&mut self, method: &str, args: &Value) -> ToolResult {
        let params = if args.is_null() { json!({}) } else { args.clone() };
        match self.call(method, params) {
            Ok(v) => ToolResult::json(&v),
            Err(e) => ToolResult::error(e),
        }
    }

    /// `ui.screenshot`: the control channel already answers base64 PNG.
    fn screenshot(&mut self, args: &Value) -> ToolResult {
        let params = if args.is_null() { json!({}) } else { args.clone() };
        match self.call("ui.screenshot", params) {
            Ok(v) => match v.get("png_base64").and_then(Value::as_str) {
                Some(data) => ToolResult { content: vec![json!({"type": "image", "data": data, "mimeType": "image/png"})], is_error: false },
                None => ToolResult::error("the app returned no PNG"),
            },
            Err(e) => ToolResult::error(e),
        }
    }
}

/// The control method a live tool name runs, or `None` when the live session doesn't offer it.
fn method_for(name: &str) -> Option<&'static str> {
    Some(match name {
        "ui_state" | "doc_inspect" => "ui.state",
        "ui_inspect" => "ui.inspect",
        "ui_click" => "ui.click",
        "ui_focus" => "ui.focus",
        "ui_type" => "ui.type",
        "ui_key" => "ui.key",
        "ui_drag" => "ui.drag",
        "ui_move" => "ui.move",
        "ui_resize" => "ui.resize",
        "ui_open" | "doc_open" => "ui.open",
        "ui_set" => "ui.set",
        "ui_quit" => "ui.quit",
        "ui_commands" | "command_list" => "ui.commands",
        _ => return None,
    })
}

impl Handler for Live {
    fn server_info(&self) -> ServerInfo {
        ServerInfo { name: "localpdf-live", title: "Local PDF (live)", version: env!("CARGO_PKG_VERSION") }
    }

    fn instructions(&self) -> String {
        "The running Local PDF app, driven through its control channel: every change is visible to \
         the user. ui_state shows the open documents and the view; ui_inspect, ui_click, ui_focus, \
         ui_type, ui_key, ui_drag and ui_move drive the real widgets and page; ui_open opens a file; \
         ui_commands and ui_command run a registry command exactly as the menu does (no parameters); \
         ui_screenshot returns the window as PNG. Saving, exporting and every other document \
         operation needs the headless server (`localpdf-cli mcp`)."
            .to_string()
    }

    fn describe(&self) -> String {
        "live app".to_string()
    }

    fn tools(&self) -> Vec<Value> {
        let any = || json!({"type": "object"});
        vec![
            tool("ui_state", "App state", "Open documents, active document, page, zoom, mode, panels and window size.", any()),
            tool("ui_inspect", "Inspect widgets", "The widget tree (AccessKit): ids, roles, labels, values, rects and states.", any()),
            tool("ui_click", "Click", "Click a widget by id or label, or the page at x, y.", any()),
            tool("ui_focus", "Focus", "Focus one widget by id or label.", any()),
            tool("ui_type", "Type text", "Type text into the focused widget.", any()),
            tool("ui_key", "Press a key", "Press a key, with optional modifiers.", any()),
            tool("ui_drag", "Drag", "Press, move and release between two points.", any()),
            tool("ui_move", "Move the pointer", "Move the pointer without pressing a button.", any()),
            tool("ui_resize", "Resize the window", "Request a native window size in logical points.", any()),
            tool("ui_set", "Set a view option", "View options and persistent preferences (page, zoom, theme.).", any()),
            tool("ui_quit", "Quit", "Normal native exit; refused while a document has unsaved edits.", any()),
            tool("ui_open", "Open a file", "Open a PDF in the app (or focus its tab).", any()),
            tool("ui_commands", "List commands", "Every registry command with its menu and shortcut.", any()),
            tool("ui_command", "Run a command", "Run a registry command as the menu would.", any()),
            tool("ui_screenshot", "Screenshot", "The window as a PNG image.", any()),
            tool("command_list", "List commands", "The live app's registry commands (ui.commands).", any()),
            tool(
                "command_run",
                "Run a command",
                "Run a registry command as the menu would (ui.command). A live command has no parameters; \
                 use the headless server for those.",
                any(),
            ),
            tool("doc_open", "Open a document", "Open a PDF in the running app (ui.open).", any()),
            tool("doc_inspect", "Inspect the app", "Open documents and session state (ui.state).", any()),
            tool("render_preview", "Render the window", "The window as a PNG, bounded by the app (ui.screenshot).", any()),
        ]
    }

    fn call_tool(&mut self, name: &str, arguments: &Value) -> ToolResult {
        match name {
            "render_preview" | "ui_screenshot" => return self.screenshot(arguments),
            "command_run" | "ui_command" => {
                let id = arguments.get("command").or_else(|| arguments.get("id")).and_then(Value::as_str);
                let Some(id) = id else {
                    return ToolResult::error("command_run needs `command`: a command id from command_list");
                };
                let has_params = arguments.get("params").and_then(Value::as_object).is_some_and(|p| !p.is_empty());
                if has_params {
                    return ToolResult::error(
                        "a live session runs commands exactly as the menu does, so parameters are not forwarded; \
                         use the headless server (`localpdf-cli mcp`) for parameterized commands",
                    );
                }
                return self.forward("ui.command", &json!({ "id": id }));
            }
            _ => {}
        }
        match method_for(name) {
            Some(method) => self.forward(method, arguments),
            None => ToolResult::error(format!(
                "unknown tool {name:?} in a live session; it offers the ui_* tools, command_list, command_run, \
                 doc_open, doc_inspect and render_preview. Start the headless server (`localpdf-cli mcp`) for the \
                 full tool set."
            )),
        }
    }
}

/// One tool definition.
fn tool(name: &str, title: &str, description: &str, schema: Value) -> Value {
    json!({"name": name, "title": title, "description": description, "inputSchema": schema})
}

/// Serve `mcp --connect <control file>` on stdio.
pub fn serve(control_file: &str) -> Result<(), String> {
    let live = Live::open(control_file)?;
    // The app answers within its own bounds; say where this session goes, on stderr only.
    let _ = writeln!(std::io::stderr().lock(), "localpdf-cli: MCP server on stdio (live app through {control_file}); close stdin to stop");
    let mut server = Server::new(live);
    server.serve(std::io::stdin().lock(), std::io::stdout().lock()).map_err(|e| e.to_string())
}
