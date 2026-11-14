# Native Android pieces in Turu's UI

Turu's windows on Android are egui, the same code as the desktop's (`frontend/src/gui/turu/`),
drawn over the game. Some of what they do works better as Android's own components: typing,
first-start questions, file dialogs. This says which pieces were moved to native Android, in both
Android apps (Turu, on Ruffle, and Turu Mas, on AIR), how each works, what stays in egui and why,
and what was learned doing it. Read `docs/ANDROID_HANDOVER.md` and `docs/TURU_MAS.md` first.

| Piece | Commit | Turu | Turu Mas |
|---|---|---|---|
| The game's text fields as a native field above the keyboard | `1c32e1adb` | `DialogField.kt`, `game_field.rs` | `ViewField.kt`, host SWF's `gameTyping` |
| Turu's own text boxes as a native field; a tap outside closes it | `8174af2e9` | `ui_field.rs` | `overlay.rs`, `Bridge` |
| "Send crash reports?" as Android's dialog | `b37f9f790` | `CrashQuestion.kt` (shared), `platform.rs` | `CrashQuestion.kt` (shared), `platform.rs` |
| File dialogs through the system picker | `c013aa4c0` (Turu had them before) | `saf.rs` | `saf.rs`, `Documents.kt`, `PickerActivity.kt` |

What both apps share lives in `turu_frontend` (`gui/turu/native_field.rs`, `documents.rs`, the
`Platform` hooks) and in the Gradle module `android/shared` (package `com.muktiarafi.turu.shared`):
`NativeField`, `CrashQuestion`, `CrashReports`, `CrashUploads`, `Tombstone`. Turu's app depends on it
(`android/settings.gradle.kts`); Turu Mas's extension includes it by path
(`air/ane/android/settings.gradle.kts`), and its classes reach the ANE's jar with the other
libraries (`aneLibs`, `air/ane/build.sh`). It builds Java 8 class files, which AIR's own `d8`
needs. Each app keeps its JNI glue, its activity and service, and what differs: where the text
field goes (Turu's `DialogField`, Turu Mas's `ViewField`), and a `CrashApp` saying who it is (name,
Sentry DSN and release name, client tag, the low-memory hint).

## Already native before this

The foreground service and its notification, the notification and battery-optimisation prompts
(`TuruActivity.askOnce`, `Prompts.kt`), account passwords in the Android Keystore (`Storage.kt`,
`Passwords.kt`), crash records and Sentry (`CrashReports.kt`, `Tombstone.kt`,
`CrashUploads.kt`), the share sheet and links (`Intent`s), and Turu's file pickers.

## Text fields

egui alone gets only keys from the soft keyboard (`KeyInput.kt` in Turu, `TuruOverlay`'s
connection in Turu Mas: `TYPE_NULL`): no suggestions, autocorrect, composing, selection handles,
text toolbar or autofill. And the keyboard covers the field being typed in. So while a field has
the focus, a real Android `EditText` sits just above the keyboard holding what the field holds
(as AQW's own Android launcher does), and edits go back to the field.

Kotlin's `NativeField` (the shared module's) is that strip, in both apps: one for the game's
focused field, one for Turu's text boxes, each with a `FieldTarget` (where edits, Send and Back go) and a `FieldStyle`
(password, `max` characters, hint, several lines, number). Positions are UTF-16 units on the
Kotlin side, characters in egui.

