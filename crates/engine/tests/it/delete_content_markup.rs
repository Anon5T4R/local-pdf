//! Deleting an added text item takes the markup annotations drawn over it with it: a highlight
//! or strikethrough over deleted text is noise. The annots' `/Rect` is user space; the item's
//! box is display space, so a turned page must map the box through the view matrix first.

use std::sync::Arc;

use localpdf_annot::{Markup, NewAnnotation, Shape, Style};
use localpdf_cos::{Dict, Document, Object, SaveOptions, Stream, write_incremental};
use localpdf_engine::{AddedText, Edit, Session};

fn fixture() -> Arc<Vec<u8>> {
    let mut doc = Document::new_empty();
    let pages = doc.get(doc.root().unwrap()).as_dict().unwrap().reference(b"Pages").unwrap();
    let mut font = Dict::new();
    font.set(b"Type".to_vec(), Object::name("Font"));
    font.set(b"Subtype".to_vec(), Object::name("Type1"));
    font.set(b"BaseFont".to_vec(), Object::name("Helvetica"));
    let mut fonts = Dict::new();
    fonts.set(b"F1".to_vec(), Object::Ref(doc.add(font)));
    let mut resources = Dict::new();
    resources.set(b"Font".to_vec(), Object::Dict(fonts));
    let body = "BT /F1 12 Tf 72 700 Td (Original) Tj ET\n";
    let stream = Object::Stream(Stream::from_raw(Dict::new(), body.as_bytes().to_vec()));
    let contents = Object::Ref(doc.add(stream));
    let mut page = Dict::new();
    page.set(b"Type".to_vec(), Object::name("Page"));
    page.set(b"Parent".to_vec(), Object::Ref(pages));
    page.set(b"MediaBox".to_vec(), Object::Array(vec![0.into(), 0.into(), 612.into(), 792.into()]));
    page.set(b"Resources".to_vec(), Object::Dict(resources));
    page.set(b"Contents".to_vec(), contents);
    let page = doc.add(page);
    doc.update_dict(pages, |d| {
        d.set(b"Kids".to_vec(), Object::Array(vec![Object::Ref(page)]));
        d.set(b"Count".to_vec(), Object::Int(1));
    })
    .unwrap();
    Arc::new(write_incremental(&doc, &SaveOptions::default()).unwrap())
}

/// The text box the added text occupies, in display space.
fn text_box() -> AddedText {
    AddedText { text: "Nota".into(), rect: [72.0, 300.0, 250.0, 330.0], ..Default::default() }
}

/// A highlight quad covering the text box, in user space (an unturned page: the same numbers).
fn highlight() -> NewAnnotation {
    NewAnnotation {
        page: 0,
        shape: Shape::TextMarkup { kind: Markup::Highlight, quads: vec![[72.0, 300.0, 250.0, 300.0, 72.0, 330.0, 250.0, 330.0]] },
        style: Style::default(),
        contents: "grifo".into(),
        author: "tester".into(),
    }
}

fn annot_count(session: &Session, id: localpdf_engine::DocId) -> usize {
    // Save and re-open: the page's /Annots length. One markup annot while it exists, none after
    // the cascade.
    let bytes = session.save_bytes(id).unwrap();
    let doc = Document::open(bytes).unwrap();
    let p = localpdf_model::pages(&doc)[0].clone();
    p.dict.get(b"Annots").map(|a| doc.resolve(a).as_array().map(|a| a.len()).unwrap_or(0)).unwrap_or(0)
}

#[test]
fn deleting_added_text_takes_its_highlight_with_it() {
    let mut session = Session::new();
    let id = session.open("fixture.pdf", None, fixture(), None).unwrap();
    session.apply(id, Edit::AddText { page: 0, text: text_box() }).unwrap();
    session.apply(id, Edit::AddAnnotation(highlight())).unwrap();
    assert_eq!(annot_count(&session, id), 1, "the highlight is on the page");

    session.apply(id, Edit::DeleteContent { page: 0, index: 0 }).unwrap();
    assert_eq!(annot_count(&session, id), 0, "the highlight over the deleted text goes with it");
}

#[test]
fn deleting_added_text_keeps_a_highlight_elsewhere() {
    let mut session = Session::new();
    let id = session.open("fixture.pdf", None, fixture(), None).unwrap();
    session.apply(id, Edit::AddText { page: 0, text: text_box() }).unwrap();
    let mut far = highlight();
    far.shape = Shape::TextMarkup { kind: Markup::Highlight, quads: vec![[400.0, 600.0, 500.0, 600.0, 400.0, 630.0, 500.0, 630.0]] };
    session.apply(id, Edit::AddAnnotation(far)).unwrap();
    assert_eq!(annot_count(&session, id), 1, "the far highlight is on the page");

    session.apply(id, Edit::DeleteContent { page: 0, index: 0 }).unwrap();
    assert_eq!(annot_count(&session, id), 1, "a highlight over other content stays");
}
