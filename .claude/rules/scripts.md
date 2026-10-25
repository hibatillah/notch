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
