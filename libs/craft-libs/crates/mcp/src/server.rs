//! An MCP server over stdio in front of an app's [`Handler`].
//!
//! The server owns the protocol: the JSON-RPC envelope, the lifecycle, the method names and their
//! errors. The app owns everything else: which tools exist, what they do, which resources and
//! prompts it serves. That split is what makes a second app cheap: it implements [`Handler`] over
//! its own commands and reuses this server unchanged.
//!
//! This skeleton is single-threaded on purpose: one request at a time, answered before the next
//! line is read. An app that wants to hear the client during a long call (cancellation, progress,
//! `ping`) grows that on top; the apps that already have it keep it. Correctness and a small
//! surface come first for a new app.

use std::io::{BufRead, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};

use serde_json::{Value, json};

use crate::jsonrpc::{
    INTERNAL_ERROR, INVALID_PARAMS, INVALID_REQUEST, MAX_BATCH_MESSAGES, MAX_LINE_BYTES, METHOD_NOT_FOUND, PARSE_ERROR, RESOURCE_NOT_FOUND,
    complete_result, error, negotiate, read_line_capped, response, write_line,
};

/// What `initialize` answers with in `serverInfo`.
#[derive(Clone, Copy, Debug)]
pub struct ServerInfo {
    pub name: &'static str,
    pub title: &'static str,
    pub version: &'static str,
}

/// The result of `tools/call`: MCP content blocks plus the `isError` flag.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolResult {
    pub content: Vec<Value>,
    pub is_error: bool,
}

impl ToolResult {
    /// A text block.
    pub fn text(t: impl Into<String>) -> Self {
        Self { content: vec![json!({"type": "text", "text": t.into()})], is_error: false }
    }

    /// A pretty-printed JSON value.
    pub fn json(v: &Value) -> Self {
        Self::text(serde_json::to_string_pretty(v).unwrap_or_default())
    }

    /// An error the agent can read (and act on), with `isError` set.
    pub fn error(t: impl Into<String>) -> Self {
        Self { content: vec![json!({"type": "text", "text": t.into()})], is_error: true }
    }

    /// The `result` object of `tools/call`.
    pub fn to_value(&self) -> Value {
        json!({"content": self.content, "isError": self.is_error})
    }
}

/// What a `resources/read` returns.
#[derive(Clone, Debug, PartialEq)]
pub struct ResourceContent {
    pub mime_type: String,
    pub text: String,
}

/// Why `resources/read` found nothing (`-32002`), was asked something malformed (`-32602`), or the
/// app failed (`-32603`).
#[derive(Clone, Debug, PartialEq)]
pub enum ReadError {
    NotFound(String),
    Invalid(String),
    Backend(String),
}

/// What an app provides to an MCP server.
///
/// Every method has a default so an app starts with the one thing it needs (`call_tool`) and adds
/// resources or prompts when it has them. The values are already shaped as MCP expects them
/// (`{"name": ., "description": ., "inputSchema": .}` for a tool), because only the app knows its
/// schema.
pub trait Handler {
    /// `serverInfo`.
    fn server_info(&self) -> ServerInfo;

    /// The short instructions `initialize` returns: what this app is, and how to drive it.
    fn instructions(&self) -> String {
        String::new()
    }

    /// A one-line description of where calls go ("headless", "remote 127.0.0.1:7979"), added to the
    /// instructions.
    fn describe(&self) -> String {
        String::new()
    }

    /// The tools `tools/list` returns.
    fn tools(&self) -> Vec<Value> {
        Vec::new()
    }

    /// Run one tool. A tool that fails returns [`ToolResult::error`]: the agent reads the reason,
    /// the protocol call itself succeeded.
    fn call_tool(&mut self, name: &str, arguments: &Value) -> ToolResult;

    /// The fixed resources `resources/list` returns.
    fn resources(&self) -> Vec<Value> {
        Vec::new()
    }

    /// The URI templates `resources/templates/list` returns.
    fn resource_templates(&self) -> Vec<Value> {
        Vec::new()
    }

    /// Read one resource URI (fixed or from a template).
    fn read_resource(&mut self, uri: &str) -> Result<ResourceContent, ReadError> {
        Err(ReadError::NotFound(uri.to_string()))
    }

