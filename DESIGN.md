---
name: Turu Monitor
description: The web dashboard Turu serves itself, for checking on bots left running overnight.
colors:
  nila-ground: "#0e1226"
  nila-field: "#141936"
  nila-raise: "#1c2248"
  nila-sunk: "#0a0d1e"
  indigo-rule: "#2a3266"
  indigo-rule-soft: "#1f2651"
  mori-ink: "#eee6d3"
  mori-ink-2: "#b5b4cc"
  mori-ink-3: "#8d91b5"
  soga: "#d8995e"
  soga-hover: "#e4aa72"
  soga-ink: "#1d1307"
  soga-soft: "rgb(216 153 94 / .14)"
  jade: "#7fd3a7"
  kunyit: "#ebc55c"
  mengkudu: "#ef6f63"
  mana-blue: "#93a6ff"
  wax: "#eee6d3"
  wax-dim: "#343d78"
typography:
  title:
    fontFamily: "system-ui, -apple-system, \"Segoe UI\", Roboto, \"Noto Sans\", sans-serif"
    fontSize: "18px"
    fontWeight: 650
    lineHeight: 1.45
    letterSpacing: "-0.01em"
  headline:
    fontFamily: "system-ui, -apple-system, \"Segoe UI\", Roboto, \"Noto Sans\", sans-serif"
    fontSize: "16px"
    fontWeight: 650
    lineHeight: 1.45
  body:
    fontFamily: "system-ui, -apple-system, \"Segoe UI\", Roboto, \"Noto Sans\", sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.45
    fontFeature: "\"tnum\""
  label:
    fontFamily: "system-ui, -apple-system, \"Segoe UI\", Roboto, \"Noto Sans\", sans-serif"
    fontSize: "13px"
    fontWeight: 550
    lineHeight: 1.45
    fontFeature: "\"tnum\""
  caption:
    fontFamily: "system-ui, -apple-system, \"Segoe UI\", Roboto, \"Noto Sans\", sans-serif"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.45
    fontFeature: "\"tnum\""
  mono:
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.55
rounded:
  xs: "4px"
  sm: "6px"
  md: "8px"
  lg: "10px"
  pill: "99px"
spacing:
  xxs: "4px"
  xs: "6px"
  sm: "8px"
  md: "12px"
  row: "14px"
  lg: "16px"
  xl: "20px"
components:
  button:
    backgroundColor: "transparent"
    textColor: "{colors.mori-ink}"
    rounded: "{rounded.sm}"
    padding: "0 12px"
    height: "32px"
  button-hover:
    backgroundColor: "{colors.nila-raise}"
  button-primary:
    backgroundColor: "{colors.soga}"
    textColor: "{colors.soga-ink}"
    rounded: "{rounded.sm}"
    padding: "0 12px"
    height: "32px"
  button-primary-hover:
    backgroundColor: "{colors.soga-hover}"
  button-danger:
    backgroundColor: "transparent"
    textColor: "{colors.mengkudu}"
    rounded: "{rounded.sm}"
    padding: "0 12px"
    height: "32px"
  button-pressed:
    backgroundColor: "{colors.soga-soft}"
    textColor: "{colors.soga}"
  button-icon:
    rounded: "{rounded.sm}"
    size: "32px"
  input:
    backgroundColor: "{colors.nila-sunk}"
    textColor: "{colors.mori-ink}"
    rounded: "{rounded.sm}"
    padding: "0 10px"
    height: "34px"
  segmented:
    backgroundColor: "{colors.nila-sunk}"
    rounded: "9px"
    padding: "3px"
  segmented-option:
    backgroundColor: "transparent"
    textColor: "{colors.mori-ink-2}"
    rounded: "{rounded.sm}"
    padding: "0 12px"
    height: "28px"
  segmented-option-selected:
    backgroundColor: "{colors.nila-raise}"
    textColor: "{colors.mori-ink}"
  sheet:
    backgroundColor: "{colors.nila-field}"
    rounded: "{rounded.md}"
  roster-row:
    backgroundColor: "{colors.nila-field}"
    textColor: "{colors.mori-ink}"
    padding: "9px 14px"
    height: "56px"
  roster-row-current:
    backgroundColor: "{colors.nila-raise}"
  roster-row-picked:
    backgroundColor: "{colors.soga-soft}"
  top-bar:
    backgroundColor: "{colors.nila-field}"
    textColor: "{colors.mori-ink-2}"
    padding: "0 20px"
    height: "56px"
  side-panel:
    backgroundColor: "{colors.nila-field}"
    width: "440px"
  dialog:
    backgroundColor: "{colors.nila-field}"
    textColor: "{colors.mori-ink}"
    rounded: "{rounded.lg}"
    padding: "20px"
    width: "420px"
  selection-bar:
    backgroundColor: "{colors.nila-raise}"
    rounded: "{rounded.lg}"
    padding: "8px 8px 8px 14px"
  toast:
    backgroundColor: "{colors.nila-raise}"
    rounded: "{rounded.lg}"
    padding: "9px 14px"
  update-chip:
    backgroundColor: "{colors.soga-soft}"
    textColor: "{colors.soga}"
    rounded: "{rounded.pill}"
    padding: "3px 9px"
  log:
    backgroundColor: "{colors.nila-sunk}"
    textColor: "{colors.mori-ink}"
    typography: "{typography.mono}"
    rounded: "{rounded.sm}"
    padding: "10px 12px"
