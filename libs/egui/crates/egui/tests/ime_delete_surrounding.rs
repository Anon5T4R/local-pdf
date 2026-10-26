use egui::{Context, Event, Id, ImeEvent, Key, Modifiers, RawInput, Rect, TextEdit, pos2, vec2};

/// Text before a replaced range: a letter with a combining accent, a precomposed letter, a flag
/// and a ZWJ family emoji. Deleting by grapheme cluster or by Backspace would cut into it.
const PREFIX: &str = "e\u{301}é🇯🇵👨\u{200d}👩\u{200d}👧";

struct Field {
    ctx: Context,
    id: Id,
    text: String,
    time: f64,
}

impl Field {
    fn new(text: &str) -> Self {
        let mut field = Self {
            ctx: Context::default(),
            id: Id::unique("field"),
            text: text.to_owned(),
            time: 0.0,
        };
        // Focus, then let the undoer settle on the starting text.
        field.frame(vec![]);
        field.time += 2.0;
        field.frame(vec![]);
        field
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.time += 0.1;
        let id = self.id;
        let text = &mut self.text;
        let mut output = self.ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0))),
                time: Some(self.time),
                events,
                ..Default::default()
            },
            |ui| {
                ui.memory_mut(|mem| mem.request_focus(id));
                ui.add(TextEdit::singleline(text).id(id));
            },
        );
        output.textures_delta.clear();
    }

    fn undo(&mut self) {
        self.frame(vec![Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        }]);
    }
}

fn delete(before_bytes: usize, after_bytes: usize) -> Event {
    Event::Ime(ImeEvent::DeleteSurroundingBytes {
        before_bytes,
        after_bytes,
    })
}

fn commit(text: &str) -> Event {
    Event::Ime(ImeEvent::Commit(text.to_owned()))
}

fn preedit(text: &str) -> Event {
    let end = text.chars().count();
    Event::Ime(ImeEvent::Preedit {
        text: text.to_owned(),
        active_range_chars: (end > 0).then_some(end..end),
    })
}

#[test]
fn press_and_hold_replaces_only_the_letter_and_undoes_in_one_step() {
    let original = format!("{PREFIX}e");
    let mut field = Field::new(&original);

    // The accent menu marks the letter it replaces: delete it, then show it as pre-edit text.
    field.frame(vec![delete(1, 0), commit(""), preedit("e")]);
    assert_eq!(field.text, original);
    // Picking an accent commits it in place of the pre-edit text.
    field.frame(vec![preedit(""), commit("è")]);
    assert_eq!(field.text, format!("{PREFIX}è"));

    field.undo();
    assert_eq!(field.text, original);
}

#[test]
fn a_deletion_with_its_commit_removes_exact_bytes() {
    // An emoji sequence replaced as a whole.
    let mut field = Field::new(PREFIX);
    field.frame(vec![
        delete("👨\u{200d}👩\u{200d}👧".len(), 0),
        commit("🙂"),
    ]);
    assert_eq!(field.text, "e\u{301}é🇯🇵🙂");
    field.undo();
    assert_eq!(field.text, PREFIX);

    // A combining accent replaced without its letter.
    let mut field = Field::new(&format!("{PREFIX}e\u{301}"));
    field.frame(vec![delete('\u{301}'.len_utf8(), 0), commit("\u{300}")]);
    assert_eq!(field.text, format!("{PREFIX}e\u{300}"));

    // One regional indicator of a flag.
    let mut field = Field::new("a🇯🇵");
    field.frame(vec![delete('🇵'.len_utf8(), 0), commit("")]);
    assert_eq!(field.text, "a🇯");
}

#[test]
fn a_count_ending_inside_a_character_leaves_it() {
    let mut field = Field::new("ab👧");
    field.frame(vec![delete(3, 0), commit("")]);
    assert_eq!(field.text, "ab👧");
    field.frame(vec![delete(5, 0), commit("")]);
    assert_eq!(field.text, "a");

    // More bytes than the text holds delete all of it.
    let mut field = Field::new("ab");
    field.frame(vec![delete(10, 0), commit("x")]);
    assert_eq!(field.text, "x");
}
