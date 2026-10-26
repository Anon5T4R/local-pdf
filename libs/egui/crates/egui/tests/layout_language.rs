use egui::{
    Context, FontSelection, Label, TextEdit, UiBuilder, WidgetText,
    text::{LayoutJob, TextFormat},
};

/// Widgets lay out their text in the language of their `Ui`,
/// so line breakers and shaping get it, unless the text sets its own.
#[test]
fn widgets_lay_out_text_in_the_language_of_their_ui() {
    let ctx = Context::default();
    ctx.set_language(Some("th".to_owned()));

    let mut languages = Vec::new();
    let mut text = "สวัสดี".to_owned();
    let output = ctx.run_ui(Default::default(), |ui| {
        let (_, galley, _) = Label::new("ภาษาไทย").layout_in_ui(ui);
        languages.push(galley.job.language.clone());

        let galley = TextEdit::singleline(&mut text).show(ui).galley;
        languages.push(galley.job.language.clone());

        ui.scope_builder(UiBuilder::new().language("ja"), |ui| {
            let galley =
                WidgetText::from("日本語").into_galley(ui, None, 100.0, FontSelection::Default);
            languages.push(galley.job.language.clone());
        });

        let mut job = LayoutJob::single_section("한국어".to_owned(), TextFormat::default());
        job.language = Some("ko".to_owned());
        let (_, galley, _) = Label::new(job).layout_in_ui(ui);
        languages.push(galley.job.language.clone());
    });
    output.drop_without_applying_deltas();

    assert_eq!(
        languages,
        [Some("th"), Some("th"), Some("ja"), Some("ko")].map(|l| l.map(str::to_owned))
    );
}
