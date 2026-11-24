//! The AI assistant: the suite's chat panel (`craft-ai-ui`) over this app's BYOK client, plus OCR
//! by a vision model whose text is saved into the file as an invisible layer.
//!
//! The experience - the roll, the composer, the strip, the connections form and what actually goes
//! to the model - is `craft-ai-ui`, shared with the other apps of the suite. What lives here is the
//! app's side of the contract: the system prompt, the document (or the page) as context, copying an
//! answer out, and driving the shared `local-ai` client with what the panel hands over.

use std::sync::Arc;

use crate::LocalPDFApp;
use crate::widgets;
use craft_ai_ui as ui;
pub use localpdf_ai::Connection;
use localpdf_ai::{Job, Stream};

/// How much document text one chat request may carry (about 30k tokens of English).
const CONTEXT_LIMIT: usize = 120_000;

/// The whole request budget: the turns plus the document.
const BUDGET_CHARS: usize = 160_000;

/// What the panel holds between frames.
pub struct AiState {
    pub open: bool,
    pub settings_open: bool,
    panel: ui::ChatPanel,
    strip: ui::StatusStrip,
    connections: ui::ConnectionsPanel,
    /// The connection in use, loaded from the suite's store.
    connection: Option<Connection>,
    /// Whether the store was read already (once per state).
    loaded: bool,
    stream: Option<Stream>,
    /// The models the provider answered with, and the fetch that is filling them.
    models_job: Option<Job<Vec<String>>>,
    /// The OCR-by-AI job: (page, the lines it read).
    pub ocr_job: Option<Job<(usize, Vec<String>)>>,
    /// Whole document or just the current page as the chat's context.
    pub whole_document: bool,
    /// What the context was built from (document id, page, whole/one page): the document's text is
    /// only re-read when one of them changes.
    context_key: Option<(localpdf_engine::DocId, usize, bool)>,
    /// The last error outside the panel (the connections file, the keychain, OCR).
    pub error: Option<String>,
}

impl Default for AiState {
    fn default() -> Self {
        let mut panel = ui::ChatPanel::default();
        panel.set_policy(ui::WindowPolicy { turns: ui::Turns::LastN(8), total_budget_chars: BUDGET_CHARS });
        Self {
            open: false,
            settings_open: false,
            panel,
            strip: ui::StatusStrip::default(),
            connections: ui::ConnectionsPanel::default(),
            connection: None,
            loaded: false,
            stream: None,
            models_job: None,
            ocr_job: None,
            whole_document: false,
            context_key: None,
            error: None,
        }
    }
}

