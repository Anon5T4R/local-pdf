//! The AI assistant: a movable window with the user's own connections (BYOK), the model picker
//! that reads each provider's `/models`, chat about the open document, and OCR by a vision
//! model whose text is saved into the file as an invisible layer.
//!
//! Nothing here talks to a LocalPDF server: there is none. Every call goes straight to the
//! provider the user configured, with the key the OS keychain holds.

use std::sync::Arc;

use crate::LocalPDFApp;
use crate::theme::{self, Tokens};
use localpdf_ai::{Connection, Job, Msg, PROVIDERS, provider};

/// How much document text one chat request may carry (about 30k tokens of English).
const MAX_CONTEXT: usize = 120_000;

/// A chat turn as the panel shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum Turn {
    User(String),
    Assistant(String),
    /// The request failed; shown apart, not part of the next request.
    Failed(String),
}

/// What the panel holds between frames.
#[derive(Default)]
pub struct AiState {
    pub open: bool,
    pub connections: Vec<Connection>,
    pub selected: usize,
    /// The key being typed for the selected connection (cleared once saved).
    pub key_draft: String,
    pub editing_key: bool,
    /// The models the provider answered with, and the fetch that is filling them.
    pub models: Vec<String>,
    pub models_job: Option<Job<Vec<String>>>,
    pub chat: Vec<Turn>,
    pub input: String,
    pub job: Option<Job<String>>,
    /// The OCR-by-AI job: (page, the lines it read).
    pub ocr_job: Option<Job<(usize, Vec<String>)>>,
    /// Whole document or just the current page as the chat's context.
    pub whole_document: bool,
    /// The last error outside a turn (connections file, keychain).
    pub error: Option<String>,
}

impl AiState {
    /// Load the saved connections once.
    pub fn load(&mut self) {
        if !self.connections.is_empty() {
            return;
        }
        match Connection::load_all() {
            Ok(all) => self.connections = all,
            Err(e) => self.error = Some(e),
        }
    }

    fn selected_conn(&self) -> Option<&Connection> {
        self.connections.get(self.selected)
    }

    /// The connection the calls use, or why there is none.
    fn ready(&self) -> Result<Connection, String> {
        let conn = self.selected_conn().ok_or("no connection: add one first")?;
        if !conn.has_key() {
            return Err("the connection has no API key".into());
        }
        if conn.model.is_empty() {
            return Err("no model chosen".into());
        }
        Ok(conn.clone())
    }

    fn save(&mut self) {
        if let Err(e) = Connection::save_all(&self.connections) {
            self.error = Some(e);
        }
    }
}

/// Poll the background jobs; called every frame before the window draws.
fn poll(app: &mut LocalPDFApp) {
    if app.ai.models_job.as_ref().is_some_and(|j| j.poll().is_some()) {
        let job = app.ai.models_job.take();
        if let Some(Ok(models)) = job.and_then(|j| j.poll()) {
            app.ai.models = models;
        }
    }
    if app.ai.job.as_ref().is_some_and(|j| j.poll().is_some()) {
        let job = app.ai.job.take();
        if let Some(Ok(answer)) = job.and_then(|j| j.poll()) {
            app.ai.chat.push(Turn::Assistant(answer));
        }
    }
    if app.ai.ocr_job.as_ref().is_some_and(|j| j.poll().is_some()) {
        let job = app.ai.ocr_job.take();
        if let Some(Ok((page, lines))) = job.and_then(|j| j.poll()) {
            match ocr_lines_into_file(app, page, &lines) {
                Ok(words) => app.notify_fmt("AI text recognition: {words} words saved into the file", &[("words", &words.to_string())]),
                Err(e) => app.ai.error = Some(e),
            }
        }
    }
}

/// The page the active view shows.
fn current_page(app: &LocalPDFApp) -> usize {
    app.active.and_then(|i| app.views.get(i)).map(|v| v.current).unwrap_or(0)
}

/// The document text the chat sees: the current page's lines, or every page's, capped.
fn context_text(app: &mut LocalPDFApp) -> String {
    let Some((_, id)) = app.active_ids() else { return String::new() };
    let Some(doc) = app.session.get(id) else { return String::new() };
    let page_count = doc.info.pages.len();
    let pages: Vec<usize> = if app.ai.whole_document { (0..page_count).collect() } else { vec![current_page(app).min(page_count.saturating_sub(1))] };
    let mut out = String::new();
    'pages: for (index, page) in pages.iter().enumerate() {
        if index > 0 {
            out.push_str("\n\n--- page break ---\n\n");
        }
        for line in doc.text_lines(*page) {
            if out.len() + line.text.len() > MAX_CONTEXT {
                out.push_str("\n[the document continues; the text was cut to fit one request]");
                break 'pages;
            }
            out.push_str(&line.text);
            out.push('\n');
        }
    }
    out
}

