use std::collections::HashSet;

use super::{ALL, Icon};

#[test]
fn every_concept_has_an_svg() {
    for icon in Icon::EVERY {
        let svg = std::str::from_utf8(icon.svg()).expect("utf-8");
        assert!(svg.contains("<svg"), "{icon:?} -> {} is not an SVG", icon.name());
    }
}

#[test]
fn catalog_matches_the_concepts_one_to_one() {
    let names: HashSet<&str> = ALL.iter().map(|(name, _)| *name).collect();
    assert_eq!(names.len(), ALL.len(), "duplicate name in ALL");
    let used: HashSet<&str> = Icon::EVERY.iter().map(|icon| icon.name()).collect();
    assert_eq!(used.len(), Icon::EVERY.len(), "two concepts share an icon");
    assert_eq!(names, used, "ALL and Icon::name disagree");
}

#[test]
fn svgs_follow_the_tinting_conventions() {
    for (name, bytes) in ALL {
        let svg = std::str::from_utf8(bytes).expect("utf-8");
        assert!(svg.contains("stroke=\"currentColor\""), "{name}: no currentColor stroke");
        assert!(svg.contains("stroke-width=\"2\""), "{name}: no default stroke width");
        assert!(!svg.contains("<script"), "{name}: script in an embedded SVG");
    }
}

#[test]
fn the_theme_cycles_through_all_three_and_round_trips_its_id() {
    use super::ThemeMode;
    let mut mode = ThemeMode::default();
    assert_eq!(mode, ThemeMode::System);
    for _ in 0..3 {
        assert_eq!(ThemeMode::from_id(mode.id()), mode);
        mode = mode.next();
    }
    assert_eq!(mode, ThemeMode::System);
    assert_eq!(ThemeMode::from_id("nao-existe"), ThemeMode::System);
}
