---
paths:
  - "scripts/**"
---

# Scripts

- Write entry scripts as `.sh` for Git Bash and keep them thin: parse options, build, start.
- Put logic that needs Windows APIs (moving the mouse, reading process counters) in a Rust example: a `.rs` file in
  `scripts/` with an `[[example]]` entry in `Cargo.toml`, run with `cargo run --release --example <name>`. The project
  has no PowerShell files.
- Begin every script with `cd "$(dirname "$0")/.."` so it works from any directory.
- Redirect the output of a detached `cmd start` (`>/dev/null 2>&1 </dev/null`) so the caller's pipe can close.
- Asset generation (images, icons) may be a Python file in `scripts/` run by a `.sh` wrapper through
  `uv run --no-project --with <package>` (`scripts/export-icon.sh`). Commit the generated output; the build and the
  checks never run Python.
- List every file a build reads beside `src` (`build.rs`, `assets/branding/*`) in `scripts/watch.sh`'s `watched` array.