---

# Design System: Turu Monitor

> **Scope.** This world, **Nila**, covers the Turu Monitor web surface only: `bot/assets/monitor/index.html`, the page Turu serves at `/`. The desktop and Android app (egui, `frontend/src/gui/turu/theme.rs`) keeps its own Mugetsu palette (dark panels, `#7C6CF0` accent) and is **not** part of this world. Do not apply these tokens, names or rules to the egui UI.

## Overview

**Creative North Star: "Reading the Cloth"**

Checking on a sleeping bot is reading a length of indigo-dyed batik. The page is one deep nila field in three layers, written on in unbleached mori cream. Soga brown is the only accent, and it marks only what you act on or have selected. Three natural dyes carry state: jade for playing, kunyit yellow for degraded, mengkudu red for trouble. Each bot is a single row, and its last hour is drawn as a line of wax dots (the cecek trace).

The page is dense and calm. Rows show the whole bot at once, columns stay fixed so values change in place without reflowing, and trouble is ranked first in its own band. Depth comes from tonal layers and hairline indigo rules. Shadows are kept for things that float over the page. Type is the platform's own sans with tabular figures, so changing numbers stay steady.

The page is built for a short check-in on a phone or another PC, often at night. Everything has to read at a glance at 360px wide. A default dark card grid with a purple accent and a stat-tile header is the thing this world refuses.

**Key Characteristics:**
- One indigo field in three tonal layers (ground, field, raise) plus a sunk well for inputs and logs.
- Cream ink in three strengths; soga brown as the single accent, for actions and selection only.
- State is told by dye colour plus a word, never colour alone.
- Hairline 1px indigo rules divide everything; radii stay small (4 to 10px).
- Fixed-column whole-row roster; the cecek trace is the signature.
- Light theme remaps every role (follows the OS, or a stored choice), keeping the same structure.

## Colors

An indigo dye bath with cream ink, one warm brown accent and three state dyes. The frontmatter holds the dark (default) theme; the light theme (`prefers-color-scheme: light` or `data-theme="light"`) remaps every role to the values in `.impeccable/design.json` → `extensions.themes.light`.

### Primary
- **Soga Brown** (soga): the only accent. Primary buttons, selected checkboxes, focus rings, link text, the name of the open bot, pressed toggles, text selection. Hover deepens to **Soga Glow** (soga-hover); text on soga is **Soga Ink** (soga-ink). **Soga Wash** (soga-soft) tints picked rows, pressed toggles, the update chip and the input focus halo.

### Tertiary (state dyes)
- **Jade** (jade): playing, healthy, the live dot, HP bars, the kills chart.
- **Kunyit Yellow** (kunyit): degraded (paused, relogging, reconnecting), the stale-data notice, the drops chart.
- **Mengkudu Red** (mengkudu): trouble. The "Needs you" band, error states, danger buttons, target HP.
- **Mana Blue** (mana-blue): data only. MP bars and the XP chart. It is never used as an interface accent.

### Neutral
- **Nila Ground** (nila-ground): the page behind everything.
- **Nila Field** (nila-field): the top bar, roster sheets, side panel, dialogs.
- **Nila Raise** (nila-raise): hover, the current row, selected segment, floating bars and toasts.
- **Nila Well** (nila-sunk): inputs, segmented-control troughs, meters, the log.
- **Indigo Rule** (indigo-rule): 1px borders on controls, sheets and the top bar; **Soft Rule** (indigo-rule-soft) for dividers between rows, list items and track backgrounds.
- **Mori Ink** (mori-ink): primary text and figures. **Mori Ink 2** (mori-ink-2): secondary text, the night summary, inactive tabs. **Mori Ink 3** (mori-ink-3): captions, column heads, placeholders, unknown/offline marks.
- **Wax** (wax) and **Wax Dim** (wax-dim): the trace's lit and unlit dots.

### Named Rules
**The One Mordant Rule.** Soga is the only accent, and it marks action or selection. It never colours state, decoration or headings.

**The Dye Means State Rule.** Jade, kunyit and mengkudu always mean playing, degraded and trouble. Never use them for decoration, and always pair them with a word or icon.

