//! The automation root flags every Craft app takes, and the containment check they promise.
//!
//! ```text
//! --automation-read-root DIR   (may be given more than once; `--flag=DIR` also works)
//! --automation-write-root DIR
//! ```
//!
//! A path inside any of its kind's folders is allowed; a kind left out grants none of its access,
//! so `--automation-write-root` alone grants no reads and `--automation-read-root` alone grants no
//! writes. The flags are the same in every app of the suite, so one MCP client configuration works
//! for all of them.
//!
//! [`Roots`] is the check itself, for an app that has no equivalent of its own. An app that does
//! (Local Vector's engine confines the whole process, not just the automation calls) keeps its own
//! enforcement and uses [`RootFlags`] only to read the command line the same way.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

/// Read access or write access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
}

/// The folders automation may read and write; a kind with no root grants no access of its kind.
#[derive(Debug)]
pub struct Roots {
    read: Vec<Root>,
    write: Vec<Root>,
}

#[derive(Debug)]
struct Root {
    /// The folder made absolute, links followed: what paths are compared with.
    resolved: PathBuf,
    /// The same without Windows' `\\?\` prefix, for people.
    shown: PathBuf,
}

impl Root {
    /// The folder at `path`, which must exist.
    fn open(path: &Path, access: Access) -> Result<Self, String> {
        let kind = match access {
            Access::Read => "read",
            Access::Write => "write",
        };
        let resolved = std::fs::canonicalize(path)
            .and_then(|p| if p.is_dir() { Ok(p) } else { Err(std::io::ErrorKind::NotADirectory.into()) })
            .map_err(|e| format!("cannot open the automation {kind} root `{}`: {e}", path.display()))?;
        Ok(Self { shown: plain(resolved.clone()), resolved })
    }
}

impl Roots {
    /// The roots `read` and `write`, which must be existing folders (one given twice counts once);
    /// `None` when neither kind has any (nothing is confined then).
    pub fn new(read: &[&Path], write: &[&Path]) -> Result<Option<Self>, String> {
        if read.is_empty() && write.is_empty() {
            return Ok(None);
        }
        let open = |paths: &[&Path], access: Access| -> Result<Vec<Root>, String> {
            let mut roots: Vec<Root> = Vec::with_capacity(paths.len());
            for path in paths {
                let root = Root::open(path, access)?;
                if !roots.iter().any(|r| r.resolved == root.resolved) {
                    roots.push(root);
                }
            }
            Ok(roots)
        };
        Ok(Some(Self { read: open(read, Access::Read)?, write: open(write, Access::Write)? }))
    }

    /// May the file or folder at `path` be read (or written)? It must lie inside one of the roots
    /// of that kind. `Err` says why not.
    pub fn check(&self, path: &str, access: Access) -> Result<(), String> {
        let roots = match access {
            Access::Read => &self.read,
            Access::Write => &self.write,
        };
        if roots.is_empty() {
            return match access {
                Access::Read => Err(format!("automation filesystem access is not granted: read authority is absent: {path}")),
                Access::Write => Err(format!("automation filesystem access is not granted: write authority is absent: {path}")),
            };
        }
        let resolved = resolve(path)?;
        if roots.iter().any(|r| inside(&resolved, &r.resolved)) {
            return Ok(());
        }
        let shown: Vec<String> = roots.iter().map(|r| r.shown.display().to_string()).collect();
        match (shown.as_slice(), access) {
            ([root], Access::Read) => Err(format!("automation path rejected: outside the read root {root}: {path}")),
            ([root], Access::Write) => Err(format!("automation path rejected: outside the write root {root}: {path}")),
            (_, Access::Read) => Err(format!("automation path rejected: outside the read roots {roots}: {path}", roots = shown.join(", "))),
            (_, Access::Write) => Err(format!("automation path rejected: outside the write roots {roots}: {path}", roots = shown.join(", "))),
        }
    }

    /// The roots for people: `read A, B; write C` (`none` for a kind without any).
    pub fn describe(&self) -> String {
        let list = |roots: &[Root]| {
            if roots.is_empty() { "none".to_string() } else { roots.iter().map(|r| r.shown.display().to_string()).collect::<Vec<_>>().join(", ") }
        };
        format!("read {}; write {}", list(&self.read), list(&self.write))
    }
}

/// The root flags of one command line, split from the rest of its arguments.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RootFlags {
    pub read: Vec<String>,
    pub write: Vec<String>,
}

