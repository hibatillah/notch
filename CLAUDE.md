# Notch

A Windows "dynamic island" written in Rust: a black pill at the top of the screen, drawn with Direct2D, no UI
framework. It must use 0 % CPU when idle and stay under 50 MB of memory. Windows only. Read `docs/architecture.md`
before changing structure.

## Commands

```
cargo build --release                     # target\x86_64-pc-windows-msvc\release\notch.exe
cargo test                                # unit tests, no window needed
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo license                             # dependency licenses
scripts/reload.sh                         # build, start the new build, close the old one (Git Bash)
scripts/perf.sh [--no-build]              # memory and CPU checks (examples/perf.rs); moves the mouse for about two minutes
```

## Rules

The rules are in `.claude/rules/`; the ones without a `paths` filter always apply:

| File | Applies to | Covers |
| --- | --- | --- |
| `workflow.md` | always | do not kill the notch, checks before done, no commits unless asked |
| `performance.md` | always | 0 % idle CPU, memory budget, no polling |
| `contributions.md` | always | no outside contributions |
| `code-style.md` | `src/**/*.rs` | naming, comments, units, `unsafe` |
| `pages-and-host.md` | `src/pages`, `host`, `page.rs`, `gfx`, `ui` | what may depend on what, timer ids, settings keys |
| `windows-and-hover.md` | `src/host`, `src/gfx`, `Cargo.toml` | pinned crates, hover tracking, window shape |
| `testing.md` | `src/**/*.rs`, `scripts/**` | where tests live, how to write them |
| `scripts.md` | `scripts/**` | thin `.sh` runners, logic in separate files |
| `dependencies-and-license.md` | `Cargo.toml`, `Cargo.lock`, `LICENSE` | MIT, check licenses, ask before adding |
| `ci-and-release.md` | `.github/**`, `Cargo.toml` | CI on default-branch pushes, release on tags only |

## Docs

`docs/development.md` (build, run, checks, style, licenses), `docs/testing.md`, `docs/architecture.md`,
`docs/adding-a-page.md`, `docs/configuration.md`, `docs/releasing.md`.

## Debugging

`NOTCH_DEBUG=<file>` writes a timestamped log (Unix milliseconds). `NOTCH_PAGES=clock,media` picks pages,
`NOTCH_LYRICS=off` stops lyric lookups, `NOTCH_TOP=<px>` moves the pill down. Settings are in
`%APPDATA%\notch\settings.txt`.
