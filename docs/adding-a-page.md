# Adding, configuring and removing a page

A page is one self-contained feature of the notch, such as the media player or the shelf. This guide builds a
small page from scratch, then lists everything a page can do. To only switch pages on or off without recompiling,
see [configuration.md](configuration.md).

## Add a page

The shared layout fixes every open view to 86 device-independent pixels of content height, plus 50 for the
header and padding (136 total). Change `CONTENT_HEIGHT` in `src/host/chrome.rs` to adjust it for every page. The host
uses `PageSizes::expanded.width` but ignores its height; content taller than 86 px is clipped at the notch boundary.

### 1. Create the folder

Make `src/pages/<name>/mod.rs`. A page may be one file or many; keep everything it needs inside its folder. A page
can use `page`, `activity`, `gfx`, `ui` and `platform`, but never another page.

### 2. Implement `Page`

A page needs five methods; the rest are optional hooks. This is a complete page, a greeting that follows the time
of day and switches between casual and formal wording when you click it while the notch is open. Copy it to
`src/pages/greeting/mod.rs`:

```rust
//! A greeting that follows the time of day. Click it while open to switch between casual and formal wording.

use windows::Win32::System::SystemInformation::GetLocalTime;

use crate::{
    gfx::{Align, Canvas, Color, FontStyle, Rect, UI_FONT, Size, Weight},
    page::{Context, Frame, Mode, Page, PageSizes, PointerEvent, Priority, RESTING_HEIGHT, TimerId},
};

const RESTING_STYLE: FontStyle = FontStyle::new(UI_FONT, 13.0, Weight::Medium, Align::Center);
const OPEN_STYLE: FontStyle = FontStyle::new(UI_FONT, 22.0, Weight::Medium, Align::Center);
/// Settings keys start with the page id, so pages never collide.
const SETTING_FORMAL: &str = "greeting.formal";
/// The greeting changes at most once an hour, so a timer once a minute keeps it right without waking the CPU often.
const REFRESH_TIMER: TimerId = TimerId(0);
const REFRESH_INTERVAL_MILLISECONDS: u32 = 60_000;

pub struct GreetingPage {
    is_formal: bool,
}

impl GreetingPage {
    pub fn new() -> Self {
        Self { is_formal: false }
    }

    fn text(&self) -> &'static str {
        let hour = unsafe { GetLocalTime() }.wHour;
        match (hour, self.is_formal) {
            (5..=11, false) => "Morning!",
            (5..=11, true) => "Good morning",
            (12..=17, false) => "Afternoon!",
            (12..=17, true) => "Good afternoon",
            (_, false) => "Evening!",
            (_, true) => "Good evening",
        }
    }
}

impl Page for GreetingPage {
    fn id(&self) -> &'static str {
        "greeting"
    }

    /// `NORMAL` beats the `BACKGROUND` priority the other pages answer open, so this page opens first.
    fn priority(&self, _mode: Mode) -> Option<Priority> {
        Some(Priority::NORMAL)
    }

    fn sizes(&self) -> PageSizes {
        PageSizes { resting_width: 150.0, expanded: Size::new(320.0, 86.0) }
    }

    fn draw_resting(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        let bounds = frame.bounds;
        let row = Rect::new(bounds.left, bounds.top, bounds.right, bounds.top + RESTING_HEIGHT * frame.scale);
        canvas.text(&RESTING_STYLE, self.text(), row, Color::WHITE.faded(opacity));
    }

    fn draw_expanded(&self, canvas: &Canvas, frame: &Frame, opacity: f32) {
        canvas.text(&OPEN_STYLE, self.text(), frame.bounds, Color::WHITE.faded(opacity));
    }

    fn on_start(&mut self, context: &mut Context) {
        self.is_formal = context.settings.get_bool(SETTING_FORMAL, false);
        context.set_timer(REFRESH_TIMER, REFRESH_INTERVAL_MILLISECONDS);
    }

    fn on_timer(&mut self, _timer: TimerId, context: &mut Context) {
        context.request_repaint();
    }

    fn on_pointer(&mut self, event: PointerEvent, frame: &Frame, context: &mut Context) {
        if let PointerEvent::Released(point) = event
            && frame.is_open()
            && frame.bounds.contains(point)
        {
            self.is_formal = !self.is_formal;
            context.settings.set_bool(SETTING_FORMAL, self.is_formal);
            context.request_repaint();
        }
    }
}
```

What each part does:

- `priority` answers "do you want to be shown?" for the collapsed and the open notch. `None` means no. Collapsed, the
  highest priority wins; `Priority::NORMAL` beats `BACKGROUND`. Open, every page that answers `Some` can be
  reached by scrolling or by the page dots, and the highest priority is only the page shown first.
- Report the size of your content only; the notch adds 32 px to the width and 50 px to the height for its padding and
  header. The height is always 86 px whatever you ask for, so lay the content out for that.
