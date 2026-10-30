# Adding, configuring and removing an activity

An activity is something that happens while the notch is closed: a volume change, a charger being plugged in, the
microphone turning on. A page is what you see when you open the notch; an activity is what you see in the collapsed
pill. Activities are required unless their implementation opts into a user toggle; see [configuration.md](configuration.md).

## What an activity can show

An activity answers `presence()` after every event, and the host does the layout:

| Presence | Looks like | Example |
| --- | --- | --- |
| `Transient { width }` | takes over the pill for a moment, then goes | volume bar, "Copied", "Charging 56%" |
| `Badge { width }` | a small mark at the right end of the pill for as long as something lasts | the orange microphone dot |
| `Banner { width, height }` | opens the pill into a card, taller than the idle pill and smaller than the open notch, and draws into all of it; it has the pill to itself, so transients wait. Pointing at the notch does not open it while a banner shows, so the activity can hold it with `on_hover_changed` and answer a larger `presence()` while hovered | a notification |
| `None` | nothing | |

- Up to two transients show at once, side by side with a thin divider; the newest two win.
- While a transient shows, the page's collapsed content fades out and the pill resizes to the transient. Badges are
  hidden then and come back after.
- Badges shrink the room the page draws in, so the page never sits under a dot.
- Opening the notch fades every activity out, as it does the page's collapsed content.
- `width` is in device-independent pixels. `draw` gets the exact area for it, in device pixels.

## Add an activity

### 1. Create the file

Make `src/activities/<name>.rs` (or a folder when it has several parts, like `notifications/`). An activity depends on
`activity`, `page` (for `Context`), `gfx`, `ui` and `platform`, never on a page or another activity.

### 2. Implement `Activity`

Four methods are required; the rest are hooks. `src/activities/brightness.rs` is a complete small activity: it
listens to one Windows power setting and flashes a level bar when it changes.

| Method | Job |
| --- | --- |
| `id()` | stable name; the settings key is `activity.<id>` |
| `label()` | the name in the tray menu for an optional activity |
| `is_optional()` | false by default: required activities are always enabled; return true to expose a saved tray toggle |
| `presence()` | what to show right now |
| `draw()` | draw into the area the host gives |
| `header_status_width()`, `draw_header_status()` | optional: a small mark kept in the open notch's header beside the time while something lasts (muted speakers, Caps Lock on); `None` for no mark |
| `on_start()` | begin listening; called once, and only if the activity is switched on |
| `on_timer()`, `on_notify()` | a timer fired; a background thread used the `Notifier` |
| `on_hover_changed()` | the pointer entered or left the notch |
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
| one key | a low-level keyboard hook on a thread of its own that wakes the activity for that key only, then read the state from Windows; see `caps_lock.rs` |
| a folder changing | `FindFirstChangeNotificationW` and a blocking wait on a thread of its own; see `notifications/listener.rs` |

### 4. Register it

Add the module and an entry to `all()` in `src/activities/mod.rs`. Registration order controls badges and optional
tray entries; filtering the tray must preserve the registered activity indices.

## Switch it on or off

Only **Notifications** is optional among the built-in activities. Right-click the tray icon to switch it on or off.
The choice is stored as `activity.notifications=off` or `on` in `%APPDATA%\notch\settings.txt`, defaulting to on.
An optional activity that is off at start is never started; switching it off while running hides it but retains its
listeners until exit. Required activities ignore saved disable settings and reject toggle requests.

## Remove an activity

Delete its file, its `mod` line and its entry in `all()`.

## Built-in activities

| Id | Shows | Source |
| --- | --- | --- |
| `volume` | volume bar, or "Muted", and a crossed speaker in the header while muted | Core Audio endpoint volume callback; follows the default speakers; the mute state is read when listening starts |
| `brightness` | brightness bar | power setting `GUID_DEVICE_POWER_POLICY_VIDEO_BRIGHTNESS`; laptop panel only |
| `battery` | "Charging", "On battery", "Low battery" at 20 % and 10 % | power settings for the power source and battery level |
| `bluetooth` | "<device> connected" with its battery when known, or "disconnected" | `ConnectionStatusChanged` of every paired device, on a thread of its own; the battery is read two seconds after a connection from the device node that reports it |
| `microphone` | "Microphone on/off", then an orange dot | audio sessions of the default recording device; shows that an app has the microphone open, so an app that keeps it open while you are muted (Google Meet in a browser) keeps the dot |
| `camera` | "Camera on/off", then a green dot | stream start and stop events in the `Microsoft-Windows-MF-FrameServer/Camera_FrameServer` event log; misses apps that bypass the Frame Server and a camera already streaming when the notch starts |
| `clipboard` | "Copied" | clipboard change message; never reads the clipboard, skips copies that ask not to be monitored |
| `notifications` | a banner with the app's own icon (the Apps folder's, else its packaged logo, else a colored letter), who it is from and up to two lines; while the pointer is on the notch it stays and, grows taller to show up to 12 lines at the same 400 DIP width; it has no buttons | `UserNotificationListener`, read on a thread of its own when the notification folder changes; arrives a few seconds after Windows' own banner |
| `caps_lock` | "Caps Lock on/off", and an arrow in the header while it is on | a low-level keyboard hook on a thread of its own that only wakes the activity when the Caps Lock key goes down; the lock state is then read from Windows, and no other key is looked at |
