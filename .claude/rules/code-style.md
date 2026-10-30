---
paths:
  - "src/**/*.rs"
  - "scripts/**"
  - "build.rs"
---

# Code style

- Name things in full: `present_track_ids`, not `ptids`.
- Comment only what the code cannot show: a reason, a unit, a Windows workaround or the failure it prevents. One line,
  two when the second carries a new fact, in sentence case.
- Leave undocumented what the name and signature already say, and leave out step narration, history, issue numbers and
  banners. Begin a file with a one-line `//!` only when its purpose is not clear from its name.
- Write layout sizes in device-independent pixels and multiply by `frame.scale` (or `canvas.scale()`) when drawing.
- Wrap `unsafe` around the single Windows call that needs it.
- `cargo fmt` is the formatter; `rustfmt.toml` holds its settings.
- Log a failed cleanup or recovery with `log::debug(&format!("could not <action>: {error}"))` and keep running; reserve
  `expect` and `unwrap` for tests and values that cannot fail.
