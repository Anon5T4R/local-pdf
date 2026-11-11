//! Desktop integration for Linux: an app launched from anywhere (AppImage in
//! ~/Downloads, a bare binary) registers a `.desktop` entry and its icons in
//! `$XDG_DATA_HOME`, so it shows up in the application menu. The entry's
//! `Exec` points at the exact file that is running, and a marker records it:
//! when the app moves or updates, the entry is rewritten; when the file
//! disappears, the entry is removed. Installs owned by a package manager
//! (`/usr/...`, where the package already ships the entry) and the tarball's
//! `install.sh` layout are respected — registration only ever adds the
//! user-local entry it can keep current.
//!
//! Enabled by `Updater::new` on Linux; opt out with
//! `CRAFT_NO_DESKTOP_INTEGRATION=1`. Nothing runs on other platforms.
#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};

use crate::app::App;
use crate::folders;

/// Called by `Updater::new`: registers/refreshes/removes the user-local
/// desktop entry. Best-effort — integration problems must never keep the app
/// from starting, so every error is logged at most.
pub(crate) fn ensure_registered(app: &App, version: &str) {
    if std::env::var_os("CRAFT_NO_DESKTOP_INTEGRATION").is_some_and(|v| v == "1") {
        return;
    }
    if crate::process::in_sandbox() {
        return; // Flatpak: the manifest owns the entry.
    }
    let Some(target) = launch_target() else { return };
    let target = match target {
        LaunchTarget::AppImage(path) => {
            let appdir = std::env::var_os("APPDIR").map(PathBuf::from);
            register(app, version, &path, appdir.as_deref())
        }
        LaunchTarget::Exe(path) => register(app, version, &path, None),
    };
    if let Err(e) = target {
        log::debug!("desktop integration: {e}");
    }
}

enum LaunchTarget {
    AppImage(PathBuf),
    Exe(PathBuf),
}

fn launch_target() -> Option<LaunchTarget> {
    if let Some(p) = std::env::var_os("APPIMAGE") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(LaunchTarget::AppImage(p));
        }
    }
    let exe = std::env::current_exe().ok()?;
    let exe = exe.canonicalize().unwrap_or(exe);
    if !exe.is_file() {
        return None;
    }
    // Dev builds (`cargo run`) must not pollute the menu with target/debug.
    if exe.components().any(|c| c.as_os_str() == "target") {
        return None;
    }
    Some(LaunchTarget::Exe(exe))
}

fn register(app: &App, version: &str, target: &Path, appdir: Option<&Path>) -> Result<(), String> {
    let s = target.to_string_lossy();
    if s.starts_with("/usr/") || s.starts_with("/opt/") {
        return Ok(()); // Package-manager install ships its own entry.
    }
    let data = folders::data_local_dir().ok_or("no data dir")?;
    ensure_registered_in(app, version, target, appdir, &data)
}

// ---- the core (roots injected; unit-tested) --------------------------------

const MARKER_DIR: &str = "craft-update";

fn marker_path(data: &Path, app: &App) -> PathBuf {
    data.join(MARKER_DIR).join(format!("desktop-{}.txt", app.id))
}

/// `target_path\nversion\ndesktop_file\nicon_name`
fn read_marker(data: &Path, app: &App) -> Option<(String, String, String, String)> {
    let t = std::fs::read_to_string(marker_path(data, app)).ok()?;
    let mut it = t.lines();
    Some((it.next()?.to_string(), it.next()?.to_string(), it.next()?.to_string(), it.next()?.to_string()))
}

fn write_marker(data: &Path, app: &App, target: &Path, version: &str, desktop: &str, icon: &str) {
    let p = marker_path(data, app);
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, format!("{}\n{version}\n{desktop}\n{icon}\n", target.display()));
}

/// Where the shipped desktop entry and icons live for this running copy:
/// inside the AppImage mount (`$APPDIR/usr/share`) or next to the binary
/// (`<bin>/../share`, the tarball's `install.sh` layout).
fn shipped_share(target: &Path, appdir: Option<&Path>) -> Option<PathBuf> {
    if let Some(d) = appdir {
        let p = d.join("usr/share");
        if p.is_dir() {
            return Some(p);
        }
    }
    let p = target.parent()?.parent()?.join("share");
    p.is_dir().then_some(p)
}

