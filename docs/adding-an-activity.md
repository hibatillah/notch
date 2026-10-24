# Adding, configuring and removing an activity

An activity is something that happens while the notch is closed: a volume change, a charger being plugged in, the
microphone turning on. A page is what you see when you open the notch; an activity is what you see in the collapsed
pill. To switch activities on or off without recompiling, see [configuration.md](configuration.md).

## What an activity can show

An activity answers `presence()` after every event, and the host does the layout:

| Presence | Looks like | Example |
| --- | --- | --- |
| `Transient { width }` | takes over the pill for a moment, then goes | volume bar, "Copied", "Charging 56%" |
| `Badge { width }` | a small mark at the right end of the pill for as long as something lasts | the orange microphone dot |
| `None` | nothing | |

- Up to two transients show at once, side by side with a thin divider; the newest two win.
- While a transient shows, the page's collapsed content fades out and the pill resizes to the transient. Badges are
  hidden then and come back after.
- Badges shrink the room the page draws in, so the page never sits under a dot.
- Opening the notch fades every activity out, as it does the page's collapsed content.
- `width` is in device-independent pixels. `draw` gets the exact area for it, in device pixels.

## Add an activity

### 1. Create the file

Make `src/activities/<name>.rs` (or a folder). An activity depends on `activity`, `page` (for `Context`), `gfx` and
`ui`, never on a page or another activity.

### 2. Implement `Activity`

Four methods are required; the rest are hooks. `src/activities/brightness.rs` is a complete small activity: it
listens to one Windows power setting and flashes a level bar when it changes.

| Method | Job |
| --- | --- |
| `id()` | stable name; the settings key is `activity.<id>` |
| `label()` | the name in the tray menu |
| `presence()` | what to show right now |
| `draw()` | draw into the area the host gives |
| `on_start()` | begin listening; called once, and only if the activity is switched on |
| `on_timer()`, `on_notify()` | a timer fired; a background thread used the `Notifier` |
| `on_system_message()` | a power setting or the clipboard changed (see below) |

Use `Flash` for a transient that shows and then goes: `flash.show(context, milliseconds)` shows it, again while
visible keeps it up for a fresh duration, and `flash.on_timer(timer, context)` hides it. It uses `TimerId(0)`; an
activity that needs more timers starts from `TimerId(1)`. Timer ids and wake-up messages are separate from the pages',
so an activity and a page never collide.

Draw messages with `ui::draw_level` (icon, bar, value) or `ui::draw_message` (icon, text, optional value) so every
transient looks alike, and add an `Icon` to `src/ui/icons.rs` when you need a new glyph.

### 3. Get events without polling

The notch must use no CPU while nothing happens, so an activity never polls. Pick the source that calls you:

| Source | How |
| --- | --- |
| a Windows power setting (brightness, charger, battery level) | `context.listen_to_power_setting(&GUID)` in `on_start`; changes arrive in `on_system_message`. The first message after registering is the current value, so use `is_change` to skip it. |
| the clipboard | `context.listen_to_clipboard()`; arrives as `SystemMessage::ClipboardChanged` |
| a COM callback (Core Audio) | store the change in an `Arc<Mutex<..>>` and call `context.notifier().notify()`; read it in `on_notify` |
| a Windows event log channel | `EvtSubscribe` with a callback, which runs on a system thread and calls the `Notifier`; see `camera.rs` |

### 4. Register it

Add the module and an entry to `all()` in `src/activities/mod.rs`. The order is the order in the tray menu and the
order of badges.

## Switch it on or off

Right-click the notch or its tray icon: every activity is a checkable item. The choice is stored as
`activity.<id>=off` or `on` in `%APPDATA%\notch\settings.txt`. An activity that is off at start is never started;
switching one off while the notch runs hides it but keeps what it already listens to until the notch quits.

## Remove an activity

Delete its file, its `mod` line and its entry in `all()`.

## Built-in activities

| Id | Shows | Source |
| --- | --- | --- |
| `volume` | volume bar, or "Muted" | Core Audio endpoint volume callback; follows the default speakers |
| `brightness` | brightness bar | power setting `GUID_DEVICE_POWER_POLICY_VIDEO_BRIGHTNESS`; laptop panel only |
| `battery` | "Charging", "On battery", "Low battery" at 20 % and 10 % | power settings for the power source and battery level |
| `microphone` | "Microphone on/off", then an orange dot | audio sessions of the default recording device; shows that an app has the microphone open, so an app that keeps it open while you are muted (Google Meet in a browser) keeps the dot |
| `camera` | "Camera on/off", then a green dot | stream start and stop events in the `Microsoft-Windows-MF-FrameServer/Camera_FrameServer` event log; misses apps that bypass the Frame Server and a camera already streaming when the notch starts |
| `clipboard` | "Copied" | clipboard change message; never reads the clipboard, skips copies that ask not to be monitored |
