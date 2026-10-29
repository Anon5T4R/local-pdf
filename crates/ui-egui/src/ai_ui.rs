//! The AI assistant: a movable window with the user's own connections (BYOK), the model picker
//! that reads each provider's `/models`, chat about the open document, and OCR by a vision
//! model whose text is saved into the file as an invisible layer.
//!
//! Nothing here talks to a LocalPDF server: there is none. Every call goes straight to the
//! provider the user configured, with the key the OS keychain holds.
//!
//! The window follows the app's dialog kit: `widgets::begin_dialog` field fills and borders,
//! section titles, icon buttons with washes, pill buttons, and translated labels.

use std::sync::Arc;

use crate::LocalPDFApp;
use crate::theme::{self, Tokens};
use crate::{icons, widgets};
pub use localpdf_ai::Connection;
use localpdf_ai::{Job, Msg, PROVIDERS, provider};

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

/// Light markdown style for an assistant answer run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MdStyle {
    Plain,
    Bold,
    Italic,
    Code,
}

/// A run of an answer's line, cut where its marking changes.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MdSeg {
    text: String,
    style: MdStyle,
}

/// One line of an answer: ordinary text, a list item (`- `) or a heading (`# `).
#[derive(Clone, Debug, PartialEq, Eq)]
enum MdLine {
    Text(Vec<MdSeg>),
    Item(Vec<MdSeg>),
    Heading(Vec<MdSeg>),
}

/// The light markdown over assistant answers: `- ` opens a list item, a run of `#`s and a space
/// a heading, and the inline pairs (`**bold**`, `*italic*`, backticks) cut their runs apart
/// ([`md_segs`]). Just enough to read an answer at a glance: links, tables and fences are none
/// of its business, and the user's turns are always drawn as they typed them.
fn md(text: &str) -> Vec<MdLine> {
    text.split('\n')
        .map(|line| {
            let hashes = line.chars().take_while(|&c| c == '#').count();
            // `#` is ASCII, so the character count of the opening run is its width in bytes.
            if hashes > 0
                && let Some(rest) = line[hashes..].strip_prefix(' ')
            {
                return MdLine::Heading(md_segs(rest));
            }
            if let Some(rest) = line.strip_prefix("- ") {
                return MdLine::Item(md_segs(rest));
            }
            MdLine::Text(md_segs(line))
        })
        .collect()
}

/// The runs of one line: `**bold**`, `*italic*` and `` `code` ``, plain text apart from those.
/// A marker without its pair later on the line is literal text, so `2 * 3` draws as typed, and
/// runs don't nest: each opener closes at the first pair that answers it.
fn md_segs(line: &str) -> Vec<MdSeg> {
    let cs: Vec<char> = line.chars().collect();
    let mut out: Vec<MdSeg> = Vec::new();
    let mut plain = String::new();
    let mut i = 0;
    while i < cs.len() {
        let run = if cs[i] == '`'
            && let Some(end) = (i + 1..cs.len()).find(|&k| cs[k] == '`')
        {
            Some((MdStyle::Code, i + 1, end, end + 1))
        } else if cs[i] == '*'
            && let Some(run) = md_star_pair(&cs, i)
        {
            Some(run)
        } else {
            None
        };
        match run {
            Some((style, start, end, next)) => {
                if !plain.is_empty() {
                    out.push(MdSeg { text: std::mem::take(&mut plain), style: MdStyle::Plain });
                }
                if start < end {
                    out.push(MdSeg { text: cs[start..end].iter().collect(), style });
                }
                i = next;
            }
            None => {
                plain.push(cs[i]);
                i += 1;
            }
        }
    }
    if !plain.is_empty() {
        out.push(MdSeg { text: plain, style: MdStyle::Plain });
    }
    out
}

/// What the `*` at index `i` opens: a `**` pair (bold) with where it closes, or the next single
/// `*` (italic). `None` when the rest of the line has no closer, leaving the `*` literal text.
fn md_star_pair(cs: &[char], i: usize) -> Option<(MdStyle, usize, usize, usize)> {
    if cs.get(i + 1) == Some(&'*') {
        let end = (i + 2..cs.len()).find(|&k| cs[k] == '*' && cs.get(k + 1) == Some(&'*'))?;
        return Some((MdStyle::Bold, i + 2, end, end + 2));
    }
    let end = (i + 1..cs.len()).find(|&k| cs[k] == '*')?;
    Some((MdStyle::Italic, i + 1, end, end + 1))
}

