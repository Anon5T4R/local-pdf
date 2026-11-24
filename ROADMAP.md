# Local PDF — state and queue

Independent fork of pdfcraft (ArtCraft), rebranded for the Local suite. Version and release notes:
`cargo xtask version`, tags and GitHub Releases. Where everything lives: `AGENTS.md`.

## What is ready

- **Read and navigate**: open PDFs (including password-protected), search, panels (comments,
  bookmarks, pages, forms, layers, attachments, signatures, accessibility, tags), view modes (single
  page, continuous, two pages) and direct printing on the desktop (`craft-print`).
- **Edit**: existing text and figures on the page, move/duplicate several objects, redact, stamp,
  watermark, header/footer and background; organize pages (insert, extract, split, combine, rotate).
- **Forms**: full AcroForm (fill, flatten, export/import XFDF) and static and dynamic XFA 3.3
  (`crates/xfa`); form JavaScript through a homegrown subset (`crates/js`).
- **Document**: digital signature, file comparison, optimization/compression, OCR, export
  images/text, PDF/A, accessibility and prepress audits.
- **Agent-native**: `localpdf-cli mcp` (headless, `crates/automation`) and `mcp --connect` in the
  open app, served by `craft-mcp` (`docs/mcp.md`, `docs/cli-batch.md`).
- **Suite-standard shell**: mode bar with tabs (All tools, Edit, Convert, E-Sign), AI panel,
  palette, Automatic/Light/Dark themes, update card from `craft-update`.

## What is missing

In the order it hurts daily use (surveyed 2026-10, and fixed in parts since then):

1. **Fidelity on hard PDFs**: what reads the whole specification is `crates/cos`/`crates/render`;
   pages with rare constructions (malformed Type 3 fonts, unsupported colors, broken structures)
   still diverge from Acrobat and there is no automated page-by-page comparison.
2. **XFA**: coverage of real forms is still incomplete (`crates/xfa`).
3. **Document JavaScript**: the subset in `crates/js` does not cover all of Acrobat's APIs.
4. **Mirrored interface (RTL)** and the translation catalogs still incomplete in some languages.
5. **Platforms**: this fork publishes Windows and Linux; macOS, BSD and Flatpak are not published.

Release, CI and local gate items: `AGENTS.md` (§ Regras, § Build) and `docs/git-protocol.md` in the
dev-notes. Upstream sync decisions: `docs/UPSTREAM-SYNC.md`.
