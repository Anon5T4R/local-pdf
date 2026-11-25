//! The Craft App Suite as releases know it: each app's id, name and repository.
//!
//! An app that updates itself takes its own entry (`suite::LOCALKEYS`); the launcher takes all
//! of them. Installers use the list to tell a Craft App's package from anyone else's.

use crate::app::App;

pub const PHOTOCRAFT: App = App::new("photocraft", "PhotoCraft", "Image editing", "storytold/photocraft");
pub const LOCALIMAGE: App = App {
    former_repos: &["Anon5T4R/Local-Image"],
    publisher: "LocalImage contributors",
    icon_png: Some(include_bytes!("../assets/icons/local-image.png")),
    ..App::new("local-image", "Local Image", "Image viewer", "Styx-Studios/Local-Image")
};
pub const LOCALRASTER: App = App {
    former_ids: &["localpaint"],
    former_names: &["LocalPaint", "Local Paint"],
    former_repos: &["Anon5T4R/Local-Paint"],
    publisher: "Local Raster contributors",
    icon_png: Some(include_bytes!("../assets/icons/localraster.png")),
    ..App::new("localraster", "Local Raster", "Image editing", "Styx-Studios/Local-Raster")
};
/// Local Paint's old name.
#[deprecated(note = "renamed: use LOCALRASTER")]
pub const LOCALPAINT: App = LOCALRASTER;
pub const LOCALPHOTO: App = App {
    former_repos: &["Anon5T4R/Local-Photo"],
    publisher: "Local Photo contributors",
    icon_png: Some(include_bytes!("../assets/icons/localphoto.png")),
    ..App::new("localphoto", "Local Photo", "Photo library and raw development", "Styx-Studios/Local-Photo")
};
pub const LOCALVECTOR: App = App {
    former_repos: &["Anon5T4R/Local-Vector"],
    former_names: &["LocalVector"],
    publisher: "Local Vector contributors",
    icon_png: Some(include_bytes!("../assets/icons/localvector.png")),
    ..App::new("localvector", "Local Vector", "Vector illustration", "Styx-Studios/Local-Vector")
};
pub const VECTORCRAFT: App = App::new("vectorcraft", "VectorCraft", "Vector illustration", "storytold/vectorcraft");
pub const FILMCRAFT: App = App::new("filmcraft", "FilmCraft", "Video editing, color and sound", "storytold/filmcraft");
pub const LIGHTCRAFT: App = App::new("lightcraft", "LightCraft", "Photo library and raw development", "storytold/lightcraft");
pub const PDFCRAFT: App = App {
    former_ids: &["printcraft"],
    former_names: &["PrintCraft"],
    ..App::new("pdfcraft", "PdfCraft", "Reading, organizing and protecting PDFs", "storytold/pdfcraft")
};
pub const LOCALPDF: App = App {
    former_repos: &["Anon5T4R/local-pdf"],
    former_ids: &["printcraft"],
    former_names: &["PrintCraft", "LocalPDF"],
    publisher: "Local PDF contributors",
    icon_png: Some(include_bytes!("../assets/icons/localpdf.png")),
    ..App::new("localpdf", "Local PDF", "Reading, organizing and protecting PDFs", "Styx-Studios/Local-PDF")
};
pub const EFFECTCRAFT: App = App::new("effectcraft", "EffectCraft", "Motion graphics and visual effects", "storytold/effectcraft");
pub const DESIGNCRAFT: App = App::new("designcraft", "DesignCraft", "Page layout and publishing", "storytold/designcraft");
pub const FIGCRAFT: App = App::new("figcraft", "FigCraft", "Interface design and prototyping", "storytold/figcraft");
pub const WORDCRAFT: App = App::new("wordcraft", "WordCraft", "Writing and document design", "storytold/wordcraft");
pub const LOCALWRITER: App = App {
    former_ids: &["localoffice", "wordcraft"],
    former_names: &["Local Word", "WordCraft", "LocalOffice"],
    former_repos: &["Anon5T4R/local-office"],
    publisher: "Local Writer contributors",
    icon_png: Some(include_bytes!("../assets/icons/localwriter.png")),
    ..App::new("localwriter", "Local Writer", "Writing and document design", "Styx-Studios/Local-Writer")
};
/// Local Writer's old name (Local Word).
#[deprecated(note = "renamed: use LOCALWRITER")]
pub const LOCALOFFICE: App = LOCALWRITER;
pub const SOUNDCRAFT: App = App::new("soundcraft", "SoundCraft", "Recording, editing and mixing audio", "storytold/soundcraft");
pub const DECKCRAFT: App = App::new("deckcraft", "DeckCraft", "Presentations and slide shows", "storytold/deckcraft");
pub const LOCALSLIDES: App = App {
    former_repos: &["Anon5T4R/local-slides"],
    developer: "Local Slides contributors",
    publisher: "Local Slides contributors",
    icon_png: Some(include_bytes!("../assets/icons/localslides.png")),
    ..App::new("localslides", "Local Slides", "Slide decks and presentations", "Styx-Studios/Local-Slides")
};
pub const LOCALSHEETS: App = App {
    former_repos: &["Anon5T4R/local-sheets"],
    developer: "Styx Studios",
    publisher: "Anon5T4R",
    icon_png: Some(include_bytes!("../assets/icons/localsheets.png")),
    ..App::new("localsheets", "Local Sheets", "Spreadsheets and data", "Styx-Studios/Local-Sheets")
};
pub const LOCALKEYS: App = App {
    former_repos: &["Anon5T4R/Local-Keys"],
    publisher: "LocalKeys contributors",
    icon_png: Some(include_bytes!("../assets/icons/local-keys.png")),
    ..App::new("local-keys", "Local Keys", "Password manager", "Styx-Studios/Local-Keys")
};
pub const LOCALUTILS: App = App {
    former_repos: &["Anon5T4R/Local-Utils"],
    publisher: "Local Utils contributors",
    icon_png: Some(include_bytes!("../assets/icons/local-utils.png")),
    ..App::new("local-utils", "Local Utils", "Everyday utilities", "Styx-Studios/Local-Utils")
};
/// Local Player (the player app; "RS" is the fork's old name, the binary keeps `local-player-rs`).
pub const LOCALPLAYER: App = App {
    former_repos: &["Anon5T4R/Local-Player"],
    former_ids: &["local-player-rs"],
    former_names: &["Local Player RS"],
    publisher: "Local Player RS contributors",
    icon_png: Some(include_bytes!("../assets/icons/local-player.png")),
    ..App::new("local-player", "Local Player", "Media player", "Styx-Studios/Local-Player")
};
pub const LOCALNOTEBOOK: App = App {
    former_ids: &["localobsidian"],
    former_names: &["Local Obsidian"],
    former_repos: &["Anon5T4R/Local-Obsidian"],
    publisher: "Local Notebook contributors",
    icon_png: Some(include_bytes!("../assets/icons/localnotebook.png")),
    ..App::new("localnotebook", "Local Notebook", "Notes, wikilinks and attachments", "Styx-Studios/Local-Notebook")
};
/// Local Notebook's old name (Local Obsidian).
#[deprecated(note = "renamed: use LOCALNOTEBOOK")]
pub const LOCALOBSIDIAN: App = LOCALNOTEBOOK;
pub const LOCALMINDMAP: App = App {
    former_ids: &["localmind"],
    former_names: &["Local Mind"],
    former_repos: &["Anon5T4R/Local-Mind"],
    publisher: "Local Mindmap contributors",
    icon_png: Some(include_bytes!("../assets/icons/localmindmap.png")),
    ..App::new("localmindmap", "Local Mindmap", "Mind maps with a balanced layout", "Styx-Studios/Local-Mindmap")
};
/// Local Mindmap's old name (Local Mind).
#[deprecated(note = "renamed: use LOCALMINDMAP")]
pub const LOCALMIND: App = LOCALMINDMAP;
pub const CADCRAFT: App = App::new("cadcraft", "CADCraft", "Computer-aided design and drafting", "storytold/cadcraft");
pub const GRIDCRAFT: App = App::new("gridcraft", "GridCraft", "Spreadsheets and data", "storytold/gridcraft");