/// An assistant turn drawn as light markdown: one wrapped block with the [`MdLine`]s back in
/// order — a heading a size up in the semibold face, list items behind a dim bullet, bold in
/// the house semibold, italic slanted, inline code in the monospace face over the house accent.
/// Plain prose wraps as one run, like any label.
fn show_assistant_turn(ui: &mut egui::Ui, text: &str, t: &Tokens) {
    let mut job = egui::text::LayoutJob::default();
    for (n, line) in md(text).into_iter().enumerate() {
        if n > 0 {
            job.append("\n", 0.0, egui::text::TextFormat::simple(theme::regular(13.0), t.text));
        }
        let (segs, heading) = match &line {
            MdLine::Item(segs) => {
                job.append("•  ", 0.0, egui::text::TextFormat::simple(theme::regular(13.0), t.text_muted));
                (segs, false)
            }
            MdLine::Heading(segs) => (segs, true),
            MdLine::Text(segs) => (segs, false),
        };
        for seg in segs {
            if seg.text.is_empty() {
                continue;
            }
            let format = match (heading, seg.style) {
                (_, MdStyle::Code) => egui::text::TextFormat::simple(egui::FontId::monospace(12.0), t.accent_text),
                (_, MdStyle::Bold) => egui::text::TextFormat::simple(theme::semibold(if heading { 15.0 } else { 13.0 }), t.text),
                (true, _) => egui::text::TextFormat::simple(theme::semibold(15.0), t.text),
                (false, MdStyle::Italic) => {
                    egui::text::TextFormat { font_id: theme::regular(13.0), color: t.text, italics: true, ..egui::text::TextFormat::default() }
                }
                (false, MdStyle::Plain) => egui::text::TextFormat::simple(theme::regular(13.0), t.text),
            };
            job.append(&seg.text, 0.0, format);
        }
    }
    if job.text.is_empty() {
        return;
    }
    job.wrap.max_width = ui.available_width();
    let galley = ui.painter().layout_job(job);
    let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    ui.painter().galley(rect.min, galley, t.text);
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

/// Poll a job slot once, taking the finished job with its result. One `try_recv` only:
/// polling twice eats the answer, so a "finished?" check followed by a read would drop it.
fn take_done<T: Send + 'static>(slot: &mut Option<Job<T>>) -> Option<Result<T, String>> {
    let result = slot.as_mut().and_then(|j| j.poll());
    if result.is_some() {
        *slot = None;
    }
    result
}

