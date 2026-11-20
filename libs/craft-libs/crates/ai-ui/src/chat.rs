//! The chat beside the document: the roll, the composer, and what actually goes to the model.
//!
//! The panel owns the conversation and the composer; it does not own the network. When the user
//! sends, [`ChatPanel::submit`] hands the app the exact lines to send (the app's system prompt, the
//! document cut to the budget, the last turns, the question), the app starts its own streaming
//! client, and feeds the answer back with [`ChatPanel::push`] / [`ChatPanel::finish`]. That keeps
//! domain (prompts, context, apply) and network in the app, and the experience - identical in every
//! app of the suite - here.
//!
//! The roll sticks to the bottom only while the reader is at the bottom: scrolling up during a
//! stream never pulls the view back, and a button offers the way down. `Enter` sends,
//! `Shift+Enter` breaks the line, and an `Enter` that belongs to an IME composition sends nothing.

use crate::policy::{ContextInfo, ContextSource, WindowPolicy, window};

pub use crate::policy::{Line, Role};

/// What the panel asks the app to do after a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatAction {
    None,
    /// A request is ready: the app must call [`ChatPanel::take_request`] and start the stream.
    Send,
    /// The user pressed Stop: cancel the request and call [`ChatPanel::stopped`].
    Stop,
    /// The failed request goes again, unchanged.
    Retry,
    Clear,
    /// Write the last answer into the document.
    Insert,
    /// Open the connections form.
    Settings,
    Close,
}

/// The words and callbacks the panel needs, in the app's language.
pub struct ChatLabels<'a> {
    pub title: &'a str,
    pub not_configured: &'a str,
    pub configure: &'a str,
    pub settings: &'a str,
    pub clear: &'a str,
    pub close: &'a str,
    pub send: &'a str,
    pub stop: &'a str,
    pub insert: &'a str,
    pub retry: &'a str,
    pub dismiss: &'a str,
    pub empty: &'a str,
    pub input_hint: &'a str,
    /// The chip above the composer, built from what will actually be sent.
    pub chip: &'a dyn Fn(&ContextInfo) -> String,
    /// `model` is the app's line about the connection ("llama3.2 · localhost").
    pub model: &'a str,
    pub suggestions: &'a [String],
    pub to_end: &'a str,
}

/// The conversation beside the document.
pub struct ChatPanel {
    pub open: bool,
    lines: Vec<Line>,
    input: String,
    /// The answer being streamed.
    answer: String,
    busy: bool,
    /// An IME is composing: the next Enter belongs to it, not to the panel.
    composing: bool,
    /// The failed request, kept so "retry" resends the same thing.
    error: Option<(String, Vec<Line>)>,
    /// What the app must send, once the frame that produced it ends.
    request: Option<Vec<Line>>,
    /// The last request sent, kept so a failure can send the same thing again.
    last_request: Option<Vec<Line>>,
    stick: bool,
    /// Set to scroll the roll to the end on the next frame (the "down" button, a send, a retry).
    scroll_end: bool,
    /// Not at the bottom: the "down" button is offered.
    away: bool,
    system: String,
    context: Option<ContextSource>,
    policy: WindowPolicy,
    last_context: Option<ContextInfo>,
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self {
            open: false,
            lines: Vec::new(),
            input: String::new(),
            answer: String::new(),
            busy: false,
            composing: false,
            error: None,
            request: None,
            last_request: None,
            stick: true,
            scroll_end: false,
            away: false,
            system: String::new(),
            context: None,
            policy: WindowPolicy::default(),
            last_context: None,
        }
    }
}

impl ChatPanel {
    /// The app's system prompt (its vocabulary, its language).
    pub fn set_system(&mut self, system: impl Into<String>) {
        self.system = system.into();
    }

    /// The document side of the next request.
    pub fn set_context(&mut self, context: Option<ContextSource>) {
        self.context = context;
    }

    pub fn set_policy(&mut self, policy: WindowPolicy) {
        self.policy = policy;
    }

