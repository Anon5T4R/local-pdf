#!/usr/bin/env bash
# Installs the app from this extracted tarball folder:
#
#   ./install.sh [prefix]     (default: ~/.local — this user only, no root)
#
# Copies bin/ into <prefix>/bin and share/ into <prefix>/share, then refreshes
# the desktop/MIME/icon caches, so the app shows up in the application menu.
# Running it again updates in place (self-update keeps working: craft-update
# replaces the binary where it runs from).
#
# Uninstall: remove <prefix>/bin/<app>, <prefix>/share/applications/<app>.desktop
# and <prefix>/share/icons/hicolor/*/apps/<app>.png.
set -euo pipefail
PREFIX="${1:-$HOME/.local}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

[[ -d "$HERE/bin" && -d "$HERE/share" ]] || { echo "error: run this inside the extracted tarball folder (bin/ + share/)" >&2; exit 1; }
shopt -s nullglob
BINS=("$HERE"/bin/*)
[[ ${#BINS[@]} -gt 0 ]] || { echo "error: bin/ is empty" >&2; exit 1; }

mkdir -p "$PREFIX/bin" "$PREFIX/share"
install -m 755 "${BINS[@]}" "$PREFIX/bin/"
cp -a "$HERE/share/." "$PREFIX/share/"

# Caches: every tool is optional; missing ones are skipped (the desktop picks
# the files up on the next login anyway).
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database -q "$PREFIX/share/applications" || true
fi
if [[ -d "$HERE/share/mime" ]] && command -v update-mime-database >/dev/null 2>&1; then
  update-mime-database "$PREFIX/share/mime" || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q -t -f "$PREFIX/share/icons/hicolor" 2>/dev/null || true
fi

echo "installed into $PREFIX:"
for b in "${BINS[@]}"; do echo "  bin/$(basename "$b")"; done
echo "launch it from the application menu, or run: $(basename "${BINS[0]}")"
