# egui 0.37 migration plan

Status: planning only. No production code, Cargo manifest, lockfile, branch, PR or
release is changed by this document. No build or test was run while writing it.

Scope: this repository only (`storytold/egui`). Consumer applications, `craft-libs`
and `storytold/winit` are dependencies of the plan, not repositories changed here.
The cross-repository hand-off points are called out explicitly so they cannot be
mistaken for local work.

The suite-wide acceptance protocol is `craft-libs/docs/egui-037-suite-rollout.md`.
It governs dependency publication, matched baseline/candidate measurements and
cross-application checks; stricter repository gates continue to apply.

## 1. Decision in one page

Move the fork from the coherent 0.36.2 family to **egui 0.37.0 at commit
`841a328ba2dd8f6996beb7189ebb7fc00e668a2a`**, then reapply the fork patches in
reviewable stages on top of that exact commit.

The target is the 0.37.0 release, not `origin/main` (`bf5ac0dac137af9a4861611f3d2a77ad531f50e3` at inspection
time), not a later 0.37.x snapshot, and not aiongg's branch tip wholesale. The
aiongg branch is evidence and a source of reviewed commits; it is not an
implementation that may be merged without auditing each commit and replacing its
mutable `winit` branch pin with an immutable revision.

The migration has two independent classes of work:

1. **Upstream 0.37 adaptation.** `default_fonts` is no longer a default feature,
   `WidgetType` is a deprecated alias for `accesskit::Role`, `Id::new` and
   `Ui::id` are deprecated, panel constructors take `AsIdSalt` and panel show
   methods were renamed, screenshots use `ScreenshotCallback`, and AccessKit
   names/roles are stricter. These are not fork features and must not be hidden
   behind compatibility shims.
2. **Craft fork behavior.** The 0.36.2 fork has E1-E4; the aiongg 0.37 branch
   carries E1-E21 (with E7 effectively folded into E19). The fork behavior must
   be preserved or deliberately retired, patch by patch, with tests that fail
   when the behavior is removed.

The largest technical risk is not the Cargo update. It is the text stack:
0.37.0 has a rewritten font/atlas/layout implementation, while E5, E12-E16 and
E20 change `LayoutJob`, `Row`, glyph atlas allocation, line breaking, hit testing
and cursor movement. Those patches must be rebased as text-stack changes, not
mechanically cherry-picked and squashed.

## 2. Baseline, exact revisions and evidence

### 2.1 Current checkout

- `HEAD`: `767b1cad294120689a514bb4bd68c077e46679b9`
  (`PATCHES.md: the suite's patch list`), branch
  `codex/egui-037-plan-20261022`, based on `craft/0.36.2`.
- `origin/craft/0.36.2` is the same commit. The checkout is clean.
- The current fork content is E1-E4 only, documented in `PATCHES.md`:
  - E1 `e7e37ad580bd2e6cd1e3be80f5f8c78af9c0c9d8`
  - E2+E3 `b02321d7405585d2ab76076d3bc2b199fedb4d0a`
  - E4 `c01acc2acd2514f8830ba779dbbfc901b9931ff5`
- No repository-level `AGENTS.md`, `.agents` content or nested build instructions
  exist in this checkout. The governing instructions are the planning contract and
  `/home/codex/.codex/AGENTS.md`.

### 2.2 Target and reference revisions

| Role | Revision | Notes |
|---|---|---|
| Upstream target | `841a328ba2dd8f6996beb7189ebb7fc00e668a2a` (`0.37.0`) | Annotated tag object `418687fa18fd`; release date 2026-10-15 |
| Post-release `main` | `bf5ac0dac137` | Not the target; do not use post-0.37 APIs by accident |
| aiongg egui branch | `a4f9a17f64a6bae61350fb85804c9df0d9865916` (`aiongg/craft/0.37`, same as `origin/ai-0.37`) | E1-E21 reference, 62 commits from 0.37.0, including merges |
| Earlier 0.37 trial | `59163e291c56ebe0199e191c23bf58ed157c9862` (`origin/craft/0.37.0-trial`) | E1-E4 only; historical, not the target |
| storytold winit base | `ffae5198770d7730e718043f035a772b7dbc3b1b` (`craft/0.30.13`) | W1-W15 |
| aiongg winit language branch | `8b35388bd719e8cc86a95971ba0bc026c9a8f058` (`craft/0.30.13-lang`) | W17-W27 on top of `ffae5198` |
| craft-libs PR 199 head | `b68de8c5f4f1c060904d211a7b1320aa0f7bce78` | OPEN / CONFLICTING at inspection; API-port ancestor `8614e4a04fae516fba16c22b31ff1008cb6be10b` |
| craft-libs audit merge | `ec8840af8c5fda695cf195cdd05e5ae078415c07` | Merge of `refs/review/pr199` into `audit/main-plus-199` |

Historical evidence that must not be reported as fresh validation:

- `/home/codex/work/ssd2/task-evidence/craft-fork-egui037-20261021/RESULTS.md`
  records that E1 and E2+E3 cherry-picked cleanly onto 0.37.0, E4 conflicted,
  upstream #8657 applied cleanly minus its temporary git pin, and the PRIMARY
  selection concern exists.
- `/home/codex/work/ssd2/task-evidence/craft-fork-ai-lines-20261022/SURVEY.md`
  records the aiongg branch and winit branch contents at 2026-10-22.
- Neither file is a substitute for rebuilding the final candidate.

### 2.3 Dependency identity

The 0.37.0 lockfile resolves the whole egui family at version `0.37.0`:
`ecolor`, `emath`, `epaint`, `epaint_default_fonts`, `egui`, `egui-winit`,
`egui-wgpu`, `egui_glow`, `egui_extras`, `egui_demo_lib`, `egui_kittest`,
`egui_inspection`, `eframe`. The workspace has no `[patch.crates-io]` section at
0.37.0. The current fork adds no vendor directory. The reference aiongg branch
adds:

- `unicode-bidi 0.3.18` (registry, checksum
  `5c1cb5db39152898a79168971543b1cb5020dff7fe43c8dc468b0885f5e29df5`) for E5/E12.
- `winit` from
  `git+https://github.com/aiongg/winit?branch=craft%2F0.30.13-lang#8b35388b...`
  in its lockfile.

Replace that mutable branch reference with the final reviewed/published immutable
`storytold/winit` revision containing W15P and the W27 lifecycle fixes. The inspected
`8b35388bd719e8cc86a95971ba0bc026c9a8f058` is comparison input only. Keep `dpi` on registry
`0.1.2` (the winit branch deliberately uses registry `dpi = "0.1.1"` resolution,
which resolves the same compatible family in the lockfile). Any consumer root
that depends on this fork must repeat the `winit` patch: Cargo `[patch]` sections
are not transitive through a dependency.

### 2.4 Vendor and patch inventory

- This repository has no `vendor/` or `vendored/` directory at `767b1cad` or
  `841a328b`. It also has no machine-local path patch. The only dependency
  redirection in the fork reference is `aiongg/craft/0.37`'s
  `[patch.crates-io] winit` entry, which must become an immutable `rev` pin.
- Consumer repositories are where vendored copies exist. The survey records
  `storytold/pdfcraft` carrying `vendor/egui-wgpu` and `vendor/winit`; E2+E3
  came from that vendored `egui-wgpu` (#609/#617/#1042). When pdfcraft (or
  another consumer) adopts this fork, its duplicate vendored `egui-wgpu` and
  the corresponding winit patch must be removed or reconciled in the same
  change, or the consumer will build two egui-wgpu/winit copies.
- `craft-libs` does not vendor egui at PR 199; it uses registry 0.37 packages.
  Its lockfile must be re-resolved against the final fork pin in the consumer
  plan, not copied from this repository's lockfile.
- Do not remove a consumer's other vendor patches merely because this repository
  has the same behavior. E2/E3 are vendor-derived; E4 is a port of upstream #8657. These have
  different provenance and removal conditions; each consumer needs a line-by-line vendor disposition.

## 3. Repository inventory and acceptance targets

### 3.1 Workspace packages

Workspace members at 0.37.0 (`Cargo.toml`):

`crates/ecolor`, `crates/emath`, `crates/epaint`,
`crates/epaint_default_fonts`, `crates/egui`, `crates/egui-winit`,
`crates/egui-wgpu`, `crates/egui_glow`, `crates/egui_extras`,
`crates/egui_demo_app`, `crates/egui_demo_lib`, `crates/egui_inspection`,
`crates/egui_kittest`, `crates/eframe`, `crates/egui_system_font_provider`,
`examples/*`, `tests/*`, `xtask`.

`crates/egui_plot` is present in the tree but is **not** a workspace member at
0.37.0. Do not infer its version or compatibility from the workspace; consumer
pins must be checked separately. `crates/egui_web` is only a legacy README and
changelog; it is not a crate target.

### 3.2 Entry points that must be exercised

- egui-only: `Context::run_ui`, `Context::run_pass`,
  `Context::begin_pass`/`Context::end_pass`, `Context::root_ui`,
  `Context::run_logic`.
- eframe native: `run_native`, `create_native`, `EframeWinitApplication`,
  root/deferred/immediate viewports, hidden-window logic path.
- eframe web: `WebRunner`/`AppRunner`, hidden-tab logic path, text agent,
  screen reader, browser glyph rasterizer.
- egui-winit: `State::on_window_event`, `take_egui_input`, clipboard and
  PRIMARY paths, IME events, AccessKit output.
- egui-wgpu: `RenderState::create`, `Painter::set_window`,
  `Painter::paint_and_update_textures`, screenshot capture, surface errors.
- egui_kittest: `Harness`, accesskit queries, snapshots, `eframe` app harness,
  optional `video` recording.
- Browser demo: `web_demo` and `crates/egui_demo_app` wasm builds.
- Example binaries: `confirm_exit`, `custom_3d_glow`, `custom_font`,
  `custom_font_style`, `custom_keypad`, `custom_style`, `custom_window_frame`,
  `external_eventloop`, `external_eventloop_async`, `file_dialog`,
  `font_variations`, `hello_android`, `hello_world`, `hello_world_par`,
  `hello_world_simple`, `images`, `keyboard_events`, `multiple_viewports`,
  `popups`, `puffin_profiler`, `screenshot`, `serial_windows`,
  `styling_engine`, `svg_glyph`, `user_attention`.
- Test binaries: `tests/egui_tests`, `tests/test_background_logic`,
  `tests/test_egui_extras_compilation`, `tests/test_inline_glow_paint`,
  `tests/test_inspection_hidden_window`, `tests/test_size_pass`,
  `tests/test_ui_stack`, `tests/test_viewports`; `xtask` and `web_demo` are
  support targets, not end-user binaries.

### 3.3 Feature targets

The plan must explicitly test these feature boundaries, because several 0.37
changes are feature-gated:

- `egui`/`epaint`: default (no `default_fonts`), `default_fonts`,
  `monochrome_emoji_fonts`, `color_fonts`, `serde`, `persistence`, `rayon`,
  `bytemuck`, `experimental`, `web_file`, `_override_unity`.
- `eframe`: default (`accesskit`, `clipboard`, `default_fonts`, `links`,
  `system_fonts`, `wayland`, `web_screen_reader`, `wgpu`, `x11`), `glow`,
  `wgpu_no_default_features`, `persistence`, `inspection`, `android-*`.
- `egui-winit`: default, `wayland`, `x11`, `clipboard` on/off, `accesskit`,
  `links`, `serde`; `smithay-clipboard` and `arboard` combinations.
- `egui-wgpu`: default, `winit`, `wayland`, `x11`, `capture`,
  `macos-window-resize-jitter-fix`. `wgpu_no_default_features` is an eframe
  feature, not an egui-wgpu feature.
- `egui_kittest`: `wgpu`, `snapshot`, `video`, `eframe`, `x11`.

Important 0.37 behavior: `egui` and `epaint` default to **no bundled fonts**.
`eframe` still enables `default_fonts` and adds `system_fonts`. A direct `egui`
consumer that relied on implicit bundled fonts now gets tofu unless it enables
`default_fonts` or installs a `FontProvider`. `egui_kittest` sets
`MissingGlyphPolicy::Panic` by default, so an unsupported glyph becomes a test
failure rather than a silent box.

### 3.4 Existing gates and tests

The 0.37 repository's own gates are the baseline:

- `cargo fmt --all -- --check`, `./scripts/lint.py`.
- `cargo clippy --locked --all-features --all-targets`.
- `cargo clippy --locked --all-features -p egui_extras`.
- `cargo clippy --locked --all-targets` and the no-default-features variants in
  `.github/workflows/rust.yml`.
