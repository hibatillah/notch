# Notch

A Windows "dynamic island" written in Rust: a black pill at the top of the screen, drawn with Direct2D, no UI
framework. It idles at 0 % CPU and stays under 50 MB of memory. Windows only.

## Commands

```
cargo build --release                     # target\x86_64-pc-windows-msvc\release\notch.exe
cargo test                                # unit tests, no window needed
cargo test snapshots -- --ignored         # writes pictures of the pages to target/snapshots (see docs/testing.md)
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo license                             # dependency licenses
scripts/reload.sh [--debug]               # build, start the new build, close the old one (Git Bash)
scripts/watch.sh                          # debug build and relaunch on every saved source file (Git Bash)
scripts/perf.sh [--no-build]              # memory and CPU checks (scripts/perf.rs); moves the mouse for about two minutes
```

## Rules

`.claude/rules/` holds the rules, loaded by the paths they cover. Three have no path filter and always apply:
`workflow.md` (checks before done, no commits unless asked, never kill the notch), `performance.md` (no polling, memory
budget) and `contributions.md` (no outside contributions).

## Docs

Read the doc for the work in front of you before starting it:

| Work | Read |
| --- | --- |
| restructuring, or finding where something lives | `docs/architecture.md` |
| a new page | `docs/adding-a-page.md` |
| a new activity (volume, Caps Lock, notification banner) | `docs/adding-an-activity.md` |
| layout, padding or the open and collapsed shapes | `docs/development.md` (the notch layout) |
| a test, a snapshot, the manual checklist, `perf.sh` | `docs/testing.md` |
| settings keys, environment variables, tray options | `docs/configuration.md` |
| the installer, a release, updating | `docs/releasing.md` |
| a CPU or memory number, a perf limit | `docs/performance.md` |

## Debugging

`NOTCH_DEBUG=<file>` writes a timestamped log (Unix milliseconds). `NOTCH_PAGES=shelf,media` picks pages,
`NOTCH_LYRICS=off` stops lyric lookups, `NOTCH_TOP=<px>` moves the pill down. Settings are in
`%APPDATA%\notch\settings.txt`; `activity.notifications=off` disables notifications (the tray menu does it too). Other activities stay enabled.
