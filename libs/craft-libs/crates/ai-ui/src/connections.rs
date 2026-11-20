//! The connections form: one dialog, one width, the same in every app of the suite.
//!
//! The panel is the form only: it edits a draft (provider, base URL, model, key) and returns what the
//! user asked for. The app owns the store, the keychain and the model list - it answers
//! [`ConnectionsIntent::Save`] and [`ConnectionsIntent::LoadModels`] with its own client.

/// A provider preset the app knows (its id, its label, and the base URL it fills in).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Preset {
    pub id: String,
    pub label: String,
    pub base_url: String,
}

/// What the form is editing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnectionDraft {
    pub provider: String,
    pub base_url: String,
    pub model: String,
}

/// What the form asks the app to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionsIntent {
    None,
    /// Store the draft and the key.
    Save(ConnectionDraft, String),
    /// Ask the provider for its model list, with the key as typed.
    LoadModels(ConnectionDraft, String),
    Cancel,
}

/// The form's words, in the app's language.
pub struct Labels<'a> {
    pub title: &'a str,
    pub provider: &'a str,
    pub base_url: &'a str,
    pub model: &'a str,
    pub api_key: &'a str,
    pub list_models: &'a str,
    pub save: &'a str,
    pub cancel: &'a str,
    pub hint_url: &'a str,
    pub hint_key: &'a str,
    /// Tooltip of the close X.
    pub close: &'a str,
    /// Why "list models" is off: a remote provider needs the key first.
    pub needs_key: &'a str,
    /// Why "list models" or "save" is off: no provider address yet.
    pub needs_url: &'a str,
    /// Why "save" is off: no model chosen yet.
    pub needs_model: &'a str,
}

/// Whether `base_url` points at this computer (a local server such as Ollama runs without a key).
fn is_local(base_url: &str) -> bool {
    let rest = base_url.trim().split_once("://").map_or(base_url.trim(), |(_, rest)| rest);
    let host = rest.split(['/', '?']).next().unwrap_or_default();
    let host = host.strip_prefix('[').map_or_else(|| host.split(':').next().unwrap_or_default(), |v6| v6.split(']').next().unwrap_or_default());
    matches!(host.to_ascii_lowercase().as_str(), "localhost" | "127.0.0.1" | "::1") || host.ends_with(".localhost")
}

/// What keeps "list models" off, if anything: the address, then the key (only a remote provider needs one).
fn models_blocker<'a>(draft: &ConnectionDraft, key: &str, labels: &Labels<'a>) -> Option<&'a str> {
    if draft.base_url.trim().is_empty() {
        Some(labels.needs_url)
    } else if key.trim().is_empty() && !is_local(&draft.base_url) {
        Some(labels.needs_key)
    } else {
        None
    }
}

/// What keeps "save" off, if anything.
fn save_blocker<'a>(draft: &ConnectionDraft, labels: &Labels<'a>) -> Option<&'a str> {
    if draft.base_url.trim().is_empty() {
        Some(labels.needs_url)
    } else if draft.model.trim().is_empty() {
        Some(labels.needs_model)
    } else {
        None
    }
}

/// The connections dialog.
#[derive(Debug, Default)]
pub struct ConnectionsPanel {
    pub open: bool,
    pub draft: ConnectionDraft,
    /// The key as typed (never shown back: the store keeps it in the keychain).
    pub key: String,
    pub models: Vec<String>,
    pub error: Option<String>,
    presets: Vec<Preset>,
    /// The dialog's rect from the last frame (the app's tests measure it).
    rect: Option<egui::Rect>,
}

impl ConnectionsPanel {
    /// The width of the dialog: one form, one measure.
    pub const WIDTH: f32 = 520.0;

    /// Opens the form on an existing connection (or empty, for the first one).
    pub fn open_with(&mut self, connection: Option<ConnectionDraft>) {
        self.open = true;
        self.error = None;
        self.models.clear();
        self.key.clear();
        if let Some(connection) = connection {
            self.draft = connection;
        } else {
            self.draft = ConnectionDraft::default();
        }
    }

