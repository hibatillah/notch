---
paths:
  - "src/host/settings.rs"
  - "src/host/tray.rs"
  - "src/host/window.rs"
---

# Settings and tray

- Every tray toggle is backed by one settings key (`notch.*`, `activity.<id>`, page keys start with the page id), written
  through `host/settings.rs` and read once into a field of `Notch`.
- A new key, its default and its tray label go into `docs/configuration.md` and the `README.md` tray list and defaults
  table in the same change.
- Write boolean keys as `on` or `off`, and treat a missing or unreadable value as the documented default.
