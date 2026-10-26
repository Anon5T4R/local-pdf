//! Dependency-graph verification for the egui 0.37 window stack.
//!
//! The twelve `winit_*` modules next to this one read the retained `vendor/winit` /
//! `vendor/egui-wgpu` copies. They are provenance for W1–W13 and E2+E3, not proof of what the app
//! runs: the root manifest resolves both crates from the vendored forks under `libs/` (path
//! sources), so the build never touches a network host. These tests check the resolved graph and
//! the retained evidence instead of treating the copies as the runtime source. The winit fork's
//! own W17 tests own patch behaviour.

/// Return the workspace root above the LocalPDF app crate.
fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `[[package]]` block in `Cargo.lock` whose `name` is exactly `name`.
fn locked_packages(name: &str) -> Vec<String> {
    let lock = std::fs::read_to_string(root().join("Cargo.lock")).expect("Cargo.lock");
    let needle = format!("name = \"{name}\"");
    lock.split("[[package]]").filter(|package| package.lines().any(|line| line.trim() == needle.as_str())).map(str::to_owned).collect()
}

/// The one resolved package called `name`. A duplicate name is a lockfile problem, not something
/// to resolve by taking the first match.
fn locked(name: &str) -> String {
    let mut packages = locked_packages(name);
    assert_eq!(packages.len(), 1, "Cargo.lock must contain exactly one `{name}` package, found {}", packages.len());
    packages.remove(0)
}

/// The resolved package must be a local path source (no `source` line in `Cargo.lock`): the
/// vendored fork under `libs/`, never a git URL. A git source would make the graph depend on a
/// remote host; a published path patch under `vendor/` is refused as well.
fn assert_resolved_local(name: &str, version: &str) {
    let package = locked(name);
    let expected_version = format!("version = \"{version}\"");
    assert!(package.lines().any(|line| line.trim() == expected_version.as_str()), "{name} must resolve to {version}: {package}");
    let sources: Vec<&str> = package.lines().filter_map(|line| line.trim().strip_prefix("source = ")).collect();
    assert!(sources.is_empty(), "{name} must resolve from a local path (no `source` line in Cargo.lock), not {}", sources.join(", "));

    let manifest = std::fs::read_to_string(root().join("Cargo.toml")).expect("root Cargo.toml");
    assert!(!manifest.contains(&format!("{name} = {{ path = \"vendor/")), "vendor/{name} cannot stay a published path patch");
    assert!(!manifest.contains(&format!("{name} = {{ git =")), "{name} must not be a git dependency");
}

/// Regression test: the resolved window stack comes from the vendored forks under `libs/` and the
/// retained vendor provenance files remain.
#[test]
fn vendor_provenance_window_stack_graph_is_resolved() {
    assert_resolved_local("winit", "0.30.13");
    assert_resolved_local("egui-wgpu", "0.37.0");
    // Retained provenance: the audited copies and their recorded W1–W13 / E2+E3 rows.
    let readme = std::fs::read_to_string(root().join("vendor/README.md")).expect("vendor/README.md");
    for marker in ["W1–W13", "surface_fit", "catch_errors", "e9809ef54b18499bb4f2cac945719ecc2a61061b"] {
        assert!(readme.contains(marker), "vendor/README.md lost window-stack provenance: {marker}");
    }
    let vcs = std::fs::read_to_string(root().join("vendor/winit/.cargo_vcs_info.json")).expect("vendor/winit provenance");
    assert!(vcs.contains("e9809ef54b18499bb4f2cac945719ecc2a61061b"), "vendor/winit keeps its upstream base");
    for path in [
        "vendor/winit/src/platform_impl",
        "vendor/egui-wgpu/src/winit.rs",
        "libs/winit/src/platform_impl",
        "libs/egui/crates/egui-wgpu/src/winit.rs",
    ] {
        assert!(root().join(path).exists(), "retain {path}");
    }
}

/// Renderer, security and PostScript vendors are unrelated to the UI port and must stay path patches.
#[test]
fn renderer_and_security_vendors_remain() {
    let manifest = std::fs::read_to_string(root().join("Cargo.toml")).expect("root Cargo.toml");
    for vendor in [
        "vendor/hayro",
        "vendor/hayro-interpret",
        "vendor/hayro-syntax",
        "vendor/hayro-cmap",
        "vendor/hayro-jbig2",
        "vendor/hayro-postscript",
        "vendor/lopdf",
        "vendor/vello_common",
    ] {
        assert!(manifest.contains(&format!("path = \"{vendor}\"")), "{vendor} must stay a path patch");
    }
}
