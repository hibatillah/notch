# Adding a setting

A setting is one key in `%APPDATA%\notch\settings.txt` that the notch reads and the settings window can write. The
keys, defaults and the window's sections are listed in [configuration.md](configuration.md); this page covers the code.

1. **Pick the key and the default.** Start the key with `notch.`, `activity.<id>` or the page id. Write booleans as `on`
   or `off`. The same default goes into the notch and the window, and a test does not compare them (the Behavior
   section's words are the exception: its test reads `src/host/visibility.rs`), so check both by hand.
2. **Read it in the notch.** Read the key once at start into a field of `Notch`, a page or an activity, through
   `Context::settings`. Read it again on `WM_SETTINGS_CHANGED`: `Notch::reload_settings` and `on_settings_changed` run
   then, and the re-read falls back to the field's current value, not the default. A missing or unreadable value uses
   the documented default.
3. **Add the control to the window** (`settings/src/`):
   - A toggle: add `Item::new(label, field::toggle(key, default)).description(text)` to a `Group` in the section's
     `page()`.
   - A text or number field: use `field::input(key, Kind::Text { width }, placeholder)` or
     `field::input(key, Kind::WholeNumber { default, range }, placeholder)` as the item's control. It saves on Enter or when it loses
     focus, and shows the saved value again when the typed one is rejected.
   - A choice from a list: `field::dropdown(key, choices, shown)`.
   - Every item gets a description of one to three short lines that says what the setting does.
   - A new section: create `sections/<name>.rs` with a `page() -> Page` function (title, one-line description, groups)
     and add it to `sections::all` in `sections/mod.rs`.
   - Every write goes through `sections::apply`, which saves the key and posts `WM_SETTINGS_CHANGED` to the notch.
4. **Document it** in the same change: the key in the `docs/configuration.md` table, the control in its window table, and
   the user-facing default in the `README.md` table.
5. **Check it:** `cargo +stable-x86_64-pc-windows-msvc test --workspace`, then open the window from the tray and change
   the control while the notch runs. The notch must pick the change up without a restart.

A tray toggle follows [the same rules](../.claude/rules/settings-and-tray.md): one key, read at start, re-read on
`WM_SETTINGS_CHANGED`.
