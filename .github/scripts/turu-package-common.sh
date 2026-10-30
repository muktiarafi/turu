#!/usr/bin/env bash
# Fills a release package folder with what every platform ships next to the program: the
# readme and the licenses.
#
#   .github/scripts/turu-package-common.sh <folder>
#
# The scripts aren't shipped: Scripts > Search Scripts downloads them from turu-scripts into the
# data folder and keeps them updated (bot/src/repo.rs). A copy here would go stale, and Load
# Script would open it first (desktop/src/gui/turu/files.rs looks above the executable).
set -Eeuo pipefail

out="$1"
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"

mkdir -p "$out"
cp "$root/README.md" "$out/README.md"
cp "$root/LICENSE.md" "$out/LICENSE.md"

cat > "$out/README.txt" <<'TXT'
Turu - an AdventureQuest Worlds bot client (Ruffle + Mugetsu's AQW patches).

Start the program (turu.exe, turu, or Turu.app) and log in with the game's own screen.
Scripts > Search Scripts lists the scripts: Start downloads one and runs it. At launch Turu
downloads and updates the scripts (Options > Application > Auto Update Scripts). They, your
settings, options and logs are kept in Turu's data folder (Windows %APPDATA%\Turu, macOS
~/Library/Application Support/Turu, Linux ~/.local/share/Turu); the scripts are in its
`scripts` folder, where Scripts > Load Script starts.

macOS: the app is not notarized. If macOS blocks the first start, allow it in
System Settings > Privacy & Security (Open Anyway), or run:
    xattr -dr com.apple.quarantine Turu.app

This is an alpha build: expect bugs.
TXT
