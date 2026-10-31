# Configuration

You configure the notch in four places: the settings window, the settings file, environment variables and the source.
The settings window writes the settings file, so the two always agree.

## Settings window

Open it from the tray menu's **Settings…** item. The tray starts `notch-settings.exe` from the folder of the running
`notch.exe` without waiting for it (`src/host/settings_launcher.rs`) and logs `could not start settings` when the program
is missing. The window allows one instance: starting it again focuses the open one.

| Section | What you set | Key |
| --- | --- | --- |
| General | Startup: start the notch at sign-in | the `Run` registry value, see [Start with Windows](#start-with-windows) |
| General | Startup: hide the notch when nothing is playing or happening | `notch.hide_when_idle` |
| General | Notifications: show Windows notification banners; show what a notification says | `activity.notifications`, `notifications.show_text` |
| General | Music: show lyrics | `media.lyrics` |
| General | Timer: focus, short break and long break lengths | `timer.focus_minutes`, `timer.short_break_minutes`, `timer.long_break_minutes` |
| Pages | the order of the pages, and which are on; drag a page to reorder it | `navigation.order`, `navigation.disabled` |
| Behavior | what the notch shows over a maximized window and over a full-screen app | `notch.maximized`, `notch.fullscreen` |
| Calendar | the calendar address and how often it is downloaded again; the section lists the steps for finding a Google Calendar address | `calendar.ics_url`, `calendar.refresh_minutes` |
| Advanced | the debug log file; how far the notch moves down from the top edge. Both apply after a restart of the notch | `advanced.debug_log`, `notch.top` |

How the window saves:

- A toggle or choice writes when you click it. Dragging a page, and showing or hiding one, writes at once.
- A text or number field writes on Enter or when it loses focus.
- A number outside its range (focus 5 to 120, breaks 1 to 120, calendar refresh 1 to 1440, move down 0 to 2000) is dropped
  and the field shows the saved value again.
- The calendar address shows as dots, with a button that reveals it. An empty field shows a placeholder with the
  default or an example.
- Every write posts `WM_SETTINGS_CHANGED` to the notch (see [Settings file](#settings-file)), which applies the change
  without a restart.
- The Pages section writes nothing until the notch has published `pages.known`; start the notch once, then reopen the
  window. It keeps one page on.

The window opens at 980 x 620 and resizes down to 720 x 480. The sidebar's bottom link opens the repository, and the
text is Geist, the notch's bundled font.

The window's memory and CPU figures are in [performance.md](performance.md#settings-window). To add a setting or a
section, see [adding-a-setting.md](adding-a-setting.md).

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
Old `activity.<id>=off` entries for these activities are ignored. Only **Notifications** has a toggle (in the tray menu
and the settings window), on by default. Its choice is remembered as `activity.notifications`; when off at startup, its
listener is not started.

## Settings file

`%APPDATA%\notch\settings.txt` is a plain text file with one `key=value` per line. The notch and the settings window
write it one key at a time through a temporary file, so keys another program wrote are kept.

After a write, the settings window posts `WM_SETTINGS_CHANGED` (`WM_APP + 30`) to the window of class `NotchWindow`. The
notch then re-reads the file and applies the `notch.*` keys except `notch.top`, `navigation.order`, `navigation.disabled`,
`activity.notifications`, `media.lyrics`, `timer.*_minutes`, `calendar.*` and `notifications.show_text` keys without a
restart, keeping the page that is open. A missing or unreadable value keeps the current one. A hand edit is read at the
next start, or at once if something posts that message.

| Key | Values | Meaning |
| --- | --- | --- |
| `pages` | comma-separated page ids | which pages are on (see above) |
| `media.lyrics` | `on`, `off` | show lyrics on the media page (the speech-bubble button changes this too) |
| `timer.focus_minutes` | 5 to 120 | length of a focus session (default 25); the timer page changes it in steps of 5 |
| `timer.short_break_minutes`, `timer.long_break_minutes` | minutes | break lengths (default 5 and 15) |
| `timer.state`, `timer.completed` | written by the notch | the running timer and the focus sessions finished in this round, so a restart keeps them |
| `navigation.order`, `navigation.disabled` | page ids, comma-separated | the order of the pages and the ones switched off; the Pages section and the "Edit pages" view write them, and by hand they work while that view is off |
| `notifications.show_text` | `on`, `off` | show what a notification says in its banner (off is private mode). Switch the banner itself off with `activity.notifications` |
| `calendar.ics_url` | a web address | the calendar address; the settings window sets it. It is a secret: anyone with it can read the calendar |
| `advanced.debug_log` | a file path | the file the notch appends its timestamped log to; empty means no log. Read once at start, and `NOTCH_DEBUG` overrides it |
| `notch.top` | pixels, 0 to 2000 | how far the notch sits below the top edge of the screen (default 0). Read once at start, and `NOTCH_TOP` overrides it |
| `calendar.refresh_minutes` | minutes | how often the calendar is downloaded again (default 15) |
| `notch.hide_when_idle` | `on`, `off` | off by default: hide the notch when there is nothing to show until you point at the top of the screen |
| `notch.maximized`, `notch.fullscreen` | `hidden`, `activities`, `lyrics`, `music` | what the notch shows over a maximized window (default `lyrics`) and over a full-screen app (default `hidden`); the words are in [the next table](#what-the-notch-shows-over-another-window). A missing or unknown word uses the default |
| `pages.known` | `id:Label,...`, with `:opt_in` after the label of an opt-in page | written by the notch at start, only when it changed: the pages the settings window lists (label is `Page::name`; an opt-in page shows as off until `navigation.disabled` is saved). Leave it alone: the notch owns it |
| `navigation.page` | a page id | the page that was open last; the notch opens on it and writes this when you change page |
| `activity.notifications` | `on`, `off` | on by default: show Windows notification banners in the notch |

Pages add their own keys, always starting with the page id. A page reads and writes them through
`Context::settings`.

### What the notch shows over another window

`notch.maximized` and `notch.fullscreen` take one word each. A word lists what the notch still shows; it hides the strip
for anything the word leaves out.

| Word | The notch shows |
| --- | --- |
| `hidden` | nothing, not even activities |
| `activities` | notification banners, volume changes and badges |
| `lyrics` | an activity, or music that is playing with lyrics |
| `music` | the same, plus the idle pill with nothing playing |

Three older keys (`notch.hide_for_fullscreen`, `notch.show_over_maximized`, `notch.lyrics_over_maximized`) have no
tray item. The notch reads them only when the new key is missing, at start and on a reload:

| Old key and value | New key and word |
| --- | --- |
| `hide_for_fullscreen=on` | `notch.fullscreen=hidden` |
| `hide_for_fullscreen=off` | `notch.fullscreen=music` |
| `show_over_maximized=on` | `notch.maximized=music` |
| `lyrics_over_maximized=off`, with `show_over_maximized` not `on` | `notch.maximized=activities` |
| none of those | `notch.maximized=lyrics` |

## Environment variables

| Variable | Effect |
| --- | --- |
| `NOTCH_PAGES` | comma-separated page ids to switch on (see above) |
| `NOTCH_LYRICS=off` | never look up lyrics, so nothing about your music leaves the computer |
| `NOTCH_DEBUG=<file>` | append a timestamped log of what the notch is doing to this file; overrides `advanced.debug_log` |
| `NOTCH_TOP=<pixels>` | move the notch down by this many pixels, to compare it with another notch app; overrides `notch.top` |

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
Windows** item and the settings window's General section show whether it does, and switch it: each adds or removes a
`Notch` value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` that points at the copy of `notch.exe` that is
running. The settings window points it at the `notch.exe` beside `notch-settings.exe`.
