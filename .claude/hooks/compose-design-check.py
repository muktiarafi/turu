#!/usr/bin/env python3
"""Design checks for the Android shell's Compose screens, as a Claude Code hook.

Impeccable's own hook skips every file in this repo (PRODUCT.md's platform is `adaptive`, so its
log says `skipped: native-platform`), and its detector has no Compose rules: a Kotlin file runs
only its text rules, which find nothing in Compose. This hook covers that gap with the mechanical
checks of Impeccable's craft floor and Android reference, read against the approved pass in
docs/UI.md ("Native shells", Android) and `ShellTheme.kt`, plus the owner's Kotlin rules
(CLAUDE.md). It runs `impeccable detect` on the file as well, so its rules count once they
learn Compose.

PostToolUse (Edit|Write): checks the edited file and tells the agent what it found (or that it
is clean). Stop: checks every Kotlin UI file changed against HEAD, once per turn, and asks the
agent to finish only when something is left.

Waive a finding on its line with a reason, as Impeccable's detector reads them:
    // impeccable-disable-line <rule>: <why>
    // impeccable-disable-next-line <rule>: <why>      (on the line above)
    // impeccable-disable <rule>: <why>                (anywhere: the whole file)

Run by hand: `.claude/hooks/compose-design-check.py --check <file.kt>...` (exit 1 on findings).
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(os.environ.get("CLAUDE_PROJECT_DIR") or Path(__file__).resolve().parents[2])
# Android's Kotlin with screens in it: the app and the code both Android apps share.
UI_DIRS = ("android/app/src/main/kotlin/", "android/shared/src/main/")
THEME_FILE = "ShellTheme.kt"
IMPECCABLE = ROOT / ".claude/skills/impeccable/scripts/impeccable"
MAX_FINDINGS = 12

# --- rules -------------------------------------------------------------------------------

RULES: dict[str, str] = {
    "raw-color": "a colour literal outside ShellTheme.kt: use a `Turu.*` token or a "
    "`MaterialTheme.colorScheme` role (docs/UI.md pins the palette to egui's)",
    "tiny-text": "text under 12 sp: below the M3 scale's smallest label; use a "
    "`MaterialTheme.typography` role",
    "hand-picked-type": "a hand-picked font size: map the text to an M3 type role "
    "(`MaterialTheme.typography.*`) so it follows the scale",
    "px-text": "a font size in dp or px: text is sized in sp so it follows the system font scale",
    "small-target": "a tappable element smaller than 48 dp: Android's minimum touch target "
    "(use `minimumInteractiveComponentSize()` or pad the hit area)",
    "gradient": "a gradient brush: the pass has none (emphasis comes from weight, size or the "
    "accent)",
    "glow": "a coloured shadow: elevation is tonal on M3; a coloured halo is decoration",
    "glyph-icon": "a Unicode glyph standing in for an icon: use Material Symbols "
    "(`Icons.Rounded.*`) or `RailIcons.kt`'s drawn ones",
    "em-dash": "an em dash in user-facing copy: Turu's copy uses colons, commas or full stops",
    "deprecated-suppressed": "`@Suppress(\"DEPRECATION\")`: the owner's rule is no deprecated "
    "API, hidden or not; use the AndroidX compat replacement",
    "placeholder-copy": "placeholder copy: write the product's own words",
    "toast": "a Toast for feedback: M3 wants a snackbar (keep a toast only where no snackbar "
    "host can show, and waive it saying so)",
    "hardcoded-white-text": "`Color.White` text off the accent: use `Turu.Text` or the role's "
    "`on*` colour",
}

GLYPHS = "✕✖✗✘×⋮⋯›‹»«★☆✓✔•▶◀▲▼➜→←↑↓☰⚙✎✏🔍"
STRING_RE = re.compile(r'"((?:[^"\\]|\\.)*)"')
TAPPABLE_RE = re.compile(r"\.(clickable|combinedClickable|toggleable|selectable|pointerInput)\b")
SIZE_RE = re.compile(r"\.(size|height|requiredSize|requiredHeight|width|requiredWidth)\(\s*(\d+(?:\.\d+)?)\.dp")
FONT_SIZE_RE = re.compile(r"fontSize\s*=\s*\(?\s*(\d+(?:\.\d+)?)f?\s*(?:\.(sp|dp|em)|[/*)])")
FONT_UNIT_RE = re.compile(r"fontSize\s*=\s*[^,\n)]*\.(dp)\b|fontSize\s*=\s*\d+(?:\.\d+)?\.px\b")
DISABLE_FILE_RE = re.compile(r"impeccable-disable\s+([\w-]+)")
DISABLE_LINE_RE = re.compile(r"impeccable-disable-line\s+([\w-]+)")
DISABLE_NEXT_RE = re.compile(r"impeccable-disable-next-line\s+([\w-]+)")


def code_part(line: str) -> str:
    """The line without a trailing `//` comment (outside strings, roughly)."""
    out, in_str, i = [], False, 0
    while i < len(line):
        c = line[i]
        if c == "\\" and in_str:
            out.append(line[i : i + 2])
            i += 2
            continue
        if c == '"':
            in_str = not in_str
        if not in_str and line.startswith("//", i):
            break
        out.append(c)
        i += 1
    return "".join(out)


def check_text(path: str, text: str) -> list[tuple[int, str, str]]:
    """Findings as (line, rule, the line's code)."""
    name = os.path.basename(path)
    lines = text.splitlines()
    file_waived = set(DISABLE_FILE_RE.findall(text)) - {"line", "next"}
    findings: list[tuple[int, str, str]] = []
    in_block_comment = False
    is_ui = "@Composable" in text

    for n, raw in enumerate(lines, 1):
        stripped = raw.strip()
        if in_block_comment:
            if "*/" in stripped:
                in_block_comment = False
            continue
        if stripped.startswith("/*"):
            in_block_comment = "*/" not in stripped
            continue
        if stripped.startswith("//") or stripped.startswith("*") or stripped.startswith("import "):
            continue
        code = code_part(raw)
        waived = set(file_waived) | set(DISABLE_LINE_RE.findall(raw))
        if n > 1:
            waived |= set(DISABLE_NEXT_RE.findall(lines[n - 2]))
        hits: list[str] = []

        if "@Suppress(" in code and "DEPRECATION" in code:
            hits.append("deprecated-suppressed")

        if is_ui:
            strings = STRING_RE.findall(code)
            if name != THEME_FILE and re.search(r"\bColor\(\s*0x", code):
                hits.append("raw-color")
            for m in FONT_SIZE_RE.finditer(code):
                if m.group(2) in ("dp",):
                    hits.append("px-text")
                elif float(m.group(1)) < 12:
                    hits.append("tiny-text")
                else:
                    hits.append("hand-picked-type")
            if FONT_UNIT_RE.search(code):
                hits.append("px-text")
            if TAPPABLE_RE.search(code):
                for m in SIZE_RE.finditer(code):
                    if float(m.group(2)) < 48:
                        hits.append("small-target")
                        break
            if re.search(r"Brush\.(linear|horizontal|vertical|radial|sweep)Gradient", code):
                hits.append("gradient")
            if re.search(r"\.shadow\([^)]*(spotColor|ambientColor)", code):
                hits.append("glow")
            if any(s.strip() and all(ch in GLYPHS or ch.isspace() for ch in s) for s in strings):
                hits.append("glyph-icon")
            if any("—" in s for s in strings):
                hits.append("em-dash")
            if any(re.search(r"(?i)lorem ipsum|placeholder text|TODO copy", s) for s in strings):
                hits.append("placeholder-copy")
            if re.search(r"\bToast\.makeText\b", code):
                hits.append("toast")
            if re.search(r"\b(color|contentColor)\s*=\s*Color\.White\b", code) and "Accent" not in code:
                hits.append("hardcoded-white-text")

        for rule in dict.fromkeys(hits):
            if rule not in waived:
                findings.append((n, rule, stripped[:140]))
    return findings


def is_ui_file(path: str) -> bool:
    rel = os.path.relpath(os.path.abspath(path), ROOT)
    return rel.endswith(".kt") and any(rel.startswith(d) for d in UI_DIRS) and "/build/" not in rel


def impeccable_detect(paths: list[str]) -> str:
    """Impeccable's own findings on the files, when it has any."""
    if not IMPECCABLE.exists() or not paths:
        return ""
    try:
        out = subprocess.run(
            [str(IMPECCABLE), "detect", "--json", *paths],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=20,
        )
        data = json.loads(out.stdout or "[]")
    except (OSError, subprocess.SubprocessError, ValueError):
        return ""
    items = data if isinstance(data, list) else data.get("findings", [])
    lines = []
    for f in items[:MAX_FINDINGS]:
        if isinstance(f, dict):
            where = f"{os.path.relpath(f.get('file', ''), ROOT)}:{f.get('line', '?')}"
            lines.append(f"- {where} [{f.get('antipattern') or f.get('id') or f.get('rule')}] {f.get('description') or f.get('message', '')}")
    return "\n".join(lines)


def report(per_file: dict[str, list[tuple[int, str, str]]]) -> str:
    out, count = [], 0
    for path, findings in per_file.items():
        rel = os.path.relpath(path, ROOT)
        for line, rule, code in findings:
            count += 1
            if count > MAX_FINDINGS:
                continue
            out.append(f"- {rel}:{line} [{rule}] {RULES[rule]}\n    {code}")
    if count > MAX_FINDINGS:
        out.append(f"- ... and {count - MAX_FINDINGS} more (run .claude/hooks/compose-design-check.py --check)")
    return "\n".join(out)


TRIAGE = (
    "Fix each finding, or, when it is deliberate (a data colour such as a class's, a badge's "
    "small label), waive it on its line with `// impeccable-disable-line <rule>: <why>` and say "
    "so in your reply. Unsure: ask the owner in one line."
)


def emit(event: str, context: str) -> None:
    print(json.dumps({"hookSpecificOutput": {"hookEventName": event, "additionalContext": context}}))


def changed_ui_files() -> list[str]:
    try:
        tracked = subprocess.run(
            ["git", "diff", "--name-only", "HEAD"], cwd=ROOT, capture_output=True, text=True, timeout=10
        ).stdout.split()
        untracked = subprocess.run(
            ["git", "ls-files", "--others", "--exclude-standard"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=10,
        ).stdout.split()
    except (OSError, subprocess.SubprocessError):
        return []
    return [str(ROOT / p) for p in tracked + untracked if is_ui_file(str(ROOT / p)) and (ROOT / p).exists()]


def check_files(paths: list[str]) -> dict[str, list[tuple[int, str, str]]]:
    per_file = {}
    for p in paths:
        try:
            findings = check_text(p, Path(p).read_text(encoding="utf-8"))
        except OSError:
            continue
        if findings:
            per_file[p] = findings
    return per_file


def main() -> int:
    if len(sys.argv) > 1 and sys.argv[1] == "--check":
        paths = [str(Path(p).resolve()) for p in sys.argv[2:]] or changed_ui_files()
        per_file = check_files(paths)
        print(report(per_file) or f"Compose design check: {len(paths)} file(s) clean.")
        extra = impeccable_detect(paths)
        if extra:
            print("Impeccable detect:\n" + extra)
        return 1 if per_file or extra else 0

    if os.environ.get("IMPECCABLE_HOOK_DISABLED") == "1":
        return 0
    try:
        payload = json.load(sys.stdin)
    except ValueError:
        return 0
    event = payload.get("hook_event_name", "")

    if event == "PostToolUse":
        path = (payload.get("tool_input") or {}).get("file_path", "")
        if not path or not is_ui_file(path) or not os.path.exists(path):
            return 0
        per_file = check_files([path])
        extra = impeccable_detect([path])
        rel = os.path.relpath(path, ROOT)
        if not per_file and not extra:
            emit(event, f"Compose design check: {rel} clean ({len(RULES)} rules + impeccable detect).")
            return 0
        body = report(per_file)
        if extra:
            body += ("\n" if body else "") + "Impeccable detect:\n" + extra
        emit(event, f"Compose design check found issues in {rel}:\n{body}\n{TRIAGE}")
        return 0

    if event == "Stop":
        if payload.get("stop_hook_active"):
            return 0
        paths = changed_ui_files()
        per_file = check_files(paths)
        if not per_file:
            return 0
        reason = (
            "Compose design deep pass over the Kotlin UI files changed against HEAD:\n"
            f"{report(per_file)}\n{TRIAGE}"
        )
        print(json.dumps({"decision": "block", "reason": reason}))
        return 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
