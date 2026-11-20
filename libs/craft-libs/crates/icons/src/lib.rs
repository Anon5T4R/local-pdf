//! The suite's icons for the concepts every app shares: theme, settings, language, help, update, AI,
//! file and edit commands, status. One Lucide SVG per concept (ISC, `assets/LICENSE-lucide.txt`), so the
//! same idea looks the same in every app. Tool icons that belong to one app stay in that app's catalog.
//!
//! Usage: merge [`ALL`] into the app's catalog (`craft_ui::icons::SvgIconSet::new` takes the same
//! `(name, bytes)` shape) and ask for icons through [`Icon::name`], never a string literal. The SVGs use
//! `currentColor` and `stroke-width="2"`, the conventions `SvgStyle` rewrites. Apps without a catalog of
//! their own use the `egui` feature ([`ui`]).

#[cfg(feature = "egui")]
pub mod ui;

/// A shared concept. The name it maps to is the Lucide file name and the catalog key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    /// Theme follows the system (the first of the three theme states).
    ThemeSystem,
    ThemeLight,
    ThemeDark,
    /// The gear next to the theme button, and the Settings menu item.
    Settings,
    Language,
    /// Help menu / keyboard shortcuts.
    Help,
    /// About, and neutral information.
    Info,
    CheckForUpdates,
    /// The AI panel and AI commands.
    Ai,
    Close,
    More,
    /// Search and the command palette.
    Search,
    New,
    Open,
    Save,
    Export,
    Share,
    Print,
    Undo,
    Redo,
    ZoomIn,
    ZoomOut,
    FullScreen,
    Add,
    Delete,
    Copy,
    Cut,
    Paste,
    SidePanel,
    Warning,
    Error,
    Success,
    /// A checked state (menu tick, done).
    Check,
}

impl Icon {
    /// Every concept, in declaration order.
    pub const EVERY: [Self; 33] = [
        Self::ThemeSystem,
        Self::ThemeLight,
        Self::ThemeDark,
        Self::Settings,
        Self::Language,
        Self::Help,
        Self::Info,
        Self::CheckForUpdates,
        Self::Ai,
        Self::Close,
        Self::More,
        Self::Search,
        Self::New,
        Self::Open,
        Self::Save,
        Self::Export,
        Self::Share,
        Self::Print,
        Self::Undo,
        Self::Redo,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::FullScreen,
        Self::Add,
        Self::Delete,
        Self::Copy,
        Self::Cut,
        Self::Paste,
        Self::SidePanel,
        Self::Warning,
        Self::Error,
        Self::Success,
        Self::Check,
    ];

    /// The catalog key (Lucide file name, without `.svg`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ThemeSystem => "monitor",
            Self::ThemeLight => "sun",
            Self::ThemeDark => "moon",
            Self::Settings => "settings",
            Self::Language => "languages",
            Self::Help => "circle-help",
            Self::Info => "info",
            Self::CheckForUpdates => "cloud",
            Self::Ai => "sparkles",
            Self::Close => "x",
            Self::More => "ellipsis",
            Self::Search => "search",
            Self::New => "file-plus-2",
            Self::Open => "folder-open",
            Self::Save => "save",
            Self::Export => "file-output",
            Self::Share => "share-2",
            Self::Print => "printer",
            Self::Undo => "undo-2",
            Self::Redo => "redo-2",
            Self::ZoomIn => "zoom-in",
            Self::ZoomOut => "zoom-out",
            Self::FullScreen => "maximize-2",
            Self::Add => "plus",
            Self::Delete => "trash-2",
            Self::Copy => "copy",
            Self::Cut => "scissors",
            Self::Paste => "clipboard-paste",
            Self::SidePanel => "panel-right",
            Self::Warning => "triangle-alert",
            Self::Error => "circle-x",
            Self::Success => "circle-check",
            Self::Check => "check",
        }
    }

    /// The SVG bytes.
    #[must_use]
    pub fn svg(self) -> &'static [u8] {
        let name = self.name();
        ALL.iter().find(|(n, _)| *n == name).map_or(&[], |(_, bytes)| bytes)
    }
}

/// The suite's three themes, in the order the theme button cycles them (the Local PDF reference).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ThemeMode {
    /// Follow the operating system.
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub const EVERY: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    /// The next mode the button switches to.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
        }
    }

    /// The icon that shows this mode.
    #[must_use]
    pub const fn icon(self) -> Icon {
        match self {
            Self::System => Icon::ThemeSystem,
            Self::Light => Icon::ThemeLight,
            Self::Dark => Icon::ThemeDark,
        }
    }

    /// The id a preferences file keeps.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    /// The mode a preferences file names; an unknown id follows the system (a newer build's value must not
    /// keep the app from opening).
    #[must_use]
    pub fn from_id(id: &str) -> Self {
        Self::EVERY.into_iter().find(|mode| mode.id() == id).unwrap_or_default()
    }

    /// The English label the app's catalog translates (menus, Settings, the button's tooltip).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::System => "Automatic",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
}

macro_rules! catalog {
    ($($name:literal),* $(,)?) => {
        /// Every shared SVG as `(name, bytes)`, the shape `SvgIconSet::new` takes.
        pub const ALL: &[(&str, &[u8])] = &[$(($name, include_bytes!(concat!("../assets/", $name, ".svg")))),*];
    };
}

catalog![
    "monitor",
    "sun",
    "moon",
    "settings",
    "languages",
    "circle-help",
    "info",
    "cloud",
    "sparkles",
    "x",
    "ellipsis",
    "search",
    "file-plus-2",
    "folder-open",
    "save",
    "file-output",
    "share-2",
    "printer",
    "undo-2",
    "redo-2",
    "zoom-in",
    "zoom-out",
    "maximize-2",
    "plus",
    "trash-2",
    "copy",
    "scissors",
    "clipboard-paste",
    "panel-right",
    "triangle-alert",
    "circle-x",
    "circle-check",
    "check",
];

#[cfg(test)]
mod tests;
