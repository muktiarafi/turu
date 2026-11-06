---
version: 1
slug: "bot-assets-monitor-index-html"
primary_target: "bot/assets/monitor/index.html"
related_targets: []
---

# Turu Monitor (web dashboard served by the app)

Scope: `bot/assets/monitor/index.html`, the single self-contained page Turu serves at `/`. Mode: Operate.
Audience: AQW players running several bots unattended (often overnight), checking in from a phone or another PC.
Job: glance at every bot's state, spot the one that needs attention, act on it (start/stop/relog/join), read its log, events and history.
Constraints: one file, no external fetches (the page runs on a LAN or Tailscale with no internet guarantee), works at 360px wide, the key stays in localStorage, data via `/api/events` SSE and `/api/bots/...`. Keep every function of the previous page (views, filter, bulk actions, detail tabs, streamer mode, key dialog, confirmations).

## Direction contract

FORM: Nila (indigo-dyed batik cloth, nila and soga), seed:4152c03d54227230.
THESIS: Monitoring a sleeping bot is reading a cloth: one indigo field, each bot a line of wax dots tracing its last hour. Refuses the default dark card grid with a purple accent and a stat-tile header.
OWN-WORLD: Deep nila indigo grounds in three layers, unbleached mori cream as ink, soga brown as the only accent (primary actions, selection). State dyes: mengkudu red (trouble), kunyit yellow (degraded), jade (playing). System sans, tabular figures, hairline indigo rules, small radii.
STORY: Top bar names the night (playing / running / needs you). A "Needs you" band lists trouble first with its one fixing action. The roster rows follow, grouped by machine. A row opens a docked side panel.
FIRST VIEWPORT: Bar, the needs-you band (or a quiet "all asleep and farming" line), the roster with traces.
SIGNATURE: the cecek trace: 30 dots for the last 60 minutes per bot, cream while the script ran, dye-coloured marks at events, the "now" dot breathing while it runs.
RAISES: fixed columns that never reflow on updates (split-flap); attention ranked first (star atlas); compact whole-row density (circle catalog).
