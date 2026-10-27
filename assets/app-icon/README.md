# LocalPDF app icon

<img src="localpdf-small.svg" alt="LocalPDF app icon: a document with a folded corner and text lines, on a red-to-orange gradient tile" width="128">

**Figure:** a document sheet with a folded corner and three text lines, drawn in outline.

**Style:** flat, thick round-capped strokes on a full-bleed gradient tile — the same language as
the rest of the Local suite's icons.

**Palette:**

| Colour | Hex | Used for |
|---|---|---|
| Red | `#dc2626` | gradient start (top-left) |
| Orange | `#f97316` | gradient end (bottom-right) |
| White | `#ffffff` | the sheet, fold and text lines |
| — | `#ffffff` at 92% | the signature "Taylor" at the tile's foot |

**Tile:** `viewBox="0 0 1024 1024"`, a rounded square with `rx=200` that clips everything. Windows and
Linux icons use the full-bleed tile. macOS icons put it on Apple's grid (an 824 px body centred on a
transparent 1024 px canvas).

**Provenance:** the project owner's original SVG (`source-localpdf.svg`), used as the master vector.
The 1024 px PNG is a faithful raster of it. Licence: [LICENSE.txt](LICENSE.txt) (`MIT OR Apache-2.0`,
like the repo).

## Files

| File | What it is |
|---|---|
| `localpdf.svg` | the master vector (the project owner's source SVG); every PNG, `.ico` and `.icns` is rendered from it |
| `localpdf-small.svg` | copy of the master for places where a relative path reads better, such as this README |
| `localpdf-1024.png` | 1024 px raster of the master; also the runtime Dock icon on macOS |
| `localpdf.icns` | macOS icon (16–1024 px) |
| `localpdf.ico` | Windows icon, embedded in `localpdf.exe` by `apps/localpdf/build.rs` |
| `localpdf-document.svg` | Windows PDF file icon master: a Lucide file-text outline (ISC) with the app's gradient badge and "PDF" lettering |
| `localpdf-document.ico` | Windows PDF file icon (16–256 px), embedded as resource 2 in `localpdf.exe` |
| `hicolor/<n>x<n>/apps/com.local.localpdf.png` | Linux hicolor theme, 16–512 px; the 256 px one is the runtime icon on Windows and Linux |
| `hicolor/scalable/apps/com.local.localpdf.svg` | Linux scalable icon (copy of the master) |

Where it shows: `apps/localpdf/src/main.rs` sets the window icon (Dock, taskbar, Alt-Tab, launcher) and the
Wayland app id `com.local.localpdf`; `packaging/linux/com.local.localpdf.desktop` names the
hicolor icon.

Windows PDF files use resource 2 (`"localpdf.exe",-2`) when LocalPDF is chosen as the default