    /// The prompts `prompts/list` returns.
    fn prompts(&self) -> Vec<Value> {
        Vec::new()
    }

    /// The `prompts/get` result for one prompt (`{"description": ., "messages": [.]}`).
    fn prompt(&self, name: &str, arguments: &Value) -> Result<Value, String> {
        let _ = arguments;
        Err(format!("no prompt `{name}`"))
    }

    /// The `completion/complete` result.
    fn complete(&mut self, params: &Value) -> Value {
        let _ = params;
        json!({"completion": {"values": [], "hasMore": false}})
    }
}

/// An MCP server bound to one handler.
pub struct Server<H> {
    handler: H,
    initialized: bool,
    /// Whether the client speaks the revision with cache hints.
    modern: bool,
    /// Whether this client asked for log records (`logging/setLevel`).
    logging: bool,
}

impl<H: Handler> Server<H> {
    pub fn new(handler: H) -> Self {
        Self { handler, initialized: false, modern: false, logging: false }
    }

    pub fn handler(&mut self) -> &mut H {
        &mut self.handler
    }

    /// Whether the client has sent `notifications/initialized`.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Whether the client asked for log records.
    pub fn wants_logs(&self) -> bool {
        self.logging
    }

    /// Serve newline-delimited JSON-RPC until `input` closes. stdout carries only protocol lines;
    /// an app that logs writes to stderr.
    pub fn serve<R: BufRead, W: Write>(&mut self, mut input: R, mut output: W) -> std::io::Result<()> {
        let mut buf = Vec::new();
        loop {
            if !read_line_capped(&mut input, &mut buf, MAX_LINE_BYTES)? {
                return Ok(());
            }
            if buf.len() > MAX_LINE_BYTES {
                // Overlong: answer, then drop the rest of the line so the next one starts clean.
                let reply = error(Value::Null, PARSE_ERROR, format!("request line longer than {MAX_LINE_BYTES} bytes"));
                write_line(&mut output, &reply.to_string())?;
                skip_to_newline(&mut input)?;
                continue;
            }
            let reply = match std::str::from_utf8(&buf) {
                Ok(line) => self.handle_line(line),
                Err(_) => Some(error(Value::Null, PARSE_ERROR, "parse error: invalid UTF-8").to_string()),
            };
            if let Some(reply) = reply {
                write_line(&mut output, &reply)?;
            }
        }
    }