- `cargo doc --lib --no-deps --all-features` and the private-items variant.
- `cargo test --all-features --no-fail-fast`, then doctests, then
  `cargo test -p egui_extras` without `syntect`.
- wasm: `scripts/clippy_wasm.sh`, `scripts/wasm_bindgen_check.sh`, eframe and
  egui-winit wasm checks, wasm-bindgen version `0.2.126`.
- `cargo-deny`, `cargo-shear`, `taplo`, `typos`, link checker.
- Cross checks: Windows MSVC/GNU, Android, iOS, wasm.
- Existing UI tests: `tests/egui_tests/tests/test_text_selection.rs` (34
  `#[test]`s), `test_widgets`, `test_panel_drag`, `test_window_drag`,
  `test_primary_selection`, `test_range_slider`, `test_completion`,
  `test_style_provider`, `test_svg_glyph`; `crates/egui_kittest/tests/*`
  (including `accessibility_check`, `accesskit`, `missing_glyphs`, `menu_focus`,
  `popup`, `discard_invisible_accessibility`); `crates/epaint/src/text/text_layout.rs`
  has 36 `#[test]`s before the fork patches.
- `crates/epaint/benches/benchmark.rs` currently benchmarks tessellation and
  shapes, not text layout. A text-layout/bidi benchmark must be added for this
  migration; the existing benchmark alone is insufficient.

### 3.5 Platform cfg matrix

The source has these cfg boundaries; each needs its own compile/run row:
- `egui-winit`: `target_os = "windows"` (virtual keys, clipboard, IME),
  `target_os = "macos"` (command-key layout, IME delete, window chrome),
  `target_os = "linux"` + `feature = "x11"`/`"wayland"` (activation token,
  clipboard, Latin layout), `target_os = "android"`/`"ios"`, and all `unix`
  BSD variants used by `clipboard.rs`.
- `eframe`: `target_arch = "wasm32"` vs native; `feature = "glow"` vs
  `"wgpu_no_default_features"`; Android/iOS/macOS/Windows target deps.
- `egui-wgpu`: `feature = "winit"`, `wayland`, `x11`, `capture`; native-only
  `surface_fit`/error-scope code must not leak into web builds.
- `epaint`: `target_arch != "wasm32"` for the vello multithreading dev test;
  `color_fonts` and `default_fonts` change glyph rasterization.
- `egui`: `default_fonts`, `serde`, `persistence`, `rayon`, `web_file` and
  `experimental` are independent boundaries; `web_file` is wasm-only.

## 4. Upstream 0.37 delta and migration traps

### 4.1 API and behavior changes that force edits

| Area | 0.37 source anchor | Consequence |
|---|---|---|
| UI lifecycle | `crates/egui/src/context.rs:Context::run_ui`, `Context::run_pass`, `Context::root_ui`, `ContextImpl::run_dyn` | `run_ui` creates/reuses the root `Ui` on demand; `root_ui` panics while the root is borrowed; `request_discard` can call the UI callback twice (`Options::max_passes`, default 2) |
| Logic-only | `crates/egui/src/context.rs:Context::run_logic` | No pass, no widget garbage collection, no animation advance, no focus loss; only window state is updated |
| Repaint observation | `Context::set_repaint_observer`, `RequestRepaintInfo`, `cumulative_frame_nr`, `cumulative_pass_nr`, `current_pass_index` | Use these for tests and for detecting multi-pass frames instead of wall-clock guesses |
| App split | `crates/eframe/src/epi.rs:App::logic`, `App::ui` | `logic` runs before every `ui` and may run more than once per visible frame; `ui` gets the root `Ui` |
| Hidden windows | `crates/eframe/src/native/epi_integration.rs:EpiIntegration::update_logic_only`, `crates/eframe/src/web/app_runner.rs:AppRunner::logic` | No pass while hidden; `RequestPaintWhileHidden` is required to run UI/paint anyway |
| Screenshot | `crates/egui/src/data/screenshot_callback.rs:ScreenshotCallback`, `Context::request_screenshot` | One-shot callback; clones share it; no old request-id/channel coupling; callback may run off the UI thread |
| IDs | `crates/egui/src/id.rs:Id::unique`, `Id::new` (deprecated), `IdSalt`, `AsIdSalt` | Deprecation is a warning; CI uses `-D warnings`, so old calls fail CI, not just lint locally |
| Ui IDs | `crates/egui/src/ui.rs:Ui::scope_id`, `Ui::unique_id`, `UiBuilder::scope_id` | `Ui::id()` and `UiBuilder::id()` are deprecated; `unique_id` is the globally unique, unstable ID |
| Roles | `crates/egui/src/lib.rs:WidgetType` alias and `accesskit::Role` re-export | `WidgetType::TextEdit` is `Role::TextInput`, `DragValue` is `SpinButton`, `ColorButton` is `ColorWell`, `CollapsingHeader` is `DisclosureTriangle`, `SelectableLabel` is `Button`, `Panel` is `Pane`, `ResizeHandle` is `Splitter`, `Other` is `Unknown` |
| Accessible names | `crates/egui/src/response.rs:Response::accessible_name`, `labelled_by`, `crates/egui/src/ui.rs:Ui::label_inputs_by`, `Ui::name_inputs` | `egui_kittest` fails an unnamed input widget; every icon-only button, unlabeled text field and custom control needs a name |
| Panels | `crates/egui/src/containers/panel.rs:Panel::show`, `show_switched`, `show_collapsible` | `show_inside`, `show_animated_inside`, `show_animated_between_inside` are deprecated; panel constructors take `impl AsIdSalt` |
| Fonts | `crates/egui/Cargo.toml` / `crates/epaint/Cargo.toml` `default_fonts` off by default | Direct users must opt in or install a provider; `eframe` default enables it, plus `system_fonts` |
| Clipboard | `crates/egui-winit/src/clipboard.rs:Clipboard::new` is `unsafe` | Any fork port must preserve the safety contract and the new PRIMARY API |
| Web files | `DroppedFile::web_file` opt-in | Browser file-drop code must enable `web_file` explicitly |
| Primary selection | `tests/egui_tests/tests/test_primary_selection.rs` | Upstream 0.37 added PRIMARY; E4 changes the Wayland backend, so this test is a required regression gate |

### 4.2 0.37 text-stack changes that collide with the fork

The 0.37 release rewrote or split:

- `epaint/src/text/fonts.rs` and `font.rs` into per-type files (`FaceStore`,
  `Family`, `GlyphAtlas`, `FontProvider`, `MissingGlyphPolicy`,
  `font_definitions`).
- `Glyph`/`Row` layout and atlas allocation; `FontFace` is much smaller and
  `GlyphAtlas` owns caches.
- `text_layout.rs` and `text_layout_types.rs` around rows, sections, clusters,
  `LayoutJob` hashing and cache keys.
- `egui/src/text_selection/*`, `accesskit_text.rs`, and the new
  `test_text_selection.rs`.

E5 changes the same files. A cherry-pick of `0e27d87e` alone will conflict; the
plan must rebase it commit-by-commit and then re-audit the result against the
0.37 `FontsImpl`/`FaceStore` split. Do not restore 0.36.2 layout files.

### 4.3 Migration traps checklist

- Search all struct literals after adding fields: `LayoutJob {`, `Row {`,
  `Glyph {`, `RawInput {`, `TextRun {`, `Paragraph {`. The fork adds
  `LayoutJob::direction`, `LayoutJob::language`, `LayoutJob::halign_min_is_start`,
  `Row::rtl_paragraph`, `Row::visual_order`, `Glyph::bidi_level`, and
  `RawInput::keyboard_layout_is_latin`/`latin_layout_chars`.
- Add new fields to `LayoutJob`'s `Hash` and every galley cache key. Missing
  `direction` or `language` creates stale cross-language galleys and
  non-deterministic snapshots.
- Preserve `TextShape`'s row destructuring: `visual_order` must be carried or
  reset correctly; a transformed text shape must not silently fall back to
  logical order.
- Keep the ASCII fast path in `BidiLevels::new`. ASCII-only LTR layout must not
  call `unicode-bidi`, split runs, allocate level vectors, or reorder rows.
- `TextDirection::RightToLeft` must force direction even for ASCII; do not use
  the `is_ascii` early return before checking `direction`.
- `LayoutJob::halign_min_is_start` defaults to `true`; `Label` must set it to
  `false` for explicit `halign` and for text continuing a wrapping row. A
  missing reset flips LTR labels placed after RTL content.
- `Row::visual_order` is a permutation of glyph indices; it must be invalidated
  if glyphs are changed after layout. `TextShape` transform and any test that
  edits rows must update or clear it.
- `LayoutJob::language` must flow through `GalleyCache` paragraph splitting;
  otherwise multiline text loses the language after the first paragraph.
- `FontTweak::fallback_align` must default to `Center`; defaulting to
  `Baseline` changes existing emoji/icon layout and invalidates snapshots.
- `Style::right_to_left` must default to `false`, and must not be implied by
  `Context::set_language`; language and UI direction are separate decisions.
- `Frame::set_ui_language` changes from `&mut self` to `&self` in E18. Consumer
  code taking `&mut Frame` still compiles, but code storing a `Frame` clone or
  relying on the old signature needs review.
- `winit` APIs used by E19/E21 exist only on the patched branch/targets. cfg
  gates must exclude Android/iOS/emscripten/Redox and wasm.
- `Clipboard::new` remains `unsafe`; E4's early return must initialize
  `arboard`, leave `smithay: None`, and preserve `get_image`/`set_image`.
- E2's `render_scale` must multiply `pixels_per_point` exactly once in
  `ScreenDescriptor`; applying it in both layout and paint scales twice.
- E3's `catch_errors` must pop scopes in the reverse order and must not be used
  for later reconfigures where the app already has an error handler.
- `ScreenshotCallback` is one-shot and shared by clones. Tests must assert one
  call per request, not one per clone, and must handle a callback that is never
  called (window closed/hidden).

### 4.4 Post-release reports to reproduce

These are dated upstream reports/proposals, not confirmed failures in this task.

