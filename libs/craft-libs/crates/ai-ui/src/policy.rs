//! What the panel shows versus what the model receives.
//!
//! Every app used to send everything it had: the whole document as context, every turn of the chat,
//! every time. Cost grows with the conversation and the user has no idea what leaves the machine.
//! [`WindowPolicy`] answers both: it decides what goes out, in a fixed priority - the turn being
//! sent is never cut, the last turns go whole, and the document is the first thing to shrink (it is
//! the part that changes least). [`ContextInfo`] is what the chip above the composer reports.

/// How much of the conversation goes to the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turns {
    /// The most recent whole turns; older ones stay on screen and are not sent.
    LastN(usize),
    /// Everything the panel shows.
    All,
}

/// Which end of the document survives when the budget is short.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The beginning: what a document says first.
    Head,
    /// The end: where the reader usually is.
    Tail,
}

/// What the context text *is*, for the chip's wording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextKind {
    Document,
    Selection,
    Note,
    Custom,
}

/// The document side of the request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSource {
    pub kind: ContextKind,
    /// How the app names it in the chip ("the note", "the map", ...).
    pub label: String,
    pub text: String,
    /// The most characters this source may take.
    pub budget_chars: usize,
    pub side: Side,
}

impl ContextSource {
    pub fn new(kind: ContextKind, label: impl Into<String>, text: impl Into<String>, budget_chars: usize, side: Side) -> Self {
        Self { kind, label: label.into(), text: text.into(), budget_chars, side }
    }
}

/// The budget for one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowPolicy {
    pub turns: Turns,
    /// One budget for everything that goes out (system prompt excepted).
    pub total_budget_chars: usize,
}

impl Default for WindowPolicy {
    fn default() -> Self {
        Self { turns: Turns::LastN(8), total_budget_chars: 24_000 }
    }
}

/// What the chip above the composer says about the last request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextInfo {
    pub label: String,
    /// Characters of the document that actually went.
    pub sent: usize,
    /// Characters the document had.
    pub total: usize,
    /// Turns of the conversation that went.
    pub turns: usize,
    /// Turns that stayed on screen.
    pub kept: usize,
}

impl ContextInfo {
    /// Whether the document was cut to fit.
    pub fn truncated(&self) -> bool {
        self.sent < self.total
    }
}

/// One message of the request, in the crate's own vocabulary (the app maps it to its client).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub role: Role,
    pub text: String,
}

/// Who said it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

/// The request the model receives: system prompt, the document (budgeted) and the last turns.
///
/// Priority when the budget is short: the question being sent, then the most recent turns, then the
/// document - which shrinks and says so in the [`ContextInfo`].
pub fn window(
    system: &str,
    context: Option<&ContextSource>,
    history: &[(Role, String)],
    question: &str,
    policy: &WindowPolicy,
) -> (Vec<Line>, Option<ContextInfo>) {
    let mut out = vec![Line { role: Role::System, text: system.to_string() }];
    let mut info = context.map(|source| ContextInfo { label: source.label.clone(), sent: 0, total: source.text.chars().count(), turns: 0, kept: 0 });

    // The turns, newest first while they fit; a turn is never cut in half.
    let keep = match policy.turns {
        Turns::All => history.len(),
        Turns::LastN(n) => n.min(history.len()),
    };
    let recent = &history[history.len() - keep..];
    let mut spent = question.chars().count();
    let mut chosen: Vec<(Role, String)> = Vec::new();
    for (role, text) in recent.iter().rev() {
        let cost = text.chars().count();
        if spent + cost > policy.total_budget_chars {
            break;
        }
        spent += cost;
        chosen.push((*role, text.clone()));
    }
    chosen.reverse();
    if let Some(info) = info.as_mut() {
        info.turns = chosen.len();
        info.kept = history.len();
    }
    for (role, text) in chosen {
        out.push(Line { role, text });
    }

    // The document last: it gets what is left, on the side the app asked for.
    if let (Some(source), Some(info)) = (context, info.as_mut()) {
        let room = policy.total_budget_chars.saturating_sub(spent).min(source.budget_chars);
        let text = fit(&source.text, room, source.side);
        info.sent = text.chars().count();
        out.push(Line { role: Role::System, text: format!("{}\n\n{text}", source.label) });
    }
    out.push(Line { role: Role::User, text: question.to_string() });
    (out, info)
}

/// `text` cut to `room` characters, from the end asked for.
fn fit(text: &str, room: usize, side: Side) -> String {
    let total = text.chars().count();
    if total <= room {
        return text.to_string();
    }
    match side {
        Side::Head => text.chars().take(room).collect(),
        Side::Tail => text.chars().skip(total - room).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(budget: usize, turns: usize) -> WindowPolicy {
        WindowPolicy { turns: Turns::LastN(turns), total_budget_chars: budget }
    }

    fn history(count: usize) -> Vec<(Role, String)> {
        (0..count).map(|index| (if index % 2 == 0 { Role::User } else { Role::Assistant }, format!("turno {index}"))).collect()
    }

    #[test]
    fn the_question_and_the_system_prompt_always_go() {
        let (lines, _) = window("sistema", None, &[], "pergunta", &policy(10, 8));
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], Line { role: Role::System, text: "sistema".into() });
        assert_eq!(lines[1], Line { role: Role::User, text: "pergunta".into() });
    }

    #[test]
    fn only_the_last_turns_go_and_the_rest_stay_on_screen() {
        let history = history(20);
        let (lines, info) = window("s", None, &history, "q", &policy(10_000, 3));
        // system + 3 turns + question
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[1].text, "turno 17");
        assert!(info.is_none(), "no document, no chip numbers");
    }

    #[test]
    fn the_document_shrinks_before_the_turns_do() {
        let document = "x".repeat(500);
        let source = ContextSource::new(ContextKind::Note, "a nota", document, 100_000, Side::Tail);
        let (lines, info) = window("s", Some(&source), &history(2), "q", &policy(200, 8));
        // system + 2 turns + document + question
        assert_eq!(lines.len(), 5);
        let info = info.expect("the chip's numbers");
        assert!(info.truncated(), "{info:?}");
        assert!(info.sent < 200, "{info:?}");
        assert_eq!(info.turns, 2);
    }

    #[test]
    fn the_tail_keeps_the_end_and_the_head_keeps_the_start() {
        assert_eq!(fit("abcdefgh", 3, Side::Head), "abc");
        assert_eq!(fit("abcdefgh", 3, Side::Tail), "fgh");
        assert_eq!(fit("abc", 10, Side::Tail), "abc");
    }
}
