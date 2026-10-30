#!/usr/bin/env bash
# Fills a release package folder with what every platform ships next to the program:
# the readme, the licenses and the TypeScript scripts (with the script API types).
#
#   .github/scripts/turu-package-common.sh <folder>
#
# Turu's Load Script starts in a `scripts` folder up to five levels above the executable that
# has `types/` or `core/` (desktop/src/gui/turu/files.rs), so `scripts/` sits at the package root.
set -Eeuo pipefail

out="$1"
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"

mkdir -p "$out"
cp "$root/README.md" "$out/README.md"
cp "$root/LICENSE.md" "$out/LICENSE.md"

cp -R "$root/scripts" "$out/scripts"
# The script API, where the shipped tsconfig.json finds it (the repo maps it to ../bot/js).
mkdir -p "$out/scripts/types"
cp "$root/bot/js/turu.ts" "$out/scripts/types/turu.ts"
sed -i.bak 's#"\.\./bot/js/turu\.ts"#"./types/turu.ts"#' "$out/scripts/tsconfig.json"
rm "$out/scripts/tsconfig.json.bak"
grep -q '"./types/turu.ts"' "$out/scripts/tsconfig.json"

cat > "$out/README.txt" <<'TXT'
Turu - an AdventureQuest Worlds bot client (Ruffle + Mugetsu's AQW patches).

Start the program (turu.exe, turu, or Turu.app) and log in with the game's own screen.
Scripts > Load Script opens the bundled `scripts` folder. Your settings, options and logs are
kept in Turu's data folder (Windows %APPDATA%\Turu, macOS ~/Library/Application Support/Turu,
Linux ~/.local/share/Turu).

macOS: the app is not notarized. If macOS blocks the first start, allow it in
System Settings > Privacy & Security (Open Anyway), or run:
    xattr -dr com.apple.quarantine Turu.app

This is an alpha build: expect bugs.
TXT