impl RootFlags {
    /// Split the `--automation-read-root` / `--automation-write-root` flags out of `args`, keeping
    /// every other argument in order. Both `--flag DIR` and `--flag=DIR` are taken; a flag with no
    /// folder is an error. `--` ends the flags: what follows is left to the caller.
    pub fn take(args: &[String]) -> Result<(Self, Vec<String>), String> {
        let mut roots = Self::default();
        let mut rest = Vec::with_capacity(args.len());
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            if arg == "--" {
                rest.extend(it.by_ref().cloned());
                break;
            }
            let (flag, inline) = match arg.split_once('=') {
                Some((f, v)) if f.starts_with("--automation-") => (f, Some(v.to_string())),
                _ => (arg.as_str(), None),
            };
            match flag {
                "--automation-read-root" | "--automation-write-root" => {
                    let dir = inline.or_else(|| it.next().cloned()).ok_or_else(|| format!("{flag} needs a folder"))?;
                    if flag == "--automation-read-root" {
                        roots.read.push(dir);
                    } else {
                        roots.write.push(dir);
                    }
                }
                _ => rest.push(arg.clone()),
            }
        }
        Ok((roots, rest))
    }

    /// Nothing was asked for.
    pub fn is_empty(&self) -> bool {
        self.read.is_empty() && self.write.is_empty()
    }

    /// [`Roots::new`] from these flags (the folders as given, which must exist).
    pub fn roots(&self) -> Result<Option<Roots>, String> {
        let read: Vec<&Path> = self.read.iter().map(Path::new).collect();
        let write: Vec<&Path> = self.write.iter().map(Path::new).collect();
        Roots::new(&read, &write)
    }
}

/// `path` made absolute with its links followed; for a path that doesn't exist (yet), its nearest
/// existing folder's links followed and the rest appended.
fn resolve(path: &str) -> Result<PathBuf, String> {
    if path.is_empty() {
        return Err("automation path rejected: path is empty".into());
    }
    // Checked as given: making `.\NUL` absolute gives the device path `\\.\NUL` on Windows.
    if cfg!(windows)
        && let Some(why) = windows_name_refusal(path)
    {
        return Err(format!("automation path rejected: {why}: {path}"));
    }
    let absolute = std::path::absolute(path).map_err(|e| format!("automation path rejected: cannot resolve {path}: {e}"))?;
    // The names below the nearest existing folder, last first.
    let mut missing: Vec<OsString> = vec![];
    let mut at = absolute.as_path();
    loop {
        match std::fs::canonicalize(at) {
            Ok(mut found) => {
                found.extend(missing.iter().rev());
                return Ok(found);
            }
            Err(e) => match std::fs::symlink_metadata(at) {
                Ok(m) if m.file_type().is_symlink() => return Err(format!("automation path rejected: a link that leads nowhere: {path}")),
                // There, yet its real path can't be had (no permission): nothing to compare.
                Ok(_) => return Err(format!("automation path rejected: cannot resolve {path}: {e}")),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("automation path rejected: cannot resolve {path}: {e}")),
            },
        }
        let parent = at.parent();
        match (at.components().next_back(), parent) {
            (Some(Component::Normal(name)), Some(parent)) => {
                missing.push(name.to_os_string());
                at = parent;
            }
            (Some(Component::ParentDir), _) => {
                return Err(format!("automation path rejected: parent traversal below a folder that doesn't exist: {path}"));
            }
            // Down to the root (or drive) and it doesn't exist either.
            _ => {
                let e = std::io::Error::from(std::io::ErrorKind::NotFound);
                return Err(format!("automation path rejected: cannot resolve {path}: {e}"));
            }
        }
    }
}

/// Is `path` (resolved) `root` or inside it, component by component (without regard to case in a
/// Windows drive prefix only: resolved folder names have their real case, and a folder that differs
/// in case alone may be another, on a case-sensitive Windows folder)?
fn inside(path: &Path, root: &Path) -> bool {
    let mut path = path.components();
    root.components().all(|r| path.next().is_some_and(|p| same_component(p, r)))
}

fn same_component(a: Component, b: Component) -> bool {
    if a == b {
        return true;
    }
    if !cfg!(windows) || !matches!((a, b), (Component::Prefix(_), Component::Prefix(_))) {
        return false;
    }
    match (a.as_os_str().to_str(), b.as_os_str().to_str()) {
        (Some(a), Some(b)) => a.to_lowercase() == b.to_lowercase(),
        _ => false,
    }
}

/// A Windows device name (`CON`, `NUL`, `COM1`. also with an extension), which names no file.
fn is_device_name(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or(component).trim_end_matches(' ').to_ascii_uppercase();
    let numbered = |prefix: &str| stem.strip_prefix(prefix).is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"));
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$") || numbered("COM") || numbered("LPT")
}

/// Why Windows would not take `path` as an ordinary file name, for the first of its components
/// that is a device name, or holds a `:` (a drive letter's own colon belongs to the prefix, not to
/// a name, so only a stream such as `x.svg:evil` has one). (Judged by name alone, so it can be
/// tested on any platform; [`resolve`] applies it on Windows only.)
fn windows_name_refusal(path: &str) -> Option<&'static str> {
    Path::new(path).components().find_map(|c| match c {
        Component::Normal(n) if is_device_name(&n.to_string_lossy()) => Some("reserved device names are not allowed"),
        Component::Normal(n) if n.to_string_lossy().contains(':') => Some("alternate data streams are not allowed"),
        _ => None,
    })
}