**The Mixed, Not Invented Rule.** Tints come from `color-mix()` over existing roles (danger border = mengkudu 40% into rule; hover border = ink 22% into rule; band fill = mengkudu 7% into field). Do not add new hex values for a tint.

## Typography

**Body Font:** system-ui (with -apple-system, "Segoe UI", Roboto, "Noto Sans", sans-serif)
**Label/Mono Font:** ui-monospace (with SFMono-Regular, Menlo, Consolas, monospace), for the log only

**Character:** The platform's own sans at small, steady sizes. Weight does the ranking more than size. Tabular figures are set on `body`, so every number lines up.

### Hierarchy
- **Title** (650, 18px, -0.01em): the open bot's name in the side panel. The largest type on the page.
- **Headline** (650, 16px): dialog titles. The brand wordmark sits between these (650, 15px).
- **Body** (400, 14px, 1.45): the base. Bot names step up to 600; figures inside text use `<b>` at 600.
- **Label** (550 to 650, 13px): row state, the "Needs you" heading and machine group heads (650), totals, chart captions, toasts.
- **Caption** (400 to 600, 12px): column heads, sub-lines under a cell, timestamps, meter text, side-panel section heads (600). The trace axis drops to 11px.
- **Mono** (400, 12px, 1.55): the script and debug log, pre-wrapped.

### Named Rules
**The Weight Before Size Rule.** The scale runs only from 11 to 18px. New emphasis comes from weight (500 / 550 / 600 / 650) and ink strength, not from a larger size.

**The Steady Figures Rule.** Numbers that update live (gold/h, kills, HP, run time) stay tabular and right-aligned in fixed-width columns.

## Layout

A sticky 56px top bar, then one `main` column (padding 16px 20px, 96px bottom room for the selection bar). The "Needs you" band comes first, then a toolbar (filter, view tabs, totals), then the roster grouped by machine. Each group has a 13px head with a hairline running to the right edge.