/// Poll the background jobs; called every frame before the window draws.
fn poll(app: &mut LocalPDFApp) {
    if let Some(result) = take_done(&mut app.ai.models_job) {
        match result {
            Ok(models) => app.ai.models = models,
            Err(e) => app.ai.error = Some(e),
        }
    }
    if let Some(result) = take_done(&mut app.ai.job) {
        match result {
            Ok(answer) => app.ai.chat.push(Turn::Assistant(answer)),
            Err(e) => app.ai.chat.push(Turn::Failed(e)),
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

/// The tool panel's AI entries and the palette's AI commands: open the window and run the one
/// action. Everything the entries do, the window also offers by hand.
pub fn open_and_summarize(app: &mut LocalPDFApp) {
    app.ai.open = true;
    summarize(app);
}

/// Asking opens the window: the question is typed where the answer will appear.
pub fn open_and_ask(app: &mut LocalPDFApp) {
    app.ai.open = true;
}

/// OCR opens the window (its progress and result land there) and reads the current page.
pub fn open_and_ocr(app: &mut LocalPDFApp) {
    app.ai.open = true;
    ocr_page(app);
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
    if app.ai.input.trim().is_empty() {
        return;
    }
    let conn = match app.ai.ready() {
        Ok(c) => c,
        Err(e) => {
            // The typed question stays in the field: only the answer is missing, not the input.
            app.ai.chat.push(Turn::Failed(e));
            return;
        }
    };
    let input = std::mem::take(&mut app.ai.input);
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
    egui::Window::new(tl!("AI assistant"))
        .id(egui::Id::unique("ai_window"))
        .order(egui::Order::Foreground)
        .open(&mut open)
        .collapsible(false)
        .default_width(520.0)
        .default_height(620.0)
        .show(ctx, |ui| {
            widgets::begin_dialog(ui);
            connection_section(app, ui);
            model_section(app, ui);
            actions_row(app, ui);
            error_line(app, ui);
            chat_area(app, ui, &t);
            input_row(app, ui);
            ui.add_space(4.0);
        });
    app.ai.open = open;
}

fn error_red() -> egui::Color32 {
    egui::Color32::from_rgb(0xE5, 0x48, 0x4D)
}

/// The last error, shown once: the next frame clears it.
fn error_line(app: &mut LocalPDFApp, ui: &mut egui::Ui) {
    if let Some(e) = app.ai.error.take() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(e).color(error_red()).small());
    }
}

/// The connection combo, its key/add/delete icon buttons, the key editor, and the details.
fn connection_section(app: &mut LocalPDFApp, ui: &mut egui::Ui) {
    widgets::section_title(ui, tl!("Connection"));
    let labels: Vec<String> = app.ai.connections.iter().map(|c| format!("{} — {}", c.label, provider_label(&c.provider))).collect();
    ui.horizontal(|ui| {
        let chosen = labels.get(app.ai.selected).cloned().unwrap_or_else(|| tl!("Add a connection…").to_owned());
        let shown = widgets::fit_line(&chosen, 210.0, |s| widgets::text_width(ui, s, &theme::regular(13.0)));
        egui::ComboBox::from_id_salt("ai-connection").selected_text(shown).show_ui(ui, |ui| {
            for (i, label) in labels.iter().enumerate() {
                ui.selectable_value(&mut app.ai.selected, i, label);
            }
        });
        let keyed = app.ai.selected_conn().is_some_and(|c| c.has_key());
        let key_tip = if keyed { tl!("Edit the API key") } else { tl!("Set the API key") };
        let key_icon = if keyed { "lock" } else { "lock-open" };
        if icons::button(ui, key_icon, 26.0, false, key_tip).clicked() {
            app.ai.editing_key = !app.ai.editing_key;
            app.ai.key_draft.clear();
        }
        if icons::button(ui, "plus", 26.0, false, tl!("Add a connection")).clicked() {
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
        let has = app.ai.selected_conn().is_some();
        let del = ui.add_enabled_ui(has, |ui| icons::button(ui, "trash", 26.0, false, tl!("Delete this connection"))).inner;
        if del.clicked() {
            if let Some(c) = app.ai.selected_conn() {
                let _ = c.clear_key();
            }
            app.ai.connections.remove(app.ai.selected);
            app.ai.selected = app.ai.selected.saturating_sub(1);
            app.ai.models.clear();
            app.ai.save();
        }
    });
    if let Some(conn) = app.ai.selected_conn().cloned() {
        if app.ai.editing_key {
            ui.horizontal(|ui| {
                let field = widgets::line(&mut app.ai.key_draft)
                    .password(true)
                    .hint_text(tl!("API key (stays in the OS keychain)"))
                    .desired_width((ui.available_width() - 130.0).max(140.0));
                let response = ui.add(field);
                if widgets::pill_button(ui, tl!("Save key"), true).clicked()
                    || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    if let Err(e) = conn.set_key(app.ai.key_draft.trim()) {
                        app.ai.error = Some(e);
                    }
                    app.ai.key_draft.clear();
                    app.ai.editing_key = false;
                }
            });
        }
        ui.collapsing(tl!("Connection details"), |ui| {
            egui::Grid::new("ai-conn-details").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                ui.label(tl!("Provider"));
                let provider_before = conn.provider.clone();
                egui::ComboBox::from_id_salt("ai-provider").selected_text(provider_label(&conn.provider)).show_ui(ui, |ui| {
                    for p in PROVIDERS {
                        ui.selectable_value(&mut app.ai.connections[app.ai.selected].provider, p.id.to_owned(), p.label);
                    }
                });
                // Another provider answers `/models` with another list; the stale one must go.
                if app.ai.connections[app.ai.selected].provider != provider_before {
                    app.ai.models.clear();
                }
                ui.end_row();
                ui.label(tl!("Name"));
                ui.add(widgets::line(&mut app.ai.connections[app.ai.selected].label).desired_width(220.0));
                ui.end_row();
                ui.label(tl!("Base URL"));
                let hint = provider(&conn.provider).map(|p| p.base_url).unwrap_or("");
                ui.add(widgets::line(&mut app.ai.connections[app.ai.selected].base_url).hint_text(hint).desired_width(220.0));
                ui.end_row();
            });
            ui.add_space(2.0);
            if widgets::pill_button(ui, tl!("Save connection"), false).clicked() {
                app.ai.save();
            }
        });
    }
}

fn provider_label(id: &str) -> String {
    localpdf_ai::provider(id).map(|p| p.label.to_owned()).unwrap_or_else(|| id.to_owned())
}

