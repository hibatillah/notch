# Architecture

The notch is a small window pinned to the top of the screen. It shows one **page** at a time, either collapsed
(a slim pill) or open (while the pointer is over it). While it is collapsed, **activities** (a volume change, a
charger being plugged in, the microphone turning on) take over the pill for a moment or add a small badge to it. The
code is split so that adding or removing a page or an activity never touches the host.

```
src/
  main.rs         entry point: starts the host with the registered pages and activities
  page.rs         the whole contract for a page: the Page trait, Context, Frame, events
  activity.rs     the whole contract for an activity: the Activity trait, Presence, SystemMessage, Flash
  log.rs          optional debug log (set NOTCH_DEBUG to a file path)
  host/           the notch itself: window, animation, hover, tray, settings. Knows no page.
  gfx/            drawing toolkit for pages: Canvas, fonts, colors, images, springs, geometry
  platform.rs     small Windows actions for pages: open a file or link, copy and paste text, a popup menu
  ui/             reusable widgets: IconButton, PillButton, Ring, icons, the HUD layouts for activities
  activities/     the registry (mod.rs) and one file per activity: volume, brightness, battery, Bluetooth,
                  microphone, camera, clipboard
  pages/
    mod.rs        the registry: the only place pages are listed
    clock/        fallback page: the time and date
    media/        media player: session, controls, lyrics, artwork, app icon
    notifications/ Windows notifications: the listener, the stack, the cards
    shelf/        file shelf: the list of files and their tiles
    calendar/     calendar: dates, the iCal reader, the day's agenda, the download
    timer/        focus timer: the rules, what is said in each state, the layout
    stats/        system stats: the readings and how they are written
```

Dependencies point one way. Pages use `page`, `gfx` and `ui`; activities use `activity`, `page` (for `Context`), `gfx`
and `ui`. The host uses `page`, `activity` and `gfx`. Nothing in `gfx`, `ui`, `page` or `activity` knows about a
particular page or activity, and pages and activities never use each other.

## The host

`host/` owns everything that is not specific to a page.

| File | Job |
| --- | --- |
| `window.rs` | creates the window, runs the message loop, turns Windows messages into calls on the notch |
| `notch.rs` | the `Notch`: animates the pill, tracks hover, selects pages, routes input, paints |
| `registry.rs` | decides which registered pages are switched on (`NOTCH_PAGES`, the `pages` setting) |
| `navigation.rs` | the user's page order and which pages are on, which page opens, wheel steps (pure) |
| `chrome.rs` | the padding and the header (page dots and gear) around every open page |
| `page_editor.rs` | the "Edit pages" view: chip layout, drag and double-click rules, drawing |
| `activity_layout.rs` | which transient activities show and where each one starts (pure functions) |
| `settings.rs` | the persistent key-value store pages and activities share |
| `tray.rs` | the tray icon and its menu: one checkable item per activity, then Quit |
| `display.rs` | display scale and refresh rate |

### Choosing a page

Each page answers `priority(mode)` for `Mode::Resting` (collapsed) and `Mode::Expanded` (open). `None` means
"nothing to show". The host shows the highest priority in each mode, and the earlier registered page wins ties. A page
that answers `Priority::URGENT` while open comes before the page the user chose, for as long as it does (the
notifications page does this while a notification is showing).

While collapsed the highest priority is always the page shown. While open it is only the starting point: with two or
more pages that have something to show, the user moves between them (see Navigation below), and the page they moved
to stays open until they move again.

The two modes are independent. The media page, for example, is shown collapsed only while music plays, but stays up
when open while paused so playback can be resumed. The clock answers `Some(BACKGROUND)` in both modes, so
something is always shown. The host asks again after every event, so a page changes what it shows by changing its
own state.

### The open notch

Every open page sits inside the same frame: 14 px of padding, a 16 px header (the page dots on the left, the gear on
the right), a 12 px gap, then the page's own content, 16 px of padding at the sides and 14 below. A page reports the
size of its content only (`PageSizes::expanded`) and draws in `Frame::bounds`, which is the content area. The gear
opens the "Edit pages" view in place of the page: drag a chip to reorder, double-click to remove, click an available
one to add; the order and what is off are saved as `navigation.order` and `navigation.disabled`. A page can be opt-in
(`Page::is_opt_in`), which starts it switched off.

While the screen is idle the notch can hide itself (a choice in the tray menu): it shrinks to a nearly invisible
240 x 6 px strip that still receives the pointer, shows the small pill when the pointer rests on it for 150 ms, and
opens 450 ms after that if the pointer stays.

