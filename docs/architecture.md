# Architecture

The notch is a small window pinned to the top of the screen. It shows one **page** at a time, either collapsed
(a slim pill) or open (while the pointer is over it). The code is split so that adding or removing a page never
touches the host.

```
src/
  main.rs         entry point: starts the host with the registered pages
  page.rs         the whole contract for a page: the Page trait, Context, Frame, events
  log.rs          optional debug log (set NOTCH_DEBUG to a file path)
  host/           the notch itself: window, animation, hover, tray, settings. Knows no page.
  gfx/            drawing toolkit for pages: Canvas, fonts, colors, images, springs, geometry
  ui/             reusable widgets: IconButton (ghost hover), ProgressBar
  pages/
    mod.rs        the registry: the only place pages are listed
    clock/        fallback page: the time and date
    media/        media player: session, controls, seek bar, lyrics, artwork, app icon
```

Dependencies point one way. Pages use `page`, `gfx` and `ui`. The host uses `page` and `gfx`. Nothing in `gfx`, `ui`
or `page` knows about a particular page, and pages never use each other.

## The host

`host/` owns everything that is not specific to a page.

| File | Job |
| --- | --- |
| `window.rs` | creates the window, runs the message loop, turns Windows messages into calls on the notch |
| `notch.rs` | the `Notch`: animates the pill, tracks hover, selects pages, routes input, paints |
| `registry.rs` | decides which registered pages are switched on (`NOTCH_PAGES`, the `pages` setting) |
| `settings.rs` | the persistent key-value store pages share |
| `tray.rs` | the tray icon and its Quit menu |
| `display.rs` | display scale and refresh rate |

### Choosing a page

Each page answers `priority(mode)` for `Mode::Resting` (collapsed) and `Mode::Expanded` (open). `None` means
"nothing to show". The host shows the highest priority in each mode, and the earlier registered page wins ties.

The two modes are independent. The media page, for example, is shown collapsed only while music plays, but stays up
when open while paused so playback can be resumed. The clock answers `Some(BACKGROUND)` in both modes, so
something is always shown. The host asks again after every event, so a page changes what it shows by changing its
own state.

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

### Threads

All drawing and page code runs on the single UI thread. A page that needs background work (the media page watches
the Windows media session and looks up lyrics) starts its own threads, and wakes the UI thread with a `Notifier`,
which posts a message; the page then reads the new state in `on_notify`.

### Animation

While anything moves the host runs a frame timer at the display's refresh rate; when the host's springs have
settled and no page reports `animate` as still moving, the timer stops. A collapsed notch with nothing happening
has no timers running except a page's own (the clock wakes once a minute), which is why it uses no CPU.

## Pages

`pages/mod.rs` is the registry. See [adding-a-page.md](adding-a-page.md) to write one.

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

## Tests

Unit tests live next to the code they test. `scripts/perf.sh` measures memory and CPU of a running notch. See
[testing.md](testing.md).