impl AiState {
    /// Load the saved connection once (the first one: the suite's store keeps a list).
    pub fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        match Connection::load_all() {
            Ok(all) => self.connection = all.into_iter().next(),
            Err(e) => self.error = Some(e),
        }
    }

    /// The connection in use: the settings dialog saves it, and a test replaces it with a stub so
    /// no request leaves the machine. Once set, the store is not read again.
    pub fn use_connection(&mut self, connection: Option<Connection>) {
        self.connection = connection;
        self.loaded = true;
    }

    /// The connection the calls use, or why there is none.
    fn ready_connection(&self) -> Result<Connection, String> {
        let conn = self.connection.clone().ok_or_else(|| tl!("Configure a connection first.").to_string())?;
        if !conn.has_key() {
            return Err(tl!("The connection has no API key").to_string());
        }
        if conn.model.trim().is_empty() {
            return Err(tl!("Choose a model first").to_string());
        }
        Ok(conn)
    }

    /// Whether the panel has somewhere to send a question.
    fn ready(&self) -> bool {
        self.connection.as_ref().is_some_and(|conn| conn.has_key() && !conn.model.trim().is_empty())
    }

    fn model(&self) -> &str {
        self.connection.as_ref().map(|conn| conn.model.as_str()).unwrap_or_default()
    }

    /// Whether anything is in flight (the tests check that a failed action leaves nothing running).
    #[cfg(test)]
    fn busy(&self) -> bool {
        self.stream.is_some() || self.panel.busy()
    }

    /// Stops a streaming answer; the panel keeps what arrived.
    fn stop(&mut self) {
        if let Some(stream) = &self.stream {
            stream.cancel();
        }
        self.stream = None;
        self.strip.finish();
        self.panel.stopped();
    }

    /// Closes the panel, stopping whatever was running.
    fn close(&mut self) {
        self.stop();
        self.open = false;
    }

    /// Stores the connection (key in the keychain) and keeps it in use.
    fn save(&mut self, draft: ui::ConnectionDraft, key: String) {
        let conn = connection_of(&draft);
        if let Err(e) = conn.set_key(&key) {
            self.connections.failed(e);
            return;
        }
        match Connection::save_all(std::slice::from_ref(&conn)) {
            Ok(()) => {
                self.connection = Some(conn);
                self.panel.clear();
                self.connections.saved();
                self.settings_open = false;
            }
            Err(e) => self.connections.failed(e),
        }
    }

    /// Asks the provider for its model list, on a worker thread.
    fn fetch_models(&mut self, draft: ui::ConnectionDraft, key: String) {
        let conn = connection_of(&draft);
        if let Err(e) = conn.set_key(&key) {
            self.connections.failed(e);
            return;
        }
        self.models_job = Some(Job::spawn(move || localpdf_ai::list_models(&conn)));
    }

    /// The connections form: the suite's dialog, with this app's store and keychain behind it.
    fn settings(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            self.connections.open = false;
            return;
        }
        if !self.connections.open {
            self.connections.set_presets(presets());
            self.connections.open_with(self.connection.as_ref().map(draft_of));
        }
        let labels = form_labels();
        match self.connections.show(ctx, &labels) {
            ui::ConnectionsIntent::None => {}
            ui::ConnectionsIntent::Cancel => {
                self.connections.open = false;
                self.settings_open = false;
            }
            ui::ConnectionsIntent::Save(draft, key) => self.save(draft, key),
            ui::ConnectionsIntent::LoadModels(draft, key) => self.fetch_models(draft, key),
        }
    }
}

/// Poll a job slot once, taking the finished job with its result.
fn take_done<T: Send + 'static>(slot: &mut Option<Job<T>>) -> Option<Result<T, String>> {
    let result = slot.as_mut().and_then(|j| j.poll());
    if result.is_some() {
        *slot = None;
    }
    result
}

/// What arrived since the last frame: the model list, the streamed answer and the OCR run.
fn poll(app: &mut LocalPDFApp) {
    if let Some(result) = take_done(&mut app.ai.models_job) {
        match result {
            Ok(models) => app.ai.connections.models = models,
            Err(e) => app.ai.connections.failed(e),
        }
    }
    if let Some(stream) = &app.ai.stream {
        let (pieces, done) = stream.poll();
        for piece in pieces {
            app.ai.strip.push(&piece);
            app.ai.panel.push(&piece);
        }
        if let Some(result) = done {
            app.ai.strip.finish();
            app.ai.panel.finish(result);
            app.ai.stream = None;
        }
    }
    if let Some(result) = take_done(&mut app.ai.ocr_job) {
        match result {
            Ok((page, lines)) => match ocr_lines_into_file(app, page, &lines) {
                Ok(words) => app.notify_fmt("AI text recognition: {words} words saved into the file", &[("words", &words.to_string())]),
                Err(e) => app.ai.error = Some(e),
            },
            Err(e) => app.ai.error = Some(e),
        }
    }
}

/// The tool panel's AI entries and the palette's AI commands: open the panel and run the action.
pub fn open_and_summarize(app: &mut LocalPDFApp) {
    app.ai.open = true;
    app.ai.load();
    prepare(app);
    if app.ai.panel.ask(tl!("Summarize this document: what it is, its main points and any actions it asks of the reader.")) {
        start(app);
    }
}

/// Asking opens the panel: the question is typed where the answer will appear.
pub fn open_and_ask(app: &mut LocalPDFApp) {
    app.ai.open = true;
    app.ai.load();
    prepare(app);
}

/// OCR opens the panel (its progress and result land there) and reads the current page.
pub fn open_and_ocr(app: &mut LocalPDFApp) {
    app.ai.open = true;
    ocr_page(app);
}