    pub fn policy(&self) -> &WindowPolicy {
        &self.policy
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// What the last request will send, for the chip.
    pub fn context_info(&self) -> Option<&ContextInfo> {
        self.last_context.as_ref()
    }

    /// The last answer, for "insert into the note".
    pub fn last_answer(&self) -> Option<&str> {
        self.lines.iter().rev().find(|line| line.role == Role::Assistant).map(|line| line.text.as_str())
    }

    /// The request built by the last send/retry; the app takes it and starts the stream.
    pub fn take_request(&mut self) -> Option<Vec<Line>> {
        self.request.take()
    }

    /// Sends what is in the composer. Returns whether a request is waiting for the app.
    pub fn submit(&mut self) -> bool {
        if self.busy {
            return false;
        }
        let question = self.input.trim().to_string();
        if question.is_empty() {
            return false;
        }
        let request = self.build(&question);
        self.last_request = Some(request.clone());
        self.lines.push(Line { role: Role::User, text: question });
        self.input.clear();
        self.answer.clear();
        self.error = None;
        self.busy = true;
        self.stick = true;
        self.scroll_end = true;
        self.request = Some(request);
        true
    }

    /// Asks a question the panel did not type: a tool entry's action ("Summarize the document"),
    /// sent through the same path as the composer. Returns whether a request is waiting.
    pub fn ask(&mut self, question: impl Into<String>) -> bool {
        if self.busy {
            return false;
        }
        self.input = question.into();
        self.submit()
    }

    /// Builds the request for `question`, applying the window policy.
    fn build(&mut self, question: &str) -> Vec<Line> {
        let history: Vec<(Role, String)> = self.lines.iter().map(|line| (line.role, line.text.clone())).collect();
        let (lines, info) = window(&self.system, self.context.as_ref(), &history, question, &self.policy);
        self.last_context = info;
        lines
    }
    /// Starts the failed request again (the same lines, not a re-interpretation).
    pub fn retry(&mut self) -> bool {
        let Some((_, request)) = self.error.take() else { return false };
        self.last_request = Some(request.clone());
        self.busy = true;
        self.stick = true;
        self.scroll_end = true;
        self.request = Some(request);
        true
    }

    /// A piece of the answer arrived.
    pub fn push(&mut self, delta: &str) {
        self.answer.push_str(delta);
        if self.stick {
            self.scroll_end = true;
        }
    }

    /// The answer ended. A provider may send nothing while streaming and the whole text at the end.
    pub fn finish(&mut self, result: Result<String, String>) {
        self.busy = false;
        match result {
            Ok(whole) => {
                let text = if self.answer.is_empty() { whole } else { std::mem::take(&mut self.answer) };
                if !text.trim().is_empty() {
                    self.lines.push(Line { role: Role::Assistant, text });
                }
            }
            Err(error) => {
                if !self.answer.is_empty() {
                    let partial = std::mem::take(&mut self.answer);
                    self.lines.push(Line { role: Role::Assistant, text: partial });
                }
                // The request that failed is the one to resend, byte for byte.
                let request = self.last_request.clone().unwrap_or_default();
                self.error = Some((error, request));
            }
        }
        if self.stick {
            self.scroll_end = true;
        }
    }

    /// The user stopped: what arrived stays as a normal turn and nothing is retried.
    pub fn stopped(&mut self) {
        self.busy = false;
        self.error = None;
        if !self.answer.is_empty() {
            let partial = std::mem::take(&mut self.answer);
            self.lines.push(Line { role: Role::Assistant, text: partial });
        }
        if self.stick {
            self.scroll_end = true;
        }
    }

    /// Forgets the conversation (the app keeps whatever it wants of it).
    pub fn clear(&mut self) {
        self.lines.clear();
        self.answer.clear();
        self.error = None;
        self.last_context = None;
    }

    /// Adds an assistant turn without a request (a replayed transcript, and the app's tests).
    pub fn push_answer(&mut self, text: impl Into<String>) {
        let text = text.into();
        if !text.trim().is_empty() {
            self.lines.push(Line { role: Role::Assistant, text });
            self.stick = true;
            self.scroll_end = true;
        }
    }

    /// Draws the panel. The app owns the side panel; this is its content.
    pub fn show(&mut self, ui: &mut egui::Ui, labels: &ChatLabels<'_>) -> ChatAction {
        let mut action = ChatAction::None;

        // Header: what is answering, and the three things one can do about it.
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(labels.title).strong());
            ui.label(egui::RichText::new(labels.model).small().weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::close_button(ui, labels.close).clicked() {
                    action = ChatAction::Close;
                }
                if ui.small_button(labels.settings).clicked() {
                    action = ChatAction::Settings;
                }
                if ui.add_enabled(!self.lines.is_empty(), egui::Button::new(egui::RichText::new(labels.clear).small())).clicked() {
                    action = ChatAction::Clear;
                }
            });
        });
        ui.separator();

        // The roll. `stick_to_bottom` follows the stream only while the reader is at the end.
        let height = (ui.available_height() - 96.0).max(80.0);
        let mut area = egui::ScrollArea::vertical().id_salt("ai-chat-roll").max_height(height).auto_shrink([false, false]);
        area = area.stick_to_bottom(true);
        if std::mem::take(&mut self.scroll_end) {
            area = area.vertical_scroll_offset(f32::INFINITY);
        }
        let output = area.show(ui, |ui| {
            if self.lines.is_empty() && self.answer.is_empty() && self.error.is_none() {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(labels.empty).weak());
                for suggestion in labels.suggestions {
                    if ui.button(suggestion).clicked() {
                        self.input = suggestion.clone();
                    }
                }
            }
            for line in &self.lines {
                turn(ui, line.role, &line.text);
            }
            if !self.answer.is_empty() {
                turn(ui, Role::Assistant, &self.answer);
            }
            if let Some((error, _)) = &self.error {
                ui.add_space(4.0);
                egui::Frame::new().fill(ui.visuals().warn_fg_color.gamma_multiply(0.12)).inner_margin(6.0).corner_radius(4.0).show(ui, |ui| {
                    ui.label(egui::RichText::new(error).small().color(ui.visuals().warn_fg_color));
                    if ui.small_button(labels.retry).clicked() {
                        action = ChatAction::Retry;
                    }
                });
            }
        });
        let at_bottom = output.state.clamped_offset().y >= (output.content_size.y - output.inner_rect.height() - 2.0).max(0.0);
        self.stick = at_bottom;
        self.away = !at_bottom;

        if self.away {
            // The way down: the reader scrolled up and stays there until they say otherwise.
            ui.horizontal(|ui| {
                if ui.button(egui::RichText::new(format!("↓ {}", labels.to_end))).clicked() {
                    self.stick = true;
                    self.scroll_end = true;
                }
            });
        }

        // The chip: what the last request really sent.
        if let Some(info) = &self.last_context {
            ui.label(egui::RichText::new((labels.chip)(info)).small().weak());
        }

        // The composer: one line that grows to four, Enter sends, Shift+Enter breaks.
        let rows = self.input.lines().count().clamp(1, 4);
        let response = ui.add(
            egui::TextEdit::multiline(&mut self.input)
                .id_salt("ai-chat-input")
                .hint_text(labels.input_hint)
                .desired_rows(rows)
                .desired_width(f32::INFINITY),
        );
        // An Enter that belongs to an IME composition is the IME's, not ours.
        for event in ui.input(|input| input.events.clone()) {
            if let egui::Event::Ime(ime) = event {
                match ime {
                    egui::ImeEvent::Preedit { text, .. } => self.composing = !text.is_empty(),
                    egui::ImeEvent::Commit(_) => self.composing = false,
                    _ => {}
                }
            }
        }
        let pressed_enter = response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        let shift = ui.input(|input| input.modifiers.shift);
        if pressed_enter && !shift && !self.composing && !self.busy && self.submit() {
            action = ChatAction::Send;
        }

        ui.horizontal(|ui| {
            let can_send = !self.input.trim().is_empty() && !self.busy;
            if ui.add_enabled(can_send, egui::Button::new(labels.send)).clicked() && self.submit() {
                action = ChatAction::Send;
            }
            if self.busy && ui.button(labels.stop).clicked() {
                action = ChatAction::Stop;
            }
            if !self.busy && ui.add_enabled(self.last_answer().is_some(), egui::Button::new(labels.insert)).clicked() {
                action = ChatAction::Insert;
            }
        });

        action
    }
}

