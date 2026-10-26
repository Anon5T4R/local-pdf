//! Regressions for response reads changing focus before `TextEdit` processes its selection.
//! Reference: <https://github.com/emilk/egui/issues/8763>.

use egui::text::{CCursor, CCursorRange};
use egui::{Context, Event, Id, PointerButton, RawInput, Rect, TextEdit, pos2, vec2};

struct Fixture {
    ctx: Context,
    field: Id,
    text: String,
    button: Rect,
    field_rect: Rect,
    selection_pos: egui::Pos2,
    button_first: bool,
    menu_item: Rect,
    menu_selection: Option<String>,
    restore_focus: bool,
    capture_press: bool,
    press_selection: Option<String>,
    force_discard: bool,
}

#[derive(Default)]
struct Observed {
    focus_before_read: bool,
    focus_after_read: bool,
    selected_action_text: Option<String>,
    copied_text: Vec<String>,
    passes: usize,
}

impl Fixture {
    fn new(policy: egui::SurrenderFocusOn) -> Self {
        Self::with_order(policy, false)
    }

    fn with_order(policy: egui::SurrenderFocusOn, button_first: bool) -> Self {
        let mut this = Self {
            ctx: Context::default(),
            field: Id::unique("selection-action-field"),
            text: "alpha beta".to_owned(),
            button: Rect::NOTHING,
            field_rect: Rect::NOTHING,
            selection_pos: egui::Pos2::ZERO,
            button_first,
            menu_item: Rect::NOTHING,
            menu_selection: None,
            restore_focus: false,
            capture_press: false,
            press_selection: None,
            force_discard: false,
        };
        this.ctx
            .options_mut(|options| options.input_options.surrender_focus_on = policy);
        this.frame(vec![]);
        this.ctx
            .memory_mut(|memory| memory.request_focus(this.field));
        this.frame(vec![]);
        let mut state = TextEdit::load_state(&this.ctx, this.field).expect("registered text field");
        state
            .cursor
            .set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(5))));
        state.store(&this.ctx, this.field);
        this.frame(vec![]);
        this
    }

    fn pointer(&self, button: PointerButton, pressed: bool) -> Vec<Event> {
        Self::pointer_at(self.button.center(), button, pressed)
    }

    fn pointer_at(pos: egui::Pos2, button: PointerButton, pressed: bool) -> Vec<Event> {
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: Default::default(),
            },
        ]
    }

    fn frame(&mut self, events: Vec<Event>) -> Observed {
        let mut observed = Observed::default();
        let mut output = self.ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0))),
                events,
                ..Default::default()
            },
            |ui| {
                if self.force_discard && ui.ctx().current_pass_index() == 0 {
                    ui.ctx()
                        .request_discard("selection regression: second layout pass");
                }
                observed.focus_before_read = ui.memory(|memory| memory.has_focus(self.field));
                let _ = ui.ctx().read_response(self.field);
                observed.focus_after_read = ui.memory(|memory| memory.has_focus(self.field));
                if !self.button_first {
                    let output = TextEdit::singleline(&mut self.text).id(self.field).show(ui);
                    self.field_rect = output.response.rect;
                    self.selection_pos = output.galley_pos
                        + output
                            .galley
                            .pos_from_cursor(CCursor::new(2))
                            .center()
                            .to_vec2();
                    ui.add_space(40.0);
                }
                let response = ui.button("Use selection");
                self.button = response.rect;
                let capture_press = self.capture_press
                    && response.is_pointer_button_down_on()
                    && ui.input(|input| input.pointer.any_pressed());
                if response.clicked() || response.secondary_clicked() || capture_press {
                    let range = TextEdit::load_state(ui.ctx(), self.field)
                        .and_then(|state| state.cursor.char_range())
                        .expect("selection state");
                    let [start, end] = range.sorted_cursors();
                    observed.selected_action_text = Some(
                        self.text
                            .chars()
                            .skip(start.index.0)
                            .take(end.index.0 - start.index.0)
                            .collect(),
                    );
                }
                if capture_press {
                    self.press_selection = observed.selected_action_text.clone();
                }
                if response.secondary_clicked() {
                    // A delayed menu command owns the selection from the opening action.
                    // TextEdit's long-standing unfocused-selection policy is unchanged.
                    self.menu_selection = observed.selected_action_text.clone();
                }
                response.context_menu(|ui| {
                    let item = ui.button("Apply saved selection");
                    self.menu_item = item.rect;
                    if item.clicked() {
                        observed.selected_action_text = self.menu_selection.take();
                        ui.close();
                    }
                });
                if self.button_first {
                    ui.add_space(40.0);
                    let output = TextEdit::singleline(&mut self.text).id(self.field).show(ui);
                    self.field_rect = output.response.rect;
                    self.selection_pos = output.galley_pos
                        + output
                            .galley
                            .pos_from_cursor(CCursor::new(2))
                            .center()
                            .to_vec2();
                }
                // Restore focus after all widgets have applied click-away policy,
                // regardless of the toolbar/editor layout order.
                if self.restore_focus && observed.selected_action_text.is_some() {
                    ui.memory_mut(|memory| memory.request_focus(self.field));
                }
            },
        );
        observed.passes = output.platform_output.num_completed_passes;
        output.textures_delta.clear();
        for command in output.platform_output.commands {
            if let egui::OutputCommand::CopyText(text) = command {
                observed.copied_text.push(text);
            }
        }
        observed
    }
}

