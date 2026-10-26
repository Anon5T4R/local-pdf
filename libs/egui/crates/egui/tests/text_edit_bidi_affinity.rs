//! Persisting a `TextEdit` cursor must retain its visual side of a bidi boundary.

use egui::text::{CCursor, CCursorRange};
use egui::{Context, Event, Id, Key, Modifiers, RawInput, TextEdit};

fn frame(ctx: &Context, id: Id, text: &mut String, events: Vec<Event>) -> (CCursor, f32) {
    let mut observed = None;
    ctx.run_ui(
        RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            ui.memory_mut(|memory| memory.request_focus(id));
            let output = TextEdit::singleline(text).id(id).show(ui);
            let cursor = output.cursor_range.expect("focused cursor").primary;
            observed = Some((cursor, output.galley.pos_from_cursor(cursor).min.x));
        },
    )
    .textures_delta
    .clear();
    observed.expect("text field was shown")
}

fn arrow(key: Key) -> Vec<Event> {
    [true, false]
        .into_iter()
        .map(|pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        })
        .collect()
}

#[test]
fn mixed_direction_cursor_advances_through_persisted_and_idle_frames() {
    let ctx = Context::default();
    let id = Id::unique("bidi-affinity");
    let mut text = "abc אבג def".to_owned();
    frame(&ctx, id, &mut text, vec![]);
    let mut state = TextEdit::load_state(&ctx, id).expect("text state");
    state
        .cursor
        .set_char_range(Some(CCursorRange::one(CCursor::new(0))));
    state.store(&ctx, id);
    let (_, mut previous_x) = frame(&ctx, id, &mut text, vec![]);
    for (key, expected_indices) in [
        (Key::ArrowRight, vec![1, 2, 3, 4, 6, 5, 4, 8, 9, 10, 11]),
        (Key::ArrowLeft, vec![10, 9, 8, 7, 5, 6, 7, 3, 2, 1, 0]),
    ] {
        for expected in expected_indices {
            let (cursor, x) = frame(&ctx, id, &mut text, arrow(key));
            assert_eq!(cursor.index.0, expected);
            assert!(if key == Key::ArrowRight {
                x > previous_x
            } else {
                x < previous_x
            });
            let (idle, idle_x) = frame(&ctx, id, &mut text, vec![]);
            assert_eq!(idle.index, cursor.index);
            assert_eq!(idle.prefer_next_row, cursor.prefer_next_row);
            assert_eq!(idle_x, x);
            previous_x = x;
        }
    }
    assert_eq!(text, "abc אבג def");
}

fn range_frame(
    ctx: &Context,
    id: Id,
    text: &mut String,
    multiline: bool,
    width: f32,
) -> (CCursorRange, std::sync::Arc<egui::Galley>) {
    let mut observed = None;
    ctx.run_ui(RawInput::default(), |ui| {
        ui.set_max_width(width);
        ui.memory_mut(|memory| memory.request_focus(id));
        let edit = if multiline {
            TextEdit::multiline(text)
        } else {
            TextEdit::singleline(text)
        };
        let output = edit.id(id).desired_width(width).show(ui);
        observed = Some((output.cursor_range.expect("focused range"), output.galley));
    })
    .textures_delta
    .clear();
    observed.expect("text field was shown")
}

#[test]
fn selection_endpoints_keep_affinity_across_wraps_newlines_and_clipping() {
    for (multiline, width, text) in [
        (true, 65.0, "abc אבג def repeated text wraps across rows"),
        (true, 200.0, "abc אבג def\nאבג abc"),
        (false, 40.0, "abc אבג def repeated text clips horizontally"),
    ] {
        let ctx = Context::default();
        let id = Id::unique("bidi-range");
        let mut text = text.to_owned();
        let (_, galley) = range_frame(&ctx, id, &mut text, multiline, width);
        if multiline {
            assert!(galley.rows.len() > 1);
        } else {
            assert!(galley.size().x > width, "exercise a clipped field");
        }
        let primary = CCursor::new(4);
        let secondary = CCursor {
            prefer_next_row: true,
            ..CCursor::new(7)
        };
        let mut state = TextEdit::load_state(&ctx, id).expect("text state");
        state.cursor.set_char_range(Some(CCursorRange {
            primary,
            secondary,
            h_pos: None,
        }));
        state.store(&ctx, id);
        for _ in 0..3 {
            let (range, current_galley) = range_frame(&ctx, id, &mut text, multiline, width);
            for (actual, expected) in [(range.primary, primary), (range.secondary, secondary)] {
                assert_eq!(
                    (actual.index, actual.prefer_next_row),
                    (expected.index, expected.prefer_next_row)
                );
                assert_eq!(
                    current_galley.pos_from_cursor(actual),
                    galley.pos_from_cursor(expected)
                );
            }
        }
    }
}

#[test]
fn externally_shortened_text_clamps_both_persisted_endpoints() {
    let ctx = Context::default();
    let id = Id::unique("shrinking-bidi-range");
    let mut text = "abc אבג def".to_owned();
    range_frame(&ctx, id, &mut text, true, 70.0);
    for replacement in ["abc", ""] {
        let mut state = TextEdit::load_state(&ctx, id).expect("text state");
        state.cursor.set_char_range(Some(CCursorRange::two(
            CCursor {
                prefer_next_row: true,
                ..CCursor::new(replacement.chars().count() + 1)
            },
            CCursor {
                prefer_next_row: true,
                ..CCursor::new(usize::MAX)
            },
        )));
        state.store(&ctx, id);
        text = replacement.to_owned();
        let (range, _) = range_frame(&ctx, id, &mut text, true, 70.0);
        for cursor in [range.primary, range.secondary] {
            assert_eq!(cursor.index.0, replacement.chars().count());
            assert!(!cursor.prefer_next_row);
        }
    }
}
