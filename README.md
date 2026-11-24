<p align="center">
  <img src="assets/app-icon/hicolor/256x256/apps/com.local.localpdf.png" width="96" height="96" alt="Local PDF icon">
</p>

<h1 align="center">Local PDF</h1>

<p align="center">
  A desktop PDF reader and editor for Windows and Linux. Works offline, needs no account, written in Rust.<br>
  <a href="https://github.com/Anon5T4R/local-pdf/releases/latest">Download</a> ·
  <a href="ROADMAP.md">Roadmap and status</a> ·
  <a href="#where-it-comes-from">Where it comes from</a> ·
  <a href="#how-it-is-made">How it is made</a>
</p>

<!-- README-SHOT: main window with a real document open; dark and light -->
<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/images/readme/main-dark.png">
    <img src="docs/images/readme/main-light.png" width="820" alt="Local PDF with a document open">
  </picture>
</p>

## What it does

- Read PDFs, including password-protected ones: single page, continuous and two-page views, search,
  and panels for comments, bookmarks, pages, forms, layers, attachments, signatures and tags.
- Edit existing text and images on the page; add comments, stamps, watermarks, headers and footers.
- Organize pages: insert, extract, split, combine, rotate, reorder.
- Fill, flatten and export AcroForm forms; open XFA forms.
- Sign with your own certificate, protect with a password, and redact (the content is removed).
- OCR scanned pages, export pages as images or text, compress, convert to PDF/A, compare two files.
- Print.
- Run the same tools from the command line, or let an AI agent drive them over MCP
  ([docs/mcp.md](docs/mcp.md)).

What works well, what is weak and what is planned is in the [roadmap](ROADMAP.md).

## Download

