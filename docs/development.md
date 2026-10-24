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
Right-click the tray icon, or the notch, and choose **Quit**.

## Development loop

A running `.exe` is locked by Windows, so you cannot rebuild over it. `scripts/reload.sh` (run from Git Bash) works
around that:

```
scripts/reload.sh
```

It builds, copies the build to `run/notch-<time>.exe`, starts that copy, and asks the previous instance to close once
the new one is showing. The notch never disappears between versions. `run/` is git-ignored.

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
