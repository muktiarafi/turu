# Contributing to Turu

Thanks for helping. Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and
[docs/STATUS.md](docs/STATUS.md) first: what works, what doesn't, and the tooling quirks.
Scripts are contributed to [turu-scripts](https://github.com/muktiarafi/turu-scripts), not here.

## Setup

```bash
git clone --recursive https://github.com/muktiarafi/turu.git
```

Needs Rust 1.99+, Java 17+ (Ruffle compiles its Flash classes), Node 22 (to type-check scripts),
and on Linux the packages in [.github/linux-dependencies.txt](.github/linux-dependencies.txt).
Build and run as in the [README](README.md#build).

## Pull requests

1. Branch from an up-to-date `main`. Fork if you can't push here.
2. Keep each pull request to one change. Update the docs it affects (`docs/STATUS.md`,
   `docs/SCRIPTING.md`, `docs/HEADLESS.md`, ...) in the same pull request.
3. Run the checks below, push, open the pull request against `main`.
4. **Keep it rebased.** A pull request must sit on top of the current `main` with no merge
   commits. When `main` moves:

   ```bash
   git fetch origin
   git rebase origin/main
   git push --force-with-lease
   ```

   Don't use GitHub's "Update branch" merge button or `git merge main`; CI fails a branch with
   merge commits. Pull requests are merged with **Rebase and merge** only, so `main` stays linear
   and every commit on it should build and pass the tests. Squash fixups (`git rebase -i`) before
   asking for review, or as asked during review.

### What CI checks

Every pull request runs [.github/workflows/ci.yml](.github/workflows/ci.yml), and it must pass
before merging:

| Job | What | Run it locally |
|---|---|---|
| Rebased on main | branch contains `main`, no merge commits | `git rebase origin/main` |
| Submodules pushed | `ruffle` and `scripts` point at commits on their branches (`aqw`, `main`) | push the submodule first |
| Format | `bot/` and `android/` with `cargo fmt`, the UI windows and the app's own files with `max_width = 120` | see below |
| Clippy | `turu_bot`, `turu_frontend`, `turu`, tests included; any warning fails, Ruffle's crates' too | see below |
| Clippy (Android) | `turu_android` for arm64, with the crates it builds; any warning fails | see below (NDK, cargo-ndk, libclang: docs/ANDROID.md) |
| Host SWF | `turu_host`: format, clippy, tests, and `bot/assets/turu.swf` built from `host/as3` | `cargo run -p turu_host -- --check` |
| Test | engine tests, app and frontend tests, scripts type-check | see below |

```bash
# Format: bot/ with cargo fmt; the UI windows with rustfmt (mod.rs formats them all; never cargo fmt -p turu)
cargo fmt -p turu_bot -p turu_android
rustfmt --edition 2024 --config max_width=120 egui_ui/src/gui/turu/mod.rs
# The app's own files, by name (Ruffle's desktop files in app/ keep their layout; ci.yml lists them)
rustfmt --edition 2024 --config max_width=120 app/src/{beep,client,crash_uploads,desktop_host,headless,linux,memory,shared_gpu,shared_surface,shell,shell_sockets,thread_perf}.rs

# Clippy
cargo clippy -p turu_bot -p turu_frontend -p turu --all-targets --no-default-features --features turu/software_video,turu/external_video,turu/lzma,turu/jpegxr,turu/fontconfig-dlopen -- -D warnings -A clippy::redundant_clone

# Clippy for Android (ANDROID_NDK_HOME, and BINDGEN_EXTRA_CLANG_ARGS=--sysroot=<the NDK's sysroot>
# as android/app/build.gradle.kts sets them)
cargo ndk -t arm64-v8a clippy -p turu_android --all-targets -- -D warnings -A clippy::redundant_clone

# Tests (CI runs the three crates' in one cargo-nextest command, much faster for the engine:
# `cargo nextest run --profile ci -p turu_bot`, the same flags as below; .config/nextest.toml)
cargo test -p turu_bot
cargo test -p turu -p turu_frontend --no-default-features --features turu/software_video,turu/external_video,turu/lzma,turu/jpegxr,turu/fontconfig-dlopen
cd scripts && npx -y -p typescript@5.9.3 tsc --noEmit -p .
```

CI runs on Linux. If you change platform code (macOS, Windows, Android), say how you tested it
in the pull request.

## Commits

- Messages follow `type: Summary` with a capitalised summary in the imperative, as in the
  history: `feat:`, `fix:`, `perf:`, `refactor:`, `docs:`, `test:`, `ci:`, `chore:`.
  Say *why* in the body when the summary doesn't.
- Each commit should build and pass the tests on its own (they land on `main` as they are).
- Don't commit build output, logs, crash dumps or keystores.

## Code rules

The full list is in [CLAUDE.md](CLAUDE.md) ("Rules that keep it working"). The ones reviews
check most:

- **Behaviour follows VibeSkua's C# by default** (timeouts in 100 ms tries, `ActionDelay`, packet
  formats). When unsure, read the C# and link it in the pull request. Where Turu differs on
  purpose, say why: mark the code `// Turu:` with the reason and add what a user or script
  would notice to `docs/STATUS.md`, "Differences from VibeSkua". Don't remove a marked difference to match the C#.
- **No winit, rfd or window code in `frontend/`**: it's shared with Android.
- **Never call into Flash from the UI thread**, and never block or call Flash inside
  `Bot::on_flash_call`.
- **The script API is `bot/js/turu.ts`.** Change it and its Rust side (`bot/src/api/`) together,
  copy it to `scripts/types/turu.ts`, and keep its JSDoc. Scripts stay synchronous.
- **Ruffle's files in `app/` keep upstream's style**; mark Turu changes in them with `// Turu:`.
  Changes to Ruffle itself go to [ruffle-aqw](https://github.com/muktiarafi/ruffle-aqw) first.
- **Submodules** (`ruffle/`, `scripts/`): commit and push in the submodule first, then bump it
  here in the same pull request.

## License

Turu is GPL-3.0-or-later, with the additional permission in [LICENSE-EXCEPTION](LICENSE-EXCEPTION)
(combining it with the Adobe AIR runtime, for Turu Mas). By contributing you agree your
contribution is licensed under both.
