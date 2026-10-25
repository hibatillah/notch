# Notch

A tiny notch for the top of your Windows screen, written in Rust. It sits at the top edge as a slim black pill, and opens when you hover over it. While music plays, it shows the track and scrolls the synced lyrics inline, line by line. It idles at about 15 MB of RAM and reaches about 35 MB with active content such as music and lyrics.

It draws straight to the screen with Direct2D: no UI framework, no web view. That keeps it small, fast, and at 0 % CPU while idle. Every page and every pill message is its own module, so you can switch them off, remove them, or write your own.

## What it includes

Pages (what you see when you open the notch):

- Clock
- Media player with synced lyrics
- Notifications
- File shelf
- Calendar
- Focus timer
- System stats (off until you add it)

Activities (what the collapsed pill shows while something happens):

- Volume
- Brightness
- Charger and battery
- Bluetooth devices
- Microphone in use
- Camera in use
- Copied to the clipboard

### Extensible

Nothing here is fixed. The gear in the open notch (**Edit pages**) adds, removes and reorders pages, and the tray menu switches activities on and off. To go further, write your own page or activity: each is one folder or file and one line in a registry, with no other code to touch. See [docs/adding-a-page.md](docs/adding-a-page.md) and [docs/adding-an-activity.md](docs/adding-an-activity.md).

### Limitations

- Windows only, on the primary display, centered at the top. Multiple monitors, display changes while running and fullscreen apps get no special handling. The display scale is read once at start-up.
- Microphone and camera show that an app has the device **open**, not that it is recording. An app that keeps the microphone open while you are muted (Google Meet in a browser) keeps the orange dot. The microphone follows the default recording device at start-up. The camera is found through the Windows Frame Server log, so it misses some older DirectShow apps and a camera already on at start-up. This was built on Windows 11 build 26300; other versions are untested.
- Notifications show a few seconds after Windows shows its own banner.
- Calendar reads an iCal address, not a Google sign-in. A daylight-saving event can show an hour off.
- The file shelf takes drops but cannot drag files back out; open one, or right-click it for more.
- Brightness follows the laptop panel only.
- Version 0.1: expect rough edges.

## How to use it

- **Hover** over the pill to open it; move away to close it.
- **Pages:** scroll the wheel (or swipe two fingers on a touchpad) or click a dot. Pages loop, and the notch opens on the page you left. The gear opens **Edit pages**: drag to reorder, double-click to remove, click one under "Available" to add.
- **Media:** click the buttons; the speech-bubble button turns lyrics on and off.
- **Calendar:** copy your calendar's iCal address, then right-click the tray icon and choose **Use calendar link from clipboard**. For Google Calendar, use "Secret address in iCal format" and treat it like a password.
- **Tray menu** (right-click the tray icon or the notch): switch activities on and off, set the calendar link, choose what notifications show, **Hide when idle**, and **Quit**. On Windows 11 the icon may sit under the `^` overflow menu.

## Device requirements

- Windows 10 or 11, 64-bit (tested on Windows 11)
- For the media page: a player that appears in the Windows media controls
- Internet only for lyrics and a calendar link (see [Privacy](#privacy))

## Download

Get `notch.exe` from the [latest release](https://github.com/hibatillah/notch/releases/latest), unzip it and run it. There is no installer. Or clone the repository and build it yourself; see [Development](#development).

## Contributing

This project does not take contributions: pull requests and issues may be ignored. Clone or fork it and change your copy. [docs/architecture.md](docs/architecture.md) explains how the code is organized.

## Development

Requirements for building (running a built `notch.exe` needs only Windows 10 or 11):

- [Rust](https://rustup.rs) 1.88 or newer (tested with 1.97)
- Visual Studio Build Tools with the **Desktop development with C++** workload (MSVC linker and Windows SDK)
- Git Bash, only for the `scripts/*.sh` helpers

Build and run:

```
git clone https://github.com/hibatillah/notch.git
cd notch
cargo build --release
target\x86_64-pc-windows-msvc\release\notch.exe
```

While developing, run `scripts/reload.sh` from Git Bash. It builds, starts the new build and closes the previous one without a gap (a running `.exe` cannot be overwritten on Windows).

Before finishing a change:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
scripts/perf.sh
```

Tests sit at the bottom of the source file they test. `scripts/perf.sh` measures the memory and CPU of a running notch.

| Doc                                            | What it covers                                                                      |
| ---------------------------------------------- | ----------------------------------------------------------------------------------- |
| [docs/development.md](docs/development.md)     | requirements, build, run, development loop, checks, code style, dependency licenses |
| [docs/testing.md](docs/testing.md)             | where the tests live, how to write one, the performance check                       |
| [docs/architecture.md](docs/architecture.md)   | how the code is organized                                                           |
| [docs/adding-a-page.md](docs/adding-a-page.md) | write, register and remove a page                                                   |
| [docs/adding-an-activity.md](docs/adding-an-activity.md) | write, register and remove an activity                                    |
| [docs/configuration.md](docs/configuration.md) | which pages and activities are on, settings file, environment variables             |
| [docs/releasing.md](docs/releasing.md)         | the CI and release workflows, and how to publish a release                          |

For an AI coding agent, [CLAUDE.md](CLAUDE.md) and `.claude/rules/` hold the project rules.

## Resource use

Measured by `scripts/perf.sh` on the author's machine (Windows 11, 60 Hz display):

|                                              | Result                                 |
| -------------------------------------------- | -------------------------------------- |
| Clock page, collapsed and idle               | 0 % CPU, about 15 MB of private memory |
| One open-and-close                           | about 80 ms of CPU                     |
| Memory growth after 10 open-and-close cycles | under 1 MB                             |
| Playing music with synced lyrics             | up to about 35 MB of private memory    |

Run `scripts/perf.sh` to measure your own machine. See [docs/testing.md](docs/testing.md).

## Privacy

Notch is a local program. You download it and it runs on your computer, with no account and no analytics. It reads what is playing, your notifications, the clipboard state and your shelf files only on your machine, and never reads what you copy. Settings stay in `%APPDATA%\notch\settings.txt`.

It connects out for two things only:

- **Lyrics:** when a track starts, it asks [LRCLIB](https://lrclib.net) for synced lyrics, sending the artist, title, album and length. The player itself (Spotify, a browser) is only read through Windows, never contacted. Set `NOTCH_LYRICS=off` to stop lookups; the lyrics button only hides them.
- **Calendar:** if you set a calendar link (for example Google Calendar's iCal address), it downloads that file every 15 minutes. The link sits in the settings file, so treat that file like a password.

## License

[MIT](LICENSE)
