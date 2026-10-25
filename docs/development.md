# Development

How to build, run, test and change the notch.

## Requirements for building

These are needed only to build from source. To run an already-built `notch.exe` you need just Windows 10 or 11.

- Windows 10 or 11 (developed and tested on Windows 11; Windows 10 is untested)
- [Rust](https://rustup.rs) 1.88 or newer (tested with 1.97); `Cargo.toml` sets `rust-version = "1.88"`
- Visual Studio Build Tools with the **Desktop development with C++** workload, which provides the MSVC linker and
  the Windows SDK
- [Git for Windows](https://git-scm.com/download/win), only for the `.sh` scripts (`scripts/reload.sh`,
  `scripts/perf.sh`), which run in Git Bash

`.cargo/config.toml` makes `x86_64-pc-windows-msvc` the default target. If your Rust install uses the GNU toolchain,
add the target once:

```
rustup target add x86_64-pc-windows-msvc
```

## Build

```
git clone https://github.com/hibatillah/notch.git
cd notch
cargo build --release
```

The executable is `target\x86_64-pc-windows-msvc\release\notch.exe`. A debug build (`cargo build`) works too, but
animation is noticeably slower, so use release for anything you look at.

## Run

```
target\x86_64-pc-windows-msvc\release\notch.exe
```

The notch has no window to close. It adds a tray icon (on Windows 11 it may be under the `^` overflow menu).
Right-click the tray icon and choose **Quit**.

## Development loop

A running `.exe` is locked by Windows, so you cannot rebuild over it. `scripts/reload.sh` (run from Git Bash) works
around that:

```
scripts/reload.sh
```

It builds, copies the build to `run/notch-<time>.exe`, starts that copy, and asks the previous instance to close once
the new one is showing. The notch never disappears between versions. `run/` is git-ignored.

To see a change as soon as you save it, leave this running in a terminal:

```
scripts/watch.sh
```

It does the same with a debug build (`scripts/reload.sh --debug`, about 2 s after a small edit, against about 20 s for
release) every time a file in `src/` or `Cargo.toml` changes, and checks for saved files every 2 s. A debug build is
slower and larger, so use the release build and `scripts/perf.sh` for performance numbers. If a build fails, the
error shows and the previous copy keeps running until the next save.

To see what the notch is doing, write a timestamped log:

```
NOTCH_DEBUG=C:\path\to\notch.log scripts/reload.sh
```

All settings and environment variables are in [configuration.md](configuration.md).

## Checks

Run these before considering a change finished, in this order:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
scripts/perf.sh          # when you touched the host, animation, timers or drawing
```

CI runs the first three on every push to the default branch ([releasing.md](releasing.md)); run them yourself first. Where the tests live and what they cover: [testing.md](testing.md).
`scripts/perf.sh` moves the mouse for about two minutes, so leave the mouse alone while it runs.

## The notch layout

The host owns the shape, the padding and the header. A page draws only the content area inside them, and all sizes
are in device-independent pixels (multiply by the display scale when drawing). The numbers are constants in
`src/host/chrome.rs`; the time is drawn by `src/host/clock.rs`.

**Open (pointer over the notch)**, here a page whose content is 300 x 80:

```
      flare                                              flare
         \                                                /
          \______________________________________________/     top edge of the screen
          |                                              |
          |  10 px padding top                           |
          |  [== o o o o o]        18:41            [=]  |     header, 16 px high
          |   page dots (left)   time (middle)  settings |     the time is centered on the notch
          |  10 px gap                                   |
          |  +----------------------------------------+  |
          |  |                                        |  |
    16 px |  |  THE PAGE: only this area is its own   |  | 16 px
          |  |  width  = PageSizes.expanded.width     |  |
          |  |  height = PageSizes.expanded.height    |  |
          |  |  drawn in Frame::bounds                |  |
          |  +----------------------------------------+  |
          |  14 px padding bottom                        |
          \______________________________________________/

    pill width  = page width  + 32      (16 + 16)
    pill height = page height + 50      (10 + 16 + 10 + 14)
```

- The header belongs to the host, on every page: the page dots (the long one is the open page) on the left, the time
  centered, and the settings button (it opens "Edit pages") on the right.
- The window is 260 px high, so a page's content is at most about 200 px high.
- A page cannot draw into the header or the padding.

**Collapsed**, the pill is 30 px high (plus 3 px of bottom padding) and one of three things fills it:

```
  nothing happening            music playing               activity or timer
      _________                ______________________       ___________________
     /  18:41  \              / (page draws it)  [o] \     /  (activity draws) \
     \_________/              \______________________/     \___________________/
     host: the time           the media page, plus an      a transient takes the pill for
     in a 96 px pill          optional badge at the right  a moment (volume, charging, ...)
```

- A page answers `priority(Mode::Resting)` to claim the collapsed pill; with no answer the host shows the time.
- An activity or a page can add a **badge** at the right end (the page draws to the left of it) or a **transient**
  that replaces the pill's content for a moment (`presence()`; see [adding-an-activity.md](adding-an-activity.md)).
- The time is one element: it glides from the middle of the collapsed pill to the middle of the header as the notch
  opens, and stays visible the whole way.

**Where to change what:**

| To change | Edit |
| --- | --- |
| padding, header height, the gap, the dots, the settings button | `src/host/chrome.rs` |
| the time and the collapsed pill's minimum width | `src/host/clock.rs` |
| which page shows, swiping, hover, the springs | `src/host/notch.rs` |
| a page's content and size | that page's folder in `src/pages/` |
| register a page | `src/pages/mod.rs` |
| register an activity | `src/activities/mod.rs` |

## Code style

`cargo fmt` formats the code (see `rustfmt.toml`). Beyond that:

- Use full, descriptive names: `present_track_ids`, not `ptids`.
- Comments are one line where possible and explain why, not what. Longer explanations are doc comments.
- Sizes in layout code are in device-independent pixels, and converted with the display scale when drawing.
- Keep `unsafe` to the smallest block that needs it, next to the Windows call it wraps.
- Put a page's pure logic (parsing, layout rules, state machines) in functions that can be tested without a window.
- Keep a page inside its own folder; pages never use each other.

## Dependencies and licenses

The project is MIT licensed (see `LICENSE`). Every dependency, including build-time ones, is licensed
`MIT OR Apache-2.0` (the `windows-*` crates, `proc-macro2`, `quote`, `syn`); `unicode-ident` is
`(MIT OR Apache-2.0) AND Unicode-3.0`. All are permissive and compatible with MIT. Re-check after changing
`Cargo.lock`:

```
cargo install cargo-license --locked    # once
cargo license
```

This groups the packages by license. Do not add a dependency with a copyleft license (GPL, LGPL, AGPL, MPL)
without checking what it requires. The notch is small on purpose, so prefer not to add dependencies at all.

## Next

- Write a new page: [adding-a-page.md](adding-a-page.md)
- How the code fits together: [architecture.md](architecture.md)
