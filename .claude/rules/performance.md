# Performance

Budget: 0 % CPU while idle, under 50 MB of memory. The limits `scripts/perf.sh` enforces are in `scripts/perf.rs`.

- Stop every timer and animation as soon as its output stops changing; the frame timer ends when the springs settle
  and no page reports `animate` as moving.
- Wake the UI thread from background work with a `Notifier`. Wait on what calls you: a Windows callback, a message,
  a blocking wait or an event hook. An event hook asks for the one event and the one thread it needs
  (`host/screen_mode.rs`), because a hook on every window's events wakes the notch on every mouse move.
- Draw shapes into small cached images: describe them with `Path`, rasterize with `Mask`, reuse through
  `Canvas::cached_image`. Direct2D geometry (`ID2D1PathGeometry`, `FillGeometry`, `DrawGeometry`) costs about 10 MB the
  first time it is used.
- When `scripts/perf.sh` fails, find the cause and leave the limits where they are.
- Keep the release profile (`opt-level = "s"`, LTO, `panic = "abort"`).
