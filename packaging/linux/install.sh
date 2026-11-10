#!/usr/bin/env bash
# Installs the app from this extracted tarball folder:
#
#   ./install.sh [prefix]               (default: ~/.local — this user only, no root)
#   ./install.sh --uninstall [prefix]   removes exactly what a previous install added
#
# Copies bin/ into <prefix>/bin and share/ into <prefix>/share, then refreshes
# the desktop/MIME/icon caches, so the app shows up in the application menu.
# Running it again updates in place (self-update keeps working: craft-update
# replaces the binary where it runs from). Uninstall mirrors the tarball tree:
# removes the binaries and the share/ files it provides, then prunes the empty
# directories it left behind (user data in ~/.local/share/<app> is not touched).
set -euo pipefail
PREFIX="$HOME/.local"
UNINSTALL=0
for arg in "$@"; do
  case "$arg" in
    --uninstall) UNINSTALL=1 ;;
    -h | --help) sed -n '2,9p' "$0"; exit 0 ;;
    *) PREFIX="$arg" ;;
  esac
done
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

[[ -d "$HERE/bin" && -d "$HERE/share" ]] || { echo "error: run this inside the extracted tarball folder (bin/ + share/)" >&2; exit 1; }
shopt -s nullglob
BINS=("$HERE"/bin/*)
[[ ${#BINS[@]} -gt 0 ]] || { echo "error: bin/ is empty" >&2; exit 1; }

refresh_caches() {
  # Every tool is optional; missing ones are skipped (the desktop picks
  # the files up on the next login anyway).
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q "$PREFIX/share/applications" || true
  fi
  if [[ -d "$PREFIX/share/mime" ]] && command -v update-mime-database >/dev/null 2>&1; then
    update-mime-database "$PREFIX/share/mime" || true
  fi
  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f "$PREFIX/share/icons/hicolor" 2>/dev/null || true
  fi
}

if [[ "$UNINSTALL" = 1 ]]; then
  for b in "${BINS[@]}"; do rm -f "$PREFIX/bin/$(basename "$b")"; done
  (cd "$HERE/share" && find . -type f -print0) | while IFS= read -r -d '' f; do
    rm -f "$PREFIX/share/${f#./}"
  done
  # Caches regeneráveis criados na instalação (não vêm do tarball e prendem
  # os diretórios): o sistema recria ambos quando precisar.
  rm -f "$PREFIX/share/applications/mimeinfo.cache" "$PREFIX/share/icons/hicolor/icon-theme.cache"
  # Diretórios que ficaram vazios (só os nossos; rmdir não remove não-vazios):
  # a árvore do tarball em share/, e o esqueleto bin/share do prefixo.
  (cd "$HERE/share" && find . -type d -print0) | sort -rz | while IFS= read -r -d '' d; do
    rmdir "$PREFIX/share/${d#./}" 2>/dev/null || true
  done
  rmdir "$PREFIX/share/applications" "$PREFIX/share/icons" "$PREFIX/share" "$PREFIX/bin" 2>/dev/null || true
  # Só revalida o cache de aplicações se sobrou entrada de OUTRO app no
  # prefixo (senão o refresh recriaria mimeinfo.cache vazio à toa).
  if ls "$PREFIX/share/applications/"*.desktop >/dev/null 2>&1; then
    if command -v update-desktop-database >/dev/null 2>&1; then
      update-desktop-database -q "$PREFIX/share/applications" || true
    fi
  fi
  echo "uninstalled from $PREFIX"
  exit 0
fi

mkdir -p "$PREFIX/bin" "$PREFIX/share"
install -m 755 "${BINS[@]}" "$PREFIX/bin/"
cp -a "$HERE/share/." "$PREFIX/share/"
refresh_caches

# O PATH da SESSÃO gráfica (não o do shell) é o que resolve o Exec= no menu —
# Arch/GNOME não inclui ~/.local/bin por padrão e o app fica invisível.
# Aviso + fix de uma linha (environment.d), pra nunca virar mistério.
warn_session_path() {
  local ses=""
  if command -v systemctl >/dev/null 2>&1; then
    ses="$(systemctl --user show-environment 2>/dev/null | grep '^PATH=' | cut -d= -f2- || true)"
  fi
  [[ -z "$ses" ]] && ses="$PATH"
  case ":$ses:" in
    *":$PREFIX/bin:"*) return 0 ;;
  esac
  echo "aviso: $PREFIX/bin não está no PATH da sessão gráfica — o app pode não aparecer no menu." >&2
  echo "  fix (uma vez, e re-login):" >&2
  echo "    mkdir -p ~/.config/environment.d" >&2
  echo "    printf 'PATH=$PREFIX/bin:\\\$PATH\n' > ~/.config/environment.d/90-local-bin.conf" >&2
}
warn_session_path

echo "installed into $PREFIX:"
for b in "${BINS[@]}"; do echo "  bin/$(basename "$b")"; done
echo "launch it from the application menu, or run: $(basename "${BINS[0]}")"
echo "uninstall with: ./install.sh --uninstall"
