---
paths:
  - "src/**/*.rs"
  - "scripts/**"
  - "examples/**"
---

# Testing

- Put tests in a `#[cfg(test)] mod tests` at the bottom of the file they test; this binary crate has no `tests/`
  folder.
- Name a test as a sentence about behavior: `a_paused_track_keeps_its_line_but_arms_no_timer`.
- Keep logic in functions that take and return plain values so a test needs no window. The drawing and the Windows
  integration are checked by running the notch.
- Give a test that touches settings `Settings::at(Some(path))` with a temporary path, so the real
  `%APPDATA%\notch\settings.txt` stays untouched.
- Create `Fonts::new(1.0)` in a test that measures text; DirectWrite works without a window.
- Add or update unit tests for every piece of logic you add or change.
- The memory and CPU check is `examples/perf.rs`, started by `scripts/perf.sh`.