#[test]
fn pointer_toolbar_and_secondary_actions_see_the_original_selection() {
    for policy in [
        egui::SurrenderFocusOn::Clicks,
        egui::SurrenderFocusOn::Never,
    ] {
        for button_first in [false, true] {
            for button in [PointerButton::Primary, PointerButton::Secondary] {
                let mut fixture = Fixture::with_order(policy, button_first);
                fixture.frame(fixture.pointer(button, true));
                let observed = fixture.frame(fixture.pointer(button, false));
                assert_eq!(
                    observed.selected_action_text.as_deref(),
                    Some("alpha"),
                    "{button:?}, {policy:?}, button_first={button_first}"
                );
                assert_eq!(
                    fixture.ctx.memory(|memory| memory.has_focus(fixture.field)),
                    policy == egui::SurrenderFocusOn::Never
                );
            }
        }
    }
}

#[test]
fn delayed_context_menu_command_uses_the_opening_selection() {
    let mut fixture = Fixture::new(egui::SurrenderFocusOn::Clicks);
    fixture.frame(fixture.pointer(PointerButton::Secondary, true));
    fixture.frame(fixture.pointer(PointerButton::Secondary, false));
    fixture.frame(vec![]);
    fixture.frame(vec![]);
    assert!(
        fixture.menu_item.is_positive(),
        "real context menu must be open"
    );
    let pos = fixture.menu_item.center();
    fixture.frame(Fixture::pointer_at(pos, PointerButton::Primary, true));
    let observed = fixture.frame(Fixture::pointer_at(pos, PointerButton::Primary, false));
    assert_eq!(observed.selected_action_text.as_deref(), Some("alpha"));
}

#[test]
fn secondary_click_inside_the_field_places_the_caret_and_keeps_focus() {
    for policy in [
        egui::SurrenderFocusOn::Presses,
        egui::SurrenderFocusOn::Clicks,
    ] {
        let mut fixture = Fixture::new(policy);
        let pos = fixture.selection_pos;
        fixture.frame(Fixture::pointer_at(pos, PointerButton::Secondary, true));
        fixture.frame(Fixture::pointer_at(pos, PointerButton::Secondary, false));
        assert!(fixture.ctx.memory(|memory| memory.has_focus(fixture.field)));
        let range = TextEdit::load_state(&fixture.ctx, fixture.field)
            .unwrap()
            .cursor
            .char_range()
            .unwrap();
        assert_eq!(
            range.as_sorted_char_range(),
            egui::text::CharIndex(2)..egui::text::CharIndex(2)
        );
    }
}

