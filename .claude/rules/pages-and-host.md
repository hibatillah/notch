---
paths:
  - "src/pages/**"
  - "src/host/**"
  - "src/page.rs"
  - "src/activity.rs"
  - "src/activities/**"
  - "src/gfx/**"
  - "src/ui/**"
---

# Pages and host boundaries

Read `docs/adding-a-page.md` before writing a page, `docs/adding-an-activity.md` before writing an activity and
`docs/architecture.md` before restructuring.

- A page lives in `src/pages/<name>/` and is listed only in `src/pages/mod.rs`; adding or removing one touches no
  other file.
- A page depends on `page`, `activity` (for `Presence` and `Flash`), `gfx`, `ui` and `platform` only.
- `host`, `gfx`, `ui`, `page` and `activity` stay ignorant of any particular page or activity.
- Page timer ids are `100 + page_index * 16 + n`, with `n` below 16; activity timer ids are
  `1000 + activity_index * 16 + n`.
- An activity lives in `src/activities/` and is listed only in `src/activities/mod.rs`. It depends on `activity`, `page`
  (for `Context`), `gfx` and `ui`, never on a page or another activity, and never polls: it listens to a Windows
  callback, message or blocking wait and wakes the UI thread through its `Notifier`.
- Activity settings keys are `activity.<id>`.
- A page reports the size of its content only; the host adds its padding and header (`host/chrome.rs`).
- A page that also needs the collapsed pill (a ring, an alert) answers `Page::presence` instead of depending on an
  activity; it keeps its own state.
- Page settings keys start with the page id (`media.lyrics`).
- Pages draw through `Canvas` and multiply colors by the `opacity` they are given, so content follows the open and
  close animation.
