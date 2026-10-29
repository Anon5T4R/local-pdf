//! The AI assistant window in the real shell: it opens with its sections, and a request that
//! cannot run (no connection) answers with a visible failed turn instead of silence.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use localpdf_ui_egui::LocalPDFApp;

fn harness() -> Harness<'static, LocalPDFApp> {
    let bytes = include_bytes!("../data/form.pdf").to_vec();
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(move |_cc| {
        let mut app = LocalPDFApp::new();
        app.set_option("language", "en").unwrap();
        app.set_option("theme", "dark").unwrap();
        app.open_bytes("form.pdf", None, bytes.clone()).unwrap();
        app
    });
    crate::support::settle(&mut h);
    h
}

#[test]
fn ai_window_opens_and_reports_why_it_cannot_answer() {
    let mut h = harness();
    assert!(h.state_mut().execute("tools.ai"));
    h.run_steps(5);
    assert!(h.state().ai.open, "the window opened");

    // A question with no connection configured: the turn after it says why, visibly,
    // instead of the request disappearing without an answer.
    // Hermetic: the app reloads the user's saved connections whenever the list is empty, which
    // could make a request ready. A stub connection without a key is ready for nothing, so the
    // failure turn is what we assert — and no request may leave the machine.
    h.state_mut().ai.connections = vec![localpdf_ui_egui::ai_ui::Connection {
        id: "hermetic".into(),
        provider: "gemini".into(),
        label: "Test".into(),
        base_url: String::new(),
        model: String::new(),
    }];
    h.state_mut().ai.input = "What is this document?".into();
    h.get_by_label("Send").click();
    h.run_steps(3);
    let chat = &h.state().ai.chat;
    assert_eq!(chat.len(), 1, "{chat:?}");
    assert!(format!("{:?}", chat[0]).contains("Failed"), "{chat:?}");
    // The typed question stays in the field: the failure took nothing away.
    assert_eq!(h.state().ai.input, "What is this document?");
}

/// The AI commands (palette, menus, the tool panel's entries) open the window and run their
/// action; the tool-catalog entries are not dead references anymore.
#[test]
fn ai_commands_open_the_window_and_act() {
    let mut h = harness();
    assert!(h.state_mut().execute("tools.ai"));
    h.run_steps(2);
    assert!(h.state().ai.open, "the command opened the window");
    assert!(h.state_mut().execute("tools.ai"));
    h.run_steps(2);
    assert!(!h.state().ai.open, "the command toggles it closed again");

    // Hermetic: a stub connection without a key, so the action runs without leaving the machine.
    h.state_mut().ai.connections = vec![localpdf_ui_egui::ai_ui::Connection {
        id: "hermetic".into(),
        provider: "gemini".into(),
        label: "Test".into(),
        base_url: String::new(),
        model: String::new(),
    }];
    assert!(h.state_mut().execute("ai.summary"));
    h.run_steps(2);
    assert!(h.state().ai.open, "summarize opened the window");
    assert_eq!(h.state().ai.chat.len(), 1, "{:?}", h.state().ai.chat);
    assert!(format!("{:?}", h.state().ai.chat[0]).contains("Failed"), "why it cannot run, visibly");
}

/// Not run with the suite: writes `target/ai-window.png` for a visual check of the window.
#[test]
#[ignore = "visual check helper"]
fn ai_window_screenshot() {
    let _gpu = crate::gpu();
    let mut h = harness();
    assert!(h.state_mut().execute("tools.ai"));
    h.state_mut().ai.chat = vec![
        localpdf_ui_egui::ai_ui::Turn::User("What is this document?".into()),
        localpdf_ui_egui::ai_ui::Turn::Assistant("It is a form: fields for a name, a city and a signature.".into()),
    ];
    h.run_steps(10);
    // Hovering the assistant turn: what the text looks like then (cursor and any colour change).
    let at = h.get_by_label("It is a form: fields for a name, a city and a signature.").rect().center();
    h.hover_at(at);
    h.run_steps(2);
    let image = h.render().unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ai-window.png");
    image.save(&path).unwrap();
    println!("wrote {}", path.display());
}
