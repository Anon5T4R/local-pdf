//! The AI panel in the real shell: it opens beside the document, and a request that cannot run (no
//! connection) answers with a visible failed request instead of silence.

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

/// Hermetic: a stub connection with no key is ready for nothing, so the failure is what we assert
/// and no request may leave the machine (the user's store and keychain are never read).
fn stub_connection(h: &mut Harness<'static, LocalPDFApp>) {
    h.state_mut().ai.use_connection(Some(localpdf_ui_egui::ai_ui::Connection {
        id: "hermetic-test".into(),
        provider: "custom".into(),
        label: "Test".into(),
        base_url: String::new(),
        model: String::new(),
    }));
}

#[test]
fn ai_panel_opens_and_reports_why_it_cannot_answer() {
    let mut h = harness();
    assert!(h.state_mut().execute("tools.ai"));
    h.run_steps(5);
    assert!(h.state().ai.open, "the panel opened");

    stub_connection(&mut h);
    assert!(h.state_mut().execute("ai.summary"));
    h.run_steps(4);
    // The request failed and the panel shows the reason with Retry: not silence.
    h.get_by_label("Retry");
    assert!(h.state().ai.open, "the panel stays open with the failure");
}

/// The AI commands (palette, menus, the tool panel's entries) open the panel and run their action;
/// the tool-catalog entries are not dead references anymore.
#[test]
fn ai_commands_open_the_panel_and_act() {
    let mut h = harness();
    assert!(h.state_mut().execute("tools.ai"));
    h.run_steps(2);
    assert!(h.state().ai.open, "the command opened the panel");
    assert!(h.state_mut().execute("tools.ai"));
    h.run_steps(2);
    assert!(!h.state().ai.open, "the command toggles it closed again");

    stub_connection(&mut h);
    assert!(h.state_mut().execute("ai.summary"));
    h.run_steps(3);
    assert!(h.state().ai.open, "summarize opened the panel");
    h.get_by_label("Retry");
}

/// Not run with the suite: writes `target/ai-panel.png` for a visual check of the panel.
#[test]
#[ignore = "visual check helper"]
fn ai_panel_screenshot() {
    let _gpu = crate::gpu();
    let mut h = harness();
    assert!(h.state_mut().execute("tools.ai"));
    h.run_steps(10);
    let image = h.render().unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ai-panel.png");
    image.save(&path).unwrap();
    println!("wrote {}", path.display());
}
