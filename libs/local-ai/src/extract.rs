//! The tolerant JSON extractor: what a model answers is not always the JSON it was asked for.
//!
//! A port of the original LocalMind Electron `extractJson`, shared by the suite's AI features:
//! reasoning models wrap the answer in `<think>`/`<thinking>` blocks, gateways add
//! `<|channel|>` control tokens, some models put the JSON inside a markdown fence, and many add a
//! sentence before or after. Trailing commas are dropped too - string-aware here, where the
//! original used a regular expression that could damage a comma inside a string.

use serde::de::DeserializeOwned;

/// The first JSON object or array in `text`, parsed into `T`.
pub fn extract_json<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    let mut body = strip_reasoning(text);
    if let Some(fenced) = json_fence(&body) {
        body = fenced;
    }
    let mut from = 0;
    while let Some(at) = body[from..].find(['{', '[']).map(|i| i + from) {
        if let Some(block) = balanced_slice(&body, at) {
            let cleaned = drop_trailing_commas(block);
            if let Ok(value) = serde_json::from_str(&cleaned) {
                return Ok(value);
            }
        }
        from = at + 1;
    }
    Err(format!("the model did not answer valid JSON. Answer: {}", clip(text)))
}

/// Removes the blocks that carry no answer: reasoning, control tokens, and the outer whitespace.
fn strip_reasoning(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        // `<think>...</think>` and `<thinking>...</thinking>`, case-insensitive.
        let lower = rest.to_ascii_lowercase();
        let open = lower.find("<think>").map(|at| (at, 7)).or_else(|| lower.find("<thinking>").map(|at| (at, 10)));
        match open {
            Some((at, open_len)) => {
                out.push_str(&rest[..at]);
                let after = &rest[at + open_len..];
                let after_lower = after.to_ascii_lowercase();
                let close_tag = if open_len == 7 { "</think>" } else { "</thinking>" };
                match after_lower.find(close_tag) {
                    Some(end) => rest = &after[end + close_tag.len()..],
                    None => {
                        // An unclosed block: nothing after it can be trusted as the answer.
                        rest = "";
                    }
                }
            }
            None => break,
        }
    }
    out.push_str(rest);
    // `<|channel|>`-style control tokens carry no content.
    let chars: Vec<char> = out.chars().collect();
    let mut cleaned = String::with_capacity(out.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '<' && chars.get(i + 1) == Some(&'|') {
            // Skip to `|>`; an unterminated token is kept as plain text.
            if let Some(end) = (i + 2..chars.len().saturating_sub(1)).find(|&j| chars[j] == '|' && chars[j + 1] == '>') {
                i = end + 2;
                continue;
            }
        }
        cleaned.push(chars[i]);
        i += 1;
    }
    cleaned.trim().to_string()
}

/// The inside of the first ``` fence, when there is one (a `json` tag is optional).
fn json_fence(body: &str) -> Option<String> {
    let start = body.find("```")?;
    let after = &body[start + 3..];
    let after = after.strip_prefix("json").or_else(|| after.strip_prefix("JSON")).unwrap_or(after);
    let end = after.find("```")?;
    Some(after[..end].trim().to_string())
}

/// The balanced `{...}` or `[...]` starting at `from`, skipping the brackets inside strings.
fn balanced_slice(s: &str, from: usize) -> Option<&str> {
    let bytes = s.as_bytes();
    let open = *bytes.get(from)?;
    let close = if open == b'{' { b'}' } else { b']' };
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in bytes.iter().enumerate().skip(from) {
        let byte = *byte;
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        if byte == b'"' {
            in_string = true;
        } else if byte == open {
            depth += 1;
        } else if byte == close {
            depth -= 1;
            if depth == 0 {
                return s.get(from..=offset);
            }
        }
    }
    None
}

/// Drops `,` right before a closing bracket, leaving commas inside strings alone.
fn drop_trailing_commas(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut escaped = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            out.push(ch);
            continue;
        }
        if ch == ',' {
            let mut look = chars.clone();
            let next = look.find(|c: &char| !c.is_whitespace());
            if matches!(next, Some('}') | Some(']')) {
                continue;
            }
        }
        out.push(ch);
    }
    out
}

