//! Guardas da suíte (docs/planos/padrao-craft.md, "Identidade única"): todo diálogo cabe na largura
//! mínima da janela com um documento aberto, e o destaque se lê como texto e como fundo de botão nos
//! dois temas. Os guardas do kittest (glyphs e acessibilidade) ficam ligados: nada de
//! `allow_missing_glyphs` nem `with_accessibility_check(false)`.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use localpdf_ui_egui::LocalPDFApp;

/// A largura mínima da janela do app (apps/localpdf/src/main.rs).
const NARROW: f32 = 820.0;
const NARROW_HEIGHT: f32 = 520.0;

/// Uma página A4 com uma linha de texto: um documento de verdade para os diálogos abrirem sobre ele.
fn fixture() -> Vec<u8> {
    let content = b"BT /F1 18 Tf 40 480 Td (Guardas da suite) Tj ET\n0.2 0.3 0.9 rg 40 300 200 40 re f\n";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_string(),
        format!("<< /Length {} >>\nstream\n{}\nendstream", content.len(), String::from_utf8_lossy(content)),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
    out
}

fn app(dialog: Option<&str>) -> LocalPDFApp {
    let mut app = LocalPDFApp::new();
    app.set_option("language", "en").unwrap();
    app.open_bytes("guardas.pdf", None, fixture()).unwrap();
    if let Some(dialog) = dialog {
        app.set_option("dialog", dialog).unwrap();
    }
    app
}

fn harness(app: LocalPDFApp, width: f32) -> Harness<'static, LocalPDFApp> {
    Harness::builder().with_size(egui::vec2(width, NARROW_HEIGHT)).build_eframe(|_cc| app)
}

/// Todo controle anunciado (a bounding box do accesskit) fica dentro da janela mínima, nos dois
/// eixos. A página é conteúdo rolável, mais larga que a janela no zoom de 100%, e a vista a recorta:
/// ela fica de fora.
///
/// O corpo de um diálogo mais alto que a janela rola, com o título e o rodapé de botões fora do
/// `ScrollArea`. O accesskit anuncia o conteúdo rolado com o rect sem recorte, então o que passa da
/// janela por baixo só é aceito quando é conteúdo de uma área que rola: o nó tem de vir ANTES de
/// uma barra de rolagem vertical que fique dentro da janela e cuja coluna o contém (a barra é
/// desenhada depois do conteúdo, e o rodapé depois dela). O resto é casca: essa cabe inteira. Nada
/// pode ficar acima da janela nem passar das laterais.
fn assert_inside(harness: &Harness<'_, LocalPDFApp>, what: &str, width: f32) {
    const TOL: f64 = 0.5;
    let (w, h) = (f64::from(width), f64::from(NARROW_HEIGHT));
    let mut outside = Vec::new();
    let nodes: Vec<_> = harness.root().children_recursive().collect();
    // Every vertical scroll bar in the window, by the index it was drawn at.
    let bars: Vec<(usize, egui::accesskit::Rect)> = nodes
        .iter()
        .enumerate()
        .filter_map(|(i, node)| {
            let rect = node.accesskit_node().bounding_box()?;
            let vertical = rect.y1 - rect.y0 > rect.x1 - rect.x0;
            let inside = rect.x0 >= -TOL && rect.x1 <= w + TOL && rect.y0 >= -TOL && rect.y1 <= h + TOL;
            (node.accesskit_node().role() == egui::accesskit::Role::ScrollBar && vertical && inside).then_some((i, rect))
        })
        .collect();
    for (i, node) in nodes.iter().enumerate() {
        let role = node.accesskit_node().role();
        if matches!(role, egui::accesskit::Role::Document | egui::accesskit::Role::Image) {
            continue;
        }
        let Some(rect) = node.accesskit_node().bounding_box() else { continue };
        if rect.width() <= 0.0 {
            continue;
        }
        let label = node.accesskit_node().label().unwrap_or_default();
        if rect.x0 < -TOL || rect.x1 > w + TOL {
            outside.push(format!("{role:?} {label:?} fora das laterais: {rect:?}"));
            continue;
        }
        if rect.y0 < -TOL {
            outside.push(format!("{role:?} {label:?} acima da janela: {rect:?}"));
            continue;
        }
        if rect.y1 <= h + TOL {
            continue;
        }
        if role == egui::accesskit::Role::ScrollBar {
            outside.push(format!("{role:?} {label:?} abaixo da janela de {h}: {rect:?}"));
            continue;
        }
        let scrolled = bars.iter().any(|&(bar_at, bar)| bar_at > i && rect.x1 <= bar.x1 + TOL && bar.y0 <= rect.y0 + TOL);
        if !scrolled {
            outside.push(format!("{role:?} {label:?} abaixo da janela de {h}, fora de um corpo que rola: {rect:?}"));
        }
    }
    assert!(outside.is_empty(), "{what}: fora da janela de {w}×{h}: {outside:#?}");
}

/// Os diálogos do app, pelos nomes que o `set_option("dialog", …)` aceita.
const DIALOGS: &[&str] = &[
    "properties",
    "security",
    "fonts",
    "custom",
    "advanced",
    "space-audit",
    "shortcuts",
    "split",
    "protect",
    "page-boxes",
    "header-footer",
    "watermark",
    "background",
    "export-image",
    "word-count",
    "export-text",
    "export-all-images",
    "accessibility-options",
    "recognize-text",
    "js-console",
    "document-js",
    "preferences",
    "compare-files",
    "action-wizard",
    "pdfa",
    "signature",
    "optimize",
    "number-pages",
];

