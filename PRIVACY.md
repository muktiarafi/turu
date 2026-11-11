# Privacy policy

For Turu (Windows, macOS, Linux, Android) and Turu Mas (Android). Last changed 2026-11-11.

Turu and Turu Mas are free, open-source programs made by one developer, with no accounts, no
ads and no analytics. They collect nothing about you, except crash reports, and only if you
say yes.

## What stays on your device

Your settings, scripts, logs, saved game accounts and crash records are kept in Turu's data
folder on your device. Saved account passwords are kept in your system's keychain (Windows,
macOS, Linux) or encrypted with a key in the Android Keystore (Android). None of it is sent to
the developer.

## What goes over the network, and to whom

* **The game.** Turu plays AdventureQuest Worlds: it connects to Artix Entertainment's servers
  and sends them what the game itself sends, including your login. Artix's own privacy policy
  covers that.
* **Scripts and updates.** Search Scripts and Auto Update Scripts download scripts from GitHub,
  and Turu checks GitHub for new versions of itself. GitHub sees your IP address, as it does for
  any download.
* **Crash reports, only if you say yes.** At the first start Turu asks "Send crash reports?". If
  you say yes, a crash sends a report to the developer through [Sentry](https://sentry.io): what
  crashed (the program's stack trace and error message), the device model and system version,
  and Turu's version. Turu on Android also sends how the app last ended (for example, stopped by
  the system for low memory) and counts sessions under an anonymous install ID. Reports carry no
  IP address, no account name, password or game data, no screenshots and no logs. Change your
  answer any time in Options > Application > Send Crash Reports. Builds made without crash
  reporting never ask.
* **The control API and web monitor**, if you turn them on, serve your own network; nothing is
  sent to the developer.

Turu Mas runs the game in the Adobe AIR runtime (by Wipro, formerly HARMAN). The runtime in the
app doesn't send your information to Wipro.

## Keeping and removing data

Crash reports are kept in Sentry for at most 90 days and are used only to fix Turu. To have the
reports from your device removed, or for any question, open an issue at
<https://github.com/muktiarafi/turu/issues>. Uninstalling Turu, or deleting its data folder,
removes everything it kept on your device.

## Changes

Changes to this policy are made in this file; its history is on GitHub.