- [#8731](https://github.com/emilk/egui/issues/8731): measure plain-text
  tessellation separately from layout, against old fork, stock 0.37 and candidate.
  If reproduced, compare a row-level color-glyph fast path and cached metadata;
  mixed/color emoji rows must keep identical rendering semantics.
- [#8736](https://github.com/emilk/egui/issues/8736): measure DX12 shader/pipeline
  construction separately from first frame. An override constant or equivalent
  wrap arithmetic may reduce compilation work; clamp-only arithmetic loses
  Repeat/MirroredRepeat behavior and is not an acceptable optimization.
- [#8763](https://github.com/emilk/egui/issues/8763) and proposed
  [#8764](https://github.com/emilk/egui/pull/8764): reproduce selection followed
  by a toolbar click. Preserve the intended selection for the action, test
  keyboard/pointer/context-menu paths, and review any backport independently.
- [#8748](https://github.com/emilk/egui/pull/8748) was closed unmerged. Test
  deferred-child open/close and background completion while the parent is
  hidden; do not describe its draft behavior as a release fix or run arbitrary
  hidden UI by default.
- [#8512](https://github.com/emilk/egui/pull/8512): measure system-font discovery
  and first missing-glyph lookup independently; retain the app's required glyph
  coverage when choosing an explicit provider policy.

For E5/E12/E13, pin conformance data to the selected dependency versions:
`unicode-bidi` 0.3.18 declares Unicode 16 and `unicode-segmentation` 1.13.3 declares
Unicode 17. Test the fork's logical/visual adapter and editing boundaries, not
only the libraries in isolation. For E21/W27, assert fresh outbound surrounding
text after applying deletion/commit, valid UTF-8 boundaries, deletion-only
batches and focus/reset/serial changes. Keep macOS UTF-16 NSRange conversion
distinct from Wayland byte indices and app scalar/grapheme indices.

## 5. Lifecycle and root-`Ui` design

### 5.1 Boundaries

Use these boundaries explicitly:

- **External work**: consume queue messages once; advance playback/autosave from
  monotonic elapsed time; poll audio/background work even when no paint occurs.
  Calling the hook again must not replay a consumed command or duplicate an undo step.
- **Pass / UI**: build panels and widgets from immutable or snapshot state.
- **Render**: tessellation, texture upload, GPU submission, screenshot readback.
  Never create GPU resources or drain app queues from a paint callback.
- **Accessibility tree**: produced during the pass from the same widget tree;
  language and names are pass state, not a post-pass mutation.

`App::logic` already exists in 0.36.2. It runs before `App::ui` and also via
`Context::run_logic` while hidden; its call count is not a physical-frame clock.
`App::ui` receives the root `Ui`. User actions produced there must commit through
the application's command path once, even if layout is replayed.

### 5.2 Multi-pass (`request_discard`)

`Context::run_dyn` calls the UI callback once per pass, up to
`Options::max_passes` (default 2). `App::logic` is called inside that callback by
`EpiIntegration::update` and `AppRunner::logic`, so it can run twice in one
visible frame. Preserve that documented behavior. This migration does not add
an E22 scheduler API or globally skip repeated `App::logic` calls.

Consume external queues and command IDs once, use monotonic elapsed-time clocks
for background work, and process UI input only in a stage known to have fresh
input. After a completed `run_ui`, `end_pass` has taken the output, so
`current_pass_index()` is zero even after multiple passes. A first-pass guard can
serve narrowly defined frame work under this pinned integration, but it does not
prove fresh input. Neither it nor `viewport().visible()` distinguishes a logic-only
tick from every UI case: hidden UI can be explicitly requested. Do not derive hidden-window ticks from stale input timestamps.
Per-consumer tests must force two UI passes and then several logic-only iterations:
the hook may run repeatedly, but one user action yields one edit/undo step, a new
queued result is consumed once, and background time continues advancing. Include
occluded/minimized Wayland and browser-hidden cases. A new scheduler contract would
require a separate demonstrated defect, design review and integration acceptance.

Do not solve this by moving side effects into `App::ui` and hoping the pass runs
once. That breaks hidden-window behavior and makes clicks/animations the clock.

### 5.3 Root `Ui`, panels and IDs

`Context::run_ui` now calls `run_pass` and `root_ui`, and the root `Ui` is stored
in the viewport's pass state (`ViewportState::root_ui`, `RootUi`). Consequences:

- `App::ui` gets the real root `Ui`; use `Panel::top(...).show(ui, ...)`,
  `Panel::left(...).show(ui, ...)` and `CentralPanel::default().show(ui, ...)`.
- `Context::root_ui` is for integrations that only have a `Context` and need to
  add a panel/plugin. It is **not** a replacement for passing `&mut Ui`; it
  panics if the root is already borrowed. Split UI code should take `&mut Ui`
  through its call graph, or use one `run_pass` callback that calls
  `ctx.root_ui` exactly once.
- Panel order is the order panels are shown against the same root `Ui`. Nested
  panels must use the panel's child `Ui`, not the context.
- `Window` and `Area` still take `&Context` (`Window::show(ctx, ...)`,
  `Area::show(ctx, ...)`) and use layers; do not convert them to root-`Ui`
  panels.
- Panel IDs use `AsIdSalt`; use stable string salts or `Id::unique` for global
  IDs. Never derive a panel ID from translated text.
- `Ui::scope_id` is the stable scope for stateful children. `Ui::unique_id` is
  globally unique but depends on widget order and is explicitly unstable; use
  it for transient widget/accessibility relationships, never persisted state.
- `Context::root_ui_id` is `Id::unique((ViewportId, "__top_ui"))`; do not
  recreate the old 0.36 ephemeral ID scheme.

### 5.4 Logic-only and side effects

`Context::run_logic` intentionally leaves UI state untouched. The plan must
verify, for each consumer:

- queue polling still runs while minimized/occluded and does not duplicate on
  subsequent visible passes;
- playback/audio clock advances once per hidden wake and does not stall because
  no paint happened;
- autosave is driven by `EpiIntegration::maybe_autosave` or the app's own
  interval, not by `ui` (eframe's `maybe_autosave` is outside `run_ui`);
- undo transactions are committed only when a document mutation actually
  happened, not when a discarded pass merely re-rendered; command dispatch,
  undo grouping and persistence keys stay app-owned and stable;
- `request_repaint` from logic is observed even if the window is hidden; use
  `Context::set_repaint_observer` / `RequestRepaintInfo` in tests;
- `RequestPaintWhileHidden` is sent when a tool/screenshot needs a real UI pass
  on a hidden window.

### 5.5 Panels, viewports and screenshots

- Panel regression tests must assert order, nesting, reserved size, clipping and
  stable IDs at 1.0/1.5/2.0 pixels per point.
- Viewport tests must cover root, deferred and immediate windows, embedded
  fallback, close requests, and E1's `window_created`/`window_destroyed`/
  `raw_input` callbacks exactly once per viewport.
- Screenshot tests must attach a request token in the closure, request two
  screenshots with different tokens, complete them out of order, and assert one
  callback per token and the correct viewport's pixels. A clone of
  `ScreenshotCallback` must not cause a second invocation.
- Downstream integration tests use the shared completion-slot contract: callback
  holds only a weak reference to a live request; cancellation/timeout/close or
  document replacement releases the slot; a late callback makes no file or second
  reply. Bound aggregate pending image bytes/count. Keep wake handles in the live
  slot, release locks before waking the correct viewport, and never strongly
  capture a Context inside a callback queued in that Context. Test completion
  from another thread, two viewports, reused viewport/document generations,
  out-of-order completion, shutdown and cancellation. Core one-shot tests and
  these app-adapter tests have different ownership; both are required.
- `Window`/`Area` docking-like layer behavior remains in `craft-ui`; do not add
  an egui-core docking abstraction in this migration.

## 6. Fork patch audit (E1-E21)

### 6.1 Provenance and final shape

The aiongg branch is a merge history, not a linear patch series. For the final
branch, reapply logical patches in dependency order and preserve original commit
authorship. Do not merge `a4f9a17f` as a single commit.

E7 and E19 overlap: E7 (`0670c58a`) adds the Windows virtual-key letter rule;
E19 (`d64047bf`, `642b74f7`, `baeb4515`) replaces that helper with the shared
`letter_key`/`latin_key` rule and extends it to Linux/macOS. The final patch list
should have **E7+E19 as one logical patch** (with the Windows test cases retained)
and should not apply E7 separately. E12 is a follow-up series on E5; E5 must land
first.

| ID | aiongg commit(s) | Crate/files | Upstream position at inspection | Final decision / removal condition |
|---|---|---|---|---|
| E1 | `8f3abf06` | eframe; `epi.rs`, `native/viewport_hooks.rs`, glow/wgpu integration, `lib.rs` | Not proposed | Keep. Remove only if an upstream viewport-hook API with equivalent per-viewport raw-input ordering exists. |
| E2+E3 | `9dd799ab` | egui-wgpu; `lib.rs`, `winit.rs` | Not proposed; E2 relates to emilk/egui#8361 | Keep. Remove E2 if upstream fits surfaces; remove E3 if upstream returns equivalent `WgpuError` variants. |
| E4 | `66bacf23` | egui-winit; `clipboard.rs` | emilk/egui#8657 draft | Keep on 0.37 until #8657 and winit W15 are in a release used here. PRIMARY behavior remains a gate. |
| E5 | `0e27d87e`, `777977b7`, `f7e89e23`, `5b2b8e19` | epaint, egui; text layout, font face, atlas, selection | Port of emilk/egui#8577 (open draft), includes #8698 review fixes | Keep. Re-evaluate when #8577 merges; do not drop the tests even if code is replaced. |
| E6 | `0abfabcf` | egui; `memory/mod.rs`, `text_edit/builder.rs` | Not proposed | Keep. Candidate for upstream. |
| E7 | `0670c58a` | egui-winit; `lib.rs`, Cargo | Not proposed; subsumed by E19 | Fold into E19, retain Windows table. |
| E8 | `1b03c10f` | egui; `text_edit/builder.rs` | Not proposed | Keep. Candidate for upstream; test is required. |
| E9 | `b2f5cafc`, `e1341a3d`, `5105330d`, `75a5b3d3` | egui; style, menu, popup, scroll area, window, collapsing, combo, checkbox, radio, resize | Not proposed | Keep as one RTL-chrome logical patch. |
| E10 | `dc90c13e`, `c8e573f7`, `34b2c0d4` | eframe web; `web_keys.rs`, `input.rs`, `events.rs`, `text_agent.rs`, `screen_reader.rs`, `epi.rs` | Not proposed | Keep. Browser runtime is a required gate; unit tests alone are not wasm proof. |
| E11 | `92e61afd` | epaint; `font_face.rs`, Cargo dev-dependency | emilk/egui#8641 open draft (flush only); patch is broader | Keep until an upstream equivalent; add a multithreading feature test. |
| E12 | `e955bb18`, `485c0b89`, `0185887c`, `7015ac23`, `4067b2c4`, `620e070e` | epaint, egui; `LayoutJob`, rows, reorder, elision, L1, selection | Not proposed; follow-up to #8577 | Keep, split into reviewable commits. Do not merge into E5 until the L1/isolate tests pass. |
| E13 | `5e4da7f4` | epaint, egui; grapheme module, cursor, text buffer | Not proposed | Keep. Backspace-by-code-point is deliberate; retain the table test. |
| E14 | `cbd77549`, `b3722ac0` | epaint, egui; font tweak, metrics, layout, style UI | Not proposed | Keep. Default `FallbackAlign::Center` is a compatibility guarantee. |
| E15 | `f1cae609` | epaint, egui; line breaker trait, language, cache | Not proposed | Keep. Public trait; document craft-text adapter and cache invalidation. |
| E16 | `b3ef9771`, `bed6dfe3` | epaint, egui; caret, cursor, selection, text edit | Not proposed | Keep. Largest cursor-behavior change; needs hit-test/IME tests. |
| E17 | `520435df` | egui; Context language, AccessKit text runs, response, Ui/UiBuilder | Not proposed | Keep. Re-audit against 0.37 #8581/#8604/#8608/#8612 accessibility changes. |
| E18 | `89de8f3c` | eframe; `epi.rs`, native/web integration | Fork-only | Keep; depends E17 and E10. Could be proposed upstream. |
| E19 | `d64047bf`, `642b74f7`, `baeb4515` (plus E7) | egui-winit, egui; key handling, RawInput, InputState | Fork-only until winit has the APIs | Keep as one patch; requires winit W25 and Windows `MapVirtualKeyExW`. |
| E20 | `23955535` | egui; `widget_text.rs`, Label, TextEdit, image | Fork-only; goes with E15/E17 | Keep. |
| E21 | `a70470b0` | egui-winit, egui; IME event, TextEdit | Not proposed; winit master has core `Ime::DeleteSurrounding` | Keep until winit 0.31/core API move; requires W26/W27. |

### 6.2 Patch-by-patch audit notes and required tests

**E1 — eframe `ViewportHooks`.** Anchor:
`crates/eframe/src/native/viewport_hooks.rs:ViewportHooks`,
`NativeOptions::viewport_hooks`, `init_native` install, and the calls in
`glow_integration`, `wgpu_integration`, `epi_integration`.
- Keep the static `RwLock<Option<Arc<dyn ViewportHooks>>>`; it is process-global
  and must be replaced when `NativeOptions` changes. Tests must not run in
  parallel with different hook installs unless they isolate the process.
- `window_created` must fire once for the root and each child; glow's
  `initialize_window` runs every frame, hence the existing "new for this
  viewport" check.
- `window_destroyed` must fire from the `retain` paths in both backends, not
  from a render error.
- `raw_input` must run before `App::raw_input_hook` and for immediate viewports;
  assert the hook's mutation is visible to `app.raw_input_hook` and the app.
- Required tests: replace hooks and assert old hook stops; deferred/immediate
  child with a hook; root and child windows; raw input mutation;
  `window_created` once after 10 frames; destruction once on close.

**E2+E3 — egui-wgpu surface fit and error return.** Anchors:
`crates/egui-wgpu/src/winit.rs:surface_fit`, `catch_errors`,
`Painter::add_surface`, `Painter::install_surface`,
`Painter::resize_and_generate_depth_texture_view_and_msaa_view`,
`ScreenDescriptor` scaling; `crates/egui-wgpu/src/lib.rs:WgpuError::ConfigureSurface`,
`CreateRenderer`, `RenderState::create`.
- `surface_fit` must preserve zero sides, clamp nonzero sides to 1, make the
  long side exactly `max_side`, and return scale `1.0` for in-range sizes.
- `add_surface` must remove the surface and depth/MSAA views on configure error
  and return `WgpuError::ConfigureSurface`; it must not leave a half-installed
  surface.
- `catch_errors` must catch Validation, OutOfMemory and Internal, return the
  first in that priority, pop scopes inner-to-outer, and not catch device loss
  or panics. Add a unit test using a deliberately invalid device operation inside
  `catch_errors`; the test asserts `Err` and that a subsequent valid operation
  still works.
- Required tests: `surface_fit` edge table; `render_scale` only once;
  oversized window renders (or returns an error without panic); root configure
  failure leads to `Error::Wgpu` and the app can fall back to glow; later
  reconfigure and zero-size first window are unchanged.

**E4 — Wayland clipboard.** Anchors:
`crates/egui-winit/src/clipboard.rs:Clipboard::new`, `get`, `set_text`,
`get_primary_text`, `set_primary_text`, `init_winit_wayland_clipboard`.
- The early return must initialize `arboard`, leave `smithay: None`, and set
  `winit_wayland`; only then return. Do not create a smithay data device.
- `get` falls back to arboard only for `NotFound`/`InvalidData` (absent/non-text offer).
  A successful empty text result is `Some("")` and never falls back. Test present-empty,
  absent and non-text regular clipboard separately. `set_text`
  returns after winit store even on error (preserve the current behavior and
  log).
- `get_image`/`set_image` remain arboard-only; this is a deliberate limitation.
- The candidate currently falls back to arboard for PRIMARY because E4 disables
  smithay. The final design replaces that gap with the new winit **W15P** path
  described in the companion plan: SCTK `PrimarySelectionManagerState` on the
  existing connection/event queue, with distinct per-seat PRIMARY protocol objects.
  It must not bind a second regular `wl_data_device`.
- E4's `get_primary_text`/`set_primary_text` use the W15P methods once available.
  Keep password exclusion and do not reclaim unchanged selections. Cover keyboard-
  only focus, serial freshness, owner/seat changes, empty/unavailable offers,
  self-paste and canceled/slow transfers. Preserve regular text, image and drop
  behavior. Required native evidence: wlroots/Hyprland, KWin, Mutter and X11.
- W15P lands and is reviewed before the separate E4 PRIMARY integration stage P2b.
  No silent loss of PRIMARY is an accepted default. If native evidence proves the
  design cannot satisfy a required platform, research and measure an isolated
  PRIMARY-only fallback; a documented unsupported limitation requires explicit
  acceptance, not a compile-only sign-off.
- P2b also replaces blocking W15/W15P reads on egui's event path with bounded
  asynchronous transfer handles. The candidate's two-second poll timeout is an
  inactivity timeout, not a total deadline, and per-send threads are unbounded.
  The companion fix supplies total deadlines, byte caps, bounded concurrency and
  cancellation for reads/writes. Egui records the requesting viewport, focus owner
  and an input generation, wakes that viewport on completion, and emits paste once
  only if the target is still current. Invalidate a pending request on another edit,
  focus/selection-changing action, owner change or close; mere repaint and release
  of the initiating shortcut must not cancel it. Test delayed regular/PRIMARY paste,
  self-paste, text/image fallback, trickle/oversized offers, cancellation and actual
  event-loop responsiveness. A synchronous compatibility API may remain bounded
  for other callers, but the egui event path must not wait for a slow peer.
- Completion routing uses a caller-owned `TransferHandle`, not a global event broadcast.
  `TransferOptions` carries `target_window: WindowId`, a caller request generation and
  a completion waker. Winit binds admission to the validated focused seat/origin
  for that target; no arbitrary first-seat fallback. The handle owns its result/cancel
  slot and wake closure; workers hold only Weak slot references. Egui-winit retains
  the handle in its per-window pending table with ViewportId, focused widget and input
  generation. The waker requests repaint of that viewport via Context's existing
  repaint callback to eframe's event-loop proxy. At that State's next input preparation,
  take/revalidate once and emit Paste. Closed/hidden targets cannot reroute to another
  State. Drop/cancel invalidates late completion and releases the wake closure; invoke
  wakes only after unlocking. No new generic WindowEvent/broadcast is required. Test
  two targets with delayed regular/PRIMARY transfers, focus movement before dispatch,
  close and request-ID reuse. Workers never inspect egui focus state.
- Copy/PRIMARY ownership claims remain synchronous on the triggering input turn:
  validate serial/seat, reserve admitted source bytes, create the source and call
  set_selection before reporting success. Only subsequent potentially blocking pipe
  reads/serving writes use bounded workers. A full admission budget errors before
  replacing ownership; serving overload cancels that transfer with a recorded error.
  Never delay set_selection behind slow I/O until its serial ages. Test queued slow
  transfers followed by keyboard Copy and pointer-triggered PRIMARY ownership.
- PRIMARY state tracks focus/offer/source and valid triggering serials per seat.
  A pointer-driven selection may use its triggering pointer serial; do not impose
  a keyboard-only serial rule. No arbitrary first-seat fallback is allowed.

**E5 — bidi shaping and row placement.** Anchors:
`crates/epaint/src/text/text_layout.rs:BidiLevels::new`, `layout_shaped_run`,
`emit_cluster_bitmap`, `reorder_row_visually`, `visual_order`/`visual_indices`;
`Glyph::bidi_level`, `Row::rtl_paragraph`, `Row::visual_order`.
- Keep logical glyph order and one glyph per char; marks/continuation glyphs
  must remain attached to their base.
- Keep the ASCII fast path: no bidi vector, no run split, no reorder for ASCII
  LTR.
- The `Row`/`Glyph` invariants are used by cursor, selection, AccessKit and
  `TextShape`; all must be updated together.
- Required tests: mixed Hebrew/Latin, Arabic digits/brackets, Arabic-Indic
  digits outside RTL, mark order, end caret, selection across direction
  boundaries, hit tests, `Row::text()` equals logical text, LTR snapshot
  unchanged.

**E6 — IME focus.** Anchors:
`crates/egui/src/memory/mod.rs:Memory::request_focus`,
`request_focus_next_frame`, `crates/egui/src/widgets/text_edit/builder.rs`.
- Re-requesting focus for the already-focused widget must not interrupt IME;
  moving focus or clicking/middle-pasting in a focused TextEdit must.
- Required test: two fields, re-request same focus, move focus, click, paste;
  assert `PlatformOutput::ime.should_interrupt_composition`.

**E7+E19 — letter keys.** Anchors:
`crates/egui-winit/src/lib.rs:latin_key`, `letter_key`, `key_char`,
`windows_virtual_key`, `key_from_winit_key`;
`crates/egui/src/data/input/raw_input.rs:keyboard_layout_is_latin`,
`latin_layout_chars`, `latin_layout_char`; `InputState` accessors.
- Windows: virtual-key letter replaces logical only when the layout does not
  type A-Z; Dvorak/AZERTY keep typed letters; nonletter VK keys (period, Enter)
  keep their named key.
- Linux: X11/Wayland `latin_key()` and `is_latin_layout()`; first Latin layout
  wins; Dvorak+Russian cases; punctuation that is not a letter must not become a
  letter shortcut.
- macOS: `key_with_command()` only while ⌘ is held; `Dvorak – QWERTY ⌘` gives
  QWERTY shortcuts; unmodified typed letters still win.
- `RawInput::append`/`take` must preserve `latin_layout_chars`; test ordering and
  `latin_layout_char` reverse lookup.
- Required tests: the existing table plus keyup release, `RawInput` append/take,
  Windows period/Enter, Linux no-Latin-layout, macOS unmodified Russian.
- P7 adds release pairing in egui-winit: remember the egui key actually emitted
  on keydown by physical key and use that identity for its release after a layout
  or modifier change. Keep live text/modifier reporting separate; do not redefine
  winit's modifier-sensitive `logical_key` globally. Clear tracking on focus loss,
  IME interception and viewport destruction. Define repeat and unidentified-key
  behavior explicitly, and exercise physical aliases before changing held-key
  semantics. Tests use ordinary keys and non-clipboard shortcuts (Ctrl+Z/Cmd+K):
  copy/paste shortcuts return before emitting egui Key-down and cannot by themselves
  prove a stuck `keys_down` entry. Include release-Command-before-letter, layout
  switch while held, repeats, focus loss and a subsequent fresh press.

**E8 — TextEdit vertical overshoot.** Anchor:
`crates/egui/src/widgets/text_edit/builder.rs` `text_clip_rect` and
`galley.mesh_bounds`.
- Keep horizontal clipping/scroll; extend only vertically.
- Required test: glyphs outside row but inside clip; horizontal clip still
  bounded; singleline and multiline.

**E9 — RTL chrome.** Anchors:
`crates/egui/src/style.rs:Style::right_to_left`, `set_right_to_left`,
`chrome_direction`; containers `menu.rs`, `popup.rs`, `scroll_area.rs`,
`window.rs`, `collapsing_header.rs`, `combo_box.rs`, `resize.rs`, widgets
`checkbox.rs`, `radio_button.rs`, `button.rs`, `ui.rs`.
- `Style::right_to_left` defaults false and must not alter LTR snapshots.
- Tests already cover submenu side/arrow, keys, popup alignment, collapsing
  icon/toggle, indent, combo arrow/label, checkbox/radio icon side, resize grip,
  window grip. Add title-bar close/collapse, scrollbar interaction rect vs
  painted rect, and nested RTL/LTR containers.
- E13/E17 do not replace E9: layout direction, text direction and chrome
  direction are separate.

**E10 — web keyboard and language.** Anchors:
`crates/eframe/src/web_keys.rs:is_named_key`, `text_from_key`,
`KeyResolver::key_down/key_up`, `letter_for_code`, `layout_is_latin`;
`crates/eframe/src/web/input.rs:KeyboardKeys::read_layout_map`;
`crates/eframe/src/web/text_agent.rs:TextAgent::set_lang`;
`web/screen_reader.rs:speak`.
- `is_named_key` rule must not classify multi-character text (Arabic lam-alef)
  as a named key; `text_from_key` must reject `AltGraph`, `Dead`,
  `Unidentified`, `Process`, `Compose`, F-keys.
- Non-Latin layout resolution must use `code` for `KeyA`..`KeyZ`; layout
  detection from `navigator.keyboard.getLayoutMap()` when available, otherwise
  from typed letters; keyup must release the key resolved at keydown.
- `set_lang` must be idempotent, remove the attribute when `None`, and fall back
  from text input language to UI language.
- Browser gates: Chromium with Russian/Hebrew/Greek, Arabic lam-alef, AltGr,
  Korean IME, layout switch while a key is held, speech language. Unit tests are
  native and cannot prove wasm event behavior.

**E11 — vello multithreading.** Anchor:
`crates/epaint/src/text/font_face.rs:glyph_render_context`,
`RenderContext::new_with(RenderSettings { num_threads: 0 })`.
- The default must stay single-threaded per glyph even when `vello_cpu`'s
  `multithreading` feature is on elsewhere in the workspace.
- Required test: `cargo test -p epaint` with `vello_cpu/multithreading` enabled
  (the dev-dependency is present); render glyphs and assert no panic. A mutation
  that restores `RenderContext::new` should reproduce the panic.

**E12 — direction, L1, elision, linear reorder.** Anchors:
`LayoutJob::direction`, `TextDirection`, `LayoutJob::halign_min_is_start`,
`BidiLevels`, `reset_whitespace_levels`, `reorder_row_visually`,
`replace_last_glyph_with_overflow_character`, `Row::visual_order`.
- `Auto` follows the first strong character; forced LTR/RTL overrides it.
- L1: tabs, separators and trailing whitespace take the paragraph level; RTL row
  wrap spaces go to the left end.
- L2: reverse level runs, not individual glyphs; zero-width controls at another
  level are separate blocks; `visual_order` must be a permutation.
- Elision `…` at the visual end, not logical end.
- Required tests: the existing `paragraph_direction_*`, `neutral_paragraphs_*`,
  `whitespace_at_the_end...`, `reordering_a_long_mixed...`,
  `zero_width_bidi_controls...`, `elision...`, and the selection/caret tests.
  Add a 10k-char mixed row step-count assertion and a no-sort assertion.

**E13 — grapheme editing.** Anchors:
`crates/epaint/src/text/grapheme.rs:previous_boundary`, `next_boundary`,
`backspace_start`, `cluster_around`; `TextBuffer::delete_previous_char`,
`delete_next_char`; cursor/word motion.
- Backspace removes the last code point of the preceding cluster (except whole
  emoji/CRLF sequences and variation selectors with their base); Delete removes
  the cluster after the cursor. This is deliberate and must be documented.
- Required tests: the existing table plus click at every x, word motion,
  double-click selection, empty/out-of-range, and IME interaction.

**E14 — fallback baseline.** Anchors:
`crates/epaint/src/text/font_tweak.rs:FontTweak::fallback_align`,
`FallbackAlign::Center/Baseline`, `StyledMetrics::fallback_align`,
`text_layout` mixed-face row height.
- Default `Center` preserves existing emoji/icon layout; `Baseline` grows the
  row to fit ascent/descent and puts fallback glyphs on the primary baseline.
- Required tests: default mixed-face snapshot unchanged; baseline row height;
  Thai/Vietnamese/Arabic marks not clipped; CJK fallback; no double descent.

**E15 — line breaker.** Anchors:
`crates/epaint/src/text/line_break.rs:LineBreaker`, `LineBreak`,
`LineBreakKind`, `SimpleLineBreaker`; `LayoutJob::language`;
`Fonts::set_line_breaker`, `Context::set_line_breaker`.
- `SimpleLineBreaker` must reproduce the old break points (whitespace, CJK,
  hyphen penalty 1, ASCII punctuation penalty 2); hard breaks only after UAX #14
  BK/CR/LF/NL when `break_on_newline` is set.
- Emergency breaks must be grapheme-safe; custom breakers get one paragraph and
  its language; changing the breaker clears galley caches.
- Required tests: custom breaker, old-break equivalence, language passing,
  hard break, emergency cluster, RTL reorder after custom breaks, cache
  invalidation.

**E16 — visual caret/affinity.** Anchors:
`crates/epaint/src/text/caret.rs` (`Row::visual_clusters`,
`x_offset_with_affinity`, `caret_at`, `Galley` movement),
`CCursor::prefer_next_row`, `TextEdit` password mask.
- Left/Right move one grapheme on screen; Ctrl/Alt move one word on screen;
  Home/End are reading-order row ends; Cmd+Left/Right are physical screen ends.
- Affinity picks the run at direction boundaries; Shift+arrows keep logical
  selection; password masks one bullet per cluster. Neutral paragraphs (empty
  lines, digits) inherit the previous paragraph's direction; the first is LTR.
- Required tests: the existing affinity/arrow/home/end/selection/password tests,
  plus click-drag across boundaries, row wrap, and IME cursor.

**E17 — language and AccessKit text.** Anchors:
`Context::set_language`, `Context::language`, `Options::language`,
`UiBuilder::language`, `Ui::language`, `UiStack::language`,
`Response::accessible_language`, `accesskit_text.rs`, `cursor_range.rs`.
- Root node language is applied at end of pass so a language set during the
  pass is visible; `UiBuilder::language` overrides for a subtree;
  `Response::accessible_language` overrides one widget.
- Text runs must carry real direction and logical text; RTL character positions
  count from the right edge; round-trip positions through runs.
- Required tests: root/subtree/widget language, AccessKit `language`, run
  direction/text, position round-trip, screen reader name, and kittest
  `accessibility_check`.

**E18 — eframe language.** Anchors:
`Frame::set_ui_language` (now `&self`), `Frame::set_text_input_language`,
`Frame::egui_ctx`, native/web `Frame` construction.
- Native: `set_ui_language` calls `Context::set_language`; AccessKit root sees
  it. Web: speech and text-agent `lang` fall back to `Context::language`.
- Required tests: native AccessKit root language; web hidden input `lang` and
  speech utterance `lang`; kittest `Frame::_new_kittest` has no context and does
  not panic.

**E20 — widget text language.** Anchors:
`WidgetText::into_galley`, `into_galley_impl`, `Label::layout_in_ui`,
`TextEdit` layout job, `Image` alt text.
- The `Ui` language fills `LayoutJob::language` only when the text does not set
  one; explicit job language wins. `into_galley_impl` uses `Context::language`.
- Required test: label, TextEdit, scoped `UiBuilder::language`, explicit job
  language; multiline cache preserves language.

**E21 — IME delete surrounding bytes.** Anchors:
`ImeEvent::DeleteSurroundingBytes`, `egui-winit::ime_delete_surrounding`,
`TextEdit` `surrounding_bytes_to_chars`, `delete_surrounding_chars`.
- Emit the delete event immediately before the `Ime::Commit` it belongs to;
  delete exact bytes, not whole clusters; partial UTF-8 counts leave the
  character; deletion + commit is one undo step.
- Required tests: press-and-hold accent, dictation replacement, ZWJ emoji,
  combining mark only, flag one indicator, count inside a character, more bytes
  than text, empty commit, undo.

### 6.3 Deliberate broken-behavior probes (mutation checks)

Run these manually on the candidate and record the exact failing test. They prove
the tests detect the behavior instead of merely passing:

1. Disable `BidiLevels::new` / `reorder_row_visually`: RTL visual-order and hit
   tests must fail; ASCII LTR tests must still pass.
2. Remove `reset_whitespace_levels`: L1 whitespace test must fail.
3. Replace `visual_order` with identity: selection/background/underline tests
   must fail.
4. Remove `unicode-bidi` from `epaint` dependencies: compile fails.
5. Set `num_threads` to a nonzero value in `glyph_render_context` under the
   multithreading test: it must panic, proving E11 is active.
6. Remove `surface_fit` clamp: oversized-surface unit test must fail.
7. Remove `catch_errors`: a deliberate configure error must reach the default
   uncaptured handler/panic instead of returning `ConfigureSurface`.
8. Remove `latin_layout_chars` append: keyup/layout test must fail.
9. Remove `accessible_name` from an icon-only control: kittest accessibility
   check must fail.
10. Complete a `ScreenshotCallback` twice: second call must be a no-op.
11. Deliberately replay one consumed application command across a discarded pass:
    the integration fixture must detect two edits/undo steps for one gesture.
12. Remove E21's byte conversion: exact-byte and undo tests must fail.

### E21b — produce and deliver fresh IME surrounding text

Source inspection finds a missing half of E21/W27: winit exposes
`WindowExtWayland::set_ime_surrounding_text`, but candidate egui-winit never calls
it, and `IMEOutput` contains no logical text. Incoming delete/commit support is
not full surrounding-text integration. Add this as a distinct reviewed P7b change.

Extend `IMEOutput` with an owner/context-generation identity and an explicit
available/unavailable surrounding-text state. An available payload contains
logical committed text and UTF-8 byte cursor/anchor; a valid empty document is
distinct from unavailable support. Identity remains present when text is unavailable. Keep
Clone/Eq, update Copy-dependent uses and constructors, and exclude transient text
from serialization and Debug dumps. A constructor validates byte boundaries,
excludes preedit and extracts <=4000 bytes without adding a full-document clone.
If the selection cannot fit, emit explicit unavailable context rather than
misrepresenting a truncated selection. Password input never supplies text.

TextEdit emits the payload after applying delete/commit/cursor changes, with its
preedit range removed and byte indices remapped. egui-winit sends it on text,
selection or IME-owner change through W27 on Wayland only. Clear old state on
password, unavailable context, no IME, focus loss and reset. W27 must gain an explicit
context-transition/clear operation: clear its cache and commit the required
disable/enable boundary before publishing a new owner/support state. Merely changing
purpose or sending empty text does not clear protocol state. Define cancellation
of the old preedit without inserting it into the new owner; same-owner ordinary
text/selection updates preserve composition. Cache per window and owner.
Available empty text maps to an explicit `set_surrounding_text("", 0, 0)` plus
commit in the supported context, distinct from omitting the request. W27 tracks
presence independently of string length. Unavailable uses the committed clear
boundary and omits set_surrounding_text; when support returns, enable plus the full
payload precedes commit. Ordinary empty/non-empty edits do not reset composition.
Test initially empty editors, non-empty → available-empty → non-empty and
unavailable/password → available, with ongoing composition and protocol traces.
This interpretation has primary implementation support in [Chromium's v3 client](https://chromium.googlesource.com/chromium/src/+/b715f8bd338c5aace806ecb1c5b63682f735e2d6/ui/ozone/platform/wayland/host/zwp_text_input_v3.cc),
which keeps optional surrounding state and sends an explicit request for empty text;
compositor behavior still requires native qualification. This is API/behavior prior
art only; no code from other implementations is imported.

`PlatformOutput::append` uses the latest completed pass's IME descriptor, including
authoritative `None`; replace the current `ime.or(self.ime)` accumulation. Descriptor,
owner and payload are one atomic value. Test text on pass1 then non-text/password
on pass2, plus unchanged owner over repeated passes; a previous payload must not survive.

Web consumer: `eframe/src/web/text_agent.rs::InputState::update` currently compares the
entire IMEOutput before setting DOM position. P7b changes it to a separate position key
(cursor center, canvas dimensions/offset, native pixels-per-point and zoom); transient
owner/text/selection changes alone must not rewrite identical DOM styles. Keep the full
IME descriptor separate for lifecycle correctness; do not weaken its Eq globally. Test
unchanged caret with text changes, caret movement, zoom/DPI/canvas resize, focus/None,
password and ongoing browser composition. Count style writes to detect churn. Payload
construction remains bounded on all targets; measure web allocations on large documents.

Audit every constructor/destructure and feature target. Custom document editors
must supply model-aware adapters in WordCraft `canvas.rs` (both text/math paths),
VectorCraft `canvas.rs`, FigCraft `shortcuts.rs`, EffectCraft `panels/viewer_text.rs`,
FilmCraft `panels/graphics.rs`, DesignCraft `canvas.rs`, DeckCraft `canvas.rs` and
PhotoCraft `type_tool.rs` (all under `crates/ui-egui/src`). Use logical model text,
not visually reordered display strings. Convert scalar/grapheme selections to
bytes deliberately; setting the new field to None everywhere is not acceptance.
Applications using only TextEdit inherit its producer.

Tests cover outgoing text immediately after delete+commit and deletion-only batches,
preedit exclusion, reversed selection, supplementary/combining/RTL text, cursor
movement, very long lines and over-limit selections, field/window switching,
password clearing, focus/reset/late serials and final-output multipass behavior.
A native Wayland Vietnamese/predictive-IME trace checks the outgoing protocol plus
actual editor text and one undo transaction. Measure bytes allocated and time on
large documents and repeated unchanged passes. The no-full-document-clone requirement
applies to the NEW payload work: existing TextEdit already clones the buffer for
change detection and undo. Record that baseline separately; do not assert total
TextEdit allocations become constant or expand the port into an unrelated undo rewrite.

P7b requires a new W27 lifecycle revision, not just its existing source guards:
clear pending preedit/commit/delete on leave, disable, reset and seat removal, with
owner/focus epochs. Track protocol commit serials. For a current-epoch `done` with
a mismatched serial, apply incoming edits once as the protocol requires, but defer
outgoing pending state until a matching serial; do not drop valid input. Stale
owner/epoch batches are invalidated separately. Test both cases and two windows.
The positional deletion queue is not itself proof of cross-window corruption;
preserve/test commit-deletion pairing if replacing it with tagged records.
W26 macOS UTF-16 replacement stays
a separate path; this addition makes no new macOS surrounding-text API claim.

## 7. Winit compatibility and provenance

### 7.1 Required base

- Upstream winit `0.30.13` (`e9809ef5...`, crates.io checksum
  `a6755fa58a9f8350bd1e472d4c3fcc25f824ec358933bba33306d0b63df5978d`).
- `storytold/winit` `craft/0.30.13` `ffae5198...`, W1-W15.
- `aiongg/winit` `craft/0.30.13-lang` `8b35388b...`, W17-W27 on top.
- W16 (Wayland pinch/rotation) is **not** in this cut. It belongs to
  `craft-tablet`; do not pull it in for this migration.

### 7.2 APIs this egui fork actually requires

| Egui patch | Winit patch/API | Compatibility note |
|---|---|---|
| E4 regular | W15 `platform::wayland::Clipboard` | Existing text API; preserve image fallback and file drops |
| E4 PRIMARY | New W15P in companion plan | Existing-connection, per-seat PRIMARY API; must land before P2b |
| E19 | W25 `platform::latin_layout::KeyEventExtLatinLayout` (`latin_key`, `is_latin_layout`) | X11/Wayland only; macOS uses W23 `key_with_command`; Windows uses `MapVirtualKeyExW` |
| E19 (Windows) | `PhysicalKeyExtScancode` | Existing winit API; needs `Win32_UI_Input_KeyboardAndMouse` |
| E21 | W26/W27 `take_ime_delete_surrounding` | macOS/Wayland only; commit with `Ime::DeleteSurrounding` |
| E4/E21 | W15 + W26/W27 | Both keep the single-data-device/IME event flow |

W17 is the regression-test set for W2/W3/W5-W10/W12/W13; W18/W19 improve
Windows caret/IME key delivery; W20/W21 fix X11 IME state; W22 fixes macOS IME;
W23 fixes macOS ⌘ layout names; W24 adds `Window::reset_ime`; W25 adds Latin
layout keys; W26/W27 attach delete-surrounding to commits.

### 7.3 Pin policy

- The inspected winit input is `8b35388bd719e8cc86a95971ba0bc026c9a8f058`. Final
  manifests pin the reviewed, published storytold revision including W15P and any
  required vendor reconciliation by immutable full SHA, never the moving aiongg
  branch or an old SHA that omits the new PRIMARY API.
- Put the `[patch.crates-io] winit = { git = ..., rev = ... }` in this
  repository's root for its own tests, and require every consumer root to
  repeat it. A `[patch]` in a dependency's manifest does not apply to the
  consumer workspace.
- Keep `dpi` on registry `0.1.2`; do not add a second `dpi` source. The winit
  branch declares registry `dpi = "0.1.1"` specifically to avoid a git duplicate.
- Keep `winit 0.30.13`; do not move to 0.31 in this migration. W26/W27 APIs are
  0.30-only shapes; 0.31 would require a separate IME/API plan.
- Run `cargo tree -d` and `cargo deny check` on the final lockfile; reject duplicate
  egui/winit identities and incompatible public-type boundaries. Investigate other
  duplicates individually; unrelated rendering dependencies may legitimately use
  different tiny-skia or other majors.

### 7.4 Family pin, duplicates and publication reachability

- The final consumer graph must resolve **all** egui-family crates
  (`ecolor`, `emath`, `epaint`, `epaint_default_fonts`, `egui`, `egui-winit`,
  `egui-wgpu`, `egui_glow`, `egui_extras`, `egui_demo_lib`, `egui_kittest`,
  `egui_inspection`, `eframe`) from one source revision. A mix of registry
  0.37.0 and the fork git revision is a duplicate-source bug even if the
  versions look equal.
- `egui_plot` and any similarly non-workspace crate must be pinned separately;
  do not assume the workspace pin covers them.
- Never put a machine-local `path = "/home/..."` or `path = "../..."` in a
  publishable manifest. Paths are acceptable only in a local review worktree and
  must be replaced before publication.
- A published crates.io 0.37.0 already exists. A fork that must be published
  needs a distinct version (and a deliberate semver/registry decision) or must
  be consumed by git revision. Do not republish under `0.37.0`.
- The dependency's `[patch]` does not propagate. Every consumer workspace must
  carry the winit `rev` patch and its lockfile must show the git source.
- Check `cargo tree -d` and `cargo metadata` for one `winit`, one `dpi`, one
  `accesskit_consumer` line and one egui-family source before review.

## 8. craft-libs PR 199 and shared UI helper needs

### 8.1 What PR 199 actually changes

Reviewed source: PR head `b68de8c5f4f1c060904d211a7b1320aa0f7bce78`
(OPEN / CONFLICTING), API-port ancestor `8614e4a`, and historical audit merge
`ec8840a`. Reconcile the PR against craft-libs `49190884278f16c9a9f5823d7698a8ffb5deb062`
or a recorded newer main, preserve current fixes, record conflict resolutions and
rerun its gates before handing a reviewed revision to consumers. Its port:

- bumps `egui`, `egui_extras`, `egui_kittest` to 0.37;
- maps `WidgetType` to `Role` (`Checkbox -> CheckBox`,
  `SelectableLabel -> Button`, `Other -> Unknown`);
- renames `UiBuilder::id`/`Ui::id` to `scope_id` and `Id::new` to `Id::unique`;
- changes the docking fixture's `⋮` to `…` because 0.37's bundled fonts no
  longer contain U+22EE in that path;
- touches `crates/ui/src/buttons/mod.rs`, `docking/surface.rs`,
  `docking/tests.rs`, `layout/resize.rs`, `layout/tests.rs`, `printing/tests.rs`,
  `tabs/mod.rs`, `tabs/tests.rs`, `tests/docking_workflows.rs` and
  `tests/visual/fixtures.rs` for the renames and fixture;
- otherwise claims no behavior change. The large `Cargo.lock` reduction is a
  re-resolution, not evidence that the old lockfile was wrong; audit it with
  `cargo tree -d`.

PR 199 is open/conflicting at the recorded head at inspection. It is a consumer migration, not an egui fork
patch, and it still needs its own review, `cargo tree -d`/`cargo deny`, and
consumer UI tests. The U+22EE change is a fixture workaround; do not treat it as
proof of glyph coverage.

### 8.2 Shared helper decision

Do not add an egui compatibility layer to `craft-ui`. The 0.37 APIs are small
and explicit. Add a shared helper only when at least two consumers need the same
semantics, and keep app-owned themes, assets, localization, commands, undo and
persistence in the apps.

Candidate helpers, in priority order, with adoption gates:

1. **Text language + direction.** A `craft_ui` helper that applies an explicit
   BCP 47 language and an explicit UI direction could set
   `Context::set_language`, `Style::right_to_left`, and the root `Layout`
   direction. Gate: two apps need the same language/direction mapping. Keep
   language and direction separate because not every non-English UI is RTL.
2. **Line-breaker adapter.** E15's doc example already shows a
   `craft_text::segment` adapter. Gate: two apps use Thai/Khmer/Japanese
   dictionary or kinsoku breaking. Keep the adapter app-configurable; do not
   impose a dictionary in the default `SimpleLineBreaker`.
3. **Accessible-name helpers.** Egui's `labelled_by`, `accessible_name`,
   `label_inputs_by`, `name_inputs` cover the common cases. Do not wrap them
   until a repeated third-party-widget naming pattern appears.
4. **No root-`Ui` wrapper.** Do not add a `craft_ui::run` bridge around
   `Context::root_ui`; it would encourage the blanket-context pattern and the
   nested-borrow panic. Pass `&mut Ui` explicitly.

## 9. Staged implementation PRs and rollback

Stages are separate reviewable branches/PRs stacked on their listed dependencies;
independent stages may branch from `craft/0.37.0-base`. Publication of any
branch or PR requires fresh human approval after showing the exact diff and
validation. None is granted by this plan.

| Stage | Branch/PR | Content | Depends on | Rollback |
|---|---|---|---|---|
| P0 | evidence only | Record 0.37.0 baseline, current 0.36.2 baseline, hardware, lockfiles, font files, screenshots, benchmark samples. No production changes. | — | Retain all baseline evidence |
| P1 | `craft/0.37.0-base` | `841a328b` + version/workspace pins + final reviewed immutable winit rev + `Cargo.lock`; no egui fork behavior. Add `PATCHES.md` skeleton. | P0, completed reviewed/published winit series including W15P and W27 lifecycle changes | Restore prior coherent dependency set in a new revision |
| P2 | `craft/0.37.0-runtime` | E1, E2+E3 and transferred vendor regression coverage. | P1 | Revert stage in a new revision |
| P2b | `craft/0.37.0-clipboard` | E4 regular clipboard plus W15P PRIMARY, bounded async transfer completion/cancellation and compositor acceptance. | P2, reviewed/published winit W15P/transfer revision | Restore prior coherent clipboard dependency/API set in a new revision |
| P3 | `craft/0.37.0-rtl-chrome` | E9; independent of text stack. | P1 | Drop commits |
| P4 | `craft/0.37.0-text-core` | E5, E12, E13, E16; split into reviewable commits (base bidi, L1/reorder, grapheme, caret). | P1 | Drop commits; keep 0.37 text |
| P5 | `craft/0.37.0-text-fallback` | E8, E11, E14, E15; E11 can be separate. | P4 | Drop commits |
| P6 | `craft/0.37.0-language-a11y` | E6, E10, E17, E18, E20; E10 before E18, E17 before E18/E20. | P4/P5 | Drop commits |
| P7 | `craft/0.37.0-input-ime` | E7+E19 including held-key release pairing, E21; preserve existing lifecycle semantics. | P4, winit W25/W26/W27 | Revert affected changes in a new revision |
| P7b | `craft/0.37.0-ime-surrounding` | E21b producer, bounded payload and Wayland delivery; custom-editor handoff. | P7, W27 state/serial tests | Revert with its custom-editor adapters as a coherent API set |
| P7c | `craft/0.37.0-ui-regressions` | Reproduce and fix #8763 selection-action and #8748 hidden-parent/child workflows; review proposed upstream fixes against pinned source and add native regressions. | P2/P4/P6/P7b | Revert only with an explicit unresolved rollout gate |
| P8 | `craft/0.37.0-release` | `PATCHES.md` final, docs, changelog, consumer hand-off, artifact manifest. No automatic publish. | all, including P2b, P7b and P7c | Restore coherent prior source set in a new revision; do not move/delete published tags |

Dependency DAG (text form):

```
P1 base
 ├─ P2 E1/E2/E3 -> P2b E4 regular + PRIMARY <- winit W15/W15P
 ├─ P3 E9
 └─ P4 E5 -> E12 -> {E13, E16}
        ├─ P5 {E8, E11, E14, E15}
        ├─ P6 {E6, E10, E17, E18, E20}
        └─ P7 {E7+E19, E21}
P4/P5 -> P6 E17 -> E18/E20
P4 + winit W25 -> P7 E19
P4/P5 + winit W26/W27 -> P7 E21 -> P7b E21b outgoing text
P2/P4/P6/P7b -> P7c selection + hidden-child regression fixes
P1/P2/P2b/P3/P4/P5/P6/P7/P7b/P7c -> P8 final hand-off
```

Stage rules:

- P1 waits for the completed winit prerequisite; it does not need an invented final
  SHA or a candidate interim production pin. Record the actual full SHA/tree and
  clean-fetch proof when that approved predecessor exists. Local candidate spikes
  remain evidence-only. Final consumers depend on P8, never incomplete stages.
- P7c owns any reproduced required selection/hidden-child defect. An app workaround
  requires equivalent behavior/tests and a removal condition; an unresolved failure
  blocks release rather than silently becoming accepted 0.37 behavior.
- Do not merge a later stage's lockfile change into an earlier branch; each PR
  carries its own `Cargo.lock` delta and a `cargo tree -d` report.
- Keep `craft/0.36.2` (767b1cad) and its lockfile untouched as the rollback
  point. The old family is the coherent 0.36.2 set, not a mixed set.
- Do not rewrite or force-push published stage branches. Revert by dropping
  commits on a new branch; publication approval is per push.
- E4 is last among runtime patches in the aiongg order because it needs the
  winit pin. P2b is explicit and depends on the reviewed W15P revision and its
  native evidence; consumers wait for the complete fork integration, not P1 alone.
- E7 is not a separate PR; combine with E19 to avoid two Windows key rules.
- No lifecycle scheduler API is included; repeated hooks are covered by behavior
  tests rather than changing the documented hook count.

## 10. Test plan

### 10.1 Required checks per stage

For every stage:

- `cargo fmt --all -- --check`, `./scripts/lint.py`.
- `cargo clippy --locked --all-features --all-targets`, plus the no-default
  variants relevant to changed crates.
- `cargo test --all-features --no-fail-fast`; doctests; `cargo test -p
  egui_extras` without `syntect`.
- `cargo test -p epaint` with and without `default_fonts`; with
  `vello_cpu/multithreading` for E11.
- `cargo test -p egui_kittest --all-features` (accesskit, snapshots, eframe).
- wasm: `scripts/clippy_wasm.sh`, eframe/egui-winit wasm checks; browser
  workflows for E10.
- `cargo tree -d`, `cargo deny check`, `cargo shear`.
- Windows: `cargo check`/`clippy` on the Windows clone and the actual
  `task.py`-managed Windows build for E1/E2/E3/E8/E9/E19.
- Linux: X11 and Wayland runs; software renderer and real GPU where available.
- Snapshot changes: justified per image, not regenerated blindly.

### Explicit feature and target rows

Run through the managed build workflow, on the selected fork revision. `check`
rows prove compilation only; `test` rows record actual nonzero test counts and
the regression names exercised. Do not invent expected counts from source grep
or accept zero selected tests. Keep each no-default row separate so another
workspace member cannot silently enable the feature being tested.

| Package/target | Commands or independently repeated feature sets |
|---|---|
| egui, Linux | `cargo check --locked -p egui --no-default-features`; repeat with each of `default_fonts`, `monochrome_emoji_fonts`, `serde`, `persistence`, `rayon`, `bytemuck`, `experimental`, `unity`, `unity,_override_unity`; run corresponding relevant library/behavior tests |
| epaint, Linux | `cargo check --locked -p epaint --no-default-features`; repeat with `default_fonts`, `monochrome_emoji_fonts`, `color_fonts`, `serde`, `rayon`, `bytemuck`; test color-glyph fixtures on/off and E11 with `vello_cpu/multithreading` |
| egui, wasm | `cargo check --locked -p egui --target wasm32-unknown-unknown --no-default-features --features web_file`; repeat without `web_file` |
| egui-winit, Linux X11 | `cargo check --locked -p egui-winit --no-default-features --features x11`; repeat with `x11,clipboard`, `x11,arboard`, `x11,smithay-clipboard`, `x11,accesskit,links,serde`; assert intended resolved dependency features |
| egui-winit, Wayland | Repeat the previous row with `wayland` instead of `x11`; regular/PRIMARY/drop runtime matrix is additionally required |
| egui-wgpu, Linux | `cargo check --locked -p egui-wgpu --no-default-features`; repeat with `capture`, `winit,x11`, `winit,wayland`; run capture and renderer tests with an identified adapter |
| egui-wgpu, macOS | Check default Metal/jitter behavior and `--no-default-features --features macos-window-resize-jitter-fix`; compare a supported non-Metal build with jitter feature disabled; real resize runtime evidence required |
| eframe, Linux | `cargo check --locked -p eframe --no-default-features --features glow,x11`; repeat `wgpu,x11`, `wgpu_no_default_features,x11`, `wgpu,wayland`, then explicit `default_fonts`, `system_fonts`, `persistence`, `inspection`, `clipboard` toggles on the chosen backend |
| egui_kittest, Linux | `cargo test --locked -p egui_kittest --no-default-features`; separately compile/test supported sets `snapshot`, `video`, `wgpu,snapshot`, `eframe,x11`, `eframe,wgpu,snapshot,x11`; video requires its actual encoder/runtime dependency |
| eframe, wasm | `cargo check --locked -p eframe --target wasm32-unknown-unknown --no-default-features --features wgpu,default_fonts,web_screen_reader`; repeat the glow path and test browser workflows |
| Android | Native SDK/NDK row: `cargo check --locked -p eframe --target aarch64-linux-android --no-default-features --features wgpu,android-native-activity`; game-activity is a separate row, never both backends together |
| iOS | macOS SDK row: `cargo check --locked -p egui-winit --target aarch64-apple-ios --no-default-features`; platform toolchain/runtime evidence remains explicit |
| FreeBSD | Native/cross SDK row: `cargo check --locked -p egui-winit --target x86_64-unknown-freebsd --no-default-features --features x11,clipboard`; record any upstream support failure separately |

`color_fonts` belongs to epaint; `persistence`, `experimental` and `web_file`
belong to egui. Validate these names against the actual accepted manifests. The
existing repository CI's MSVC/GNU, Android and Apple rows remain required; missing
SDKs are unresolved compile gates, never silently claimed passes. Runtime product
support and cfg compilation are reported separately.

### 10.2 User-observable assertions

Use stable fixtures and assert behavior, not implementation details:

- **Geometry/hit:** panel rects, clip rects, scrollbar drawn vs interaction
  rects, checkbox/icon sides, popup/submenu edges, resize grip corners, caret
  x positions at every grapheme boundary, hit tests at every x in LTR/RTL/mixed
  rows.
- **Pixel:** RTL/LTR mixed snapshots with fixed fonts, DPI and theme; CJK/Thai/
  Arabic fallback; color emoji; text edit overshoot; RTL chrome; `…` at the
  visual end. Tolerances must be justified (anti-aliasing/hinting/subpixel);
  do not use a blanket `0.1`.
- **Accessibility:** query by `Role`/name; assert root language, subtree
  language, widget language, text run direction/logical text, positions; assert
  every input widget has a name; assert invisible widgets are absent.
- **Keyboard/focus/IME:** arrows/Home/End/Cmd, selection, focus movement,
  composition interruption, keyup release, Latin layout chars, IME commit +
  delete bytes, undo one step.
- **Scale/theme:** 1.0/1.5/2.0 DPI, light/dark, custom font tweaks, RTL chrome.
- **Resize/dock/popout:** panel resize/clamp, window drag, deferred/immediate
  viewport open/close, embedded fallback, screenshot callback.
- **Selection:** select text, click toolbar/context action and assert the intended
  range is acted on (#8763); include keyboard and pointer paths.
- **Hidden parent:** deferred child open/close and async completion with minimized
  parent (#8748); explicitly requested hidden paint and normal hidden logic differ.
- **Save/reopen:** app state round-trip using eframe persistence; window
  position/theme/language survive restart; no duplicate autosave writes.
- **Side effects:** a counter in `logic` vs `ui`; queue drain count; audio tick
  count; undo step count across a discarded pass; autosave count while hidden.

### 10.3 Performance protocol and thresholds

Record a baseline **before** any production change, on the same machine and
environment as the candidate:

- **Identity:** exact 0.36.2 or 0.37.0 source SHA, fork patch list, `Cargo.lock`
  hash, features, crate versions; OS/kernel, CPU model, GPU/driver, renderer
  (`wgpu` backend/GL/Vulkan/DX12/Metal or software), DPI, viewport size, theme,
  font files, hashes and license manifest (the pinned fixture set below), `RUSTFLAGS`, profile (`release` with the
  workspace `opt-level = 2`, `panic = "abort"`).
- **Fixtures:** 10k-character ASCII LTR; 10k Latin+CJK; 10k Arabic/Hebrew; 10k
  mixed bidi; 1k short labels; 200 wrapped rows; a 10k-character row for
  selection; a font-fallback mixed job.
- **Measured:** text layout (cold and cache hit), galley cache eviction, row
  reorder steps, selection hit tests, caret movement, tessellation, texture
  upload count/bytes, draw calls, peak RSS, idle CPU and repaint count over 60 s,
  first-frame latency, resize/dock latency. Separately measure renderer/shader/FXC
  pipeline construction (#8736), system-font discovery and first missing-glyph
  lookup (#8512), with warm caches and cold-process startup identified.
- **Method:** Criterion or a harness with at least 10 warmups and 50 samples per
  case; retain raw samples; rotate old-fork/stock-0.37/candidate order on the same
  machine; no other builds/apps; report median, p95 and variation. Frame p99
  needs at least 1,000 measured frames; do not report p99 from 50 operations.
  if coefficient of variation > 5%, rerun and explain; software renderer and
  real GPU rows are separate.
- **Thresholds (predefined, not adjusted after seeing results):**
  - ASCII LTR text layout: median <= +3%, p95 <= +8%.
  - Non-RTL complex text: median <= +5%, p95 <= +10%.
  - RTL/mixed text: paired candidate/baseline median <= +5%, p95 <= +10% for
    the same fixture. Also report the ratio to same-length LTR as a diagnostic
    (target median <= 1.5x, p95 <= 2.0x); a pre-existing ratio failure is recorded
    separately and cannot conceal a new paired regression.
  - Selection/caret per interaction: median <= +5%, p95 <= +10%; no
    per-frame sort (assert `visual_order` reuse).
  - Frame time for the fixture UI: median <= +5%, p95 <= +10%.
  - Peak RSS: investigate a repeatable increase over +5% or +16 MiB, whichever
    threshold is reached first. Record baseline absolute RSS and allocator noise.
  - Idle: no extra repaint requests; CPU <= +2 percentage points over 60 s.
  - Texture/glyph atlas: no unbounded growth; upload bytes for ASCII LTR
    <= +5%; bidi/fallback rows bounded by glyph count.
- The suite protocol supplies matched startup runs, noise rejection and the
  sub-millisecond absolute floor. Its gates also apply; the stricter rule wins.
- Any regression beyond threshold requires a profiling report and either a
  design fix or explicit product acceptance; never update a golden or lower a
  threshold to hide it. Compile duration is not UI performance and must not be
  reported as such.

### 10.4 Baseline/candidate record template

Fill this table at P0 before editing production code. A blank value is an
unresolved acceptance item, not a zero.

| Field | Old fork 0.36.2 | Stock 0.37.0 control | Candidate fork |
|---|---|---|---|
| Source SHA, patch list, lockfile hash |  |  |  |
| OS/kernel, CPU, GPU, driver, renderer |  |  |  |
| DPI, viewport, theme, font files/versions |  |  |  |
| ASCII LTR median/p95 |  |  |  |
| Complex LTR median/p95 |  |  |  |
| RTL/mixed median/p95 |  |  |  |
| Selection/caret median/p95 |  |  |  |
| Frame median/p95 |  |  |  |
| DX12 renderer/FXC construction and first-frame time (separate) |  |  |  |
| Font discovery and first-use layout time, cold/warm (separate) |  |  |  |
| Peak RSS |  |  |  |
| Idle CPU / repaint count (60 s) |  |  |  |
| Atlas upload bytes / draw calls |  |  |  |
| Raw samples path and run count |  |  |  |

Frame p99 is reported only for >=1,000 actual frame samples. Criterion sample
means are not individual frame times. Retain stock-control results for upstream
regression attribution even when app acceptance uses paired old/candidate runs.

### 10.5 Platform gates

| Row | Required evidence |
|---|---|
| Linux X11 | host build/test; clipboard, IME, RTL keyboard; real GPU or software |
| Linux Wayland (Hyprland/wlroots) | E4 clipboard/PRIMARY, E19 Latin keys, E21 IME; real compositor |
| Linux Wayland (KWin) | same, if available |
| GNOME/Mutter | W15P/E4 PRIMARY and regular clipboard native acceptance; no presumed unsupported fallback |
| Windows 10/11 | task.py VM build + GUI: E1 windows, E2 surface limits, E8 marks, E9 RTL, E19 VK keys; E21 is macOS/Wayland only |
| macOS | real hardware required for E19 ⌘ and E21 press-and-hold; otherwise unresolved gate |
| wasm/Chromium | E10 keyboard/layout/language, text agent, screen reader, color emoji |
| wasm/Firefox/Safari | if available; otherwise unresolved |
| Android/iOS/FreeBSD | Explicit cfg compile rows above; missing SDK evidence remains an unresolved gate; native behavior is claimed only where exercised |

Headless kittest snapshots do not prove native focus, IME, file picker, clipboard
or GPU behavior. Report those as unresolved until performed.

### 10.6 Pinned font fixtures

Use [craft-fonts at `2481a8a324b986a45954e90045ed1f8f2efaaf3c`](https://github.com/storytold/craft-fonts/blob/2481a8a324b986a45954e90045ed1f8f2efaaf3c/fonts/manifest.txt),
whose manifest records source, license file and SHA256 for each font. Fetch it as
a test resource; do not copy font binaries into applications whose policy requires
the central font repository. Preserve licenses in any redistributed fixture bundle.

| Fixture | Path within craft-fonts | SHA256 | License |
|---|---|---|---|
| Arabic | `fonts/noto-sans-arabic/NotoSansArabic.ttf` | `63111b5b2e074dd48cc67692e0a2726d86ee94c1c37fe8598257b7b4e87e869e` | OFL-1.1, adjacent `OFL.txt` |
| Thai | `fonts/noto-sans-thai/NotoSansThai.ttf` | `5a1c559bb539583c8a1fd99d1c5b9491e5e14478c9cd2bd0970d5c3096cc9ef8` | OFL-1.1, adjacent `OFL.txt` |
| CJK SC / Latin | `fonts/noto-sans-cjk-sc/NotoSansCJKsc-Regular.otf` | `2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b` | OFL-1.1, adjacent `OFL.txt` |
| Japanese | `fonts/biz-ud-pgothic/BIZUDPGothic-Regular.ttf` | `258d7156c165f2ff774b6efee637c22c3b950de0d8a10e501137061bc8085d01` | OFL-1.1, adjacent `OFL.txt` |

For bundled Latin/emoji fixtures, record exact `epaint_default_fonts` files and
hashes from each pinned baseline/candidate, then use a fixed common set for paired
measurements and a separate default-policy visual comparison. The candidate files
`Ubuntu-Light.ttf` (Ubuntu Font License in adjacent UFL.txt) and
`NotoEmoji-Regular.ttf` (OFL-1.1 in adjacent OFL.txt) have SHA256
`80307b8da7649aa4ee4d484b232140e3ce1ec0ca093073d3c53c8f5a5ced7a70` and
`415dc6290378574135b64c808dc640c1df7531973290c4970c51fdeb849cb0c5` respectively.
Use these exact bytes for the controlled comparison, preserving license files.
Additional test-only fixtures are pinned directly to their licensed upstream source:

- [Noto Sans Hebrew variable font](https://github.com/google/fonts/blob/8b0a1d0f5983c89bc2b93f1b5fb55f9e252744b5/ofl/notosanshebrew/NotoSansHebrew%5Bwdth%2Cwght%5D.ttf), SHA256
  `7ef36a2c3593758cdb622e1bdef4f84523e92fbc3ccc667438dd80ff54c2de88`,
  OFL-1.1 in adjacent `OFL.txt`; fix axes to weight 400 / width 100.
- [Noto Color Emoji v2.047](https://github.com/googlefonts/noto-emoji/blob/22e564626297b4df0a40570ad81d6c05cc7c38bd/fonts/NotoColorEmoji.ttf), SHA256
  `39ee3c587e10e89669b9ff32703261d10d5f9c4dd5ad147b6b5a1c5200591817`,
  OFL-1.1 in adjacent `LICENSE`; this bitmap-color font is a controlled CBDT case,
  not proof of all COLR/SBIX formats. Format-specific fixtures remain distinct.

The font bytes were hash-checked against their Git blobs during planning; no app
font assets were changed. Fetch these into the task's explicit fixture directory,
retain their licenses and keep them out of app source. Any eventual bundled app
addition must follow its central craft-fonts policy. Before the first baseline, no unsupported glyph or missing fixture may silently skip a required
script. System-provider tests are a separate nondeterministic integration row with
OS/font inventory recorded, not pixel-golden input. Do not redistribute proprietary
system fonts. All fixture files and license hashes enter the evidence manifest.

## 11. Publication and review gates

- No push, tag, PR, comment, release or external message is authorized by this
  plan. Before each publication, the parent must show the exact branch/commit,
  diff stat, test commands/results, performance table and unresolved gates, then
  obtain explicit human approval.
- Preserve `@skylartaylor` username-only contributor credit; preserve original
  authors' commit metadata; do not add agent/model/tool signatures.
- Do not rewrite `craft/0.36.2`, `craft/0.37.0-trial`, the aiongg refs, or
  another checkout.
- A fresh adversarial reviewer must check: patch provenance, source anchors,
  manifest/lockfile coherence, lifecycle side effects, root-Ui/panel behavior,
  RTL correctness, primary clipboard, winit pin immutability, test strength
  (including mutation probes), and performance honesty.
- Implementation starts only after this plan and the consumer plans land; each
  implementation PR gets a separate fresh review.

## 12. Unresolved decisions and limitations

1. **PRIMARY on Wayland after E4.** E4 disables smithay; its arboard fallback
   does not establish PRIMARY support on every supported compositor. The selected
   repair is a winit-owned primary-selection path using the existing connection,
   event queue and per-seat lifecycle, then E4 routes get/set-primary through it.
   SCTK 0.19.2 already exposes `PrimarySelectionManagerState`, per-seat devices,
   sources and offers. This is a distinct primary-selection protocol object;
   do not create another regular `wl_data_device` or restore the clipboard/drop
   conflict. Cover keyboard-only focus, recent serials, owner change, empty data,
   seat removal, unavailable protocol, self-paste and canceled/slow transfers.
   Preserve password exclusion and the upstream policy of not reclaiming PRIMARY
   when text selection has not changed. Native wlroots/KWin/Mutter tests must
   confirm the result. Only prototype an isolated PRIMARY-only fallback if that
   design fails a demonstrated capability requirement; accepting silent loss is
   not the planned outcome.
2. **Lifecycle acceptance.** The design preserves existing hook semantics.
   Consumer command idempotence, hidden work and fresh input handling remain
   required measured tests; a new scheduler is not part of this plan.
3. **E7/E19 granularity.** Plan says fold E7 into E19; confirm no Windows-only
   behavior is lost when the final commit is split for review.
4. **Upstream absorption.** E4 (#8657), E5 (#8577), E11 (#8641) and E2 (#8361)
   are open/draft at the fork snapshot. Re-check the actual PR/release state
   before removing any patch; do not trust this plan as current upstream truth.
5. **E5/E12 divergence.** Upstream #8577 may merge in a different shape. Keep
   the fork tests and re-evaluate the code if upstream lands; do not carry a
   private implementation indefinitely without a tracked reason.
6. **`default_fonts`/`system_fonts` visual change.** 0.37's system font provider
   and color emoji can change glyph selection and metrics. Snapshot tolerances
   and performance baselines must be taken with the final font set, not assumed
   from 0.36.
7. **macOS and Windows runtime evidence.** No fresh build/GUI run exists in this
   planning turn. E19 macOS ⌘ and E21 macOS press-and-hold remain unresolved
   until real hardware evidence.
8. **`egui_plot`** is outside the workspace at 0.37.0; its consumer pin must be
   verified separately.
9. **craft-libs PR 199** is open/conflicting at the recorded head and includes a fixture workaround for U+22EE.
   Its final review, lockfile deduplication and consumer UI tests are separate
   from this plan.
10. **No measurements yet.** This document defines the protocol and thresholds,
    not measured values. The P0 baseline must be recorded before production
    edits; fabricating before/after numbers is prohibited.

### Limitations of this planning turn

- No build, test, GUI, browser, VM, benchmark or profiler was run.
- Primary GitHub discussions and release source were checked during suite
  reconciliation. Status is a dated snapshot and must be rechecked before implementation.
- Source was read from local refs with
  `git show aiongg/craft/0.37:PATH`, `git show 0.37.0:PATH` and the current
  checkout; line-number drift is expected, so anchors are symbol-based.
- Windows/macOS/Wayland compositor behavior and performance values remain to be
  measured by the implementation and acceptance stages.


## Local implementation update — 2026-10-23

The reviewed plans remain local and implementation is authorized before publication. Earlier plan-publication prerequisites are superseded; final pushes and PR publication still require approval. Research inputs advanced to egui `0f72e2ea3a059c6c5c45270099672b5472f75e7d` and winit `5b708e919e3506b8f8927651159204b1df2f648b`. Production storytold fork branches remain unchanged.

The winit delta fixes destructive macOS replacement on dropped insertText and adds useful unchanged-surrounding deduplication. It does not fix pending incoming batches crossing lifecycle boundaries or provide the outgoing egui context bridge. Windows cancel/reset still needs behavioral coverage for reentrant result strings and duplicate preedit clears. The first local repair targets incoming Wayland batch isolation; serial/outgoing-context support remains separately required.

E22 monitor metadata is a new candidate feature, not part of the reviewed E1–E21 compatibility set. Review found idle hotplug freshness, mixed-DPI coordinate-space and clone-allocation concerns. Retain existing host-owned placement while evaluating any E22 adoption separately; do not silently pull it into the migration by copying the branch tip. Physical positions can be reconstructed from its scale and logical positions (subject to f32 precision); the review claim that this information is completely discarded is not accepted.

The static review's claim that input-method-v2 has no serial is rejected: its commit request explicitly takes a serial. This is a different protocol from text-input-v3. Preserve unchanged-state deduplication and the existing rule that a mismatched text-input-v3 done still applies current-focus incoming edits. Available-empty is distinct from an absent surrounding-text request; native empty→typed transition remains required, without adopting the review's unsupported unconditional reset recommendation.

Craft-libs PR199 is now `1dcde7ed1fe136df6d2b7a52d52b55e538193e1f`, which incorporates the old main security fixes. Its current fourteen-file API delta must be reviewed onto new main `f180fc0d35d521c5d18e118c881d3efde1ec1eee`; old warnings about b68de8c5 do not describe this new tree. Source review evidence is `aiongg-diffs-refresh-20261023-1535.json`, `overnight-fork-review-20261023.md` and `craft-libs-pr199-delta-20261023.json` in the task evidence directory.


### Texture-delta destructor during panic unwinding

DeckCraft's strict missing-glyph regression exposed a second panic in `TexturesDelta::drop`,
which aborted the test process while the original UI failure was unwinding. The local repair
keeps the debug assertion for ordinary unhandled deltas, but suppresses that second assertion
only while the thread is already panicking. It does not count deltas as rendered or weaken the
normal ownership rule. Regression tests must preserve the original panic payload and still
reject an ordinary unhandled drop. Debug/release tests and a fresh adversarial review are required.


## Wayland PNG copy/paste implementation boundary — 2026-10-24

Accepted bounded boundary (from `wayland-image-plan-v3.md` + v4 dispositions and the FINAL
v5 output-contract dispositions):

- `egui-winit` gains an optional `clipboard-image` feature. It implies `clipboard`, pulls
  `image` with `default-features = false, features = ["png"]`, and is enabled by the
  crate's default clipboard support and by `eframe/clipboard`.
- The codec is process-global and only compiled for supported Unix Wayland targets with
  `clipboard-image`: one encode worker, one decode worker, one queued job per direction,
  one shared 192 MiB admission budget, 8 M pixels / 8192 px limits, and a 16 MiB
  `PngBytesWriter` permit acquired before encoded output allocation.
- Regular paste is one composed request with a 2 s transfer deadline and a 3 s total
  delivery deadline. Raw completion submits decode without requiring a UI poll. The
  composed state is `Loading -> Decoding -> Ready/Failed -> Consumed`; `Cancelled` is
  terminal. Exactly one caller terminal wake occurs outside all locks; cancellation
  never wakes. Decode-ready images retain their codec permit until the image is handed
  to the application.
- Regular image copy uses a dedicated `copy_epoch`, not the paste/input generation. A
  newer winning regular output invalidates the older pending image before origin
  resolution even if the new attempt fails. The exact captured token is reserved
  synchronously, the caller-owned `ColorImage` is moved (not cloned) into the encoder,
  and the event loop installs the encoded PNG through the original reservation after
  focus/window/hide/deadline fences. PRIMARY never invalidates a regular reservation.
- The bridge `PasteError` and encoded-PNG/decoded-image seams are cfg-independent. The
  Wayland adapter maps winit's typed `ClipboardError` at the edge. There is no generic
  MIME path, no PRIMARY image support and no fallback transport.
- Authoritative Wayland construction starts neither arboard nor smithay. Synchronous
  compatibility image methods explicitly refuse on Wayland; synchronous text is the
  bounded winit call and does not fall back after an absent, malformed or failed read.
  The supported image API is the `State` regular copy/paste event path.

### Review-repair amendment — malformed images, queued cancellation, absolute deadline

The final PNG/codec review identified three medium gaps; this local amendment records the
repair contract before the queue implementation changes.

- Image copy admission validates both the declared `ColorImage::size` product and the actual
  `pixels.len()` before any codec permit is acquired or conversion starts. A mismatch is
  `InvalidInput` and consumes no origin reservation, queue slot or codec budget. The permit also
  covers any retained `pixels` capacity beyond the logical pixel count; the straight-alpha
  conversion buffer is exactly reserved with fallible allocation and its length is fixed before
  conversion, so the conversion loop cannot grow it infallibly.
- Codec queues are a bounded cancellable mailbox (capacity one per direction) instead of a
  `sync_channel` that cannot remove an item. Each queued job exposes a cancellation handle.
  Cancelling queued work removes its envelope, drops the payload and releases its codec permit
  promptly, so a follow-up admission is not refused by a stale `Busy`. Work already running may
  finish and remains budgeted until it completes. The cancellation handle is carried through the
  `ImagePasteDecoder` trait into the composed bridge, and cancellation also cancels any queued
  decode. Worker wakeups remain outside all locks and cancellation never wakes the caller.
- The absolute composed delivery deadline is rechecked when an already-`Ready` result is
  consumed, not only while loading or decoding. A result ready before the deadline but drained
  after it becomes `TimedOut` and emits no paste. Tests use an injected test clock for the
  ready-before/drained-after and ready-before/drained-before cases.

No build, test, format, lint, metadata, VM or native command is run by this amendment.

### Qualification explicitly not performed here

- No Cargo, rustc, build, test, fmt, clippy, metadata, VM, GUI or native compositor
  command was run in this implementation turn. Compilation and test results are not
  claimed.
- Native image evidence remains unrun: Sway/KWin/Mutter/Hyprland image roundtrip, exact
  RGBA/alpha oracle, slow/trickling and oversized peers, external reads of owned PNG,
  pointer-release-during-encode serial trace, event-loop stall measurement, peak RSS and
  two-seat routing.
- Feature/platform qualification remains unrun for all-features, x11-only,
  `wayland+clipboard` without `clipboard-image`, Windows/macOS, and wasm.
- Performance qualification (copy/paste latency, peak RSS, event-loop heartbeat) is
  unmeasured.


## Integrated fork status — 2026-10-24

The PRIMARY and PNG boundary sections above record earlier implementation stages. The final
local integration includes asynchronous regular/PRIMARY reads, bounded PNG decode/encode,
reserved image ownership, failure cancellation and independent PRIMARY handling. The winit
and egui implementation sources were checked byte-for-byte against the staging snapshot
qualified by `clipboard-image-smoke-20261024-025844-315945` before this status note was added.
That private-Sway run passed 25 checks through a real egui-winit State, including external/self
text and PNG, exact RGBA/alpha, malformed/absent cases, mixed-output last-wins behavior, worker
wakes and an unchanged external PRIMARY sentinel. It does not qualify multi-seat, native
slow-peer/focus-transfer races, other compositors/platforms or performance. Final immutable
pins and canonical locks are a separate landing step for consumers. This fork now pins the
reviewed canonical winit revision `ae48c756a3b81125eb796c0cc6dccf60990d1181`; managed lock
generation and locked/offline metadata audit passed with no other package drift.