The roster is a CSS grid with fixed tracks, so live updates never reflow a column. It responds to its container: full (9 columns, including map and gold/h) above 1180px; 7 columns up to 1180px (map moves into the name's sub-line); a stacked three-line card per row at 760px and below (no header, no vitals). Rows are at least 56px high, padded 9px 14px with 14px column gaps.

Above 1100px wide, opening a bot docks a 440px side panel (sticky, full height under the bar). Below that it is a fixed drawer (`min(480px, 100vw)`) over a scrim. At 640px and below, the night summary moves from the bar into the page, padding drops to 12px, and the band's reason text wraps under the name.

Spacing runs on a 4/6/8/12/14/16/20 rhythm: 4 to 6px inside controls and button groups, 14px for row and band insets, 16px for panel insets, 20px between page sections. On coarse pointers, controls grow to 40px targets.

## Elevation & Depth

The page is flat at rest. Depth comes from tonal layers (ground, then field, then raise; the sunk well goes below) and from 1px indigo rules. One shadow token exists, and it belongs only to things that float above the page: the selection bar, toasts, dialogs and the drawer-mode side panel. Modal layers dim the page with an indigo-black scrim.

### Shadow Vocabulary
- **Float** (`box-shadow: 0 10px 30px rgb(3 5 15 / .5)`; light theme `rgb(22 27 61 / .16)`): floating bars, toasts, dialogs, the drawer.
- **Segment lift** (`box-shadow: 0 1px 2px rgb(0 0 0 / .25)`): the selected option in a segmented control. This is the only shadow inside the page flow.
- **Focus halo** (`box-shadow: 0 0 0 3px` soga-soft): text inputs on focus, with a soga border.

### Named Rules
**The Flat Cloth Rule.** Surfaces in the flow of the page have no shadow. If it scrolls with the roster, it gets depth from tone and a rule, not a shadow.

## Shapes

Small, quiet radii: 4px for checkboxes and focus outlines, 6px for buttons, inputs, segments, meters and the log, 8px for sheets, bands and notices, 10px for dialogs and the selection bar. A segmented trough is 9px so it nests around its 6px options with 3px padding. Pills (99px) appear only on the update chip. Circles appear only on dots: state marks (8px), trace dots and the live indicator. Borders are always 1px. Offline is drawn as a hollow ring (inset 1.5px), not a grey fill.

## Components

### Buttons
Quiet outlined controls; only the one action that fixes a problem is filled.
- **Shape:** gently rounded (6px), 32px tall (40px on touch), 500 weight, 6px gap for an inline 16px stroke icon.
- **Default:** transparent with an indigo rule border and ink text. Hover fills with raise and lifts the border toward ink. Active nudges down 1px. Disabled drops to 40% opacity.
- **Primary:** soga fill, soga-ink text, soga-hover on hover. Use it for the one fixing action in a band row, empty-state calls and dialog confirmations.
- **Danger:** mengkudu text with a mengkudu-tinted border, and a 12% mengkudu wash on hover.
- **Ghost / Icon:** a borderless 32px square (30px inside roster rows) whose border appears on hover. Used for top-bar tools and close buttons.
- **Pressed toggle:** soga wash, soga text and a soga-tinted border (streamer mode).
- **Focus:** a 2px soga outline offset 2px, on every focusable element.

### Segmented control (tabs)
A sunk trough (3px padding, soft rule border) holding 28px options in ink-2. The selected option rises to raise with full ink and a segment lift. It is used for the view tabs, the panel tabs and the log switch.

### Inputs / Fields
- **Style:** sunk well, 1px rule border, 6px radius, 34px tall, ink-3 placeholder. The search field carries a 16px stroke icon at 10px from the left.
- **Focus:** the border turns soga and a 3px soga-soft halo appears, with no outline.
- **Checkbox:** a custom 16px square (4px radius) that turns solid soga when checked, with a soga-ink tick drawn in CSS.

### Roster sheet and row (signature container)
A field-coloured sheet with an 8px radius and a rule border. A dimmed header row uses 12px ink-3 text, and rows are divided by soft rules. Hover tints toward raise. The open bot's row turns raise and its name turns soga. Picked rows take the soga wash. Offline rows dim every cell except the checkbox and actions to 55%. The state cell is a dye dot plus a dye-coloured word. When a state changes, the dot pings once (0.9s).

### The cecek trace (signature)
The last hour drawn as 30 dots, one per two minutes, spread across a 14px-high strip (24px in the panel, with an 11px axis). Lit wax means the script ran, a half wax (45% wax into wax-dim) means it was paused, and wax-dim means idle. Kunyit and mengkudu dots mark relogging/loading and trouble slots. Larger 7px marks show events: a soga dot for a drop, a mengkudu or kunyit dot for deaths, errors, disconnects, relogins and logouts, and a hollow wax ring for a script start or login. While a script runs, the "now" dot breathes with a 3.2s halo. Under reduced motion the halo, the ping and the bar transitions are all turned off.

### Needs-you band
A mengkudu-tinted card (7% mengkudu into field, 38% mengkudu into rule for the border, 8px radius) placed above the toolbar. Its title is a 13px mengkudu label. Each row has a dot, the bot's name (600), the reason in ink-2, and a primary fixing action plus "Details". On phones, the reason and actions wrap beneath the name.

### Navigation (top bar)
A 56px sticky field bar with a bottom rule. It holds the logo symbol, the "Turu" wordmark with "Monitor" in ink-2, a night summary (counts in ink at 600, the trouble count in mengkudu, separated by ink-3 middots), the update chip, version meta, the live indicator (a 7px dot: jade when connected, kunyit when reconnecting, ink-3 otherwise) and ghost icon tools.

### Side panel
A field-coloured panel with a left rule. Its head shows the 18px name, a state line and a close button, followed by controls, a join row, tabs and a scrolling pane. Pane sections are 20px apart, with 12px ink-3 heads. It contains meters (22px sunk tracks, a 32% dye fill and a 2px solid leading edge), fact lists (ink-3 terms, right-aligned values), the event list (time, dot, text), the log well and line charts in dye colours.

### Floating layers
The selection bar (raise, 10px radius, Float shadow, centred at the bottom), toasts (raise, 9px radius, rising in 0.25s) and dialogs (field, 10px radius, 20px padding, 420px max, with an indigo-black backdrop at 55%).

## Do's and Don'ts

### Do:
- **Do** keep Soga to action and selection: one filled primary per row or dialog, and everything else outlined or ghost.
- **Do** pair every dye with a word: "Running" in jade with a jade dot, "Script error" in mengkudu with a mengkudu dot.
- **Do** derive tints with `color-mix()` from the existing roles, and add each new token to both the dark and the light theme.
- **Do** keep roster columns on fixed grid tracks so live values change in place.
- **Do** use inline 16px stroke SVG icons (1.75 stroke, round caps) from the page's own symbol sprite.
- **Do** honour `prefers-reduced-motion`, keep touch targets at 40px on coarse pointers, and keep the page working at 360px wide with no external fetches.
- **Do** blur private fields (names, gold, room numbers) under streamer mode with the `priv` hook.

### Don't:
- **Don't** apply Nila to the egui desktop/Android UI. It keeps its Mugetsu palette.
- **Don't** introduce a purple or violet interface accent. The `#7C6CF0` in the Turu logo symbol is a brand asset and stays inside the logo; Mana Blue is for data only.
- **Don't** build a stat-tile header or a grid of bot cards. Totals stay inline text, and bots stay whole rows.
- **Don't** put shadows on in-flow surfaces (see The Flat Cloth Rule).
- **Don't** use dye colours to decorate, or soga to signal state.
- **Don't** add type above 18px or introduce a webfont for the dashboard.