/// A short, single-line preview of the answer, for the error message.
fn clip(text: &str) -> String {
    let flat: String = text.chars().map(|c| if c.is_whitespace() { ' ' } else { c }).collect();
    let clipped: String = flat.trim().chars().take(160).collect();
    if flat.trim().chars().count() > 160 { format!("{clipped}…") } else { clipped }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, serde::Deserialize)]
    struct Items {
        items: Vec<String>,
    }

    #[derive(Debug, PartialEq, serde::Deserialize)]
    struct Node {
        text: String,
        #[serde(default)]
        children: Vec<Node>,
    }

    #[test]
    fn plain_json_is_read() {
        let got: Items = extract_json(r#"{"items":["um","dois"]}"#).expect("plain");
        assert_eq!(got.items, ["um", "dois"]);
    }

    /// A reasoning model wraps the answer in a thinking block: the block is not the answer.
    #[test]
    fn reasoning_blocks_are_stripped() {
        let raw = "<think>\nI should answer with JSON. Let me think about {braces}.</think>\n{\"items\":[\"a\"]}";
        let got: Items = extract_json(raw).expect("after think");
        assert_eq!(got.items, ["a"]);
        let raw = "<THINKING>json</THINKING>{\"items\":[\"b\"]}";
        assert_eq!(extract_json::<Items>(raw).expect("case-insensitive").items, ["b"]);
    }

    /// Gateways add harmony/channel control tokens around the answer.
    #[test]
    fn control_tokens_are_dropped() {
        let raw = "<|channel|>final<|message|>{\"items\":[\"c\"]}";
        assert_eq!(extract_json::<Items>(raw).expect("tokens").items, ["c"]);
    }

    #[test]
    fn a_markdown_fence_is_unwrapped() {
        let raw = "Here is the map:\n```json\n{\"items\":[\"d\"]}\n```\nAnything else?";
        assert_eq!(extract_json::<Items>(raw).expect("fence").items, ["d"]);
        let raw = "```\n{\"items\":[\"e\"]}\n```";
        assert_eq!(extract_json::<Items>(raw).expect("untagged fence").items, ["e"]);
    }

    /// Prose before the JSON, and a second object after it: the first complete block wins.
    #[test]
    fn the_first_balanced_block_wins() {
        let raw = "Sure! Here you go: {\"items\":[\"f\"]} and also {\"items\":[\"g\"]}";
        assert_eq!(extract_json::<Items>(raw).expect("first").items, ["f"]);
    }

    /// A brace inside a string does not close the object early.
    #[test]
    fn braces_inside_strings_do_not_count() {
        let raw = r#"{"items":["a } b { c"]}"#;
        assert_eq!(extract_json::<Items>(raw).expect("braces").items, ["a } b { c"]);
    }

    #[test]
    fn trailing_commas_are_dropped() {
        let raw = "{\"items\":[\"h\",\"i\",],}";
        assert_eq!(extract_json::<Items>(raw).expect("trailing").items, ["h", "i"]);
    }

    /// The comma-dropper must not touch a comma inside a string that is followed by a bracket.
    #[test]
    fn a_comma_inside_a_string_survives() {
        let raw = r#"{"items":["a,}"]}"#;
        assert_eq!(extract_json::<Items>(raw).expect("string comma").items, ["a,}"]);
    }

    #[test]
    fn a_tree_nests() {
        let raw = r#"{"text":"raiz","children":[{"text":"a","children":[{"text":"a1"}]}]}"#;
        let node: Node = extract_json(raw).expect("tree");
        assert_eq!(node.text, "raiz");
        assert_eq!(node.children[0].children[0].text, "a1");
    }

    /// Something that is not JSON at all is an error that shows what arrived.
    #[test]
    fn no_json_is_an_error_with_the_answer_in_it() {
        let error = extract_json::<Items>("desculpe, não posso ajudar com isso").expect_err("must fail");
        assert!(error.contains("desculpe"), "{error}");
    }

    #[test]
    fn an_unclosed_block_is_reported_not_guessed() {
        assert!(extract_json::<Items>("{\"items\":[\"a\"").is_err());
    }
}
