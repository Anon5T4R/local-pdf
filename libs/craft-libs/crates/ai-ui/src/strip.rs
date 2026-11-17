//! The strip above the composer while the model is answering: spinner, seconds, the tail of the
//! stream, and Stop.
//!
//! One line, always: the tail is cut to a fixed width so a burst of tokens never changes the panel's
//! layout. An error is the same strip with the reason and a dismiss, so the panel never grows a
//! second place for bad news.

use std::time::Instant;

/// How many characters of the stream the strip shows.
const TAIL_CHARS: usize = 120;

/// What the strip asks the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StripAction {
    None,
    /// The user pressed Stop: the app cancels the request and keeps what arrived.
    Stop,
    /// The error was dismissed.
    Dismiss,
}

/// The strip's state and drawing.
#[derive(Debug, Default)]
pub struct StatusStrip {
    started: Option<Instant>,
    tail: String,
    error: Option<String>,
}

impl StatusStrip {
    /// A request is starting.
    pub fn begin(&mut self) {
        self.started = Some(Instant::now());
        self.tail.clear();
        self.error = None;
    }

    /// A piece of the answer arrived: the strip keeps the end of it.
    pub fn push(&mut self, delta: &str) {
        self.tail.push_str(delta);
        let total = self.tail.chars().count();
        if total > TAIL_CHARS {
            self.tail = self.tail.chars().skip(total - TAIL_CHARS).collect();
        }
        // A newline in the tail would turn the one-line strip into two.
        self.tail = self.tail.replace('\n', " ");
    }

    /// The request ended (with or without an answer).
    pub fn finish(&mut self) {
        self.started = None;
        self.tail.clear();
    }

    /// The request failed.
    pub fn fail(&mut self, error: impl Into<String>) {
        self.started = None;
        self.tail.clear();
        self.error = Some(error.into());
    }

    pub fn running(&self) -> bool {
        self.started.is_some()
    }

    pub fn seconds(&self) -> u64 {
        self.started.map(|started| started.elapsed().as_secs()).unwrap_or(0)
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Draws the strip when there is something to say. Returns what the user asked for.
    pub fn show(&mut self, ui: &mut egui::Ui, labels: StripLabels<'_>) -> StripAction {
        if let Some(error) = &self.error {
            let mut action = StripAction::None;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(error).small().color(ui.visuals().warn_fg_color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("x").on_hover_text(labels.dismiss).clicked() {
                        action = StripAction::Dismiss;
                    }
                });
            });
            if action == StripAction::Dismiss {
                self.error = None;
            }
            return action;
        }
        if !self.running() {
            return StripAction::None;
        }
        let mut action = StripAction::None;
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(egui::RichText::new(format!("{}s", self.seconds())).small().weak());
            // The tail is a label with a fixed width: the row never grows or shrinks with the stream.
            ui.add_sized([ui.available_width() - 70.0, 16.0], egui::Label::new(egui::RichText::new(self.tail.clone()).small().weak()).truncate());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button(labels.stop).clicked() {
                    action = StripAction::Stop;
                }
            });
        });
        action
    }
}

/// The two words the strip needs, in the app's language.
#[derive(Debug, Clone, Copy)]
pub struct StripLabels<'a> {
    pub stop: &'a str,
    pub dismiss: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tail_keeps_only_the_end_and_never_a_line_break() {
        let mut strip = StatusStrip::default();
        strip.begin();
        strip.push(&"palavra ".repeat(40));
        let tail = &strip.tail;
        assert!(tail.chars().count() <= TAIL_CHARS, "{}", tail.chars().count());
        assert!(tail.ends_with("palavra "), "{tail:?}");
        strip.push("\nmais uma linha");
        assert!(!strip.tail.contains('\n'), "{:?}", strip.tail);
    }

    #[test]
    fn finishing_clears_the_tail_and_the_clock() {
        let mut strip = StatusStrip::default();
        strip.begin();
        assert!(strip.running());
        strip.push("oi");
        strip.finish();
        assert!(!strip.running());
        assert!(strip.tail.is_empty());
    }

    #[test]
    fn an_error_takes_over_and_a_dismiss_clears_it() {
        let mut strip = StatusStrip::default();
        strip.begin();
        strip.fail("sem chave");
        assert_eq!(strip.error(), Some("sem chave"));
        assert!(!strip.running());
    }
}