/// `path` without Windows' verbatim (`\\?\`) prefix, for messages.
fn plain(path: PathBuf) -> PathBuf {
    let Some(s) = path.to_str() else { return path };
    let stripped = s.strip_prefix(r"\\?\UNC\").map(|r| format!(r"\\{r}")).or_else(|| s.strip_prefix(r"\\?\").map(str::to_string));
    match stripped {
        Some(clean) => PathBuf::from(clean),
        None => path,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// A fresh folder holding `in` (read root), `out` (write root) and `outside`.
    fn folders(name: &str) -> (PathBuf, Roots) {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!("craft-mcp-roots-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        for d in ["in/sub", "out", "outside", "in2"] {
            std::fs::create_dir_all(base.join(d)).unwrap();
        }
        std::fs::write(base.join("in/a.svg"), "<svg/>").unwrap();
        std::fs::write(base.join("outside/secret.svg"), "<svg/>").unwrap();
        let roots = Roots::new(&[&base.join("in")], &[&base.join("out")]).unwrap().unwrap();
        (base, roots)
    }

    fn s(p: PathBuf) -> String {
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn paths_inside_the_roots_pass_and_others_dont() {
        let (base, roots) = folders("inside");
        assert_eq!(roots.check(&s(base.join("in/a.svg")), Access::Read), Ok(()));
        assert_eq!(roots.check(&s(base.join("in")), Access::Read), Ok(()), "the root itself");
        assert_eq!(roots.check(&s(base.join("in/sub/new.svg")), Access::Read), Ok(()), "missing files are checked by their folder");
        assert_eq!(roots.check(&s(base.join("out/new/deeper/x.png")), Access::Write), Ok(()), "and by their nearest folder");
        let e = roots.check(&s(base.join("outside/secret.svg")), Access::Read).unwrap_err();
        assert!(e.starts_with("automation path rejected: outside the read root"), "{e}");
        // Read and write access are separate.
        let e = roots.check(&s(base.join("in/a.svg")), Access::Write).unwrap_err();
        assert!(e.contains("outside the write root"), "{e}");
        assert!(roots.check(&s(base.join("out/x.png")), Access::Read).unwrap_err().contains("outside the read root"));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn a_sibling_with_the_same_prefix_is_outside() {
        let (base, roots) = folders("sibling");
        let e = roots.check(&s(base.join("in2/a.svg")), Access::Read).unwrap_err();
        assert!(e.contains("outside the read root"), "{e}");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn a_kind_left_out_grants_nothing() {
        let (base, _) = folders("kinds");
        let read_only = Roots::new(&[&base.join("in")], &[]).unwrap().unwrap();
        assert!(read_only.check(&s(base.join("in/a.svg")), Access::Write).unwrap_err().contains("write authority is absent"));
        let write_only = Roots::new(&[], &[&base.join("out")]).unwrap().unwrap();
        assert!(write_only.check(&s(base.join("in/a.svg")), Access::Read).unwrap_err().contains("read authority is absent"));
        assert!(Roots::new(&[], &[]).unwrap().is_none(), "no roots, no confinement");
        assert!(Roots::new(&[&base.join("missing")], &[]).is_err(), "a root must exist");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn windows_names_are_refused_by_name_on_every_platform() {
        for bad in [r"in/NUL", r"in/a.svg:evil", r"in/com1.txt"] {
            assert!(windows_name_refusal(bad).is_some(), "{bad}");
        }
        assert!(windows_name_refusal("in/ok.svg").is_none());
    }

    #[test]
    fn the_flags_split_off_and_keep_the_rest_in_order() {
        let args: Vec<String> =
            ["--connect", "7979", "--automation-read-root", "a", "--automation-write-root=b,c", "--automation-read-root", "d", "--headless"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        let (roots, rest) = RootFlags::take(&args).unwrap();
        assert_eq!(roots.read, ["a", "d"]);
        assert_eq!(roots.write, ["b,c"]);
        assert_eq!(rest, ["--connect", "7979", "--headless"]);
        assert!(!roots.is_empty());

        let e = RootFlags::take(&["--automation-read-root".to_string()]).unwrap_err();
        assert!(e.contains("needs a folder"), "{e}");
        let (none, rest) = RootFlags::take(&["--headless".to_string()]).unwrap();
        assert!(none.is_empty());
        assert_eq!(rest, ["--headless"]);
    }

    #[test]
    fn a_double_dash_ends_the_flags() {
        let args: Vec<String> = ["--", "--automation-read-root", "x"].iter().map(|s| s.to_string()).collect();
        let (roots, rest) = RootFlags::take(&args).unwrap();
        assert!(roots.is_empty());
        assert_eq!(rest, ["--automation-read-root", "x"]);
    }

    #[test]
    fn describe_names_both_kinds() {
        let (base, roots) = folders("describe");
        let text = roots.describe();
        assert!(text.starts_with("read "), "{text}");
        assert!(text.contains("; write "), "{text}");
        let _ = std::fs::remove_dir_all(base);
    }
}