    /// The dialog rect of the last frame (the app may assert its size).
    pub fn rect(&self) -> Option<egui::Rect> {
        self.rect
    }

    pub fn set_presets(&mut self, presets: Vec<Preset>) {
        self.presets = presets;
    }

    /// The app saved it: the dialog closes.
    pub fn saved(&mut self) {
        self.open = false;
        self.error = None;
        self.key.clear();
    }

    /// The app failed (store, keychain, model list): the dialog stays, with the reason.
    pub fn failed(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    /// Draws the dialog.
    pub fn show(&mut self, ctx: &egui::Context, labels: &Labels<'_>) -> ConnectionsIntent {
        if !self.open {
            return ConnectionsIntent::None;
        }
        let mut intent = ConnectionsIntent::None;
        let mut open = true;
        // The panel draws its own header (title and X on one row, the X centred on the title); egui's title bar
        // put the X on its own baseline.
        let window = egui::Window::new(labels.title)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .min_width(Self::WIDTH)
            .max_width(Self::WIDTH)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(labels.title).heading());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if crate::close_button(ui, labels.close).clicked() {
                            open = false;
                        }
                    });
                });
                ui.separator();
                // One width for the whole form: the fields ask the dialog, never the screen.
                let field = ui.available_width();
                ui.label(egui::RichText::new(labels.provider).small().weak());
                let selected = self
                    .presets
                    .iter()
                    .find(|preset| preset.id == self.draft.provider)
                    .map(|preset| preset.label.clone())
                    .unwrap_or_else(|| if self.draft.provider.is_empty() { "—".to_string() } else { self.draft.provider.clone() });
                egui::ComboBox::from_id_salt("ai-connections-provider").selected_text(selected).width(field).show_ui(ui, |ui| {
                    for preset in &self.presets {
                        if ui.selectable_label(preset.id == self.draft.provider, &preset.label).clicked() {
                            self.draft.provider = preset.id.clone();
                            // A preset is a starting point: the URL stays editable after it.
                            if !preset.base_url.is_empty() {
                                self.draft.base_url = preset.base_url.clone();
                            }
                        }
                    }
                });

                ui.add_space(4.0);
                ui.label(egui::RichText::new(labels.base_url).small().weak());
                // The placeholder is the chosen provider's own address, never a guess.
                let placeholder =
                    self.presets.iter().find(|preset| preset.id == self.draft.provider).map(|preset| preset.base_url.clone()).unwrap_or_default();
                ui.add(egui::TextEdit::singleline(&mut self.draft.base_url).hint_text(placeholder).desired_width(field));
                ui.label(egui::RichText::new(labels.hint_url).small().weak());

                ui.add_space(4.0);
                ui.label(egui::RichText::new(labels.model).small().weak());
                ui.horizontal(|ui| {
                    let list = 96.0;
                    ui.add(egui::TextEdit::singleline(&mut self.draft.model).desired_width((field - list).max(120.0)));
                    let blocker = models_blocker(&self.draft, &self.key, labels);
                    let button = ui.add_enabled(blocker.is_none(), egui::Button::new(labels.list_models));
                    let button = match blocker {
                        Some(reason) => button.on_disabled_hover_text(reason),
                        None => button,
                    };
                    if button.clicked() {
                        intent = ConnectionsIntent::LoadModels(self.draft.clone(), self.key.trim().to_string());
                    }
                });
                if !self.models.is_empty() {
                    egui::ScrollArea::vertical().max_height(120.0).id_salt("ai-connections-models").show(ui, |ui| {
                        for model in self.models.clone() {
                            if ui.selectable_label(self.draft.model == model, &model).clicked() {
                                self.draft.model = model;
                            }
                        }
                    });
                }

                ui.add_space(4.0);
                ui.label(egui::RichText::new(labels.api_key).small().weak());
                ui.add(egui::TextEdit::singleline(&mut self.key).password(true).desired_width(field));
                ui.label(egui::RichText::new(labels.hint_key).small().weak());

                if let Some(error) = &self.error {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(error).small().color(ui.visuals().warn_fg_color));
                }

                ui.add_space(10.0);
                // The actions sit together, bottom right, where the hand looks for them.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    let blocker = save_blocker(&self.draft, labels);
                    let button = ui.add_enabled(blocker.is_none(), egui::Button::new(labels.save));
                    let button = match blocker {
                        Some(reason) => button.on_disabled_hover_text(reason),
                        None => button,
                    };
                    if button.clicked() {
                        intent = ConnectionsIntent::Save(self.draft.clone(), self.key.trim().to_string());
                    }
                    if ui.button(labels.cancel).clicked() {
                        intent = ConnectionsIntent::Cancel;
                    }
                });
            });
        self.rect = window.map(|window| window.response.rect);
        if !open && intent == ConnectionsIntent::None {
            intent = ConnectionsIntent::Cancel;
        }
        intent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_with_a_connection_copies_its_draft_and_clears_the_key() {
        let mut panel = ConnectionsPanel { key: "velha".into(), ..ConnectionsPanel::default() };
        let draft = ConnectionDraft { provider: "ollama".into(), base_url: "http://x/v1".into(), model: "m".into() };
        panel.open_with(Some(draft.clone()));
        assert!(panel.open);
        assert_eq!(panel.draft, draft);
        assert!(panel.key.is_empty(), "the key never comes back from the store");
    }

    #[test]
    fn a_save_closes_the_form_and_a_failure_keeps_it_open() {
        let mut panel = ConnectionsPanel::default();
        panel.open_with(None);
        panel.failed("sem chave");
        assert!(panel.open);
        assert_eq!(panel.error.as_deref(), Some("sem chave"));
        panel.saved();
        assert!(!panel.open);
        assert!(panel.error.is_none());
    }

    fn labels() -> Labels<'static> {
        Labels {
            title: "t",
            provider: "p",
            base_url: "u",
            model: "m",
            api_key: "k",
            list_models: "l",
            save: "s",
            cancel: "c",
            hint_url: "hu",
            hint_key: "hk",
            close: "x",
            needs_key: "needs key",
            needs_url: "needs url",
            needs_model: "needs model",
        }
    }

    #[test]
    fn local_addresses_are_told_apart_from_remote_ones() {
        for local in ["http://localhost:11434/v1", "http://127.0.0.1:1234", "http://[::1]:8080/v1", "localhost", "http://ai.localhost/v1"] {
            assert!(is_local(local), "{local}");
        }
        for remote in ["https://api.openai.com/v1", "https://localhost.example.com/v1", "http://10.0.0.2:11434", ""] {
            assert!(!is_local(remote), "{remote}");
        }
    }

    /// Listing models is off (with the reason) until there is an address, and a remote one also needs the key;
    /// a local server lists without one.
    #[test]
    fn listing_models_says_what_is_missing() {
        let labels = labels();
        let mut draft = ConnectionDraft::default();
        assert_eq!(models_blocker(&draft, "", &labels), Some("needs url"));
        draft.base_url = "https://api.example.com/v1".into();
        assert_eq!(models_blocker(&draft, " ", &labels), Some("needs key"));
        assert_eq!(models_blocker(&draft, "sk-1", &labels), None);
        draft.base_url = "http://localhost:11434/v1".into();
        assert_eq!(models_blocker(&draft, "", &labels), None);
    }

    #[test]
    fn saving_says_what_is_missing() {
        let labels = labels();
        let mut draft = ConnectionDraft::default();
        assert_eq!(save_blocker(&draft, &labels), Some("needs url"));
        draft.base_url = "http://localhost:11434/v1".into();
        assert_eq!(save_blocker(&draft, &labels), Some("needs model"));
        draft.model = "llama".into();
        assert_eq!(save_blocker(&draft, &labels), None);
    }
}