#[test]
fn reading_a_response_preserves_focus_until_the_widget_is_shown() {
    for policy in [
        egui::SurrenderFocusOn::Presses,
        egui::SurrenderFocusOn::Clicks,
        egui::SurrenderFocusOn::Never,
    ] {
        let mut fixture = Fixture::new(policy);
        let observed = if policy == egui::SurrenderFocusOn::Presses {
            fixture.frame(fixture.pointer(PointerButton::Primary, true))
        } else {
            fixture.frame(fixture.pointer(PointerButton::Primary, true));
            fixture.frame(fixture.pointer(PointerButton::Primary, false))
        };
        assert!(
            observed.focus_before_read,
            "{policy:?}: focused before the read"
        );
        assert!(
            observed.focus_after_read,
            "{policy:?}: response inspection must not surrender focus"
        );
        assert_eq!(
            fixture.ctx.memory(|memory| memory.has_focus(fixture.field)),
            policy == egui::SurrenderFocusOn::Never
        );
    }
}

#[test]
fn keyboard_copy_keeps_the_selection_and_copies_selected_text() {
    let mut fixture = Fixture::new(egui::SurrenderFocusOn::Clicks);
    let observed = fixture.frame(vec![Event::Copy]);
    assert_eq!(observed.copied_text, ["alpha"]);
    assert!(fixture.ctx.memory(|memory| memory.has_focus(fixture.field)));
    let state = TextEdit::load_state(&fixture.ctx, fixture.field).unwrap();
    let range = state.cursor.char_range().unwrap();
    assert_eq!(
        range.as_sorted_char_range(),
        egui::text::CharIndex(0)..egui::text::CharIndex(5)
    );
}

#[test]
fn toolbar_can_restore_editing_after_widgets_in_either_widget_order() {
    for button_first in [false, true] {
        let mut fixture = Fixture::with_order(egui::SurrenderFocusOn::Clicks, button_first);
        fixture.restore_focus = true;
        fixture.frame(fixture.pointer(PointerButton::Primary, true));
        let action = fixture.frame(fixture.pointer(PointerButton::Primary, false));
        assert_eq!(action.selected_action_text.as_deref(), Some("alpha"));
        fixture.frame(vec![]);
        assert!(fixture.ctx.memory(|memory| memory.has_focus(fixture.field)));
        fixture.frame(vec![Event::Text("B".to_owned())]);
        assert_eq!(
            fixture.text, "B beta",
            "selection survives focus restoration, button_first={button_first}"
        );
    }
}

#[test]
fn presses_policy_actions_capture_the_selection_at_press() {
    for button_first in [false, true] {
        let mut fixture = Fixture::with_order(egui::SurrenderFocusOn::Presses, button_first);
        fixture.capture_press = true;
        fixture.frame(fixture.pointer(PointerButton::Primary, true));
        assert_eq!(
            fixture.press_selection.as_deref(),
            Some("alpha"),
            "button_first={button_first}"
        );
        // The release-time field state depends on widget order. The saved
        // press-time value is independent of that later unfocused layout.
        fixture.frame(fixture.pointer(PointerButton::Primary, false));
        fixture.frame(vec![]);
        assert_eq!(fixture.press_selection.as_deref(), Some("alpha"));
    }
}

#[test]
fn toolbar_selection_and_restored_focus_survive_a_discarded_pass() {
    for button_first in [false, true] {
        let mut fixture = Fixture::with_order(egui::SurrenderFocusOn::Clicks, button_first);
        fixture.restore_focus = true;
        fixture.frame(fixture.pointer(PointerButton::Primary, true));
        fixture.force_discard = true;
        let action = fixture.frame(fixture.pointer(PointerButton::Primary, false));
        assert_eq!(action.passes, 2, "the test must exercise a discarded pass");
        assert_eq!(action.selected_action_text.as_deref(), Some("alpha"));
        fixture.force_discard = false;
        fixture.frame(vec![Event::Text("B".to_owned())]);
        assert_eq!(fixture.text, "B beta", "button_first={button_first}");
    }
}
