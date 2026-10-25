---
paths:
  - "src/**/*.rs"
  - "scripts/**/*.rs"
---

# Code style

- Name things in full: `present_track_ids`, not `ptids`.
- Write comments as one line that says why; use a doc comment when the reason needs more.
- Write layout sizes in device-independent pixels and multiply by `frame.scale` (or `canvas.scale()`) when drawing.
- Wrap `unsafe` around the single Windows call that needs it.
- `cargo fmt` is the formatter; `rustfmt.toml` holds its settings.
