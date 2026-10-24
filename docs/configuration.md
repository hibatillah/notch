# Configuration

There are three ways to configure the notch, from what a user can change to what a developer changes in code.

## Which pages are on

Without any setting, every registered page is on. To use only some of them, list their ids, separated by commas:

- **Environment variable:** `NOTCH_PAGES=clock,media`
- **Settings file:** a line `pages=clock,media` in `%APPDATA%\notch\settings.txt`

The environment variable wins when both are set. Page ids are the values returned by each page's `id()`; the built-in
ones are `clock` and `media`.

- Pages keep their registration order, whatever order you list them in.
- Ids that match no page are ignored. If none match, every page stays on.
- Keep `clock` on: it is the fallback shown when no other page has anything to show.

Example, to run without the media page:

```
set NOTCH_PAGES=clock
notch.exe
```

To change which pages are *compiled in*, edit `src/pages/mod.rs`; see [adding-a-page.md](adding-a-page.md).

## Which activities are on

Activities (volume, brightness, charger and battery, microphone, camera, copied) are all on by default. Right-click the
notch or its tray icon and tick or untick them: the choice is remembered. It is stored as `activity.<id>` in the
settings file; an activity that is off at start is never started, so it costs nothing.

## Settings file

`%APPDATA%\notch\settings.txt` is a plain text file with one `key=value` per line. The notch writes it when you
change something; you can also edit it by hand while the notch is not running.

| Key | Values | Meaning |
| --- | --- | --- |
| `pages` | comma-separated page ids | which pages are on (see above) |
| `media.lyrics` | `on`, `off` | show lyrics on the media page (the speech-bubble button changes this) |
| `timer.focus_minutes` | 5 to 120 | length of a focus session; the timer page changes it in steps of 5 |
| `timer.short_break_minutes`, `timer.long_break_minutes` | minutes | break lengths (default 5 and 15); set by hand |
| `timer.state`, `timer.completed` | written by the notch | the running timer and the focus sessions finished in this round, so a restart keeps them |
| `navigation.order`, `navigation.disabled` | page ids, comma-separated | the order of the pages and the ones switched off; the "Edit pages" view writes them |
| `notifications.enabled`, `notifications.show_text` | `on`, `off` | show notifications in the notch; show what they say (off is private mode); the tray menu changes them |
| `calendar.ics_url` | a web address | the calendar address; the tray menu sets it from the clipboard. It is a secret: anyone with it can read the calendar |
| `calendar.refresh_minutes` | minutes | how often the calendar is downloaded again (default 15); set by hand |
| `notch.hide_when_idle` | `on`, `off` | hide the notch until you point at the top of the screen; the tray menu changes it |
| `navigation.page` | a page id | the page that was open last; the notch opens on it (the notch writes this when you change page) |
| `activity.<id>` | `on`, `off` | show that activity; ids are `volume`, `brightness`, `battery`, `microphone`, `camera`, `clipboard` (the tray menu changes this) |

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
| which activities exist and their order (tray menu, badges) | `src/activities/mod.rs` |
| when a page is shown | the page's `priority()` |
| how big a page is | the page's `sizes()` |
| corner radius, spring stiffness, window size | `src/host/notch.rs` |
| text styles | `const FontStyle`s in each page |
| the limits of the performance test | `scripts/perf.sh` |

## Start with Windows

This is not built in. To start the notch at sign-in, put a shortcut to `notch.exe` in the folder opened by
pressing Win+R and running `shell:startup`.
