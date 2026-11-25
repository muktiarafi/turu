#!/usr/bin/env bash
# Adds Turu to the application menu (and the desktop's launcher) with its icon, for the user
# only: no root. Run it from the unpacked folder; `--uninstall` removes what it added.
set -Eeuo pipefail

here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
apps="$data/applications"
icons="$data/icons/hicolor"

if [[ "${1:-}" == "--uninstall" ]]; then
  rm -f "$apps/turu.desktop" "$icons/256x256/apps/turu.png" "$icons/512x512/apps/turu.png"
  command -v update-desktop-database >/dev/null && update-desktop-database "$apps" || true
  command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$icons" || true
  echo "Removed Turu's launcher."
  exit 0
fi

[[ -x "$here/turu" ]] || { echo "turu not found next to this script" >&2; exit 1; }

mkdir -p "$apps" "$icons/256x256/apps" "$icons/512x512/apps"
install -m 644 "$here/turu.png" "$icons/256x256/apps/turu.png"
install -m 644 "$here/turu-512.png" "$icons/512x512/apps/turu.png"
# The entry needs the program's absolute path: the folder can be anywhere.
sed -e "s|^Exec=.*|Exec=\"$here/turu\"|" -e "s|^Path=.*||" "$here/turu.desktop" > "$apps/turu.desktop"
echo "Path=$here" >> "$apps/turu.desktop"
chmod 644 "$apps/turu.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$apps" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$icons" || true
echo "Added Turu to the application menu. Move the folder and run this again."