/// Sets what every request carries: the system prompt and the document side of the request. The
/// document's text is only read again when the page or the scope changes (a big PDF is megabytes).
fn prepare(app: &mut LocalPDFApp) {
    app.ai.panel.set_system(SYSTEM_PROMPT);
    let page = current_page(app);
    let key = app.active_ids().map(|(_, id)| (id, page, app.ai.whole_document));
    if key.is_some() && key == app.ai.context_key {
        return;
    }
    app.ai.context_key = key;
    let context = context_source(app);
    app.ai.panel.set_context(context);
}

/// Hands the panel's request to the shared client.
fn start(app: &mut LocalPDFApp) {
    let Some(lines) = app.ai.panel.take_request() else { return };
    let conn = match app.ai.ready_connection() {
        Ok(conn) => conn,
        Err(e) => {
            app.ai.panel.finish(Err(e));
            return;
        }
    };
    let messages = lines
        .into_iter()
        .map(|line| match line.role {
            ui::Role::System => localpdf_ai::Msg::System(line.text),
            ui::Role::User => localpdf_ai::Msg::User(line.text),
            ui::Role::Assistant => localpdf_ai::Msg::Assistant(line.text),
        })
        .collect();
    app.ai.strip.begin();
    app.ai.stream = Some(Stream::start(conn, messages));
}

/// The system prompt: the same for every turn of the conversation (model-facing, not UI).
const SYSTEM_PROMPT: &str = "You help with a PDF the user has open in Local PDF. Answer in the user's \
     language; be concise; quote the document when that helps. The document's text arrives as \
     context; you never change the file yourself.";

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
            if out.len() + line.text.len() > CONTEXT_LIMIT {
                out.push_str("\n[the document continues; the text was cut to fit one request]");
                break 'pages;
            }
            out.push_str(&line.text);
            out.push('\n');
        }
    }
    out
}

/// The document as the panel's context, within the budget.
fn context_source(app: &mut LocalPDFApp) -> Option<ui::ContextSource> {
    let text = context_text(app);
    if text.trim().is_empty() {
        return None;
    }
    let name = doc_name(app);
    let label = if app.ai.whole_document {
        i18n_fmt(tl!("{name} (whole document)"), &[("name", &name)])
    } else {
        i18n_fmt(tl!("{name} (this page)"), &[("name", &name)])
    };
    Some(ui::ContextSource::new(ui::ContextKind::Document, label, text, CONTEXT_LIMIT, ui::Side::Head))
}

/// The open document's name, for the context chip.
fn doc_name(app: &mut LocalPDFApp) -> String {
    app.active_ids().and_then(|(_, id)| app.session.get(id).map(|d| d.name.clone())).unwrap_or_else(|| "document".into())
}

