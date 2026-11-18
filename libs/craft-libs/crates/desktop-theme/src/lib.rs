//! Makes `egui::Context::system_theme` answer on Linux.
//!
//! winit reports no system theme on Linux (every desktop, X11 and Wayland alike), so an app's
//! "Use system setting" stayed on its fallback there. The XDG Settings portal does answer, on
//! GNOME, KDE and XFCE; `craft-platform`'s watcher subscribes to it off the UI thread.
//!
//! [`follow`] installs an egui plugin that copies the watcher's answer into each frame's
//! `RawInput::system_theme` when the window integration left it empty. Everything downstream
//! then works unchanged: `ctx.system_theme()`, `ThemePreference::System`, an app's own palette
//! choice, and the native window decorations. On Windows and macOS winit already answers, no
//! watcher starts, and nothing is installed.
//!
//! Call it once from the app binary, never from tests: kittest runs must not depend on the
//! machine's desktop theme. Tests that need a desktop answer use [`install`] with a stand-in.

use std::sync::Mutex;

use craft_platform::appearance::{self, Watcher};

/// Start following the desktop's light/dark choice on `ctx`. Returns whether a watcher runs
/// (false off Linux, and on Linux without a session bus); calling it again does nothing more.
pub fn follow(ctx: &egui::Context) -> bool {
    let wake = ctx.clone();
    let Some(watcher) = Watcher::start(move || wake.request_repaint()) else { return false };
    let watcher = Mutex::new(watcher);
    install(ctx, move || {
        watcher.lock().ok().and_then(|w| w.current()).map(|theme| match theme {
            appearance::Theme::Dark => egui::Theme::Dark,
            appearance::Theme::Light => egui::Theme::Light,
        })
    });
    true
}

/// Install the plugin over any source of the desktop's answer: [`follow`] passes the portal
/// watcher, a test passes a stand-in. Installing twice keeps the first.
pub fn install(ctx: &egui::Context, observed: impl Fn() -> Option<egui::Theme> + Send + Sync + 'static) {
    ctx.add_plugin(DesktopTheme { observed: Box::new(observed) });
}

/// The plugin, with the answer behind a closure so tests can stand in for the portal.
struct DesktopTheme {
    observed: Box<dyn Fn() -> Option<egui::Theme> + Send + Sync>,
}

impl egui::Plugin for DesktopTheme {
    fn debug_name(&self) -> &'static str {
        "desktop theme (XDG portal)"
    }

    fn input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        let observed = (self.observed)();
        // The window integration wins when it knows; the portal only fills the gap.
        if input.system_theme.is_none() {
            input.system_theme = observed;
        }
        // Linux window backends cannot resolve `SystemDefault` for their own decorations.
        let observed = observed.map(|theme| match theme {
            egui::Theme::Dark => appearance::Theme::Dark,
            egui::Theme::Light => appearance::Theme::Light,
        });
        appearance::sync_native_window_theme(ctx, observed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU8, Ordering};

    /// A stand-in portal installed on `ctx`: 0 = no answer, 1 = dark, 2 = light.
    fn portal(ctx: &egui::Context) -> Arc<AtomicU8> {
        let answer = Arc::new(AtomicU8::new(0));
        let read = Arc::clone(&answer);
        install(ctx, move || match read.load(Ordering::Relaxed) {
            1 => Some(egui::Theme::Dark),
            2 => Some(egui::Theme::Light),
            _ => None,
        });
        answer
    }

    fn frame(ctx: &egui::Context, system_theme: Option<egui::Theme>) -> Option<egui::Theme> {
        let mut output = ctx.run_ui(egui::RawInput { system_theme, ..Default::default() }, |_| {});
        output.textures_delta.clear();
        ctx.system_theme()
    }

    #[test]
    fn the_portal_answer_fills_a_missing_system_theme_and_follows_changes() {
        let ctx = egui::Context::default();
        let answer = portal(&ctx);
        assert_eq!(frame(&ctx, None), None, "no answer yet: the app keeps its fallback");
        answer.store(1, Ordering::Relaxed);
        assert_eq!(frame(&ctx, None), Some(egui::Theme::Dark));
        answer.store(2, Ordering::Relaxed);
        assert_eq!(frame(&ctx, None), Some(egui::Theme::Light));
    }

    #[test]
    fn the_window_integration_answer_wins_over_the_portal() {
        let ctx = egui::Context::default();
        let answer = portal(&ctx);
        answer.store(1, Ordering::Relaxed);
        assert_eq!(frame(&ctx, Some(egui::Theme::Light)), Some(egui::Theme::Light));
    }

    #[test]
    fn system_preference_resolves_through_the_portal() {
        let ctx = egui::Context::default();
        let answer = portal(&ctx);
        ctx.set_theme(egui::ThemePreference::System);
        ctx.options_mut(|o| o.fallback_theme = egui::Theme::Light);
        answer.store(1, Ordering::Relaxed);
        let _ = frame(&ctx, None);
        assert_eq!(ctx.theme(), egui::Theme::Dark);
    }

    /// Off Linux winit answers, so nothing starts and nothing is installed.
    #[test]
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    fn other_platforms_install_nothing() {
        assert!(!follow(&egui::Context::default()));
    }
}