### Navigation

With two or more pages that answer `Some` for `Mode::Expanded`, the open notch shows one dot per page in a strip at
its bottom (`navigation::STRIP_HEIGHT`, 16 px) and the page draws in the space above it. The user moves between pages
by scrolling the mouse wheel (up or left goes back, down or right goes forward), swiping two fingers on a touchpad
(sent as scrolling), or clicking a dot. One gesture moves one page and stops at the first and last page; the content
slides sideways while the pill resizes to the new page. The page left open is remembered as `navigation.page` in the
settings, so the notch opens on it next time. `host/navigation.rs` holds the rules as pure functions; `notch.rs`
applies them.

### Sizes

A page reports `sizes()`: its collapsed width and its open size, in device-independent pixels. The host animates
the pill toward them with springs, so a page that changes its mind (the media page widens for lyrics) gets a smooth
transition for free. The collapsed height is fixed at `RESTING_HEIGHT`.

### Painting

The notch is a layered window with per-pixel alpha. Each frame the host draws the black pill and then calls
`draw_resting` on the page chosen for the collapsed mode and `draw_expanded` on the one chosen for the open mode,
with opacities that cross-fade as the notch opens. Only the rectangle around the pill is redrawn and shown, and the
window shrinks and grows with the pill, so the screen outside the pill is never covered and never blocks clicks.

### Input and timers

Windows messages reach `window.rs`, which calls the notch. The notch converts positions to a common space and
routes pointer events to the page chosen for the open mode. A page's timers (`Context::set_timer`) are Windows
timers whose ids encode the page and the timer number, so each page has its own.

### Activities

The host asks every running activity for its `presence()` after each event. A **transient** (up to two, newest win)
replaces the page's collapsed content: the pill resizes to the transient widths and a blend spring cross-fades the
two. A **badge** stays beside the page's content at the right end of the pill, which the page is told to leave free.
Activities start only when switched on (`activity.<id>` in the settings, or the tray menu). The host registers the
window for Windows power-setting and clipboard messages on their behalf (`Context::listen_to_power_setting`,
`listen_to_clipboard`) and hands them to every running activity as a `SystemMessage`. Activity timers use ids from
1000 up and their wake-ups use their own window message, so they never clash with a page's. See
[adding-an-activity.md](adding-an-activity.md).

### Threads

All drawing and page code runs on the single UI thread. A page that needs background work (the media page watches
the Windows media session and looks up lyrics) starts its own threads, and wakes the UI thread with a `Notifier`,
which posts a message; the page then reads the new state in `on_notify`.

### Animation

While anything moves the host runs a frame timer at the display's refresh rate; when the host's springs have
settled and no page reports `animate` as still moving, the timer stops. A collapsed notch with nothing happening
has no timers running except a page's own (the clock wakes once a minute), which is why it uses no CPU.

## Pages

`pages/mod.rs` is the registry. See [adding-a-page.md](adding-a-page.md) to write one. A page may also answer
`presence()` to show something in the collapsed pill beside or instead of the page on show (the timer does); the host
treats it like an activity's presence.

The media page is the largest example of how to split a page:

| File | Job |
| --- | --- |
| `mod.rs` | the `MediaPage` type: state, and the `Page` implementation |
| `session.rs` | mirrors the Windows media session into plain data, on its own threads |
| `artwork.rs`, `app_icon.rs` | decode the album art and find the player's icon |
| `lyrics/` | the lyric data model and `.lrc` parser, and the lookup client (`client.rs`) |
| `lyric_follower.rs` | follows which lyric line is being sung and animates the change |
| `layout.rs` | where everything in the open view goes |
| `resting.rs`, `expanded.rs` | draw the collapsed and open views |
| `controls.rs`, `seek.rs`, `lyric_panel.rs` | the buttons, the progress bar and the scrolling lyrics |
| `styles.rs` | the page's text styles |

The timer page is the example of a page that also has a collapsed presence:

| File | Job |
| --- | --- |
| `mod.rs` | the `TimerPage` type: input, timers, the alert, the `Page` implementation |
| `session.rs` | the rules: phases, the session counter, pause, extend, when to wake next, saving; pure |
| `view.rs` | what is said and offered in each state, and the compact text; pure |
| `layout.rs` | where the title, the time and the buttons go |

## Tests

Unit tests live next to the code they test. `scripts/perf.sh` measures memory and CPU of a running notch. See
[testing.md](testing.md).