* **Turu's text boxes** (`native_field.rs`). Each box calls `native_field::follow(ui, &mut
  response, text, hint, kind)` right after it's added. While it has the focus, it records what it
  holds for the frame and takes the native field's edit, if one came, a frame later
  (`response.mark_changed()`, so the caller's `changed()` sees it). After each frame the app reads
  the focused box (`take_focused`) and keeps the strip in step (`NativeField::sync`: `Show`,
  `Update`, `Hide`). Send is Enter in egui, sent only once the last edit is in the box
  (`input_events`), or Enter would act on the old text. Back takes the focus off the box.
  **A new text box must call `follow` too**, or it falls back to the keys-only keyboard and
  `pan.rs` (which moves its window above the keyboard). `widgets::text_box` and
  `widgets::touch_number` already do.
* **The game's fields** (`game_field.rs`, `TuruAirHost.as`): the field's text and selection come
  from the player (Turu) or the host SWF (Turu Mas); each edit is typed into the game as the change
  it is, so the game sees ordinary typing.
* While one of Turu's boxes has the strip, the game's steps aside (and comes back after, if the
  game's field still has the focus).
* **A tap outside the strip and the keyboard closes both**, as Back does, and still reaches what it
  landed on. Turu: the dialog watches outside touches (`FLAG_WATCH_OUTSIDE_TOUCH` with
  `FLAG_NOT_TOUCH_MODAL`, `setCanceledOnTouchOutside`); a tap on the keyboard isn't one, its window
  is above. Turu Mas: `TuruOverlay` sees every touch first and calls
  `Bridge.touchedOutsideFields`.

How the strip is placed differs:

* **Turu** draws through NativeActivity, whose views don't show, so the strip is a `Dialog` of its
  own along the bottom. Its bottom padding is how much of its window the keyboard covers
  (`fitAboveKeyboard`, checked before each draw): nothing on Android 15+, where the system keeps
  the dialog above the keyboard, the keyboard's height elsewhere.
* **Turu Mas** puts the strip in AIR's view hierarchy, right of the rail: a window of its own
  would take the window focus from AIR, which drops the game field's focus. Turu's overlay is a
  `SurfaceView` with `setZOrderOnTop`, drawn over every view, so it leaves the strip's area empty:
  Kotlin reports the strip's corner (`Bridge.fieldsMoved`, `nativeFieldPlace`) and the overlay
  clips egui below and right of it (`overlay::set_field_place`), the rail left of it still drawn.

## "Send crash reports?"

A one-time question at first start, so it belongs with the notification and battery prompts as an
`AlertDialog` (`CrashQuestion.kt`, shared). The Rust side still decides when to ask and
what it says, and keeps the answer: `show_crash_upload_question` (`logs.rs`) builds a
`platform::Question` and calls `Platform::ask_crash_uploads`; Android shows it and answers through
`answer_crash_upload_question` (any thread); where there's no native dialog (the desktop) egui asks
as before. Privacy policy opens the link and leaves the dialog up; closing it without an answer
asks again at the next start. On Turu Mas the dialog has AIR's focus while it's up: acceptable once,
at first start.

## File dialogs

The bot windows' `Platform::pick_file` and `save_file` answer with a path, as on a desktop, so
their callers work unchanged. `turu_frontend::documents` holds what both apps share: one dialog at
a time (`begin`), what the picker is asked for (`open_request`: MIME types and, for a script, the
scripts folder's `Imported/`; `save_request`: MIME type and suggested name), and the answers
(`picked`; `created`, which copies the window's file into the document once it stops changing).

* **Turu**: `TuruActivity.openDocument` / `createDocument` start the picker; `Storage.kt` copies
  and writes documents.
* **Turu Mas**: AIR's activity is AIR's, and the AIR SDK's license allows only documented ways into
  the runtime, so the picker is started from `PickerActivity`, a see-through activity of the
  extension's own (declared in `air/app/app.xml`), which hands the result to `Documents.kt`. The
  picker's grant on the document ends with that activity, before the copy or the save is done, so
  `Documents.hold` takes a persistable grant and gives it up after.

## Platform calls

The bot windows' links, share sheet and clipboard are one Kotlin method each, called from Rust over
JNI: Turu's `TuruActivity.openUrl`, `shareText`, `copyText` and `clipText` (`android/src/platform.rs`,
which spelled the `Intent`s and the `ClipboardManager` out as JNI call chains before), as Turu Mas's
`Bridge` has them. Rust names each by its JNI signature, which nothing checks at build time: a
renamed method or a changed parameter fails only when it's called (`dexdump` on the APK lists the
methods with their signatures). Logs > Copy and Share exercise `copyText` and `shareText`;
`openUrl` and `clipText` (the game's Ctrl+V) are harder to reach on a phone: a link only shows in
some windows (an update's release page, Options' web control), and the native field holds the
keyboard while the game's field has the focus.

## Kept in egui

* **The nav rail and the bot windows** (about 40,000 lines): the desktop shares them, `CLAUDE.md`
  wants VibeSkua's UI, and egui draws with the game in one pass. A Compose port would be a second
  UI to keep in step.
* **Modals and confirmations** (26: Remove account, Clear stats, Delete loadout, ...) and the
  **Tab Limit notice**: they lead back into Turu's windows ("Raise Tab Limit" opens Options), and on
  Turu Mas a native dialog takes AIR's focus each time.
* **Drop-downs and number boxes**: egui's, sized for a finger (`widgets::dropdown`,
  `widgets::touch_number`); a number box's typing goes through the native field.

## Still open

From the survey these came from, not done:

* Installing an update's APK in the app (`DownloadManager`, `PackageInstaller`) instead of the
  browser.
* Drop and script-stopped alerts as Android notifications, heard with the screen off.
* Turu from `NativeActivity` to `GameActivity` (a real view hierarchy: the strip as a view, the
  platform's text input, activity results), if more native views come.

## Pitfalls

* **The strip floated a strip's height above the keyboard** (Turu): its padding was the keyboard's
  height read from the dialog's own insets while the keyboard slid up, stale once Android 15+
  moved the window above it. Placed from where the window is now (`fitAboveKeyboard`).
* **An empty title row** above the strip: the activity's theme gives dialogs one
  (`FEATURE_NO_TITLE`).
* **Turu's windows drew over Turu Mas's strip**: the overlay is a surface on top of every view.
* **The first save came out empty** (Turu Mas): the picker's grant ended with `PickerActivity`.
* **rustfmt on `frontend/src/gui/turu/mod.rs`** with `max_width = 120` rewrites unrelated lines;
  format the files you changed one at a time, as `CLAUDE.md` says.
* **Testing on the emulator**: `uiautomator dump` gives the strip's bounds, `dumpsys
  input_method | grep mInputShown` whether the keyboard is up, `dumpsys window` the dialog's frame.
  Turu Mas's local builds have no Sentry DSN (`TURU_MAS_SENTRY_DSN`), so they never ask about crash
  reports. Start the apps with `am start -n com.muktiarafi.turu/.TuruActivity` and
  `am start -n com.muktiarafi.turumas/.AIRAppEntry`: `adb shell monkey -p <package> -c
  android.intent.category.LAUNCHER 1` finds neither and exits with code -5.
* **A change to `android/shared` needs both apps rebuilt and tried**: Turu's with
  `./gradlew assembleDebug`, Turu Mas's with `air/app/build.sh`, which wants `AIR_JAVA_HOME` set to a
  JDK 17 (a newer one breaks AIR's Gradle). A class the ANE's jar lacks shows only at run time, as
  `NoClassDefFoundError` in logcat when Turu Mas starts.

## Known gap

On the Android 16 arm64 emulator (2026-11-14), Turu Mas's game stays black after the app comes
back to the screen, after Home and back or a file dialog's picker alike: AIR says `activate`, but
its view keeps its black placeholder and never draws; Turu's windows draw. A new activity (Back,
then opening the app) draws again. Not tried on a phone yet, nor known whether an earlier build did
it (`docs/TURU_MAS.md`, Known gaps).