Windows 10/11 (x64): `.msi` installer or portable `.zip`. Linux (x86_64): `.AppImage`, `.deb`,
`.rpm`, Arch `.pkg.tar.zst` or `.tar.gz`. All in
[Releases](https://github.com/Anon5T4R/local-pdf/releases/latest). macOS is not published.

Each file is signed with [minisign](https://jedisct1.github.io/minisign/), and each release has a
`SHA256SUMS.txt`. To check a file:

```sh
minisign -Vm <file> -P RWTqN+hTTCFbDCGCA1cEJRLp2FSluxIUiJfqdoAYwkwNNVH5MTYH8qSN
```

The public key is also in [packaging/minisign.pub](packaging/minisign.pub). It is the same key for
every Local app.

## Network and privacy

- Your documents never leave the computer.
- On start, the app asks GitHub if a newer version exists. If you accept an update, it downloads it
  from this repository's releases. There is no setting to turn the check off yet.
- AI features do nothing until you add a provider and your own key (any OpenAI-compatible service, or
  a local server such as llama.cpp or Ollama). The text you send goes only to that provider.
- No account, analytics or crash reporting.

Security notes, all checkable in this repository:

- The PDF pipeline (parser, decryption, fonts, rendering, forms, scripts, OCR) is Rust. There is no
  C or C++ PDF library such as PDFium, MuPDF or Poppler.
- The app's own code (34 crates and two binaries) has no `unsafe` blocks. Lower layers do, as every
  desktop app's do: the window system (winit) and the GPU (wgpu).
- Form JavaScript runs in a sandboxed interpreter (boa) with no file or network access.
- Releases are built only by GitHub Actions from a tagged commit, with locked dependencies. Every
  dependency passes `cargo-deny` (advisories, licenses, bans), and none comes from a git URL.
- The program you install contains no `ring`, `aws-lc` or OpenSSL: TLS is rustls with RustCrypto.

## Where it comes from

Local PDF is a fork of [PdfCraft](https://github.com/storytold/pdfcraft) by
[ArtCraft](https://getartcraft.com), licensed MIT or Apache-2.0. The PDF engine (parser, renderer,
forms, editing) is their work, and so is the decision to write it all in Rust.

**Why a fork.** We wanted a set of desktop apps that look and behave the same, update and sign the
same way, and follow our own priorities: Windows and Linux first, files that stay local, AI only with
your own key, and agents able to drive every app. A fork lets us decide that without asking the
upstream to change direction.

We also wanted "written in Rust" to hold for everything the app runs, not only the PDF engine. In
the upstream apps the updater's TLS runs on `ring`, which is partly C and assembly, and `aws-lc`
(C) is in their dependency tree. Here the updater and every other network path use pure-Rust
cryptography (RustCrypto), and neither library is in the program you install.

**What changed here:**

- Name, icon and app identifiers.
- The shell shared by all Local apps: mode tabs, command palette (`Ctrl+K`), Automatic/Light/Dark
  themes, the AI panel, and the update card.
- Our own updater, which checks minisign signatures, and our own release pipeline (Windows and
  Linux packages).
- TLS moved from `ring` to a pure-Rust provider (RustCrypto).
- The web (WebAssembly) build was removed; macOS and FreeBSD are not published.
- Live automation of the open app goes through `craft-mcp`, shared by every Local app.

**What we still take from upstream:** bug fixes that matter to us (crashes, data loss, security,
file compatibility), reviewed one by one and adapted. New features and UI changes are not merged
automatically. Each sync is recorded in [docs/UPSTREAM-SYNC.md](docs/UPSTREAM-SYNC.md).

## How the app is built

One Cargo workspace: 34 library crates in layers, the desktop app `localpdf`, and `localpdf-cli`.
A crate may depend only on lower layers, and nothing below the front ends knows about the UI;
`cargo xtask layers` enforces it.

| Layer | Crates | Role |
|---|---|---|
| Foundation | `geom`, `filters`, `crypt` | geometry, stream filters, the PDF security handler |
| Objects | `cos` | tolerant parser with repair, copy-on-write object graph, incremental writer |
| Document | `content`, `model`, `fonts` | content streams, page tree, font metrics |
| Services | `render`, `annot`, `forms`, `js`, `xfa` | rendering, annotations, forms, scripts |
| Tools | `edit`, `organize`, `redact`, `sign`, `ocr`, `export`, … | one crate per tool family |
| Façade | `engine` | open documents, edit history, the command registry |
| Front ends | `ui-egui`, `automation`, `platform` | the desktop UI, the CLI and MCP tools, OS services |

An edit runs on a copy-on-write clone of the document and becomes one incremental revision; the page
on screen is rendered from the same bytes that Save writes. Details: [docs/architecture.md](docs/architecture.md).

## How it is made

Local PDF follows the same rules as every Local app: local files and no account, Rust only
(including the UI and the cryptography), one self-contained repository per app, forks with selective
sync, governed dependencies, signed releases built from a tag, optional AI with your own provider,
and MCP so agents can use the app. Most of the code is written by AI coding agents (Claude Code and
OpenCode) from written plans, reviewed by the maintainer and held to the same checks and CI. The
reasons for each decision:
[How we make the Local apps](https://github.com/Styx-Studios/.github/blob/main/profile/README.md#how-we-make-the-local-apps).

## Build from source

Rust 1.95 or newer.

```sh
git clone https://github.com/Anon5T4R/local-pdf
cd local-pdf
cargo run --release -p localpdf     # the app
cargo xtask models                  # optional: OCR models
cargo xtask ci                      # all local checks
```

Low on disk: see the `low-disk` profile in [docs/development.md](docs/development.md).

## Keyboard shortcuts

`Ctrl+K` lists every command with its shortcut. The common ones: `Ctrl+O` open, `Ctrl+S` save,
`Ctrl+Shift+S` save as, `Ctrl+F` find, `Ctrl+Shift+N` go to page, `Ctrl+1` / `Ctrl+2` / `Ctrl+0`
actual size / fit width / whole page, `Ctrl+L` full screen, `Ctrl+P` print, `Ctrl+Z` /
`Ctrl+Shift+Z` undo / redo.

## The Local apps

[Local Word](https://github.com/Anon5T4R/local-office) ·
[Local Sheets](https://github.com/Anon5T4R/local-sheets) ·
[Local Slides](https://github.com/Anon5T4R/local-slides) ·
[Local PDF](https://github.com/Anon5T4R/local-pdf) ·
[Local Keys](https://github.com/Anon5T4R/Local-Keys) ·
[Local Image](https://github.com/Anon5T4R/Local-Image) ·
[Local Photo](https://github.com/Anon5T4R/Local-Photo) ·
[Local Paint](https://github.com/Anon5T4R/Local-Paint) ·
[Local Vector](https://github.com/Anon5T4R/Local-Vector) ·
[Local Player](https://github.com/Anon5T4R/Local-Player) ·
[Local Obsidian](https://github.com/Anon5T4R/Local-Obsidian) ·
[Local Mind](https://github.com/Anon5T4R/Local-Mind) ·
[Local Utils](https://github.com/Anon5T4R/Local-Utils)

## License and credits

MIT or Apache-2.0, at your option ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE),
[NOTICE](NOTICE)). The original PdfCraft authors' credits, and every bundled font, icon, model and
image with its license, are in [ATTRIBUTION.md](ATTRIBUTION.md). Release builds embed the Japanese
fonts of [craft-fonts](https://github.com/storytold/craft-fonts/blob/main/ATTRIBUTION.md) (SIL Open
Font License 1.1).
