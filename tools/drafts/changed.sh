#!/usr/bin/env bash
# Which drafts a change to the scripts submodule touches, for CI's "Changed drafts" job
# (docs/REPLAY_TESTS_PROPOSAL.md, phase 4). Compares the submodule's commit at <base> with the one
# at <head> (default HEAD) and prints one line:
#
#   skip                     no draft changed (or the submodule didn't move)
#   DRAFT_FILTER=<a>,<b>     the changed drafts, paths under scripts/
#   DRAFT_EVERY=16           a Core file changed (every draft imports Core), or a library more
#                            than 60 drafts import: a sample
#
#   tools/drafts/changed.sh origin/main [HEAD]
#
# A draft is a .ts file with cs2ts's "not yet read through by hand" line and a `main`, as
# bot/tests/draft_smoke.rs picks them. A changed library (a file with no `main`, such as
# ultraslw/CoreLoneWolf.ts) brings in the drafts that import it. Needs both submodule commits
# fetched (CI fetches the base's).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
base="$1"
head="${2:-HEAD}"

sub() { git -C "$root" ls-tree "$1" scripts | awk '{print $3}'; }
old="$(sub "$base")"
new="$(sub "$head")"
if [[ -z "$old" || -z "$new" || "$old" == "$new" ]]; then
  echo skip
  exit 0
fi

changed="$(git -C "$root/scripts" diff --name-only --diff-filter=AMR "$old" "$new" -- '*.ts')"
if grep -q '^core/' <<<"$changed"; then
  echo "DRAFT_EVERY=16"
  exit 0
fi

is_draft() {
  local text
  text="$(git -C "$root/scripts" show "$new:$1" 2>/dev/null || true)"
  grep -q 'not yet read through by hand' <<<"$text" && grep -q 'export default function main' <<<"$text"
}

candidates=()
while IFS= read -r path; do
  [[ -z "$path" ]] && continue
  # Read first, not piped into `grep -q`: with pipefail, grep quitting early fails the pipe.
  text="$(git -C "$root/scripts" show "$new:$path" 2>/dev/null || true)"
  if grep -q 'export default function main' <<<"$text"; then
    candidates+=("$path")
  else
    # A library: the scripts that import it (`from "../CoreLoneWolf"`, with or without `.ts`).
    module="$(basename "$path" .ts)"
    while IFS= read -r importer; do
      candidates+=("${importer#"$new":}")
    done < <(git -C "$root/scripts" grep -lE "from \"[^\"]*/${module}(\.ts)?\"" "$new" -- '*.ts' || true)
  fi
done <<<"$changed"

drafts=()
if (( ${#candidates[@]} > 0 )); then
  while IFS= read -r path; do
    is_draft "$path" && drafts+=("$path")
  done < <(printf '%s\n' "${candidates[@]}" | sort -u)
fi
if (( ${#drafts[@]} > 60 )); then
  echo "DRAFT_EVERY=16"
  exit 0
fi

if (( ${#drafts[@]} == 0 )); then
  echo skip
else
  (IFS=,; echo "DRAFT_FILTER=${drafts[*]}")
fi