/// The open document's name, for the system turn.
fn doc_name(app: &mut LocalPDFApp) -> String {
    app.active_ids().and_then(|(_, id)| app.session.get(id).map(|d| d.name.clone())).unwrap_or_else(|| "document".into())
}

/// Send the chat input: the doc context as the system turn, then the conversation so far.
fn send(app: &mut LocalPDFApp) {
    let input = std::mem::take(&mut app.ai.input);
    if input.trim().is_empty() {
        return;
    }
    let conn = match app.ai.ready() {
        Ok(c) => c,
        Err(e) => {
            app.ai.chat.push(Turn::Failed(e));
            return;
        }
    };
    let name = doc_name(app);
    let context = context_text(app);
    let mut history = vec![Msg::System(format!(
        "You help with a PDF the user has open in LocalPDF. The document is {name:?}. Its text follows after the marker. Answer in the user's language; be concise; quote the document when it helps.\n\n--- document text ---\n{context}"
    ))];
    for turn in &app.ai.chat {
        match turn {
            Turn::User(t) => history.push(Msg::User(t.clone())),
            Turn::Assistant(t) => history.push(Msg::Assistant(t.clone())),
            Turn::Failed(_) => {}
        }
    }
    history.push(Msg::User(input.clone()));
    app.ai.chat.push(Turn::User(input));
    app.ai.job = Some(Job::spawn(move || localpdf_ai::chat(&conn, &history)));
}

/// Summarize: one user turn asking for a summary of the context.
fn summarize(app: &mut LocalPDFApp) {
    let conn = match app.ai.ready() {
        Ok(c) => c,
        Err(e) => {
            app.ai.chat.push(Turn::Failed(e));
            return;
        }
    };
    let name = doc_name(app);
    let context = context_text(app);
    let history = vec![
        Msg::System(format!(
            "You help with a PDF the user has open in LocalPDF. The document is {name:?}. Its text follows after the marker. Answer in the document's language; be concise; use short paragraphs and, when it helps, a short list.\n\n--- document text ---\n{context}"
        )),
        Msg::User("Summarize this document: what it is, its main points and any actions it asks of the reader.".into()),
    ];
    app.ai.chat.push(Turn::User("Summarize this document.".into()));
    app.ai.job = Some(Job::spawn(move || localpdf_ai::chat(&conn, &history)));
}

/// OCR by AI: render the page, ask a vision model to transcribe it, one line per text line.
pub fn ocr_page(app: &mut LocalPDFApp) {
    let Some((page, id)) = app.active_ids() else { return };
    let conn = match app.ai.ready() {
        Ok(c) => c,
        Err(e) => {
            app.ai.error = Some(e);
            return;
        }
    };
    let Some(doc) = app.session.get(id) else { return };
    let config = localpdf_render::RenderConfig { password: doc.password.as_deref().map(Arc::from), ..Default::default() };
    let bytes = doc.bytes.clone();
    app.ai.ocr_job = Some(Job::spawn(move || {
        let mut renderer = localpdf_render::PageRenderer::new(bytes, config);
        let res =
            renderer.render(localpdf_render::RenderRequest { page, kind: localpdf_render::RequestKind::Pixels, scale: 2.0, ..Default::default() });
        if let Some(e) = res.error {
            return Err(e);
        }
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, res.width, res.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
            writer.write_image_data(&res.rgba).map_err(|e| e.to_string())?;
        }
        let instruction = "Transcribe ALL text visible in this page image, in reading order. Output ONLY the transcribed text: one line of plain text per visual line, no commentary, no markdown, no numbering. Keep the original language and spelling.";
        let answer = localpdf_ai::vision(&conn, instruction, &png)?;
        let lines: Vec<String> = answer.lines().map(|l| l.trim_end().to_owned()).filter(|l| !l.is_empty()).collect();
        Ok((page, lines))
    }));
}

