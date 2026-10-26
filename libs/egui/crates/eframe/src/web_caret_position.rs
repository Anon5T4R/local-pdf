//! Position-only key for the web IME text agent.
//!
//! IME descriptor changes such as text, owner, selection or support state must not rewrite
//! the DOM caret style. Only geometry that changes the text agent position participates in
//! this key.

/// Inputs that determine where the hidden web text agent is placed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CaretGeometry {
    pub cursor_center_x: f32,
    pub cursor_center_y: f32,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub canvas_offset_left: i32,
    pub canvas_offset_top: i32,
    pub native_pixels_per_point: f32,
    pub zoom_factor: f32,
}

/// Canvas inputs that affect the text agent position.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CanvasMetrics {
    pub width: u32,
    pub height: u32,
    pub offset_left: i32,
    pub offset_top: i32,
    pub native_pixels_per_point: f32,
}

impl CaretGeometry {
    /// The geometry that determines where the text agent is placed for `ime`.
    ///
    /// Only the caret rectangle and the canvas/zoom inputs are read. IME purpose, owner,
    /// context generation, selection and surrounding text deliberately do not participate.
    pub fn from_ime(
        ime: &egui::output::IMEOutput,
        canvas: CanvasMetrics,
        zoom_factor: f32,
    ) -> Self {
        Self {
            cursor_center_x: ime.cursor_rect.center().x,
            cursor_center_y: ime.cursor_rect.center().y,
            canvas_width: canvas.width,
            canvas_height: canvas.height,
            canvas_offset_left: canvas.offset_left,
            canvas_offset_top: canvas.offset_top,
            native_pixels_per_point: canvas.native_pixels_per_point,
            zoom_factor,
        }
    }
}

/// Exact position key, using the bit patterns of the geometry values.
///
/// It deliberately contains no IME text, owner, purpose or selection data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CaretPositionKey {
    cursor_center_x: u32,
    cursor_center_y: u32,
    canvas_width: u32,
    canvas_height: u32,
    canvas_offset_left: i32,
    canvas_offset_top: i32,
    native_pixels_per_point: u32,
    zoom_factor: u32,
}

impl CaretPositionKey {
    pub fn new(geometry: CaretGeometry) -> Self {
        let CaretGeometry {
            cursor_center_x,
            cursor_center_y,
            canvas_width,
            canvas_height,
            canvas_offset_left,
            canvas_offset_top,
            native_pixels_per_point,
            zoom_factor,
        } = geometry;
        Self {
            cursor_center_x: cursor_center_x.to_bits(),
            cursor_center_y: cursor_center_y.to_bits(),
            canvas_width,
            canvas_height,
            canvas_offset_left,
            canvas_offset_top,
            native_pixels_per_point: native_pixels_per_point.to_bits(),
            zoom_factor: zoom_factor.to_bits(),
        }
    }
}

/// Tracks the last caret position written to the DOM.
#[derive(Default)]
pub(crate) struct CaretPositionTracker {
    last_written: Option<CaretPositionKey>,
}

impl CaretPositionTracker {
    /// Return the key to write, or `None` if the DOM already has this position.
    ///
    /// `None` means there is no IME owner and forgets the last written position, so focusing
    /// the same caret again writes it even if its geometry is unchanged.
    pub fn update(&mut self, key: Option<CaretPositionKey>) -> Option<CaretPositionKey> {
        if key == self.last_written {
            None
        } else {
            self.last_written = key;
            key
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry() -> CaretGeometry {
        CaretGeometry {
            cursor_center_x: 20.0,
            cursor_center_y: 30.0,
            canvas_width: 800,
            canvas_height: 600,
            canvas_offset_left: 10,
            canvas_offset_top: 20,
            native_pixels_per_point: 2.0,
            zoom_factor: 1.0,
        }
    }

    fn key() -> CaretPositionKey {
        CaretPositionKey::new(geometry())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn unchanged_caret_position_is_not_written_again() {
        let mut tracker = CaretPositionTracker::default();
        assert_eq!(tracker.update(Some(key())), Some(key()));
        assert_eq!(tracker.update(Some(key())), None);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn caret_movement_changes_the_key() {
        let mut tracker = CaretPositionTracker::default();
        assert!(tracker.update(Some(key())).is_some());

        let mut moved = geometry();
        moved.cursor_center_x += 1.0;
        assert!(tracker.update(Some(CaretPositionKey::new(moved))).is_some());
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn zoom_dpi_and_canvas_geometry_change_the_key() {
        let base = key();

        let mut zoomed = geometry();
        zoomed.zoom_factor = 1.25;
        assert_ne!(CaretPositionKey::new(zoomed), base);

        let mut scaled = geometry();
        scaled.native_pixels_per_point = 1.5;
        assert_ne!(CaretPositionKey::new(scaled), base);

        let mut resized = geometry();
        resized.canvas_width += 1;
        assert_ne!(CaretPositionKey::new(resized), base);

        let mut moved_canvas = geometry();
        moved_canvas.canvas_offset_top += 1;
        assert_ne!(CaretPositionKey::new(moved_canvas), base);
    }

    fn ime(text: &str, owner: u8, generation: u64, cursor_x: f32) -> egui::output::IMEOutput {
        use egui::output::{IMEOutput, IMESurroundingText};

        let mut ime = IMEOutput::new(
            egui::Id::unique(owner),
            generation,
            egui::IMEPurpose::Normal,
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 20.0)),
            egui::Rect::from_min_size(egui::pos2(cursor_x, 2.0), egui::vec2(1.0, 16.0)),
        );
        ime.surrounding_text = IMESurroundingText::from_text(text, 0, 0, None);
        ime
    }

    fn canvas() -> CanvasMetrics {
        CanvasMetrics {
            width: 800,
            height: 600,
            offset_left: 10,
            offset_top: 20,
            native_pixels_per_point: 2.0,
        }
    }

    fn key_for(ime: &egui::output::IMEOutput) -> CaretPositionKey {
        CaretPositionKey::new(CaretGeometry::from_ime(ime, canvas(), 1.0))
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn text_owner_selection_purpose_and_cause_do_not_affect_the_position() {
        let short = ime("a", 1, 1, 20.0);
        let mut long = ime(&"a".repeat(5000), 2, 7, 20.0);
        long.purpose = egui::IMEPurpose::Password;
        long.change_cause = egui::ImeChangeCause::InputMethod;

        assert_eq!(key_for(&short), key_for(&long));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn losing_and_regaining_focus_writes_the_same_position_again() {
        let mut tracker = CaretPositionTracker::default();
        assert_eq!(tracker.update(Some(key())), Some(key()));
        assert_eq!(tracker.update(None), None);
        assert_eq!(
            tracker.update(Some(key())),
            Some(key()),
            "focus restoration must restore the caret position"
        );
    }
}
