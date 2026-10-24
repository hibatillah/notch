# Performance

Budget: 0 % CPU while idle, under 50 MB of memory (about 19 MB with the clock page).

- Stop every timer and animation as soon as its output stops changing; the frame timer ends when the springs settle
  and no page reports `animate` as moving.
- Wake the UI thread from background work with a `Notifier`, never by polling.
- Draw shapes into small cached images: describe them with `Path`, rasterize with `Mask`, reuse through
  `Canvas::cached_image`. Direct2D geometry (`ID2D1PathGeometry`, `FillGeometry`, `DrawGeometry`) costs about 10 MB the
  first time it is used.
- When `scripts/perf.sh` fails, find the cause. The limits live in `examples/perf.rs`.
- Keep the release profile (`opt-level = "s"`, LTO, `panic = "abort"`).
