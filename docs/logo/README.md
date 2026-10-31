# Turu logo

A sleeping bot: a rounded tile with two closed eyes. The eyes are the two u's of "turu"
(Javanese for "sleep"), because the bot plays while you sleep.

| File | Use |
|---|---|
| `turu-logo-light.svg` | Symbol + wordmark, for light backgrounds |
| `turu-logo-dark.svg` | Symbol + wordmark, for dark backgrounds |
| `turu-symbol.svg` | Symbol, 48 px and up |
| `turu-symbol-small.svg` | Symbol without lashes, for 32 px and below |
| `turu-symbol-mono.svg` | One colour; the eyes are holes |
| `turu-symbol-macos.svg`, `turu-symbol-macos-small.svg` | Symbol on Apple's 824/1024 icon grid |

Colours: violet `#7C6CF0` (the UI accent), white eyes, wordmark `#11131A` on light and
`#E8EAF2` on dark. The wordmark is drawn from strokes, not a font.

`python3 make_logo.py` regenerates the SVGs. The app's raster copies are made from them:
`app/assets/turu-icon-64.rgba` (window icon, from `turu-symbol.svg`),
`app/assets/favicon.ico` (Windows; small cut up to 32 px),
`app/assets/Assets.xcassets/RuffleMacIcon.iconset` (macOS) and
`app/assets/about_logo.png` (About dialog, from `turu-logo-dark.svg`).