/// ArtCraft Launcher, described like a Craft App so it updates itself with the same machinery
/// (asset names `artcraft-launcher-<ver>-.`, bundle id `ai.storyteller.artcraft-launcher`,
/// `ArtCraft Launcher.app`). Not part of [`SUITE`].
pub const LAUNCHER: App = App::new("artcraft-launcher", "ArtCraft Launcher", "Installs and updates the Craft App Suite", "storytold/craft-launcher");

/// The Craft App Suite, in getartcraft.com's display order.
pub const SUITE: [App; 26] = [
    PHOTOCRAFT,
    LOCALIMAGE,
    LOCALRASTER,
    LOCALPHOTO,
    LOCALVECTOR,
    VECTORCRAFT,
    FILMCRAFT,
    LIGHTCRAFT,
    PDFCRAFT,
    LOCALPDF,
    EFFECTCRAFT,
    DESIGNCRAFT,
    FIGCRAFT,
    WORDCRAFT,
    LOCALWRITER,
    SOUNDCRAFT,
    DECKCRAFT,
    LOCALSLIDES,
    LOCALSHEETS,
    LOCALKEYS,
    LOCALUTILS,
    LOCALPLAYER,
    LOCALNOTEBOOK,
    LOCALMINDMAP,
    CADCRAFT,
    GRIDCRAFT,
];

