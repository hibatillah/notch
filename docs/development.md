# Development

How to build, run, test and change the notch.

## Requirements for building

These are needed only to build from source. To run an already-built `notch.exe` you need just Windows 10 or 11.

- Windows 10 or 11 (developed and tested on Windows 11; Windows 10 is untested)
- [Rust](https://rustup.rs) 1.88 or newer (tested with 1.97); `Cargo.toml` sets `rust-version = "1.88"`
- Visual Studio Build Tools with the **Desktop development with C++** workload, which provides the MSVC linker and
  the Windows SDK
- [Git for Windows](https://git-scm.com/download/win), only for the `.sh` scripts (`scripts/reload.sh`,
  `scripts/watch.sh`, `scripts/perf.sh`), which run in Git Bash
- [Inno Setup](https://jrsoftware.org/isinfo.php), only to build the installer by hand; the release workflow installs it

`.cargo/config.toml` makes `x86_64-pc-windows-msvc` the default target. If your Rust install uses the GNU toolchain,
add the target once:

```
rustup target add x86_64-pc-windows-msvc
```

The settings window (`settings/`, GPUI) builds only with the MSVC host toolchain: install it with
`rustup toolchain install stable-x86_64-pc-windows-msvc` and run its commands as `cargo +stable-x86_64-pc-windows-msvc`.

## Build

```
git clone https://github.com/hibatillah/notch.git
cd notch
cargo build --release
```

The executable is `target\x86_64-pc-windows-msvc\release\notch.exe`. The workspace's default member is the notch, so
this does not build the settings window; `cargo +stable-x86_64-pc-windows-msvc build --release -p notch-settings` builds
`notch-settings.exe` beside it, and `--workspace` builds both. A debug build (`cargo build`) works too, but
animation is noticeably slower, so use release for anything you look at.

## App icon

`assets/branding/logo.png` is the orange master logo; the shaded black and white variants live beside it.
`assets/branding/logo.ico` is the Windows export, with transparent 16, 20, 24, 32, 40, 48, 64, 96, 128 and 256 px frames.
After changing the master, regenerate the export from Git Bash with `scripts/export-icon.sh` (requires `uv`; Pillow
is used only for this export, not by the app or normal builds).

`build.rs` uses the Windows SDK resource compiler to embed `logo.rc` and `logo.ico` in the executable. The window and
tray load this same resource at the current display scale. The installer uses the same ICO, and its Start menu
shortcut and installed-app entry use the executable's embedded icon. No image files are needed beside the installed
executable. Rebuild and restart Notch to see an icon change; updating an installed copy requires the newer installer.
Windows may cache an old shortcut icon until its icon cache refreshes.

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
release) every time a file in `src/`, `store/` or `Cargo.toml` changes (it rebuilds the notch only, not `settings/`), and checks for saved files every 2 s. A debug build is
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

The checks above cover only the notch; add `--workspace` (with the MSVC toolchain) to include `store/` and `settings/`.
CI runs the first three on every push to the default branch ([releasing.md](releasing.md)); run them yourself first. Where the tests live and what they cover: [testing.md](testing.md).
`scripts/perf.sh` moves the mouse for about two minutes, so leave the mouse alone while it runs.

## The notch layout

The host owns the shape, the padding and the header. A page draws only the content area inside them, and all sizes
are in device-independent pixels (multiply by the display scale when drawing). The numbers are constants in
`src/host/chrome.rs`; the time is drawn by `src/host/clock.rs`.

**Open (pointer over the notch)**, here a page whose content is 300 px wide (the height is always 86):

```
      flare                                                flare
         \                                                  /
          \________________________________________________/     top edge of the screen
          |                                                |
          |  10 px padding top                             |
          |  [== o o o]               (o) |||  18:41       |     header, 16 px high
          |   page dots (left)     status marks  time      |     marks and time sit at the right edge
          |  10 px gap                                     |
          |  +------------------------------------------+  |
          |  |                                          |  |
    16 px |  |  THE PAGE: only this area is its own     |  | 16 px
          |  |  width  = PageSizes.expanded.width       |  |
          |  |  height = 86 (chrome::CONTENT_HEIGHT)    |  |
          |  |  drawn in Frame::bounds                  |  |
          |  +------------------------------------------+  |
          |  14 px padding bottom                          |
          \________________________________________________/

    pill width  = page width + 32     (16 + 16)
    pill height = 86 + 50 = 136       (10 + 16 + 10 + 86 + 14)
```

- The header belongs to the host, on every page: the page dots (the long one is the open page) on the left, and at the
  right the status marks (the music wave, the timer ring, muted speakers, Caps Lock) and the time. The settings button
  that opens "Edit pages" is switched off (`chrome::SETTINGS_BUTTON_SHOWN`).
- Every page has the same 86 px of content height, whatever it asks for. Content that is taller is clipped, so lay a
  page out for 86 px (the stats tiles and the calendar do).
- A page cannot draw into the header or the padding.

**Collapsed**, the pill is 33 px high (`RESTING_HEIGHT`, plus 3 px of bottom padding) and at least 150 px wide
(`RESTING_MIN_WIDTH`), and one of four things fills it:

```
  nothing happening     music playing        activity or timer      notification banner
     _________        ____________________    __________________    ____________________
    /  18:41  \      / (page draws it) [o] \  / (activity draws) \  | [D] Rafi - Discord  |
    \_________/      \____________________/  \__________________/  |  Deploy is green... |
    host: the time    the media page, plus    a transient takes      \____________________/
    in a 150 px pill  an optional badge       the pill for a moment  an activity opens the pill
                      at the right            (volume, Caps Lock)    into a card, 48 to 70 px high (400 px wide)
```

- A page answers `priority(Mode::Resting)` to claim the collapsed pill; with no answer the host shows the time.
- An activity or a page can add a **badge** at the right end (the page draws to the left of it), a **transient** that
  replaces the pill's content for a moment, or, for an activity, a **banner** that opens the pill into a card
  (`presence()`; see [adding-an-activity.md](adding-an-activity.md)).
- The time is one element: it glides from the middle of the collapsed pill to the right end of the header as the notch
  opens, and stays visible the whole way.
- By default, over a maximized window the pill hides to a strip unless lyrics play, and over a full-screen window the
  notch is off the screen; see
  [architecture.md](architecture.md#hiding).

**Where to change what:**

| To change | Edit |
| --- | --- |
| padding, header height, the gap, the dots, the content height | `src/host/chrome.rs` |
| the time and the collapsed pill's minimum width | `src/host/clock.rs` |
| which page shows, swiping, hover, the springs, hiding | `src/host/notch.rs` |
| maximized and full-screen detection | `src/host/screen_mode.rs` |
| the tray menu | `src/host/tray.rs` (the items), `src/host/window.rs` (what they do) |
| the settings window | `settings/src/sections/` (one file per section), `settings/src/field.rs` (text fields); `docs/configuration.md` |
| start with Windows | `src/host/startup.rs` (the tray item), `installer/notch.iss` (the installer's choice) |
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

The project is MIT licensed (see `LICENSE`). Every dependency of `notch.exe`, including build-time ones, is licensed
`MIT OR Apache-2.0` (the `windows-*` crates, `proc-macro2`, `quote`, `syn`); `unicode-ident` is
`(MIT OR Apache-2.0) AND Unicode-3.0`. `settings/` adds GPUI and its dependency tree. In the spike's `cargo license` run (441 packages) most are MIT or
Apache-2.0 or MIT, 14 are Apache-2.0 only (`gpui` and its `gpui_*` crates among them), and no GPL, LGPL, AGPL or MPL
crate is in the Windows build ([spike](superpowers/spikes/2026-10-30-gpui-spike.md#licenses)). All are permissive and
compatible with MIT. The license texts ship in `THIRD-PARTY-LICENSES.txt`
([releasing.md](releasing.md#third-party-licenses)). Re-check after changing `Cargo.lock`:

```
cargo install cargo-license --locked    # once
cargo license
```

This groups the packages by license. Do not add a dependency with a copyleft license (GPL, LGPL, AGPL, MPL)
without checking what it requires. The notch is small on purpose, so prefer not to add dependencies at all.

## Next

- Write a new page: [adding-a-page.md](adding-a-page.md)
- How the code fits together: [architecture.md](architecture.md)
