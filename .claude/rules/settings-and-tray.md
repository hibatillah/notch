---
paths:
  - "src/host/settings.rs"
  - "src/host/tray.rs"
  - "src/host/window.rs"
  - "settings/src/**"
---

# Settings and tray

- Every tray toggle and every settings-window control is backed by one settings key (`notch.*`, `activity.<id>`, `advanced.*`, page
  keys start with the page id), written through `host/settings.rs` or `sections::apply`, read into a field of `Notch` at
  start and re-read on `WM_SETTINGS_CHANGED` (`Notch::reload_settings`, then `on_settings_changed` on every page and
  started activity). A re-read falls back to the field's current value, not the default.
- A new key, its default and its label go into `docs/configuration.md` and the `README.md` tray list and defaults
  table in the same change. The steps are in `docs/adding-a-setting.md`.
- Write boolean keys as `on` or `off`, and at start treat a missing or unreadable value as the documented default.
