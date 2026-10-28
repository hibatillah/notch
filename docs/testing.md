# Testing

There are two kinds of automated checks: unit tests for logic, and a performance check for memory and CPU.

## Unit tests

```
cargo test
```

They run in a fraction of a second and need no window. They cover the logic that can be checked without drawing:

- lyric parsing and timing (`.lrc` text, which line is being sung, when the next one starts)
- cleaning titles and artists for lyric lookups, and URL encoding
- springs, geometry, colors and image slots
- settings: parsing, defaults and saving
- page selection: priority rules, ties, switching pages off, timer keys
- the media page's rules: when it shows, how wide it asks to be
- layout of the open view and the hit areas of the buttons, the shelf's files and the calendar's days
- the lyric follower, which tracks the current line and arms timers

What they do not cover is the Windows integration (the window, the tray, the media session). Check that by running the
notch: see the manual checklist below.

### Pictures of the pages

```
cargo test snapshots -- --ignored
```

These tests are ignored by default. They draw every page off-screen with example content and write the pictures to
`target/snapshots` (BMP files; `host/notch/snapshots.rs` holds them): each open page, the collapsed pill in its
states, the short messages, the notification banner and the shelf with files dragged over it. Look at the pictures
after a change to drawing or layout. Two more ignored tests need a real screen and open windows for a moment:

```
cargo test a_real_maximized -- --ignored      # a maximized and a full-screen window are told apart
cargo test this_windows_has -- --ignored      # the Windows entry points behind the dark tray menu exist
```

### Where the tests live

Each test sits in the file it tests, in a `#[cfg(test)] mod tests` block at the bottom. There is no separate `tests/`
folder, because the notch is a binary crate and an outside test folder could not reach its internals. The tests are
spread over many files:

| Area | Files with tests |
| --- | --- |
| page contract | `src/page.rs` |
| activity contract | `src/activity.rs` (change detection) |
| host | `src/host/notch.rs` (page choice, badge room), `navigation.rs` (page steps, wheel, swipe limit, dots), `activity_layout.rs`, `registry.rs`, `settings.rs`, `tray.rs` (menu commands), `screen_mode.rs` (maximized or full screen), `startup.rs`, `notch/snapshots.rs` (the hiding rules) |
| drawing toolkit | `src/gfx/color.rs`, `geometry.rs`, `spring.rs`, `image.rs`, `path.rs`, `raster.rs`, `canvas.rs` (flare shape) |
| widgets | `src/ui/icon_button.rs`, `icons.rs`, `outline.rs`, `ring.rs`, `hud.rs` |
| timer page | `src/pages/timer/session.rs` (rules, saving), `view.rs`, `layout.rs` |
| stats page | `src/pages/stats/format.rs` (numbers), `sampler.rs` (the readings), `mod.rs` (layout) |
| shelf page | `src/pages/shelf/items.rs` (the list, wording, saving), `mod.rs` (places, hit targets, name lines, double click, arrival) |
| platform | `src/platform.rs` (clipboard text, pixel conversion of Explorer pictures) |
| calendar page | `src/pages/calendar/dates.rs`, `ics.rs` (reading and repeating), `agenda.rs` (what a day says), `fetch.rs` |
| notifications activity | `src/activities/notifications/items.rs` (the wording), `mod.rs` (banner height, lines) |
| editing pages | `src/host/page_editor.rs`, `chrome.rs`, `navigation.rs` (the page list) |
| activities | `src/activities/battery.rs` (charge rules), `volume.rs`, `camera.rs`, `privacy.rs`, `caps_lock.rs` |
| media page | `src/pages/media/mod.rs`, `layout.rs`, `resting.rs`, `session.rs`, `lyric_follower.rs`, `lyrics/mod.rs`, `lyrics/client.rs` |

To run one file's tests, or a single test, filter by name:

```
cargo test lyric_follower
cargo test a_paused_track_keeps_its_line_but_arms_no_timer
cargo test -- --nocapture
```

### Writing a test

Put a `#[cfg(test)] mod tests` at the bottom of the file you are changing and test through its normal functions.
Name tests as sentences about behavior, for example `a_paused_track_keeps_its_line_but_arms_no_timer`. Tests that
need text measurement create `Fonts::new(1.0)`; DirectWrite works in the test run without a window.

Keep logic testable by keeping it out of the Windows calls: a function that takes plain values and returns plain
values can be tested; one that talks to the window cannot.

## Performance check

```
scripts/perf.sh               # builds, then runs the checks (about two minutes)
scripts/perf.sh --no-build    # reuse the existing release build
scripts/perf.sh --with-media  # also print numbers with every page on; play music first
scripts/perf.sh --extended    # also measure held hover, swipes, release snap, and recovery
scripts/perf.sh --lyrics-only # live lyrics on/off, collapsed and open, in isolated test profiles
```

The script starts a copy of the notch with only the shelf page (`NOTCH_PAGES=shelf`), so the result does not depend
on what you are listening to. It moves the mouse to open and close the notch, so leave the mouse alone while it
runs. It also asks a running notch to close; start yours again with `scripts/reload.sh`.

It checks, and fails when a limit is exceeded:

| Check | Limit |
| --- | --- |
| the notch settles after start-up and after each animation | CPU back to idle within 20 s |
| CPU while collapsed and idle | 0.1 % of one core, measured over 20 s |
| memory (private working set) after start | 30 MB |
| CPU for one open-and-close | 400 ms |
| memory growth over 10 open-and-closes | 3 MB |
| handle growth over 10 open-and-closes | 20 handles |
| hover events logged | exactly two per cycle, so none were missed or doubled |
| CPU while idle again after the animations | 0.1 % — the frame timer must have stopped |

