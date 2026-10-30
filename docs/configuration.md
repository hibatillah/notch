# Configuration

There are three ways to configure the notch, from what a user can change to what a developer changes in code.

## Which pages are on

Without any setting, every registered page is on. To use only some of them, list their ids, separated by commas:

- **Environment variable:** `NOTCH_PAGES=shelf,media`
- **Settings file:** a line `pages=shelf,media` in `%APPDATA%\notch\settings.txt`

The environment variable wins when both are set. Page ids are the values returned by each page's `id()`; the built-in
ones are `media`, `shelf`, `calendar`, `timer` and `stats`.

- Pages keep their registration order, whatever order you list them in.
- Ids that match no page are ignored. If none match, every page stays on.

Example, to run without the media page:

```
set NOTCH_PAGES=shelf
notch.exe
```

To change which pages are *compiled in*, edit `src/pages/mod.rs`; see [adding-a-page.md](adding-a-page.md).

## Which activities are on

Volume, brightness, charger and battery, Bluetooth, microphone, camera, copied and Caps Lock are always enabled.
Old `activity.<id>=off` entries for these activities are ignored. Only **Notifications** has a tray toggle, on by
default. Its choice is remembered as `activity.notifications`; when off at startup, its listener is not started.

## Settings file

`%APPDATA%\notch\settings.txt` is a plain text file with one `key=value` per line. The notch writes it when you
change something, one key at a time through a temporary file, so keys another program wrote are kept. Another program
(the settings window) that changes the file posts `WM_SETTINGS_CHANGED` (`WM_APP + 30`) to the window of class
`NotchWindow`; the notch then re-reads the file and applies the `notch.*`, `navigation.order`, `navigation.disabled`,
`activity.notifications`, `media.lyrics`, `timer.*_minutes`, `calendar.*` and `notifications.show_text` keys without a
restart, keeping the page that is open. A missing or unreadable value keeps the current one. A hand edit is read at the
next start, or at once if something posts that message.

| Key | Values | Meaning |
| --- | --- | --- |
| `pages` | comma-separated page ids | which pages are on (see above) |
| `media.lyrics` | `on`, `off` | show lyrics on the media page (the speech-bubble button changes this) |
| `timer.focus_minutes` | 5 to 120 | length of a focus session; the timer page changes it in steps of 5 |
| `timer.short_break_minutes`, `timer.long_break_minutes` | minutes | break lengths (default 5 and 15); set by hand |
| `timer.state`, `timer.completed` | written by the notch | the running timer and the focus sessions finished in this round, so a restart keeps them |
| `navigation.order`, `navigation.disabled` | page ids, comma-separated | the order of the pages and the ones switched off; the "Edit pages" view writes them, and by hand they work while that view is off |
| `notifications.show_text` | `on`, `off` | show what a notification says in its banner (off is private mode); set by hand. Switch the banner itself off with `activity.notifications` |
| `calendar.ics_url` | a web address | the calendar address; the tray menu sets it from the clipboard. It is a secret: anyone with it can read the calendar |
| `calendar.refresh_minutes` | minutes | how often the calendar is downloaded again (default 15); set by hand |
| `notch.hide_when_idle` | `on`, `off` | off by default: hide the notch when there is nothing to show until you point at the top of the screen |
| `notch.maximized`, `notch.fullscreen` | `hidden`, `activities`, `lyrics`, `music` | what the notch shows over a maximized window (default `lyrics`) and over a full-screen app (default `hidden`); a missing or unknown word uses the default. `hidden` suppresses the notch, activities included. `activities` shows only banners, volume changes and badges. `lyrics` shows an activity, or playing music that has lyrics. `music` also keeps the idle pill up with nothing playing. What a word leaves out hides to the strip. The settings window's Behavior section writes them |
| `notch.hide_for_fullscreen`, `notch.show_over_maximized`, `notch.lyrics_over_maximized` | `on`, `off` | retired, with no tray item: read only to write `notch.fullscreen` (`on` becomes `hidden`, `off` becomes `music`) and `notch.maximized` (`show_over_maximized=on` becomes `music`, else `lyrics_over_maximized=off` becomes `activities`, else `lyrics`) when the new key is missing, at start and on a reload |
| `pages.known` | `id:Label,...`, with `:opt_in` after the label of an opt-in page | written by the notch at start, only when it changed: the pages the settings window lists (label is `Page::name`; an opt-in page shows as off until `navigation.disabled` is saved). Do not edit; with it missing the window's Pages section shows a note and writes nothing. The window never switches off the last enabled page, so one page stays reachable |
| `navigation.page` | a page id | the page that was open last; the notch opens on it (the notch writes this when you change page) |
| `activity.notifications` | `on`, `off` | on by default: show Windows notification banners in the notch; the tray menu changes this |

Pages add their own keys, always starting with the page id. A page reads and writes them through
`Context::settings`.

## Environment variables

| Variable | Effect |
| --- | --- |
| `NOTCH_PAGES` | comma-separated page ids to switch on (see above) |
| `NOTCH_LYRICS=off` | never look up lyrics, so nothing about your music leaves the computer |
| `NOTCH_DEBUG=<file>` | append a timestamped log of what the notch is doing to this file |
| `NOTCH_TOP=<pixels>` | move the notch down by this many pixels, to compare it with another notch app |

## In the code

Things a developer tunes by editing source:

| What | Where |
| --- | --- |
| which pages exist and their order (the order breaks priority ties) | `src/pages/mod.rs` |
| which activities exist and their order (optional tray entries, badges) | `src/activities/mod.rs` |
| when a page is shown | the page's `priority()` |
| how big a page is | the page's `sizes()` |
| corner radius, spring stiffness, window size | `src/host/notch.rs` |
| text styles | `const FontStyle`s in each page |
| the limits of the performance test | `scripts/perf.rs` |

## Start with Windows

The installer starts the notch at sign-in unless that is unticked in its options. The tray menu's **Start with
Windows** item shows whether it does, and switches it: it adds or removes a `Notch` value under
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run` that points at the copy of `notch.exe` that is running.

## Settings window

The tray menu's **Settings…** item starts `notch-settings.exe` from the folder of the running `notch.exe` and does not
wait for it (`src/host/settings_launcher.rs`). A missing program is logged as `could not start settings`.

`notch-settings.exe` allows one open window; starting it again focuses the existing one. Its sections are General, Pages,
Behavior (one choice of the four words each for `notch.maximized` and `notch.fullscreen`, applied on click), Notifications, Calendar, Media, Timer and Advanced. Toggles apply at once and write `on` or
`off`; the Pages section applies each drag to reorder and each show or hide at once, writing `navigation.order` and
`navigation.disabled`. Every write posts `WM_SETTINGS_CHANGED` to the notch (see Settings file). Text and minute fields
save on Enter or when they lose focus, and a minute value outside its
range (focus 5 to 120, breaks 1 to 120, calendar refresh 1 to 1440) is dropped and the field shows the saved value
again. **Start with Windows** writes the same `Run` value as the tray item, pointing at the `notch.exe` beside the
settings program. The Advanced section only lists the environment variables above. The window's memory and CPU
figures are in [performance.md](performance.md#settings-window).