/// The model picker (the provider's own list) and its refresh button.
fn model_section(app: &mut LocalPDFApp, ui: &mut egui::Ui) {
    widgets::section_title(ui, tl!("Model"));
    ui.horizontal(|ui| {
        let fetching = app.ai.models_job.is_some();
        if app.ai.models.is_empty() {
            // No list yet: a free-text model name still works.
            if app.ai.selected_conn().is_some() {
                let field = widgets::line(&mut app.ai.connections[app.ai.selected].model)
                    .hint_text(tl!("Model id, or fetch the list"))
                    .desired_width((ui.available_width() - 40.0).max(160.0));
                ui.add(field);
            }
        } else {
            let current = app.ai.selected_conn().map(|c| c.model.clone()).unwrap_or_default();
            let shown = widgets::fit_line(if current.is_empty() { tl!("Choose a model…") } else { current.as_str() }, 240.0, |s| {
                widgets::text_width(ui, s, &theme::regular(13.0))
            });
            egui::ComboBox::from_id_salt("ai-model").selected_text(shown).show_ui(ui, |ui| {
                for m in &app.ai.models {
                    ui.selectable_value(&mut app.ai.connections[app.ai.selected].model, m.clone(), m);
                }
            });
        }
        // The refresh slot: the spinner takes the button's place while the list is on its way.
        if fetching {
            ui.add(egui::Spinner::new().size(20.0));
        } else {
            let can_fetch = app.ai.selected_conn().is_some();
            let refresh = ui.add_enabled_ui(can_fetch, |ui| icons::button(ui, "rotate-cw", 26.0, false, tl!("Fetch the model list"))).inner;
            if refresh.clicked()
                && let Some(conn) = app.ai.selected_conn().cloned()
            {
                app.ai.models_job = Some(Job::spawn(move || localpdf_ai::list_models(&conn)));
            }
        }
    });
}

/// What the chat answers about, and the two one-shot actions, as the app's pills.
fn actions_row(app: &mut LocalPDFApp, ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        widgets::radio_value(ui, &mut app.ai.whole_document, false, tl!("Current page"));
        widgets::radio_value(ui, &mut app.ai.whole_document, true, tl!("Whole document"));
        ui.add_space(2.0);
        ui.separator();
        ui.add_space(2.0);
        if widgets::icon_pill(ui, "sparkles", tl!("Summarize"), false).clicked() {
            summarize(app);
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
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(2.0);
        let can_clear = !app.ai.chat.is_empty() && !app.ai.job.is_some();
        if ui.add_enabled_ui(can_clear, |ui| widgets::pill_button(ui, tl!("Clear"), false)).inner.clicked() {
            app.ai.chat.clear();
        }
    });
}

/// The conversation, anchored at the input: the newest turn sits just above it and the rest
/// scroll upward — a chat fills from the bottom of its box, not the top.
fn chat_area(app: &mut LocalPDFApp, ui: &mut egui::Ui, t: &Tokens) {
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(2.0);
    // The chat takes what is left of the window above the input row, within the window the user
    // sized; without the bound the scroll area asks for the whole screen and the window grows.
    let busy = app.ai.job.is_some();
    let scroll_height = (ui.available_height() - 54.0).max(120.0);
    egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).max_height(scroll_height).show(ui, |ui| {
        // Bottom-up: what is allocated first sits lowest. The busy line, then the newest turn,
        // then the older ones, then the empty state, stack upward from the input.
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            if busy {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(14.0));
                    ui.label(egui::RichText::new(tl!("thinking…")).color(t.text_faint));
                });
                ui.add_space(6.0);
            }
            for turn in app.ai.chat.iter().rev() {
                ui.add_space(6.0);
                match turn {
                    Turn::User(text) => {
                        // Bottom-up allocates upward: the text first, its eyebrow above it.
                        ui.label(egui::RichText::new(text).font(theme::regular(13.0)));
                        ui.label(egui::RichText::new(tl!("You")).color(t.text_faint).small());
                    }
                    Turn::Assistant(text) => {
                        show_assistant_turn(ui, text, t);
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(tl!("Assistant")).color(t.text_faint).small());
                        ui.separator();
                    }
                    Turn::Failed(text) => {
                        ui.label(egui::RichText::new(format!("✗ {text}")).color(error_red()).small());
                        ui.add_space(4.0);
                    }
                }
                ui.add_space(6.0);
            }
            if app.ai.chat.is_empty() && !busy {
                ui.label(egui::RichText::new(tl!("Ask about the document, or press Summarize.")).color(t.text_faint).small());
                ui.add_space(12.0);
            }
        });
    });
}

/// The input field and Send, as the window's primary action.
fn input_row(app: &mut LocalPDFApp, ui: &mut egui::Ui) {
    let busy = app.ai.job.is_some();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let field = widgets::line(&mut app.ai.input)
            .hint_text(tl!("Ask about the document…"))
            .desired_width((ui.available_width() - 96.0).max(120.0))
            .lock_focus(true);
        let response = ui.add(field);
        let entered = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !busy;
        let sent = ui.add_enabled_ui(!busy, |ui| widgets::icon_pill(ui, "send", tl!("Send"), true)).inner.clicked();
        if entered || sent {
            send(app);
        }
    });
}