/// The shipped entry is the one whose `Exec` runs this app's binary — file
/// names differ across the suite (`<id>.desktop`, `com.local.<id>.desktop`).
fn discover_desktop(share: &Path, app: &App) -> Option<(String, String)> {
    let dir = share.join("applications");
    let mut fallback: Option<(String, String)> = None;
    for entry in std::fs::read_dir(dir).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".desktop") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else { continue };
        let program = crate::desktop_entry::exec_program(&text);
        let icon = icon_of(&text);
        match program.as_deref() {
            Some(p) if Path::new(p).file_name().is_some_and(|f| f == app.id) => return Some((name, icon.unwrap_or_else(|| app.id.to_string()))),
            _ => fallback = fallback.or_else(|| Some((name, icon.unwrap_or_else(|| app.id.to_string())))),
        }
    }
    fallback
}

fn icon_of(desktop: &str) -> Option<String> {
    desktop.lines().find_map(|l| l.strip_prefix("Icon=")).map(str::trim).map(str::to_string)
}

/// Rewrites the program part of the `Exec` line, keeping the field codes
/// (`%F`, `%U`, …). Returns the full desktop file text.
fn retarget_exec(text: &str, target: &Path) -> Option<String> {
    let quoted = crate::desktop_entry::exec_quote(target)?;
    let mut out = String::with_capacity(text.len());
    let mut done = false;
    for line in text.lines() {
        if !done && let Some(rest) = line.strip_prefix("Exec=") {
            let args = rest.split_whitespace().skip(1).collect::<Vec<_>>().join(" ");
            out.push_str("Exec=");
            out.push_str(&quoted);
            if !args.is_empty() {
                out.push(' ');
                out.push_str(&args);
            }
            done = true;
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    done.then_some(out)
}

fn entry_text(app: &App, share: &Path, target: &Path) -> Option<(String, String, String)> {
    // (desktop file name, icon name, content)
    if let Some((name, icon)) = discover_desktop(share, app) {
        let text = std::fs::read_to_string(share.join("applications").join(&name)).ok()?;
        let text = retarget_exec(&text, target)?;
        return Some((name, icon, text));
    }
    // Bare binary with no shipped entry: a minimal one keeps it findable.
    let quoted = crate::desktop_entry::exec_quote(target)?;
    let name = format!("{}.desktop", app.id);
    let text = format!(
        "[Desktop Entry]\nType=Application\nName={}\nExec={} %F\nIcon={}\nTerminal=false\nStartupNotify=false\nStartupWMClass={}\nCategories=Utility;\n",
        app.name, quoted, app.id, app.id
    );
    Some((name, app.id.to_string(), text))
}

fn copy_icons(share: &Path, data: &Path, icon: &str) {
    let src_root = share.join("icons/hicolor");
    let Ok(levels) = std::fs::read_dir(&src_root) else { return };
    for level in levels.flatten() {
        // hicolor/<size>/apps/<icon>.{png,svg}
        let apps = level.path().join("apps");
        for ext in ["png", "svg", "svgz"] {
            let src = apps.join(format!("{icon}.{ext}"));
            if src.is_file() {
                let dst = data.join("icons/hicolor").join(level.file_name()).join("apps");
                let _ = std::fs::create_dir_all(&dst);
                let _ = std::fs::copy(&src, dst.join(format!("{icon}.{ext}")));
            }
        }
    }
}

fn remove_entry(data: &Path, desktop: &str, icon: &str) {
    let _ = std::fs::remove_file(data.join("applications").join(&desktop));
    if let Ok(levels) = std::fs::read_dir(data.join("icons/hicolor")) {
        for level in levels.flatten() {
            for ext in ["png", "svg", "svgz"] {
                let _ = std::fs::remove_file(level.path().join("apps").join(format!("{icon}.{ext}")));
            }
        }
    }
}

fn ensure_registered_in(app: &App, version: &str, target: &Path, appdir: Option<&Path>, data: &Path) -> Result<(), String> {
    // Self-cleaning: our previous target is gone — the user deleted or moved
    // the file away from us. Remove the entry we wrote (only ours).
    if let Some((prev, _v, desktop, icon)) = read_marker(data, app) {
        if !Path::new(&prev).is_file() {
            remove_entry(data, &desktop, &icon);
            let _ = std::fs::remove_file(marker_path(data, app));
        }
    }
    // Fast path: same target, same version, entry still there.
    let installed = data.join("applications");
    if let Some((prev, v, desktop, icon)) = read_marker(data, app) {
        if prev == target.display().to_string() && v == version {
            let entry = installed.join(&desktop);
            if entry.is_file() {
                return Ok(());
            }
        }
    }
    // A target that vanished between resolution and here registers nothing.
    if !target.is_file() {
        return Ok(());
    }
    let share = shipped_share(target, appdir);
    let (name, icon, text) = match share.as_deref().and_then(|s| entry_text(app, s, target)) {
        Some(x) => x,
        None => {
            let quoted = crate::desktop_entry::exec_quote(target).ok_or("unquotable path")?;
            let name = format!("{}.desktop", app.id);
            let text = format!(
                "[Desktop Entry]\nType=Application\nName={}\nExec={} %F\nIcon={}\nTerminal=false\nStartupNotify=false\nStartupWMClass={}\nCategories=Utility;\n",
                app.name, quoted, app.id, app.id
            );
            (name, app.id.to_string(), text)
        }
    };
    std::fs::create_dir_all(&installed).map_err(|e| e.to_string())?;
    std::fs::write(installed.join(&name), text).map_err(|e| e.to_string())?;
    if let Some(s) = &share {
        copy_icons(s, data, &icon);
    }
    write_marker(data, app, target, version, &name, &icon);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;

    fn app() -> &'static App {
        App::by_id("local-image").unwrap()
    }

    fn layout(share: &Path, desktop_name: &str, exec: &str, icon: &str, with_icon: bool) {
        std::fs::create_dir_all(share.join("applications")).unwrap();
        std::fs::write(
            share.join("applications").join(desktop_name),
            format!("[Desktop Entry]\nType=Application\nName=Local Image\nExec={exec} %F\nIcon={icon}\nTerminal=false\nStartupWMClass=local-image\n"),
        )
        .unwrap();
        if with_icon {
            std::fs::create_dir_all(share.join("icons/hicolor/128x128/apps")).unwrap();
            std::fs::write(share.join("icons/hicolor/128x128/apps").join(format!("{icon}.png")), b"png").unwrap();
        }
    }

    #[test]
    fn retargets_exec_keeping_field_codes() {
        let out = retarget_exec("Exec=local-image %F\nIcon=x\n", Path::new("/opt/a b/li.AppImage")).unwrap();
        assert!(out.contains(r#"Exec="/opt/a b/li.AppImage" %F"#), "{out}");
        assert!(out.contains("Icon=x"));
    }

    #[test]
    fn discovers_entry_by_exec_program() {
        let d = tempfile::tempdir().unwrap();
        let share = d.path().join("share");
        layout(&share, "com.local.localimage.desktop", "local-image", "local-image", false);
        layout(&share, "outro.desktop", "other-bin", "other", false);
        let (name, icon) = discover_desktop(&share, &app()).unwrap();
        assert_eq!(name, "com.local.localimage.desktop");
        assert_eq!(icon, "local-image");
    }

    #[test]
    fn registers_rewrites_and_cleans() {
        let d = tempfile::tempdir().unwrap();
        let share = d.path().join("share");
        layout(&share, "local-image.desktop", "local-image", "local-image", true);
        let data = d.path().join("data");
        let a = d.path().join("Apps/local-image");
        let b = d.path().join("Elsewhere/local-image");
        std::fs::create_dir_all(a.parent().unwrap()).unwrap();
        std::fs::create_dir_all(b.parent().unwrap()).unwrap();
        std::fs::write(&a, b"bin").unwrap();
        std::fs::write(&b, b"bin").unwrap();

        // First run: entry + icon + marker.
        ensure_registered_in(&app(), "1.0", &a, None, &data).unwrap();
        let entry = std::fs::read_to_string(data.join("applications/local-image.desktop")).unwrap();
        assert!(entry.contains(&a.display().to_string()));
        assert!(data.join("icons/hicolor/128x128/apps/local-image.png").is_file());
        assert!(marker_path(&data, &app()).is_file());

        // Same target+version: still there (fast path, nothing to assert but no error).
        ensure_registered_in(&app(), "1.0", &a, None, &data).unwrap();

        // Moved file: entry follows the new path.
        ensure_registered_in(&app(), "1.1", &b, None, &data).unwrap();
        let entry = std::fs::read_to_string(data.join("applications/local-image.desktop")).unwrap();
        assert!(entry.contains(&b.display().to_string()));

        // Old path gone: everything we wrote is removed.
        std::fs::remove_file(&b).unwrap();
        ensure_registered_in(&app(), "1.1", &d.path().join("gone/local-image"), None, &data).unwrap();
        assert!(!data.join("applications/local-image.desktop").exists());
        assert!(!data.join("icons/hicolor/128x128/apps/local-image.png").exists());
        assert!(!marker_path(&data, &app()).exists());
    }

    #[test]
    fn bare_binary_gets_minimal_entry() {
        let d = tempfile::tempdir().unwrap();
        let data = d.path().join("data");
        let exe = d.path().join("Downloads/local-image");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"bin").unwrap();
        ensure_registered_in(&app(), "1.0", &exe, None, &data).unwrap();
        let entry = std::fs::read_to_string(data.join("applications/local-image.desktop")).unwrap();
        assert!(entry.contains("Name=Local Image"));
        assert!(entry.contains(&exe.display().to_string()));
    }
}
