# Settings window, behavior matrix, Windows 11 tray, Google sign-in: design

Decisions from the brainstorm with the project owner (2026-10-30). Items marked *call* are the assistant's choice, not the owner's.

## Why

People who do not edit `settings.txt` cannot change most settings, and the tray menu cannot hold them all. A settings window makes every setting reachable. Calendar setup by pasting an iCal address is hard, so Google sign-in replaces it. The notch should also behave differently over regular, maximized and full-screen windows.

## Four projects, in this order

1. **Settings window**: a separate process (`notch-settings.exe`) built with GPUI, editing the same `settings.txt`. Includes page reorder and show/hide.
2. **Behavior per context**: one choice each for maximized and full-screen windows: hidden, activities only, music even without lyrics, music only with lyrics.
3. **Windows 11 tray menu**: larger, rounded, translucent. Spike first; the look may need our own popup window with a system backdrop.
4. **Google sign-in**: replaces pasting an iCal address. Research first (verification policy, token storage, loopback sign-in).

## Goals

- Every key in `docs/configuration.md` reachable from the window.
- The notch process keeps its budget (0 % CPU idle, under 50 MB); the settings process exists only while its window is open.
- Settings file stays the single source of truth; hand editing keeps working.

## Non-goals

- No UI framework in `notch.exe`.
- No account system of our own; Google sign-in is only for reading the user's calendar.

## Anti-goals

- A settings window that overwrites keys the notch changed (lost updates).
- GPUI adopted without measured memory, size and license numbers.

## Decided by the owner

- Window layout: left sidebar (General, Pages, Behavior, Notifications, Calendar, Media, Timer, Advanced).
- Technology: GPUI, in a separate process, started from a tray item; adoption gated by a measured spike.
- Work happens on branch `settings-window`, not `master`.

## Calls by the assistant

- Apply instantly for toggles, choices and reordering; commit-on-Enter or on blur for text and number fields; no Save button.
- A shared `notch-store` crate holds the settings file code so both programs write per key with an atomic rename.
- The window tells the notch to reload by posting `WM_SETTINGS_CHANGED` (`WM_APP + 30`) to the `NotchWindow` window class.
- The notch publishes its pages as `pages.known=id:Label,...` at startup so the window can list them without depending on the notch crate.
- Spike gate (guess): settings window under 60 MB working set, all dependencies MIT-compatible.
- Old behavior keys migrate once: full-screen `hide_for_fullscreen=on` becomes `hidden`; maximized `show_over_maximized=on` becomes `music`, else `lyrics_over_maximized=on` (default) becomes `lyrics`, else `activities`.