- `sizes` is the size you want, in device-independent pixels. Ask again whenever your content changes size; the host
  asks after every event and animates the change.
- `draw_resting` and `draw_expanded` draw your content. Multiply sizes by `frame.scale` to get device pixels, and fade
  your colors with the `opacity` you are given so the content follows the open and close animation.
  During navigation, each visible page keeps its requested width in `frame.bounds` and `frame.expanded.width`.
  The notch clips the visible portion; do not stretch your content to the neighbouring page's width. Pointer events
  use the same shifted frame as drawing, including while the user rests between pages.
- `on_start` runs once. Read settings and start timers or background work here.
- `on_timer` runs when a timer from `context.set_timer` fires. Request a repaint when something visible changed.
- `on_pointer` receives pointer events while your page is visible, including beside another page during a swipe.
  Positions are in the same space as its own shifted `frame.bounds`.

### 3. Register it

Open `src/pages/mod.rs`, declare the module and add the page to `all()`:

```rust
mod greeting;
mod media;

pub fn all() -> Vec<Box<dyn Page>> {
    vec![
        Box::new(media::MediaPage::new()),
        Box::new(greeting::GreetingPage::new()),
    ]
}
```

The order matters only for ties: when two pages have the same priority, the earlier one wins. Here the greeting and
the media page are both `NORMAL`, so the media page wins while music plays and the greeting shows the rest of the
time.

### 4. Run it

`scripts/reload.sh` builds and starts it. To see your page on its own, hide the others:

```
set NOTCH_PAGES=shelf,greeting
```

Nothing else in the project needs to change: the host, the drawing code and the other pages do not know your page
exists.

## Remove a page

Delete its folder, its `mod` line and its entry in `all()` in `src/pages/mod.rs`. Nothing else refers to it. When no
page has a priority, the collapsed pill shows the time and the open notch only its header.

To only turn a page off without deleting it, list the pages you want in `NOTCH_PAGES` or the `pages` setting; see
[configuration.md](configuration.md).

## Reference

### `Page` methods

