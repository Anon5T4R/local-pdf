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
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    // #3: o runtime AppImage exporta APPIMAGE/APPDIR no ambiente — que pode
    // ter sido herdado de OUTRO AppImage (launcher, terminal). Só confia no
    // par se ESTE processo veio do mount em questão.
    if let (Some(p), Some(dir)) = (std::env::var_os("APPIMAGE"), std::env::var_os("APPDIR"))
        && let Ok(mount) = PathBuf::from(&dir).canonicalize()
        && exe.starts_with(&mount)
    {
        let p = PathBuf::from(p);
        // #4: AppImage apagado enquanto roda — o env aponta pra um
        // arquivo que não existe e current_exe é o mount efêmero
        // (/tmp/.mount_XXX). Nada durável pra registrar.
        return p.is_file().then_some(LaunchTarget::AppImage(p));
    }
    if !exe.is_file() {
        return None;
    }
    // Mount efêmero sem env válido: idem #4.
    if exe.starts_with("/tmp/.mount_") {
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
    if s.starts_with("/usr/") || s.starts_with("/opt/") || s.starts_with("/nix/") {
        return Ok(()); // Package-manager install ships its own entry (/nix: profile owns it).
    }
    let data = folders::data_local_dir().ok_or("no data dir")?;
    ensure_registered_in(app, version, target, appdir, &data)
}

/// Write-to-temp-then-rename in the same directory: a crash mid-write (or a
/// concurrent instance) never leaves a truncated file where menus and icon
/// caches read them. The temp name carries the pid so parallel instances
/// don't fight over it.
fn atomic_write(path: &Path, contents: &str) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, contents)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

fn atomic_copy(src: &Path, dst: &Path) -> std::io::Result<()> {
    let tmp = dst.with_extension(format!("tmp{}", std::process::id()));
    std::fs::copy(src, &tmp)?;
    match std::fs::rename(&tmp, dst) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
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
    let _ = atomic_write(&p, &format!("{}\n{version}\n{desktop}\n{icon}\n", target.display()));
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
/// Deliberately no fallback: in a shared applications dir (`~/.local/share`,
/// the whole suite), "first entry that didn't match" is ANOTHER app's —
/// cloning it would rewrite that app's desktop to run this binary.
fn discover_desktop(share: &Path, app: &App) -> Option<(String, String)> {
    let dir = share.join("applications");
    for entry in std::fs::read_dir(dir).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".desktop") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else { continue };
        let program = crate::desktop_entry::exec_program(&text);
        if let Some(p) = program.as_deref()
            && Path::new(p).file_name().is_some_and(|f| f == app.id)
        {
            let icon = icon_of(&text).unwrap_or_else(|| app.id.to_string());
            return Some((name, icon));
        }
    }
    None
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
    // Fluxo install.sh em ~/.local: os ícones já estão NO data dir — copiar
    // cada arquivo sobre ele mesmo zera o conteúdo (fs::copy abre o destino
    // com truncate). Nada a fazer.
    if let (Ok(a), Ok(b)) = (share.canonicalize(), data.canonicalize())
        && a == b
    {
        return;
    }
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
                let _ = atomic_copy(&src, &dst.join(format!("{icon}.{ext}")));
            }
        }
    }
}