/// The suite's app with this id.
pub fn by_id(id: &str) -> Option<&'static App> {
    SUITE.iter().find(|a| a.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_app_has_a_unique_id() {
        let mut ids: Vec<_> = SUITE.iter().map(|a| a.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), SUITE.len());
    }

    /// Local Player kept its filename (`local-player-rs`) and its pre-rename display name as former
    /// names, so a copy installed before the rename is still recognised (and upgraded in place).
    #[test]
    fn the_player_carries_its_old_name_and_binary() {
        let player = by_id("local-player").expect("the player is in the suite");
        assert_eq!((player.name, player.repo), ("Local Player", "Styx-Studios/Local-Player"));
        assert_eq!(player.publisher, "Local Player RS contributors", "the MSI's Manufacturer");
        assert!(player.names().any(|name| name == "Local Player RS"));
        assert!(player.stems().any(|stem| stem == "local-player-rs"));
    }

    #[test]
    fn the_upstream_apps_are_named_after_their_ids_and_repositories() {
        for a in &SUITE {
            if a.repo.starts_with("storytold/") {
                assert_eq!(a.repo, format!("storytold/{}", a.id));
                assert_eq!(a.name.to_lowercase(), a.id);
            }
        }
    }

    #[test]
    fn the_suites_own_apps_keep_their_rebranded_identity() {
        assert!(by_id(LAUNCHER.id).is_none());
        assert_eq!(by_id("pdfcraft").map(|a| a.former_ids), Some(&["printcraft"][..]));
        assert_eq!(by_id("localpdf").map(|a| a.former_names), Some(&["PrintCraft", "LocalPDF"][..]));
        assert_eq!(by_id("localwriter").map(|a| (a.name, a.publisher)), Some(("Local Writer", "Local Writer contributors")));
        assert_eq!(by_id("localraster").map(|a| a.repo), Some("Styx-Studios/Local-Raster"));
        assert_eq!(
            by_id("localphoto").map(|a| (a.name, a.publisher, a.repo)),
            Some(("Local Photo", "Local Photo contributors", "Styx-Studios/Local-Photo"))
        );
        assert_eq!(by_id("localvector").map(|a| (a.publisher, a.repo)), Some(("Local Vector contributors", "Styx-Studios/Local-Vector")));
        assert_eq!(by_id("localslides").map(|a| (a.name, a.publisher)), Some(("Local Slides", "Local Slides contributors")));
        assert_eq!(by_id("localsheets").map(|a| (a.name, a.publisher, a.repo)), Some(("Local Sheets", "Anon5T4R", "Styx-Studios/Local-Sheets")));
        assert_eq!(by_id("local-image").map(|a| a.publisher), Some("LocalImage contributors"));
        assert_eq!(by_id("local-keys").map(|a| a.publisher), Some("LocalKeys contributors"));
        assert_eq!(by_id("local-utils").map(|a| a.publisher), Some("Local Utils contributors"));
        assert_eq!(by_id("local-player").map(|a| a.publisher), Some("Local Player RS contributors"));
        assert_eq!(
            by_id("localnotebook").map(|a| (a.name, a.publisher, a.repo)),
            Some(("Local Notebook", "Local Notebook contributors", "Styx-Studios/Local-Notebook"))
        );
        assert_eq!(
            by_id("localmindmap").map(|a| (a.name, a.publisher, a.repo)),
            Some(("Local Mindmap", "Local Mindmap contributors", "Styx-Studios/Local-Mindmap"))
        );
    }

    #[test]
    fn every_org_app_still_finds_its_releases_under_the_personal_account() {
        // The repos move to Styx-Studios one by one; until each moves, its releases live under Anon5T4R.
        for app in SUITE.iter().filter(|a| a.repo.starts_with("Styx-Studios/")) {
            assert!(app.former_repos.iter().any(|r| r.starts_with("Anon5T4R/")), "{}", app.id);
        }
        assert!(SUITE.iter().all(|a| !a.repo.starts_with("Anon5T4R/")));
    }
}
