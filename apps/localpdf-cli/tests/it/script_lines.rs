//! `run --script` narrows nothing: it reads a JSON array or one `{"tool", "args"}` object per line
//! (JSON Lines), the way the rest of the suite takes its job files.

use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_localpdf-cli");

fn script(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("localpdf-script-lines-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn a_script_may_be_one_step_per_line() {
    let path = script(
        "job.jsonl",
        "{\"tool\":\"doc_create\",\"args\":{\"from\":\"blank\",\"pages\":1,\"width\":100,\"height\":100}}\n\n{\"tool\":\"doc_inspect\",\"args\":{}}\n",
    );
    let out = Command::new(BIN).args(["run", "--script"]).arg(&path).arg("--report").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is one JSON document");
    assert_eq!(report["ok"], true);
    let steps = report["steps"].as_array().cloned().unwrap_or_default();
    assert_eq!(steps.len(), 2, "{report}");
    assert_eq!(steps[0]["tool"], "doc_create");
    assert_eq!(steps[1]["tool"], "doc_inspect");
}

#[test]
fn a_broken_line_names_its_step_and_line() {
    let path = script("broken.jsonl", "{\"tool\":\"doc_inspect\",\"args\":{}}\n{oops}\n");
    let out = Command::new(BIN).args(["run", "--script"]).arg(&path).output().unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("step 2 (line 2)"), "{stderr}");
    assert!(out.stdout.is_empty(), "nothing runs after a malformed step is refused");
}
