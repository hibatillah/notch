---
paths:
  - "src/**/*.rs"
  - "scripts/**"
---

# Testing

- Put tests in a `#[cfg(test)] mod tests` at the bottom of the file they test; this binary crate has no `tests/`
  folder.
- Name a test as a sentence about behavior: `a_paused_track_keeps_its_line_but_arms_no_timer`.
- Keep logic in functions that take and return plain values so a test needs no window.
- Check drawing by rendering it: `cargo test snapshots -- --ignored` writes pictures to `target/snapshots` (see
  `docs/testing.md`); look at them. The Windows integration is checked by running the notch.
- Give a test that touches settings `Settings::at(Some(path))` with a temporary path, so the real
  `%APPDATA%\notch\settings.txt` stays untouched.
- Create `Fonts::new(1.0)` in a test that measures text; DirectWrite works without a window.
- Add or update unit tests for every piece of logic you add or change.
- The memory and CPU check is `scripts/perf.rs`, started by `scripts/perf.sh`.
- Add a gate or flag to `scripts/perf.rs` together with its line in the `scripts/perf.sh` header and its row in
  `docs/testing.md`.
- Name a regression test after the behavior that broke, as a sentence a user would recognize (`session_recovery_clears_stale_hover_and_allows_a_hidden_notch_to_open_again`).