/// One turn of the roll: who said it, and the text in the colour of that role.
fn turn(ui: &mut egui::Ui, role: Role, text: &str) {
    ui.add_space(4.0);
    match role {
        Role::User => {
            ui.label(egui::RichText::new(format!("› {text}")).strong());
        }
        Role::Assistant => {
            ui.label(egui::RichText::new(text).size(14.0));
        }
        Role::System => {
            ui.label(egui::RichText::new(text).small().weak());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{ContextKind, Side, Turns};

    fn panel() -> ChatPanel {
        let mut panel = ChatPanel::default();
        panel.set_system("sistema");
        panel.set_policy(WindowPolicy { turns: Turns::LastN(2), total_budget_chars: 200 });
        panel
    }

    #[test]
    fn a_sent_question_becomes_a_turn_and_a_request() {
        let mut panel = panel();
        panel.input = "  o que é sepse?  ".into();
        assert!(panel.submit());
        let request = panel.take_request().expect("a request");
        assert_eq!(request.last().map(|line| line.text.as_str()), Some("o que é sepse?"));
        assert_eq!(panel.lines().len(), 1);
        assert!(panel.input.is_empty());
        assert!(panel.busy());
    }

    #[test]
    fn the_windows_policy_cuts_the_document_before_the_turns() {
        let mut panel = panel();
        let source = ContextSource::new(ContextKind::Note, "a nota", "x".repeat(500), 1_000, Side::Tail);
        panel.set_context(Some(source));
        panel.input = "pergunta".into();
        assert!(panel.submit());
        let info = panel.context_info().expect("the chip's numbers");
        assert!(info.truncated());
        assert!(info.sent <= 200);
    }

    #[test]
    fn a_failure_keeps_the_partial_answer_and_the_request_to_retry() {
        let mut panel = panel();
        panel.input = "pergunta".into();
        assert!(panel.submit());
        let _ = panel.take_request();
        panel.push("meia resposta");
        panel.finish(Err("sem rede".into()));
        assert_eq!(panel.last_answer(), Some("meia resposta"), "what arrived stays");
        assert!(!panel.busy());
        assert!(panel.retry(), "the failed request is still there");
        assert!(panel.busy());
        assert!(panel.take_request().is_some());
    }

    #[test]
    fn stopping_keeps_the_partial_answer_and_offers_nothing_to_retry() {
        let mut panel = panel();
        panel.input = "p".into();
        assert!(panel.submit());
        let _ = panel.take_request();
        panel.push("até aqui");
        panel.stopped();
        assert_eq!(panel.last_answer(), Some("até aqui"));
        assert!(!panel.retry());
    }

    #[test]
    fn an_empty_or_busy_composer_sends_nothing() {
        let mut panel = panel();
        assert!(!panel.submit(), "nothing typed");
        panel.input = "p".into();
        assert!(panel.submit());
        panel.input = "outra".into();
        assert!(!panel.submit(), "one request at a time");
    }

    #[test]
    fn an_asked_question_takes_the_composer_and_the_same_path() {
        let mut panel = panel();
        assert!(panel.ask("Resuma o documento"), "an entry's action sends");
        assert!(panel.busy());
        assert!(panel.input.is_empty(), "the question left the composer");
        let request = panel.take_request().expect("the request");
        assert!(request.iter().any(|line| line.role == Role::User && line.text == "Resuma o documento"));
        assert!(!panel.ask("outra"), "one request at a time");
        assert!(panel.lines().iter().any(|line| line.role == Role::User && line.text == "Resuma o documento"));
    }
}