| Method | Required | When it is called |
| --- | --- | --- |
| `id()` | yes | a stable name for logs, settings keys and `NOTCH_PAGES` |
| `priority(mode)` | yes | after every event, for `Mode::Resting` and `Mode::Expanded` |
| `sizes()` | yes | after every event, for the page being shown |
| `draw_resting(canvas, frame, opacity)` | yes | each frame while this page is the collapsed one |
| `draw_expanded(canvas, frame, opacity)` | yes | each frame while this page is the open one |
| `on_start(context)` | no | once, after the window exists |
| `on_hover_changed(is_hovering, context)` | no | on every page, when the pointer enters or leaves the notch |
| `on_pointer(event, frame, context)` | no | pointer input, for the page shown open |
| `on_shown_changed(is_shown, context)` | no | when this page starts or stops being on screen (the notch is open and shows it); start and stop measuring here |
| `name()`, `icon()`, `is_opt_in()` | no | what the "Edit pages" view calls the page and draws beside it, and whether it starts switched off |
| `accepts_drops()`, `on_drop(paths, context)` | no | for a page that takes files dropped on the notch |
| `on_file_drag_changed(is_over, context)` | no | on the page that takes drops, when files are dragged over the notch (`true`) or leave or are dropped (`false`); show that it is ready |
| `menu_items()`, `on_menu_item(index, context)` | no | items the page adds to the tray menu |
| `on_timer(timer, context)` | no | when one of the page's timers fires |
| `on_notify(context)` | no | after a background thread used the page's `Notifier` |
| `animate(elapsed_seconds)` | no | each frame while the notch animates; return true while the page is still moving |
| `draw_backdrop(canvas, frame, pill, radius, opacity)` | no | behind everything in the open notch, header included (the media page's lyric glow); `pill` is the whole notch |
| `header_status_width()`, `draw_header_status(canvas, area, scale, opacity)`, `header_status_rank()` | no | a small mark kept in the header beside the time, whichever page is open (the wave, the timer's ring); the lowest rank sits nearest the time |
| `shows_live_text()` | no | true while the collapsed pill shows text being read as it changes (the lyric line); lets it stay over a maximized window when the user asked |
| `presence()` and `draw_presence(canvas, area, scale, opacity)` | no | after every event: something the page shows in the collapsed pill even when it is not the page on show, as an activity does |

A page that also has something to say while closed (the timer's ring and minutes, the alert when a phase ends) answers
`presence()` with `Presence::Badge { width }` (a mark kept beside the page on show) or `Presence::Transient { width }`
(takes over the pill for a moment; `Presence::Banner` is for activities), and draws it in `draw_presence` into the area the host gives. It can use
`activity::Flash` for a transient that comes and goes, and its own timers and settings as usual. See
[adding-an-activity.md](adding-an-activity.md) for how the host lays these out.

`PointerEvent` is `Moved`, `Pressed`, `Released`, `SecondaryReleased` (a right click), `Left` (the pointer left the
notch) and `CaptureLost`. Positions are device pixels. `Frame::bounds` is the page's own content area when the notch
is open (the notch draws its padding and header around it), so a page lays itself out from the top-left of `bounds`.

### `Context`

Passed to every hook that can change something.

| Call | What it does |
| --- | --- |
| `request_repaint()` | draw the notch again after the hook returns |
| `request_animation()` | start the frame timer so `animate` runs; use it after starting an animation of your own |
| `set_timer(TimerId(n), milliseconds)` / `kill_timer(TimerId(n))` | repeating timers; `n` must be below 16; setting an existing timer re-arms it |
| `listen_to_power_setting(&GUID)`, `listen_to_clipboard()` | for activities; see [adding-an-activity.md](adding-an-activity.md) |
| `notifier()` | a handle a background thread calls `notify()` on to wake the page |
| `upload_image(&ImageData)` | make an image drawable; usually through `ImageSlot::sync` |
| `fonts()` | measure text with `fonts().measure(&style, text)` |
| `settings` | `get_bool`, `set_bool` and `get_string`; start keys with your page id |

### `Frame`

| Field | Meaning |
| --- | --- |
| `scale` | device pixels per device-independent pixel |
| `bounds` | the pill in device pixels, in the same space as pointer positions |
| `expansion` | 0 collapsed, 1 open; it can overshoot a little while animating |
| `expanded` | the animated open size in device-independent pixels |
| `is_open()` | true once the notch is open far enough for its controls to work |

### Drawing: `Canvas`

| Call | What it draws |
| --- | --- |
| `text(&style, text, rect, color)` | one run of text, aligned and trimmed by the style |
| `fill_rounded_rect(rect, radius, color)` | a rounded rectangle |
| `fill_circle(center, radius, color)` | a circle |
| `cached_image(key, build)` | an image you draw once on the CPU (for example with `Mask`) and reuse by `key`; this is how icons are drawn |
| `fill_top_flares(bounds, flare, color)` | the concave notch-style corners where the pill meets the screen top (the host uses it) |
| `image(&brush, rect, radius, opacity)` | an uploaded image in a rounded rectangle |
| `upload(&image)` | upload an image to draw with `image`, for a page that keeps its own brushes and drops them when done (the shelf's previews) |
| `clipped(rect, draw)` | run `draw` with everything outside `rect` cut away |
| `scaled_about(point, factor, draw)` | run `draw` scaled around a point; text stays sharp |

Text styles are `const FontStyle::new(family, size, weight, align)`, declared in the page. Add `.wrapped()` to wrap
long text instead of cutting it with an ellipsis. `UI_FONT` is the text font (Geist, bundled in the executable; see `src/gfx/fonts.rs`). Icons are shapes built from a `Path` and drawn
into small cached images by `ui::Icon`, not font glyphs. Do not use Direct2D geometry (`ID2D1PathGeometry`): its first use adds about 10 MB of memory. `Spring` eases a value toward a target; call `step(elapsed_seconds)` on it from the page's `animate` and return true while `is_settled()` is false.

### Reusable widgets: `ui`

| Widget | Use |
| --- | --- |
| `IconButton` | an icon button (an `Icon`) with the ghost hover: nothing until the pointer is over it, then a faint rounded wash |
| `PillButton` | a rounded button with a label; primary (accent) or destructive (dark red) |
| `Ring` | a circular progress mark |
| `Icon` | a glyph drawn from a path into a cached image; add one in `src/ui/icons.rs` |
| `draw_dashed_outline` | a dashed rounded outline that bends at the corners (the shelf's drop zone) |

Text helpers in `gfx`: `clamp_lines` breaks text into at most a number of lines with an ellipsis, and `balance_lines` evens the lines out.

### Background work

For anything slow or blocking (network, files, media APIs), start a thread in `on_start`, give it
`context.notifier()`, update shared state behind a lock, and call `notifier.notify()`. The host then calls your
`on_notify` on the UI thread, where you read the state and request a repaint. The media page does exactly this.

### Conventions

- Settings keys start with the page id, for example `greeting.formal`.
- Keep pure logic (parsing, layout rules, state machines) in functions you can unit test; see
  [testing.md](testing.md).
- Do not keep timers or animations running while nothing changes. The notch uses no CPU when idle, and
  `scripts/perf.sh` checks that.
- Split a page into files by job once it grows. The media page is the model: see
  [architecture.md](architecture.md#pages).

## If your page does not show up

- Another page has a higher priority, or the same priority and an earlier place in `all()`.
- `priority` returns `None` for the mode you are looking at.
- `NOTCH_PAGES` or the `pages` setting leaves it out.
- `sizes()` returns zero.
- Set `NOTCH_DEBUG=<file>`: the log lists the registered pages and any that are switched off.
