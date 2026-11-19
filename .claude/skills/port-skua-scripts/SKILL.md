---
name: port-skua-scripts
description: Port Skua's C# scripts (auqw/Scripts, checked out at ../Scripts) to Turu's TypeScript scripts (the scripts/ submodule). Use when catching up with upstream Skua script changes, or porting a C# script or folder that has no port yet.
---

# Porting Skua scripts

Turu's `scripts/` submodule is a port of auqw/Scripts (branch `Skua`, checked out read-only at
`../Scripts`). [`scripts/docs/PORTING.md`](../../../scripts/docs/PORTING.md) is the source of truth
for conventions, what each area leaves out, and the C# bugs fixed on purpose; read its
Conventions and cs2ts sections before writing a port. Porting one file follows
[PORT-FILE.md](PORT-FILE.md).

A port is never a transcription. Where the C# logic is wrong (a wait that never waits, a quest
redone on every run, state left behind for the next script), fix it, mark the line with a `C#`
comment saying what C# did, and list it under the area's "C# bugs fixed" in PORTING.md.

## Catching up with upstream

1. **Find the baseline.** PORTING.md's intro names the upstream commit the ports follow. Trust
   that line, never commit dates or the time a port was made: `git -C ../Scripts reflog --date=iso`
   shows where the checkout really was. Done when you have one commit sha.

2. **Update the checkout.** `origin` is SSH, which sessions can't use: fetch over HTTPS
   (`git -C ../Scripts fetch https://github.com/auqw/Scripts.git Skua`) and fast-forward
   (`git -C ../Scripts merge --ff-only FETCH_HEAD`). cs2ts reads the working tree, so it must be
   at the new head. Done when `../Scripts` HEAD is the fetched head.

3. **List every change.** `git -C ../Scripts diff --name-status -M <baseline>..HEAD` with no path
   filter, then read each diff (`git -C ../Scripts diff <baseline>..HEAD -- <file>`). Sort every
   file into one of:
   - **edit to a ported file:** port the change into its TS twin (same path, lowercased folders);
   - **new script:** port it ([PORT-FILE.md](PORT-FILE.md));
   - **rename:** `git mv` the port and fix its `// Port of Scripts/...` line;
   - **`Skills/AdvancedSkills.json`:** Turu ships it as `bot/assets/AdvancedSkills.json` in
     turu-next, with LF endings: `tr -d '\r' < ../Scripts/Skills/AdvancedSkills.json > bot/assets/AdvancedSkills.json`;
   - **skip:** generated data (`scripts.json`, `QuestData.json`, `.last_version`), older versions
     (`Ultras/`, `Ultrasv2/`, names ending in V1/V2 next to a newer one), `Templates/`, and the
     skip tables in PORTING.md (Tools, Army). A file the TS side can't do (raw sockets, Skua's
     files) joins those tables with its reason.

   Done when every listed file has a bucket. Then sweep for what was never ported at all,
   comparing full lowercased paths, not basenames:
   `git -C ../Scripts ls-files '*.cs'` against `git -C scripts ls-files '*.ts'`. Anything left
   that no skip covers goes into a bucket too.

4. **Port.** Core files first (`CoreBots`, `CoreAdvanced`, `CoreFarms`, `CoreStory`, area cores),
   since scripts call what they add; then edits to scripts; then new scripts. Before porting a
   core member, read its TS twin: earlier ports already fixed some C# bugs there, and a fix
   stays. Grep every call site of a changed signature or value (an item name that became an ID
   still has log messages and blacklists that need the name). Done when each bucketed file is
   ported, skipped with a reason, or named to the user as left.

5. **Check.** In `scripts/`, each must pass:
   ```bash
   npx -y -p typescript@5.9.3 tsc --noEmit -p .
   npx -y prettier@3.6.2 --write <changed files>; npx -y prettier@3.6.2 --check "**/*.{ts,mjs}"
   node tools/check-library-options.mjs
   node tools/build-index.mjs > /dev/null
   ```
   and in turu-next, `cargo nextest run -p turu_bot` (it loads every repo script:
   `every_repo_script_declares_its_options_without_error`, `repo_scripts_transpile`; the
   `skill` tests cover the shipped skill sets). None of this proves a script works in game; say so.

6. **Record.** In PORTING.md: move the baseline line to the new head (and note what this batch
   covered), add each new port and skip to its area, each C# fix to its list. Update the README's
   "What's ported" table if an area's coverage changed.

## Committing (only when asked)

Commit in `scripts/` first, on its `main`, split by kind as earlier batches did
(`fix: Port the C# changes made upstream up to <sha>`, `feat: Port <new scripts>`,
`docs: ...`). Then in turu-next on `main`: the skill sets
(`feat: Ship upstream's skill sets as of auqw/Scripts <sha>`) and the submodule bump
(`chore: Bump scripts (<summary>)`). When turu-next's checkout is on another branch or has
unrelated uncommitted work, commit through a temporary worktree on `main` and set the gitlink
with `git update-index --cacheinfo 160000,<scripts sha>,scripts`, leaving the user's checkout
as it was. Push only when asked.