    /// Handle one line; returns the reply line (None for notifications and blank lines).
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let reply = match serde_json::from_str::<Value>(line) {
            Ok(value) => self.handle(value),
            Err(e) => Some(error(Value::Null, PARSE_ERROR, format!("parse error: {e}"))),
        };
        reply.map(|r| r.to_string())
    }

    /// Handle one parsed message, or a batch of them.
    pub fn handle(&mut self, msg: Value) -> Option<Value> {
        match msg {
            Value::Array(batch) => {
                // Batches were removed in 2025-06-18; still answer older clients sensibly.
                if batch.is_empty() {
                    Some(error(Value::Null, INVALID_REQUEST, "empty batch"))
                } else if batch.len() > MAX_BATCH_MESSAGES {
                    Some(error(Value::Null, INVALID_REQUEST, format!("batch of {} messages exceeds {MAX_BATCH_MESSAGES}", batch.len())))
                } else {
                    let replies: Vec<Value> = batch.into_iter().filter_map(|m| self.handle(m)).collect();
                    (!replies.is_empty()).then_some(Value::Array(replies))
                }
            }
            msg => self.handle_one(msg),
        }
    }

    /// Handle one JSON-RPC message; `None` for notifications and responses.
    fn handle_one(&mut self, msg: Value) -> Option<Value> {
        let Value::Object(o) = &msg else { return Some(error(Value::Null, INVALID_REQUEST, "message must be an object")) };
        let id = o.get("id").cloned();
        let Some(method) = o.get("method").and_then(Value::as_str) else {
            // A response to a server->client request (we send none): ignore; anything else is invalid.
            if o.contains_key("result") || o.contains_key("error") {
                return None;
            }
            return Some(error(id.unwrap_or(Value::Null), INVALID_REQUEST, "missing `method`"));
        };
        // JSON-RPC 2.0 is the only envelope this server speaks.
        if o.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            // A notification has no id to address a reply to; ignoring it is the only answer.
            return id.map(|id| error(id, INVALID_REQUEST, "expected `\"jsonrpc\": \"2.0\"`"));
        };
        let params = o.get("params").cloned().unwrap_or(Value::Null);
        let Some(id) = id else {
            self.notification(method, &params);
            return None;
        };
        if !(id.is_string() || id.is_number()) {
            return Some(error(Value::Null, INVALID_REQUEST, "`id` must be a string or number"));
        }
        Some(match self.request(method, &params) {
            Ok(r) => response(id, complete_result(method, &params, self.modern, r)),
            Err((code, message)) => error(id, code, message),
        })
    }

    fn notification(&mut self, method: &str, _params: &Value) {
        match method {
            "notifications/initialized" => self.initialized = true,
            "notifications/cancelled" | "notifications/progress" | "notifications/roots/list_changed" => {}
            _ => {}
        }
    }

    fn request(&mut self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
        match method {
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": self.handler.tools()})),
            "resources/list" => Ok(json!({"resources": self.handler.resources()})),
            "resources/templates/list" => Ok(json!({"resourceTemplates": self.handler.resource_templates()})),
            "prompts/list" => Ok(json!({"prompts": self.handler.prompts()})),
            "logging/setLevel" => {
                self.logging = true;
                Ok(json!({}))
            }
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(crate::jsonrpc::PROTOCOL_VERSION);
                let version = negotiate(asked);
                self.modern = crate::jsonrpc::is_modern(version);
                let info = self.handler.server_info();
                let mut instructions = self.handler.instructions();
                let backend = self.handler.describe();
                if !backend.is_empty() {
                    if !instructions.is_empty() {
                        instructions.push(' ');
                    }
                    instructions.push_str(&format!("Backend: {backend}."));
                }
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": {
                        "tools": {},
                        "resources": {},
                        "prompts": {"listChanged": false},
                        "completions": {},
                        "logging": {},
                    },
                    "serverInfo": {"name": info.name, "title": info.title, "version": info.version},
                    "instructions": instructions,
                }))
            }
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).ok_or((INVALID_PARAMS, "missing tool `name`".to_string()))?;
                let args = params.get("arguments").cloned().unwrap_or(Value::Null);
                let handler = &mut self.handler;
                Ok(match guarded(|| handler.call_tool(name, &args)) {
                    Ok(r) => r.to_value(),
                    Err(why) => ToolResult::error(format!("internal error in `{name}`: {why} (please report this bug)")).to_value(),
                })
            }
            "resources/read" => {
                let uri = params.get("uri").and_then(Value::as_str).ok_or((INVALID_PARAMS, "missing `uri`".to_string()))?;
                let handler = &mut self.handler;
                let read =
                    guarded(|| handler.read_resource(uri)).map_err(|why| (INTERNAL_ERROR, format!("internal error in resources/read: {why}")))?;
                let content = read.map_err(|e| match e {
                    ReadError::NotFound(uri) => (RESOURCE_NOT_FOUND, format!("resource not found: {uri}")),
                    ReadError::Invalid(why) => (INVALID_PARAMS, why),
                    ReadError::Backend(why) => (INTERNAL_ERROR, why),
                })?;
                Ok(json!({"contents": [{"uri": uri, "mimeType": content.mime_type, "text": content.text}]}))
            }
            "prompts/get" => {
                let name = params.get("name").and_then(Value::as_str).ok_or((INVALID_PARAMS, "missing prompt `name`".to_string()))?;
                let args = params.get("arguments").cloned().unwrap_or(Value::Null);
                self.handler.prompt(name, &args).map_err(|e| (INVALID_PARAMS, e))
            }
            "completion/complete" => {
                let handler = &mut self.handler;
                Ok(guarded(|| handler.complete(params)).unwrap_or_else(|why| json!({"completion": {"values": [], "hasMore": false}, "error": why})))
            }
            other => Err((METHOD_NOT_FOUND, format!("method not found: {other}"))),
        }
    }
}