/// OCR by AI: render the page, ask a vision model to transcribe it, one line per text line.
pub fn ocr_page(app: &mut LocalPDFApp) {
    let Some((page, id)) = app.active_ids() else { return };
    let conn = match app.ai.ready_connection() {
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

/// The panel beside the document. Drawn from the app's layout, before the page.
pub fn panel(app: &mut LocalPDFApp, ui: &mut egui::Ui) {
    if !app.ai.open {
        return;
    }
    app.ai.load();
    prepare(app);
    let model = app.ai.model().to_string();
    let ready = app.ai.ready();
    egui::Panel::right("ai-chat").resizable(true).default_size(360.0).max_size(560.0).show(ui, |ui| {
        let stop = app.ai.strip.show(ui, ui::StripLabels { stop: tl!("Stop"), dismiss: tl!("Dismiss") });
        if stop == ui::StripAction::Stop {
            app.ai.stop();
        }
        actions_row(app, ui);
        if let Some(error) = app.ai.error.take() {
            ui.label(egui::RichText::new(error).small().color(ui.visuals().warn_fg_color));
        }
        ui.separator();
        let suggestions = [tl!("Summarize the document").to_string()];
        let labels = chat_labels(&model, ready, &suggestions);
        match app.ai.panel.show(ui, &labels) {
            ui::ChatAction::None => {}
            ui::ChatAction::Send => start(app),
            ui::ChatAction::Retry => {
                if app.ai.panel.retry() {
                    start(app);
                }
            }
            ui::ChatAction::Stop => app.ai.stop(),
            ui::ChatAction::Clear => app.ai.panel.clear(),
            ui::ChatAction::Insert => {
                if let Some(answer) = app.ai.panel.last_answer().map(str::to_owned) {
                    ui.ctx().copy_text(answer);
                    app.notify_tr("The answer was copied.");
                }
            }
            ui::ChatAction::Settings => app.ai.settings_open = true,
            ui::ChatAction::Close => app.ai.close(),
        }
    });
}

/// What the chat answers about, and the two one-shot actions, as the app's pills.
fn actions_row(app: &mut LocalPDFApp, ui: &mut egui::Ui) {
    ui.add_space(2.0);
    ui.horizontal_wrapped(|ui| {
        widgets::radio_value(ui, &mut app.ai.whole_document, false, tl!("Current page"));
        widgets::radio_value(ui, &mut app.ai.whole_document, true, tl!("Whole document"));
        ui.separator();
        if widgets::icon_pill(ui, craft_icons::Icon::Ai.name(), tl!("Summarize"), false).clicked() {
            open_and_summarize(app);
        }
        let ocr_busy = app.ai.ocr_job.is_some();
        let ocr = ui.add_enabled_ui(!ocr_busy, |ui| widgets::icon_pill(ui, "scan-text", tl!("OCR (AI) → save in file"), false)).inner;
        if ocr
            .on_hover_text(tl!("A vision model reads this page; its text is saved into the PDF as an invisible layer, so it stays forever"))
            .clicked()
        {
            ocr_page(app);
        }
        if ocr_busy {
            ui.add(egui::Spinner::new().size(14.0));
        }
    });
    ui.separator();
}

/// The connections dialog and the requests in flight: drawn where the other dialogs are.
pub fn show(app: &mut LocalPDFApp, ctx: &egui::Context) {
    poll(app);
    app.ai.settings(ctx);
}

/// The chat labels, in the app's language.
fn chat_labels<'a>(model: &'a str, ready: bool, suggestions: &'a [String]) -> ui::ChatLabels<'a> {
    ui::ChatLabels {
        title: tl!("AI assistant"),
        not_configured: if ready { "" } else { tl!("Configure a connection first.") },
        configure: tl!("Settings"),
        settings: tl!("Settings"),
        clear: tl!("Clear"),
        close: tl!("Close"),
        send: tl!("Send"),
        stop: tl!("Stop"),
        insert: tl!("Copy"),
        retry: tl!("Retry"),
        dismiss: tl!("Dismiss"),
        empty: tl!("Ask about the document."),
        input_hint: tl!("Write to the assistant."),
        chip: &chip,
        model,
        suggestions,
        to_end: tl!("To the end"),
    }
}

/// The context chip: what actually goes to the model.
fn chip(info: &ui::ContextInfo) -> String {
    i18n_fmt(
        tl!("reading: {label} · {sent} of {total} chars · {kept} of {turns} turns"),
        &[
            ("label", &info.label),
            ("sent", &info.sent.to_string()),
            ("total", &info.total.to_string()),
            ("kept", &info.kept.to_string()),
            ("turns", &info.turns.to_string()),
        ],
    )
}

fn form_labels<'a>() -> ui::ConnectionsLabels<'a> {
    ui::ConnectionsLabels {
        title: tl!("AI connections"),
        provider: tl!("Provider"),
        base_url: tl!("Base URL"),
        model: tl!("Model"),
        api_key: tl!("API key"),
        list_models: tl!("Fetch models"),
        save: tl!("Save"),
        cancel: tl!("Cancel"),
        hint_url: tl!("Empty uses the provider's own address."),
        hint_key: tl!("The key stays in the operating system's keychain."),
        close: tl!("Close"),
        needs_key: tl!("Paste the API key first: this provider asks for one."),
        needs_url: tl!("Choose a provider or type its address first."),
        needs_model: tl!("Choose a model first (Fetch models lists them)."),
    }
}

fn presets() -> Vec<ui::Preset> {
    localpdf_ai::PROVIDERS.iter().map(|p| ui::Preset { id: p.id.to_owned(), label: p.label.to_owned(), base_url: p.base_url.to_owned() }).collect()
}

fn draft_of(conn: &Connection) -> ui::ConnectionDraft {
    ui::ConnectionDraft { provider: conn.provider.clone(), base_url: conn.base_url.clone(), model: conn.model.clone() }
}

fn connection_of(draft: &ui::ConnectionDraft) -> Connection {
    let label = localpdf_ai::provider(&draft.provider).map(|p| p.label.to_owned()).unwrap_or_else(|| draft.provider.clone());
    Connection {
        id: localpdf_ai::APP.id.to_lowercase(),
        provider: draft.provider.clone(),
        label,
        base_url: draft.base_url.clone(),
        model: draft.model.clone(),
    }
}

fn i18n_fmt(template: &str, args: &[(&str, &str)]) -> String {
    crate::i18n::fmt(template, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> LocalPDFApp {
        LocalPDFApp::default()
    }

    /// The context is the page or the whole document, with one budget for everything that leaves.
    #[test]
    fn the_document_is_the_panels_context() {
        let mut a = app();
        let policy = a.ai.panel.policy();
        assert_eq!(policy.turns, ui::Turns::LastN(8));
        assert_eq!(policy.total_budget_chars, BUDGET_CHARS);
        a.ai.whole_document = true;
        // No document open: no context, and the panel says so through its own empty state.
        assert!(context_source(&mut a).is_none());
    }

    /// A tool entry's action goes through the same path as the composer: the panel records the
    /// failed request, so Retry brings it back.
    #[test]
    fn an_action_without_a_connection_lands_in_the_roll_as_a_failed_request() {
        let mut a = app();
        // Hermetic: never read the user's store or keychain from a test.
        a.ai.loaded = true;
        open_and_summarize(&mut a);
        assert!(a.ai.open, "the panel opens");
        assert!(!a.ai.busy(), "no request is running");
        assert!(a.ai.panel.retry(), "the request is kept for Retry");
    }

    /// The key the ignored real-model test uses: the environment first, then the suite's
    /// `ai-test.env` in `%USERPROFILE%\Segredos` (never printed).
    fn real_key() -> String {
        if let Some(key) = std::env::var("OPENROUTER_API_KEY").ok().filter(|key| !key.trim().is_empty()) {
            return key.trim().to_string();
        }
        let path = std::path::PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default()).join("Segredos").join("ai-test.env");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("OPENROUTER_API_KEY or {}: {error}", path.display()));
        for line in text.lines() {
            let line = line.trim();
            if let Some(value) = line.strip_prefix("OPENROUTER_API_KEY=") {
                return value.trim().trim_matches('"').to_string();
            }
        }
        panic!("OPENROUTER_API_KEY is in neither the environment nor {}", path.display());
    }

    /// The real-model end-to-end of the chat path: network and a key, so `#[ignore]`. A free model
    /// works with the suite's zero-credit key. Run with
    /// `AI_TEST_MODEL=nvidia/nemotron-3-ultra-550b-a55b:free cargo test -p localpdf-ui-egui --lib ai_ui::tests::a_real_model_answers -- --ignored --nocapture`.
    #[test]
    #[ignore = "network + a real provider key"]
    fn a_real_model_answers() {
        let key = real_key();
        let conn = Connection {
            id: "real".into(),
            provider: "openrouter".into(),
            label: "OpenRouter".into(),
            base_url: String::new(),
            model: std::env::var("AI_TEST_MODEL").unwrap_or_else(|_| "nvidia/nemotron-3-ultra-550b-a55b:free".into()),
        };
        let history =
            vec![localpdf_ai::Msg::System(SYSTEM_PROMPT.to_string()), localpdf_ai::Msg::User("Em uma frase, o que é um arquivo PDF?".into())];
        let mut pieces = 0;
        let whole = localpdf_ai::chat_stream_with_key(&conn, &key, &history, &mut |_: &str| pieces += 1).expect("a real answer");
        println!("--- answer ({pieces} pieces) ---\n{whole}\n--------------");
        assert!(whole.split_whitespace().count() > 5, "real content, not a stub: {whole}");
        assert!(pieces > 1, "the answer streamed in pieces");
    }
}