/// Cada diálogo cabe na janela mínima, nos dois eixos: com um documento aberto, nenhum controle
/// passa da borda e o título e os botões ficam sempre na tela (o corpo rola quando não cabe).
#[test]
fn every_dialog_fits_the_narrow_window() {
    let mut problems: Vec<String> = Vec::new();
    for &name in DIALOGS {
        // One dialog's failure (a missing accessible name, a control past the edge) is reported with
        // the rest instead of hiding them: the whole list comes out of one run.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut h = harness(app(Some(name)), NARROW);
            h.run_steps(4);
            assert_inside(&h, name, NARROW);
        }));
        if let Err(panic) = outcome {
            let message = panic.downcast_ref::<String>().map(String::as_str).or(panic.downcast_ref::<&str>().copied()).unwrap_or("?");
            problems.push(format!("{name}: {message}"));
        }
    }
    assert!(problems.is_empty(), "{} diálogo(s) fora do padrão:\n{}", problems.len(), problems.join("\n"));
}

/// A casca (barra de cima, abas de modo, paleta e barra de status) cabe na janela mínima.
#[test]
fn the_shell_fits_the_narrow_window() {
    let mut h = harness(app(None), NARROW);
    h.run_steps(3);
    assert_inside(&h, "shell", NARROW);

    h.state_mut().set_option("palette", "").unwrap();
    h.run_steps(3);
    assert_inside(&h, "palette", NARROW);
}

/// Por volta de 1150 px a caixa "Localizar ferramentas e comandos" encostava nas abas do modo
/// (Assinatura digital, IA): ela encolhe com a janela e nunca cobre uma aba.
#[test]
fn the_search_box_never_covers_the_mode_tabs() {
    for width in [1150.0, 1100.0, 900.0, NARROW] {
        let mut h = harness(app(None), width);
        h.run_steps(3);
        let tabs: Vec<egui::Rect> = ["All tools", "Edit", "Convert", "E-Sign"]
            .into_iter()
            .map(|label| {
                let label = label.to_owned();
                h.get_by(move |n| n.role() == egui::accesskit::Role::Button && n.label().as_deref() == Some(label.as_str())).rect()
            })
            .collect();
        assert_eq!(tabs.len(), 4, "as abas do modo estão na barra ({width} px)");
        if let Some(search) = h.query_by_label("Find tools and commands").map(|node| node.rect()) {
            for tab in &tabs {
                assert!(!search.intersects(*tab), "em {width} px a caixa de busca {search:?} cobre a aba {tab:?}");
            }
        }
        assert_inside(&h, "mode bar", width);
    }
}

/// O cartão de atualização (`updater.offer`) cabe na janela mínima, nos dois eixos.
#[test]
fn the_update_card_fits_the_narrow_window() {
    let release = craft_update::Release { version: craft_update::version::parse("0.99.0"), tag: "v0.99.0".into(), ..Default::default() };
    let mut prompt = craft_update::StartupPrompt::new(&craft_update::suite::LOCALPDF, release, env!("CARGO_PKG_VERSION"));
    let mut h = Harness::builder().with_size(egui::vec2(NARROW, NARROW_HEIGHT)).build_ui(|ui| {
        prompt.show(ui.ctx());
    });
    h.run_steps(3);
    for node in h.root().children_recursive() {
        let Some(rect) = node.accesskit_node().bounding_box() else { continue };
        if rect.width() <= 0.0 {
            continue;
        }
        assert!(
            rect.x0 >= 0.0 && rect.x1 <= f64::from(NARROW) && rect.y0 >= 0.0 && rect.y1 <= f64::from(NARROW_HEIGHT),
            "cartão fora da janela de {NARROW}×{NARROW_HEIGHT}: {rect:?}"
        );
    }
}

/// Luminância relativa (WCAG 2.x).
fn luminance(color: egui::Color32) -> f32 {
    let channel = |c: u8| {
        let c = f32::from(c) / 255.0;
        if c <= 0.039_28 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
}

fn contrast(a: egui::Color32, b: egui::Color32) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// O destaque como texto (no painel e no cartão) e como fundo de botão (o rótulo em cima) passa de
/// 4,5:1 nos dois temas.
#[test]
fn the_accent_reads_on_both_themes() {
    use localpdf_ui_egui::theme::{ThemeKind, Tokens};
    for kind in [ThemeKind::Light, ThemeKind::Dark] {
        let t = Tokens::for_kind(kind);
        let name = if t.dark() { "dark" } else { "light" };
        for (what, fg, bg) in [
            ("accent text on the card", t.accent_text, t.card),
            ("accent text on the panel", t.accent_text, t.panel),
            ("accent on the panel", t.accent, t.panel),
            ("label on an accent button", t.on_accent, t.accent),
        ] {
            let ratio = contrast(fg, bg);
            assert!(ratio >= 4.5, "{name}: {what} {ratio:.2}:1 ({fg:?} em {bg:?})");
        }
    }
}
