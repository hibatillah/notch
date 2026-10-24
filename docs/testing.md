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
- layout of the open view, and the progress bar and button hit areas
- the lyric follower, which tracks the current line and arms timers

What they do not cover is the drawing itself and the Windows integration (the window, the tray, the media session).
Check those by running the notch: see the manual checklist below.

### Where the tests live

Each test sits in the file it tests, in a `#[cfg(test)] mod tests` block at the bottom. There is no separate `tests/`
folder, because the notch is a binary crate and an outside test folder could not reach its internals. The tests are
spread over many files:

| Area | Files with tests |
| --- | --- |
| page contract | `src/page.rs` |
| host | `src/host/notch.rs` (page choice), `registry.rs`, `settings.rs` |
| drawing toolkit | `src/gfx/color.rs`, `geometry.rs`, `spring.rs`, `image.rs`, `path.rs`, `raster.rs`, `canvas.rs` (flare shape) |
| widgets | `src/ui/icon_button.rs`, `icons.rs`, `progress_bar.rs` |
| clock page | `src/pages/clock/mod.rs` |
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
```

The script starts a copy of the notch with only the clock page (`NOTCH_PAGES=clock`), so the result does not depend
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

A healthy build is far under every limit (typically 0 % idle CPU, 19 MB, 80 ms per cycle). The limits are set in
constants at the top of `examples/perf.rs`; they exist to catch regressions, so fix the cause rather than raising
one.

`--with-media` prints memory and CPU with every page on and your music playing. It has no limits, because the result
depends on the track and on how often its lyrics change.

### The two files

- `scripts/perf.sh` is only a runner: it parses the options, builds, copies the executable to `run/` and starts the
  measuring program.
- `examples/perf.rs` does the measuring. It is a Rust program (run with `cargo run --release --example perf`) that
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
