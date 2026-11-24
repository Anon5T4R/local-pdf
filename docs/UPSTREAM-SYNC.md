# Upstream sync

The Local suite's port/rebrand: main is ours, the upstream is a source of candidate changes and
nothing enters without review. General rules in `docs/planos/licoes-de-porta.md` (dev-notes).

## Base

- Upstream: `storytold/pdfcraft`, commit **64e0868b** (`feat(ui): menus from craft-menu, native on
  macOS (#4752)`, 2026-10-26). The tree started from that commit in the fork (`f8e52c5`, Oct 26),
  with no history.
- Proof (checked 2026-11-21 against the shallow clone at `C:\Dev\storytold\pdfcraft`): we carry the
  change from `64e0868b` — 8 of the 9 testable files of `01b696df2..64e0868b` match (e.g.
  `crates/ui-egui/tests/it/editing.rs`; the ninth is `Cargo.toml`, which is ours) — and **none** of
  the 46 testable files from the 11 following commits on `main` entered. The first child,
  `d28d047a9` (the engine split), is not here: `crates/engine/src/tests.rs` still imports only
  `super::*`, without `super::document::is_placeholder_title`/`super::edit::check_permission`.
- The hashes cited in `ROADMAP.md` (`99941daa`, `33c14e07`, `a6bc63a`) come from entries inherited
  from the upstream (their internal rebases); the `merge-base` of the three is `33c14e07` (Oct 10)
  and says nothing about our base — the base is 16 days later, on the `main` of Oct 26.
- Fork: `Styx-Studios/Local-PDF`, own version on the `0.14.x` line.
- Nothing is read from `C:\Dev\storytold` in build, test or release; the folder is only a local
  reference for the `sync-upstream` skill.

## Vendored

An upstream fork this app vendors and whose sync this document covers; the other vendored forks
(`craft-libs`, `egui`, `winit`, `local-ai`) have their rev in `AGENTS.md`.

| Crate | Upstream | Rev | What entered |
|-------|----------|-----|--------------|
| `libs/winprint.rs` | `storytold/winprint.rs` | `f084d35` (2026-11-21) | PDF job by stream (`17f5f1e`, `b334ebb`, `03a4fb6`, `800a6f9`), page rendered at the job's resolution (`6f3012d`), multi-thread Direct2D factory (`f256775`), XML ticket (`2390a37`, `cbffb07`), `PrintTicket::from_dev_mode` and `PrinterDevice::is_xps_driver` unused for now (`8ae097e`, `bc06a7c`) and the teardown of the tests' file ports (`f084d35`) |

Left out: the upstream's CI, StepSecurity and Harden-Runner commits, and `6758393` (makes the
PDFium download's build-dependencies optional behind the `pdfium` feature), which only touches
build's `Cargo.toml`/`build.rs` — the `build.rs` here is still the one from `5f80b9b`.

## Vendored craft-update: the suite's permanent fork (outside the sync)

`libs/craft-libs/crates/update` is **out of scope** for this upstream sync. It is our crate, hosted
in the Local suite's `craft-libs`, and a **conscious permanent fork** of the upstream
`craft-update` — unified across the 10 apps on 2026-11-13, with a diff of thousands of lines
against `storytold/craft-libs/crates/update` (MSI context/migrate, launcher pins, `offers_update`,
`desktop_register`, single app catalog). No sync diff fits it.

Bugfix and feature go **straight into the single source** — repo `Styx-Studios/local-libs` (crate
`crates/update`) — and from there are replicated to the apps by the sync script (`sync.ps1 -Mode
push`, ×10 hash guard; the copy vendored here is a mirror). Never edit the local copy directly: the
next sync overwrites it. Upstream contributions only through a **manual Issue/PR**, in English,
when the problem also exists there — the same policy as local-office's `UPSTREAM-CONTRIBUTIONS.md`.
The rest of `craft-libs` follows the normal flow of this document.