fn remove_entry(data: &Path, desktop: &str, icon: &str) {
    let _ = std::fs::remove_file(data.join("applications").join(desktop));
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
    let mut old: Option<(String, String, String, String)> = read_marker(data, app);
    if let Some((prev, _v, desktop, icon)) = old.clone()
        && !Path::new(&prev).is_file()
    {
        remove_entry(data, &desktop, &icon);
        let _ = std::fs::remove_file(marker_path(data, app));
        old = None;
    }
    // Fast path: same target, same version, entry still there.
    let installed = data.join("applications");
    if let Some((prev, v, desktop, _icon)) = old.clone()
        && prev == target.display().to_string()
        && v == version
    {
        let entry = installed.join(&desktop);
        if entry.is_file() {
            return Ok(());
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
    // Cópia nova com outro nome de entrada (layout diferente, rename no
    // empacotamento): remove a antiga, senão vira morta no menu pra sempre.
    if let Some((_prev, _v, old_desktop, old_icon)) = &old
        && *old_desktop != name
    {
        let _ = std::fs::remove_file(installed.join(old_desktop));
        if old_icon != &icon
            && let Ok(levels) = std::fs::read_dir(data.join("icons/hicolor"))
        {
            for level in levels.flatten() {
                for ext in ["png", "svg", "svgz"] {
                    let _ = std::fs::remove_file(level.path().join("apps").join(format!("{old_icon}.{ext}")));
                }
            }
        }
    }
    std::fs::create_dir_all(&installed).map_err(|e| e.to_string())?;
    atomic_write(&installed.join(&name), &text).map_err(|e| e.to_string())?;
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
        let (name, icon) = discover_desktop(&share, app()).unwrap();
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
        ensure_registered_in(app(), "1.0", &a, None, &data).unwrap();
        let entry = std::fs::read_to_string(data.join("applications/local-image.desktop")).unwrap();
        assert!(entry.contains(&a.display().to_string()));
        assert!(data.join("icons/hicolor/128x128/apps/local-image.png").is_file());
        assert!(marker_path(&data, app()).is_file());

        // Same target+version: still there (fast path, nothing to assert but no error).
        ensure_registered_in(app(), "1.0", &a, None, &data).unwrap();

        // Moved file: entry follows the new path.
        ensure_registered_in(app(), "1.1", &b, None, &data).unwrap();
        let entry = std::fs::read_to_string(data.join("applications/local-image.desktop")).unwrap();
        assert!(entry.contains(&b.display().to_string()));

        // Old path gone: everything we wrote is removed.
        std::fs::remove_file(&b).unwrap();
        ensure_registered_in(app(), "1.1", &d.path().join("gone/local-image"), None, &data).unwrap();
        assert!(!data.join("applications/local-image.desktop").exists());
        assert!(!data.join("icons/hicolor/128x128/apps/local-image.png").exists());
        assert!(!marker_path(&data, app()).exists());
    }

    #[test]
    fn bare_binary_gets_minimal_entry() {
        let d = tempfile::tempdir().unwrap();
        let data = d.path().join("data");
        let exe = d.path().join("Downloads/local-image");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"bin").unwrap();
        ensure_registered_in(app(), "1.0", &exe, None, &data).unwrap();
        let entry = std::fs::read_to_string(data.join("applications/local-image.desktop")).unwrap();
        assert!(entry.contains("Name=Local Image"));
        assert!(entry.contains(&exe.display().to_string()));
    }

    #[test]
    fn icons_survive_when_share_is_the_data_dir() {
        // Fluxo install.sh em ~/.local: shipped_share == data. Copiar cada
        // ícone sobre ele mesmo o zerava (fs::copy com truncate).
        let d = tempfile::tempdir().unwrap();
        let data = d.path().join("data");
        let bin = d.path().join("bin/local-image");
        std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
        std::fs::write(&bin, b"bin").unwrap();
        // layout FHS: <root>/bin + <root>/share, instalado EM data
        std::fs::create_dir_all(data.join("applications")).unwrap();
        std::fs::create_dir_all(data.join("icons/hicolor/128x128/apps")).unwrap();
        std::fs::write(data.join("applications/local-image.desktop"), "Exec=local-image %F\nIcon=local-image\n").unwrap();
        std::fs::write(data.join("icons/hicolor/128x128/apps/local-image.png"), b"PNG-BYTES").unwrap();
        ensure_registered_in(app(), "1.0", &bin, None, &data).unwrap();
        let icon = std::fs::read(data.join("icons/hicolor/128x128/apps/local-image.png")).unwrap();
        assert_eq!(icon, b"PNG-BYTES");
    }

    #[test]
    fn no_temp_files_left_behind() {
        // Escritas atômicas (tmp+rename): nenhum resíduo tmpN sobra na árvore
        // do data dir — menus/caches leem esses arquivos a qualquer momento.
        let d = tempfile::tempdir().unwrap();
        let data = d.path().join("data");
        let a = d.path().join("a");
        std::fs::create_dir_all(a.join("bin")).unwrap();
        std::fs::write(a.join("bin/local-image"), b"bin").unwrap();
        layout(&a.join("share"), "local-image.desktop", "local-image", "local-image", true);
        ensure_registered_in(app(), "1.0", &a.join("bin/local-image"), None, &data).unwrap();
        let mut leftovers = Vec::new();
        for e in walk_files(&data) {
            if e.file_name().to_string_lossy().contains("tmp") {
                leftovers.push(e.path());
            }
        }
        assert!(leftovers.is_empty(), "sobrou tmp: {leftovers:?}");
        // e o ícone copiado chegou inteiro
        assert!(data.join("icons/hicolor/128x128/apps/local-image.png").is_file());
    }

    fn walk_files(dir: &Path) -> Vec<std::fs::DirEntry> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                if e.path().is_dir() {
                    out.extend(walk_files(&e.path()));
                } else {
                    out.push(e);
                }
            }
        }
        out
    }

    #[test]
    fn shared_share_leaves_foreign_entries_alone() {
        // Share compartilhado (~/.local/share: a suíte inteira mora lá) sem
        // entrada PRÓPRIA deste app. O fallback antigo devolvia o primeiro
        // .desktop alheio, clonava e sobrescrevia a entrada do OUTRO app com
        // o Exec deste binário. Sem fallback: entrada própria mínima, a
        // alheia intocada byte a byte.
        let d = tempfile::tempdir().unwrap();
        // layout FHS em <d>: bin + share; data == share (compartilhado)
        let bin = d.path().join("bin/local-image");
        std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
        std::fs::write(&bin, b"bin").unwrap();
        let data = d.path().join("share");
        std::fs::create_dir_all(data.join("applications")).unwrap();
        let foreign = data.join("applications/local-keys.desktop");
        let foreign_text = "[Desktop Entry]\nType=Application\nName=Local Keys\nExec=local-keys %F\nIcon=local-keys\nStartupWMClass=local-keys\n";
        std::fs::write(&foreign, foreign_text).unwrap();
        ensure_registered_in(app(), "1.0", &bin, None, &data).unwrap();
        // este app ganha entrada própria (mínima, Exec absoluto)
        let mine = std::fs::read_to_string(data.join("applications/local-image.desktop")).unwrap();
        assert!(mine.contains(&bin.display().to_string()), "entrada própria com Exec absoluto");
        // a entrada alheia permanece exatamente como estava
        assert_eq!(std::fs::read_to_string(&foreign).unwrap(), foreign_text, "entrada de outro app deve ficar intacta");
    }

    #[test]
    fn renamed_entry_replaces_the_old_one() {
        // Cópia nova com nome de .desktop diferente (layout variando entre
        // versões): a entrada antiga sai, senão vira órfã no menu.
        let d = tempfile::tempdir().unwrap();
        let data = d.path().join("data");
        // primeira cópia: layout "flat" (<a>/bin + <a>/share com local-image.desktop)
        let a = d.path().join("a");
        std::fs::create_dir_all(a.join("bin")).unwrap();
        std::fs::write(a.join("bin/local-image"), b"bin").unwrap();
        layout(&a.join("share"), "local-image.desktop", "local-image", "local-image", true);
        ensure_registered_in(app(), "1.0", &a.join("bin/local-image"), None, &data).unwrap();
        assert!(data.join("applications/local-image.desktop").is_file());
        // segunda cópia: layout rDNS (com.local.localimage.desktop)
        let b = d.path().join("b");
        std::fs::create_dir_all(b.join("bin")).unwrap();
        std::fs::write(b.join("bin/local-image"), b"bin").unwrap();
        layout(&b.join("share"), "com.local.localimage.desktop", "local-image", "com.local.localimage", true);
        ensure_registered_in(app(), "1.1", &b.join("bin/local-image"), None, &data).unwrap();
        assert!(data.join("applications/com.local.localimage.desktop").is_file());
        assert!(!data.join("applications/local-image.desktop").exists(), "entrada antiga deve sumir");
    }
}
