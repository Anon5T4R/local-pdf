//! `localpdf-cli mcp --connect <control file>`: the live session forwards MCP tool calls to a
//! running app's control channel (loopback, token-authenticated). A fake control server stands in
//! for the app, so the bridge itself is what is under test.

#![cfg(feature = "mcp")]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_localpdf-cli");

/// A 1x1 PNG, as the control channel answers `ui.screenshot` with.
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("localpdf-live-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The fake app: accepts one connection, answers `auth`, `ui.state`, `ui.commands` and
/// `ui.screenshot`, and records every method it received.
fn fake_app() -> (u16, Arc<Mutex<Vec<String>>>, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let methods = Arc::clone(&seen);
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut writer = stream;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap() == 0 {
                break;
            }
            let request: Value = serde_json::from_str(line.trim()).unwrap();
            let method = request["method"].as_str().unwrap_or_default().to_string();
            methods.lock().unwrap().push(method.clone());
            let result = match method.as_str() {
                "auth" => json!({}),
                "ui.state" => json!({ "documents": [] }),
                "ui.commands" => json!([]),
                "ui.screenshot" => json!({ "png_base64": PNG, "width": 1, "height": 1 }),
                _ => json!({}),
            };
            writeln!(writer, "{}", json!({"jsonrpc": "2.0", "id": request["id"], "result": result})).unwrap();
        }
    });
    (port, seen, handle)
}

fn control_file(dir: &Path, port: u16) -> PathBuf {
    let path = dir.join("control.json");
    std::fs::write(&path, json!({"port": port, "token": "test-token", "pid": 0}).to_string()).unwrap();
    path
}

fn session(file: &Path, requests: &[Value]) -> (Vec<Value>, String) {
    let mut child =
        Command::new(BIN).args(["mcp", "--connect"]).arg(file).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut input = String::new();
    for r in requests {
        input.push_str(&format!("{r}\n"));
    }
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let replies = String::from_utf8(out.stdout).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    (replies, String::from_utf8(out.stderr).unwrap())
}

#[test]
fn the_live_session_forwards_to_the_running_app() {
    let (port, seen, handle) = fake_app();
    let dir = temp_dir("forwards");
    let file = control_file(&dir, port);
    let (replies, stderr) = session(
        &file,
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "ui_state", "arguments": {}}}),
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "render_preview", "arguments": {}}}),
            json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "command_run", "arguments": {"command": "x", "params": {"a": 1}}}}),
        ],
    );
    handle.join().unwrap();
    assert!(stderr.contains("live app through"), "{stderr}");
    let by_id = |id: i64| replies.iter().find(|r| r["id"] == id).cloned().unwrap_or(Value::Null);

    let init = by_id(1);
    assert_eq!(init["result"]["serverInfo"]["name"], "localpdf-live");
    assert!(init["result"]["instructions"].as_str().unwrap_or_default().contains("ui_state"));

    let tools = by_id(2)["result"]["tools"].as_array().cloned().unwrap_or_default();
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"ui_state") && names.contains(&"render_preview") && names.contains(&"command_run"), "{names:?}");

    let state = by_id(3);
    let text = state["result"]["content"][0]["text"].as_str().unwrap_or_default();
    assert!(text.contains("\"documents\""), "{text}");

    let image = by_id(4);
    assert_eq!(image["result"]["content"][0]["type"], "image");
    assert_eq!(image["result"]["content"][0]["mimeType"], "image/png");
    assert_eq!(image["result"]["content"][0]["data"], PNG);

    let refused = by_id(5);
    assert_eq!(refused["result"]["isError"], true, "{refused}");
    assert!(refused["result"]["content"][0]["text"].as_str().unwrap_or_default().contains("parameters are not forwarded"));

    let methods = seen.lock().unwrap().clone();
    assert_eq!(methods.first().map(String::as_str), Some("auth"), "{methods:?}");
    assert!(methods.contains(&"ui.state".to_string()) && methods.contains(&"ui.screenshot".to_string()), "{methods:?}");
    assert!(!methods.contains(&"ui.command".to_string()), "a command with params is refused before it runs");
}

#[test]
fn connect_needs_the_control_file_not_a_bare_port() {
    let out = Command::new(BIN).args(["mcp", "--connect", "7979"]).stdin(Stdio::null()).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("7979"), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn connect_refuses_the_headless_confinement_flags() {
    let out = Command::new(BIN).args(["mcp", "--connect", "some.json", "--automation-read-root", "."]).stdin(Stdio::null()).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("enforces its own automation roots"));
}