/// Run `f`, turning a panic into the reason (a bug fails this request, not the server).
fn guarded<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|p| {
        p.downcast_ref::<&str>().map(|s| (*s).to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "panicked".to_string())
    })
}

/// Drop bytes up to and including the next newline (the tail of an overlong line).
fn skip_to_newline(reader: &mut impl BufRead) -> std::io::Result<()> {
    loop {
        let chunk = reader.fill_buf()?;
        if chunk.is_empty() {
            return Ok(());
        }
        match chunk.iter().position(|&b| b == b'\n') {
            Some(i) => {
                reader.consume(i + 1);
                return Ok(());
            }
            None => {
                let len = chunk.len();
                reader.consume(len);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    struct Fake {
        calls: usize,
        panic_on: Option<&'static str>,
    }

    impl Handler for Fake {
        fn server_info(&self) -> ServerInfo {
            ServerInfo { name: "fake", title: "Fake", version: "0.0.0" }
        }

        fn instructions(&self) -> String {
            "A fake app for tests.".to_string()
        }

        fn describe(&self) -> String {
            "headless test".to_string()
        }

        fn tools(&self) -> Vec<Value> {
            vec![json!({"name": "echo", "description": "Echoes its argument", "inputSchema": {"type": "object"}})]
        }

        fn call_tool(&mut self, name: &str, arguments: &Value) -> ToolResult {
            self.calls += 1;
            if self.panic_on == Some(name) {
                panic!("the handler went up in smoke");
            }
            match name {
                "echo" => ToolResult::json(arguments),
                other => ToolResult::error(format!("unknown tool `{other}`")),
            }
        }

        fn resources(&self) -> Vec<Value> {
            vec![json!({"uri": "fake://document", "name": "document", "mimeType": "application/json"})]
        }

        fn read_resource(&mut self, uri: &str) -> Result<ResourceContent, ReadError> {
            match uri {
                "fake://document" => Ok(ResourceContent { mime_type: "application/json".into(), text: "{\"ok\":true}".into() }),
                other => Err(ReadError::NotFound(other.to_string())),
            }
        }

        fn prompts(&self) -> Vec<Value> {
            vec![json!({"name": "hello", "description": "Says hello"})]
        }

        fn prompt(&self, name: &str, _arguments: &Value) -> Result<Value, String> {
            match name {
                "hello" => Ok(json!({"description": "Says hello", "messages": [{"role": "user", "content": {"type": "text", "text": "hi"}}]})),
                other => Err(format!("no prompt `{other}`")),
            }
        }
    }

    fn server() -> Server<Fake> {
        Server::new(Fake { calls: 0, panic_on: None })
    }

    fn call(s: &mut Server<Fake>, id: u64, method: &str, params: Value) -> Value {
        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string();
        let reply = s.handle_line(&line).expect("a request gets a reply");
        serde_json::from_str(&reply).expect("the reply is JSON")
    }

    fn text_of(v: &Value) -> String {
        v["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string()
    }

    #[test]
    fn the_lifecycle_lists_tools_and_runs_one() {
        let mut s = server();
        let init = call(&mut s, 1, "initialize", json!({"protocolVersion": "2025-06-18"}));
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(init["result"]["serverInfo"]["name"], "fake");
        assert!(init["result"]["instructions"].as_str().unwrap_or_default().ends_with("Backend: headless test."));
        assert_eq!(s.handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#), None);
        assert!(s.is_initialized());

        let list = call(&mut s, 2, "tools/list", json!({}));
        assert_eq!(list["result"]["tools"][0]["name"], "echo");

        let ran = call(&mut s, 3, "tools/call", json!({"name": "echo", "arguments": {"x": 1}}));
        assert_eq!(ran["result"]["isError"], false);
        assert_eq!(text_of(&ran).replace([' ', '\n'], ""), "{\"x\":1}");
        assert_eq!(s.handler().calls, 1);

        let bad = call(&mut s, 4, "tools/call", json!({"name": "nope", "arguments": {}}));
        assert_eq!(bad["result"]["isError"], true);
    }

    #[test]
    fn resources_and_prompts_are_served_or_refused() {
        let mut s = server();
        let list = call(&mut s, 1, "resources/list", json!({}));
        assert_eq!(list["result"]["resources"][0]["uri"], "fake://document");
        let read = call(&mut s, 2, "resources/read", json!({"uri": "fake://document"}));
        assert_eq!(read["result"]["contents"][0]["mimeType"], "application/json");
        let missing = call(&mut s, 3, "resources/read", json!({"uri": "fake://nope"}));
        assert_eq!(missing["error"]["code"], RESOURCE_NOT_FOUND);

        let prompts = call(&mut s, 4, "prompts/list", json!({}));
        assert_eq!(prompts["result"]["prompts"][0]["name"], "hello");
        let got = call(&mut s, 5, "prompts/get", json!({"name": "hello"}));
        assert_eq!(got["result"]["description"], "Says hello");
        let e = call(&mut s, 6, "prompts/get", json!({"name": "nope"}));
        assert_eq!(e["error"]["code"], INVALID_PARAMS);
    }

    #[test]
    fn protocol_errors_are_json_rpc_shaped() {
        let mut s = server();
        let unknown = call(&mut s, 1, "nope/nothing", json!({}));
        assert_eq!(unknown["error"]["code"], METHOD_NOT_FOUND);
        let no_name = call(&mut s, 2, "tools/call", json!({}));
        assert_eq!(no_name["error"]["code"], INVALID_PARAMS);
        let parse = s.handle_line("not json").unwrap_or_default();
        assert!(parse.contains("\"code\":-32700") && parse.contains("parse error"), "{parse}");
        let envelope = s.handle_line(r#"{"id":3,"method":"ping"}"#);
        assert!(envelope.is_some_and(|l| l.contains("\"code\":-32600")), "no jsonrpc 2.0");
        assert_eq!(s.handle_line(""), None);
        assert_eq!(s.handle_line(r#"{"jsonrpc":"2.0","result":{}}"#), None, "a response is ignored");
    }

    #[test]
    fn a_panicking_handler_fails_the_request_not_the_server() {
        let mut s = Server::new(Fake { calls: 0, panic_on: Some("boom") });
        let r = call(&mut s, 1, "tools/call", json!({"name": "boom", "arguments": {}}));
        assert_eq!(r["result"]["isError"], true);
        assert!(text_of(&r).contains("internal error"), "{}", text_of(&r));
        // Still serving.
        let ok = call(&mut s, 2, "tools/call", json!({"name": "echo", "arguments": {"alive": true}}));
        assert_eq!(ok["result"]["isError"], false);
    }

    #[test]
    fn a_batch_is_answered_with_the_replies_of_its_requests() {
        let mut s = server();
        let line = json!([
            {"jsonrpc": "2.0", "id": 1, "method": "ping"},
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
        ])
        .to_string();
        let reply: Value = serde_json::from_str(&s.handle_line(&line).unwrap()).unwrap();
        let replies = reply.as_array().cloned().unwrap_or_default();
        assert_eq!(replies.len(), 2, "the notification gets no reply");
    }

    #[test]
    fn serving_reads_lines_and_writes_one_reply_each() {
        let mut s = server();
        let input = concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n",
            "\n",
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"echo\",\"arguments\":{\"y\":2}}}\n",
        );
        let mut output = Vec::new();
        s.serve(Cursor::new(input.as_bytes().to_vec()), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].contains("\"protocolVersion\""));
        assert!(lines[1].contains("\"isError\":false"));
    }

    #[test]
    fn an_overlong_line_is_refused_and_the_next_one_still_runs() {
        let mut s = server();
        let mut input = vec![b'x'; MAX_LINE_BYTES + 10];
        input.extend_from_slice(b"tail\n{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"ping\"}\n");
        let mut output = Vec::new();
        s.serve(Cursor::new(input), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        let mut lines = text.lines();
        assert!(lines.next().is_some_and(|l| l.contains("-32700")), "the overlong line is refused");
        assert!(lines.next().is_some_and(|l| l.contains("\"id\":9")), "the server kept going");
    }
}