/// Turn the AI's lines into placed words and save them into the file (one undoable step).
/// The lines have no boxes: they are laid out from the top of the page, one line per row, in
/// display space (an unturned page's display space is its user space).
fn ocr_lines_into_file(app: &mut LocalPDFApp, page: usize, lines: &[String]) -> Result<usize, String> {
    let Some((_, id)) = app.active_ids() else { return Err("no document".into()) };
    let Some(doc) = app.session.get(id) else { return Err("no document".into()) };
    let config = localpdf_render::RenderConfig { password: doc.password.as_deref().map(Arc::from), ..Default::default() };
    let bytes = doc.bytes.clone();
    let mut renderer = localpdf_render::PageRenderer::new(bytes, config);
    let res = renderer.render(localpdf_render::RenderRequest { page, kind: localpdf_render::RequestKind::Pixels, scale: 1.0, ..Default::default() });
    if let Some(e) = res.error {
        return Err(e);
    }
    let (width, height) = (f64::from(res.width), f64::from(res.height));
    let mut words: Vec<localpdf_ocr::PlacedWord> = Vec::new();
    let line_height = 11.0f64;
    let margin = 28.0f64;
    let top = height - margin - line_height;
    for (i, line) in lines.iter().enumerate() {
        let y = top - i as f64 * line_height;
        if y < margin {
            break;
        }
        // One placed word per line: the invisible layer is for search and copy, not layout.
        let width_estimate = (line.chars().count() as f64 * 5.0).min(width - 2.0 * margin).max(1.0);
        words.push(localpdf_ocr::PlacedWord { text: line.clone(), origin: [margin, y], across: [width_estimate, 0.0], up: [0.0, line_height] });
    }
    let count = words.len();
    app.session.apply(id, localpdf_engine::Edit::AddOcrText { page, words }).map_err(|e| e.to_string())?;
    Ok(count)
}

/// The window. Called every frame from `ui`.
pub fn show(app: &mut LocalPDFApp, ctx: &egui::Context) {
    poll(app);
    if !app.ai.open {
        return;
    }
    app.ai.load();
    let t = Tokens::get(ctx);
    let mut open = app.ai.open;
    egui::Window::new("AI assistant").open(&mut open).default_width(560.0).default_height(620.0).constrain(false).show(ctx, |ui| {
        connections_row(app, ui);
        ui.add_space(6.0);
        model_row(app, ui);
        ui.add_space(6.0);
        if let Some(e) = app.ai.error.take() {
            ui.label(egui::RichText::new(e).color(error_red()).small());
        }
        chat_area(app, ui, &t);
        input_row(app, ui);
    });
    app.ai.open = open;
}

fn error_red() -> egui::Color32 {
    egui::Color32::from_rgb(0xE5, 0x48, 0x4D)
}

/// The connection combo and its buttons: key, new, delete.
fn connections_row(app: &mut LocalPDFApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label("Connection");
        let labels: Vec<String> = app.ai.connections.iter().map(|c| format!("{} — {}", c.label, provider_label(&c.provider))).collect();
        egui::ComboBox::from_id_salt("ai-connection").selected_text(labels.get(app.ai.selected).map(String::as_str).unwrap_or("add one…")).show_ui(
            ui,
            |ui| {
                for (i, label) in labels.iter().enumerate() {
                    ui.selectable_value(&mut app.ai.selected, i, label);
                }
            },
        );
        let has = app.ai.selected_conn().is_some();
        let keyed = app.ai.selected_conn().is_some_and(|c| c.has_key());
        if ui.add_enabled(has, egui::Button::new(if keyed { "🔑 ✓" } else { "🔑 set key" })).clicked() {
            app.ai.editing_key = !app.ai.editing_key;
            app.ai.key_draft.clear();
        }
        if ui.button("＋").on_hover_text("Add a connection").clicked() {
            let id = format!("conn-{}", app.ai.connections.len() + 1);
            app.ai.connections.push(Connection {
                id,
                provider: "gemini".into(),
                label: "Gemini".into(),
                base_url: String::new(),
                model: String::new(),
            });
            app.ai.selected = app.ai.connections.len() - 1;
            app.ai.editing_key = true;
            app.ai.key_draft.clear();
            app.ai.models.clear();
        }
        let r = ui.add_enabled(has, egui::Button::new("🗑"));
        if r.on_hover_text("Delete this connection").clicked() {
            if let Some(c) = app.ai.selected_conn() {
                let _ = c.clear_key();
            }
            app.ai.connections.remove(app.ai.selected);
            app.ai.selected = app.ai.selected.saturating_sub(1);
            app.ai.models.clear();
            app.ai.save();
        }
    });
    // The key editor and the connection's fields, under the row.
    if let Some(conn) = app.ai.selected_conn().cloned() {
        if app.ai.editing_key {
            ui.horizontal(|ui| {
                let field = egui::TextEdit::singleline(&mut app.ai.key_draft).password(true).hint_text("API key (stays in the OS keychain)");
                let response = ui.add(field);
                if ui.button("Save key").clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    if let Err(e) = conn.set_key(app.ai.key_draft.trim()) {
                        app.ai.error = Some(e);
                    }
                    app.ai.key_draft.clear();
                    app.ai.editing_key = false;
                }
            });
        }
        ui.collapsing("Connection details", |ui| {
            egui::Grid::new("ai-conn-details").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                ui.label("Provider");
                egui::ComboBox::from_id_salt("ai-provider").selected_text(provider_label(&conn.provider)).show_ui(ui, |ui| {
                    for p in PROVIDERS {
                        ui.selectable_value(&mut app.ai.connections[app.ai.selected].provider, p.id.to_owned(), p.label);
                    }
                });
                ui.end_row();
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut app.ai.connections[app.ai.selected].label));
                ui.end_row();
                ui.label("Base URL");
                let hint = provider(&conn.provider).map(|p| p.base_url).unwrap_or("");
                ui.add(egui::TextEdit::singleline(&mut app.ai.connections[app.ai.selected].base_url).hint_text(hint));
                ui.end_row();
            });
            if ui.small_button("Save connection").clicked() {
                app.ai.save();
            }
        });
    }
}

