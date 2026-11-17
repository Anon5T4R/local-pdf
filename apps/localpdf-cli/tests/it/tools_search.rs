//! `tools --search` narrows the automation catalogue by name, title or description, like the
//! suite's `commands --search`.

use std::process::Command;

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_localpdf-cli");

fn tools(args: &[&str]) -> Vec<Value> {
    let out = Command::new(BIN).arg("tools").args(args).output().unwrap();
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn search_keeps_only_matching_tools() {
    let all = tools(&[]);
    let found = tools(&["--search", "text"]);
    assert!(!found.is_empty() && found.len() < all.len(), "{} of {}", found.len(), all.len());
    // Every kept tool is one of the catalogue's (the title, which the search also matches, is not
    // printed by `tools`).
    for t in &found {
        assert!(all.iter().any(|a| a["name"] == t["name"]), "{} is not a catalogued tool", t["name"]);
    }
}

#[test]
fn a_word_no_tool_mentions_returns_an_empty_list() {
    let found = tools(&["--search", "zzzznothingzzz"]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn search_needs_a_value() {
    let out = Command::new(BIN).args(["tools", "--search"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--search"));
}