The last check is the important one. A common mistake is leaving a timer or animation running, which makes the
notch use CPU forever; it shows up there as several percent of a core.

The latest recorded baseline and its workload are summarized in [README.md](../README.md#resource-use).
Run the check for your current machine. The limits are set
in constants at the top of `scripts/perf.rs`; they exist to catch regressions, so fix the cause rather than raising one.

`--with-media` prints memory and CPU with every page on and your music playing. It has no limits, because the result
depends on the track and on how often its lyrics change.

`--extended` adds 20-second samples while hovered open, after the touchpad release snap, and collapsed after swiping. Its swipe
workload sends horizontal wheel messages to its own Notch window and verifies that the window width changes.
`--lyrics-only` requires a playing Windows media session. It uses separate temporary settings under `run/` to compare
the real display toggle on and off without editing your settings. It verifies that the synced-lyrics pane actually
opens before reporting active-lyrics results; missing lyrics or stopped playback leave this comparison incomplete.
These extra scenarios are informational, and CPU is reported as a percentage of one core. RAM means private working
set; the reported swipe peak is sampled at each end of a swipe, not a continuously traced maximum.

### The two files

- `scripts/perf.sh` is only a runner: it parses the options, builds, copies the executable to `run/` and starts the
  measuring program.
- `scripts/perf.rs` does the measuring. It is a Rust program (run with `cargo run --release --example perf`) that
  starts the notch, moves the pointer with `SetCursorPos`, and reads CPU time, handles and private memory from the
  process. A shell script cannot move the mouse or read those numbers, and an example is not part of the shipped
  `notch.exe`. The limits are constants at the top of the file.

Besides the notch's own settings file, the check writes only a copy of the executable under `run/` and a temporary
log.

## Manual checklist

For a change that affects drawing or input, run `scripts/reload.sh` and check:

- the collapsed pill is centered vertically and has a flat top and rounded bottom corners
- hovering opens it, and moving away closes it, every time
- play, pause, previous, next, shuffle and repeat work, and the hovered button gets a faint wash
- clicking and dragging the progress bar jumps to that point
- the lyric button hides and shows lyrics, and the choice is remembered after restarting
- the tray icon's Quit closes the notch and removes the icon

For a change to the notch's frame or hiding: open the notch and check the dots, the status marks and the time, tick "Hide when idle" in the tray menu and check that the notch
disappears, shows its small pill when you rest the pointer on the top of the screen, and opens if you stay. To check the "Edit pages" view, set `SETTINGS_BUTTON_SHOWN` to true in `chrome.rs` and
add, drag, double-click and remove a page. The mouse-driven checks need an unlocked screen.

For lock/unlock recovery, enable Hide when idle, focus a maximized app, lock Windows, then unlock. Rest the pointer
on the hidden strip and verify that it reveals and opens. Switch between two maximized apps, a restored window and
the desktop: Notch must remain above ordinary app windows and hover must still work. Fullscreen hiding must still
win. Automated regressions demote an owned test window and reset stale interaction state; simulated unlock messages
exercise the running app, but do not replace this physical lock/unlock check.

For a change to the timer, seed a short session by hand (stop the notch, put `timer.state=running focus 30000 <end>` in
the settings file, where `<end>` is the Unix time in milliseconds 30 seconds from now, and start it) and check: the ring
and the time show in the collapsed pill, the alert and sound arrive at the end, the open page then offers the break, and
pause, +5 min and stop work.

For a change to navigation, play something so the media page appears, then check: scrolling the wheel (and swiping a
touchpad, and scrolling sideways) moves one page per notch (several quick notches go several pages) and loops from the last page to the first, a
touchpad swipe drags the pages along and settles on the nearest one, clicking a dot opens that page, and the notch opens on the page you left.

For a change to the shelf: drop files on the notch (it turns blue and the files drop in), check the pictures match Explorer, double-click one to open it, drag one into a folder (it is copied, and leaves the shelf), right-click one and press its cross to remove it, and check the original files are untouched.

For a change to the calendar: with a calendar linked, click each day of the week (the events change, and today stays blue), and close and open the notch (today again).

For a change to activities, also check (each one is a real event, so run them on a laptop):

- maximize a browser: the notch shrinks to a strip, wakes at the very top edge after a short wait, and still shows the volume; press F11 (or play a video full screen): it disappears entirely until you leave full screen. `cargo test a_real_maximized -- --ignored` opens real windows to check the detection
- change the volume and press mute: a bar with the value, or "Muted", for about a second and a half
- change the brightness with the slider and with the brightness keys: a bar with the value
- copy something: "Copied"; copy a password from a password manager: nothing
- press Caps Lock: "Caps Lock on" and an arrow beside the time while the notch is open; press it again: "Caps Lock off"
- mute the speakers: "Muted", and a crossed speaker beside the time while the notch is open
- start recording in any app: "Microphone on", then an orange dot until it stops, then "Microphone off"
- open the camera in a browser or a video-call app: "Camera on", then a green dot, then "Camera off"
- plug and unplug the charger, and let the battery fall through 20 % and 10 %: the matching message
- receive a notification: a banner with the app's logo for about four seconds; point at the notch while it shows and it stays until you leave
- the activity section contains only Notifications; untick it and verify it stays off after restarting
- old disable settings for volume, brightness and other required activities do not suppress them
- Show notch over maximized windows keeps music without lyrics and the idle clock visible, even with Hide when idle on
- turning that override off restores lyrics-only visibility; fullscreen hiding remains independent
- open the tray menu with Windows in dark mode: the menu is dark; tick "Start with Windows" and check the entry under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