fn provider_label(id: &str) -> String {
    localpdf_ai::provider(id).map(|p| p.label.to_owned()).unwrap_or_else(|| id.to_owned())
}

/// The model combo (the provider's own list) and its refresh button.
fn model_row(app: &mut LocalPDFApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label("Model");
        let fetching = app.ai.models_job.is_some();
        if fetching {
            ui.add(egui::Spinner::new().size(16.0));
        }
        if app.ai.models.is_empty() {
            // No list yet: a free-text model name still works.
            if app.ai.selected_conn().is_some() {
                ui.add(egui::TextEdit::singleline(&mut app.ai.connections[app.ai.selected].model).hint_text("model id (or fetch the list)"));
            }
        } else {
            let current = app.ai.selected_conn().map(|c| c.model.clone()).unwrap_or_default();
            egui::ComboBox::from_id_salt("ai-model").selected_text(if current.is_empty() { "choose…" } else { current.as_str() }).show_ui(ui, |ui| {
                for m in &app.ai.models {
                    ui.selectable_value(&mut app.ai.connections[app.ai.selected].model, m.clone(), m);
                }
            });
        }
        let r = ui.add_enabled(!fetching && app.ai.selected_conn().is_some(), egui::Button::new("⟳"));
        if r.on_hover_text("Fetch the provider's model list").clicked() {
            if let Some(conn) = app.ai.selected_conn().cloned() {
                app.ai.models_job = Some(Job::spawn(move || localpdf_ai::list_models(&conn)));
            }
        }
    });
}

/// The conversation, scrolled to the bottom.
fn chat_area(app: &mut LocalPDFApp, ui: &mut egui::Ui, t: &Tokens) {
    ui.horizontal(|ui| {
        if ui.selectable_label(!app.ai.whole_document, "Current page").clicked() {
            app.ai.whole_document = false;
        }
        if ui.selectable_label(app.ai.whole_document, "Whole document").clicked() {
            app.ai.whole_document = true;
        }
        if ui.button("Summarize").clicked() {
            summarize(app);
        }
        let ocr = ui.add_enabled(app.ai.ocr_job.is_none(), egui::Button::new("🔍 OCR (AI) → save in file"));
        if ocr.on_hover_text("A vision model reads this page; the text is saved into the PDF as an invisible layer, so it stays forever").clicked() {
            ocr_page(app);
        }
        if app.ai.ocr_job.is_some() {
            ui.add(egui::Spinner::new().size(14.0));
        }
    });
    ui.add_space(4.0);
    egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
        if app.ai.chat.is_empty() {
            ui.label(egui::RichText::new("Ask about the document, or press Summarize.").color(t.text_faint));
        }
        for turn in &app.ai.chat {
            match turn {
                Turn::User(text) => {
                    ui.label(egui::RichText::new("You").color(t.text_faint).small());
                    ui.label(egui::RichText::new(text).font(theme::regular(13.0)));
                }
                Turn::Assistant(text) => {
                    ui.separator();
                    ui.label(egui::RichText::new("Assistant").color(t.text_faint).small());
                    ui.label(egui::RichText::new(text).font(theme::regular(13.0)));
                }
                Turn::Failed(text) => {
                    ui.label(egui::RichText::new(format!("✗ {text}")).color(error_red()).small());
                }
            }
            ui.add_space(4.0);
        }
        if app.ai.job.is_some() {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().size(14.0));
                ui.label(egui::RichText::new("thinking…").color(t.text_faint));
            });
        }
    });
}

/// The input box and Send.
fn input_row(app: &mut LocalPDFApp, ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
        let busy = app.ai.job.is_some();
        if ui.add_enabled(!busy, egui::Button::new("Send")).clicked() {
            send(app);
        }
        let field = egui::TextEdit::singleline(&mut app.ai.input).hint_text("Ask about the document…").desired_width(f32::INFINITY).lock_focus(true);
        let response = ui.add(field);
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !busy {
            send(app);
        }
    });
}
