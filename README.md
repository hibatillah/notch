# Notch

A small "dynamic island" for the top of your Windows screen, written in Rust. It sits at the top edge as a slim
black pill that shows what is happening (a volume change, the microphone turning on, the music playing), and opens
when you hover over it to show a page: a clock, the media player, a focus timer, a file shelf, a calendar, your
notifications or system stats. It draws directly with Direct2D, with no UI framework and no web view, so it stays
small and uses no CPU while idle.

## What it includes

Everything below is a separate module, so the list is **extensible**: switch any of it off, remove it, or add your own
page or activity. In the notch, the gear opens **Edit pages** to add, remove and reorder pages; the tray menu switches
activities on and off. To write a new one, see [docs/adding-a-page.md](docs/adding-a-page.md) and
[docs/adding-an-activity.md](docs/adding-an-activity.md).

**Pages** (what you see when you open the notch):

- Clock
- Media player with synced lyrics
- Notifications
- File shelf
- Calendar
- Focus timer
- System stats (off until you add it)

**Activities** (what the collapsed pill shows while something happens):

- Volume
- Brightness
- Charger and battery
- Bluetooth devices
- Microphone in use
- Camera in use
- Copied to the clipboard

Limits of the microphone and camera activities:

- They show that an app has the device **open**, not that it is recording or sending. An app that keeps the device
  open while you are muted (Google Meet in a browser keeps the microphone open) keeps the orange dot.
- The microphone follows the default recording device at the time the notch starts.
- The camera is found through the Windows Frame Server log, so it misses apps that reach the camera another way (some
  older DirectShow programs) and a camera that is already on when the notch starts. This relies on how Windows 11
  build 26300 behaves; other Windows versions are untested.

Other limits:

- **Notifications** appear in the notch a few seconds after Windows shows its own banner (reading the list takes a
  few seconds when many notifications are waiting), and the notch adds to the Windows notification center without
  replacing it.
- **Calendar** reads an iCal address (for Google Calendar, "Secret address in iCal format" in the calendar's
  settings), not a Google sign-in. Treat that address like a password. An event in a time zone with daylight saving
  can show an hour off while it is in effect.
- **File shelf** takes files dropped on the notch, but a file cannot be dragged out of it; open it, or right-click for
  Show in folder or Copy path.
- **Brightness** follows the laptop panel only.

## Device requirements

- Windows 10 or 11, 64-bit (developed and tested on Windows 11; Windows 10 is untested)
- For the media page: a player that shows up in the Windows media controls
- Internet access only for lyrics and for a calendar address (see [Privacy](#privacy))

It is a single `notch.exe` with no installer. Typical use on the author's machine: 0 % CPU when idle and about
19 MB of memory with the clock page, about 33 MB while playing music with lyrics (see [Resource use](#resource-use)).

## Download

Download the zip from the repository's Releases page, unzip it and run the executable. Releases are built by GitHub
Actions when a version tag is pushed ([docs/releasing.md](docs/releasing.md)). To build it yourself, see
[Development](#development).

## Contributing

This project does not take contributions: pull requests and issues may be ignored. To change or extend it, clone or
fork the repository and customize your copy. [docs/adding-a-page.md](docs/adding-a-page.md) shows how to add your
own page, and [docs/architecture.md](docs/architecture.md) explains how the code is organized.

## Using it

- **Hover** over the pill at the top of the screen to open it; move away to close it.
- **Pages:** scroll the mouse wheel (or swipe two fingers on a touchpad) or click a dot to move between pages. The
  notch opens on the page you left. Click the gear to add, remove and reorder pages: drag a page to move it,
  double-click to remove it, click one under "Available" to add it.
- **Media page:** click the buttons; the speech-bubble button turns the lyrics on and off, and your choice is
  remembered.
- **Calendar:** copy your calendar's iCal address, then right-click the tray icon and choose **Use calendar link from
  clipboard**.
- **Tray menu** (right-click the tray icon or the notch): switch activities on and off, set the calendar link, choose
  what notifications show, **Hide when idle** (the notch is invisible until you point at the top of the screen, then
  shows a small pill and opens if you stay), and **Quit**. On Windows 11 the tray icon may be under the `^` overflow
  menu.

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

While developing, run `scripts/reload.sh` from Git Bash. It builds, starts the new build and closes the previous one
without a gap (a running `.exe` cannot be overwritten on Windows).

Before finishing a change:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
scripts/perf.sh
```

Tests sit at the bottom of the source file they test. `scripts/perf.sh` measures the memory and CPU of a running
notch.

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
| Clock page, collapsed and idle               | 0 % CPU, about 19 MB of private memory |
| One open-and-close                           | about 80 ms of CPU                     |
| Memory growth after 10 open-and-close cycles | under 1 MB                             |
| Playing music with synced lyrics             | about 33 MB of private memory          |

Run `scripts/perf.sh` to measure your own machine. See [docs/testing.md](docs/testing.md).

## Privacy

Nothing is sent anywhere except lyric lookups and the download of the calendar address you give it. When a new track
starts, the media page asks [LRCLIB](https://lrclib.net) for synced lyrics, which sends the track's artist, title,
album and length to that service. If you set a calendar address, the calendar page downloads that file from it
every 15 minutes. There are no accounts, no analytics and no other network use. Notifications, the clipboard and the
files on the shelf are only read on your computer; the notch never reads what you copy. Settings stay on your computer
in `%APPDATA%\notch\settings.txt`, which also holds the calendar address; treat it like a password.

To turn lookups off, set the environment variable `NOTCH_LYRICS=off`. The lyrics button in the notch only hides
lyrics; it does not stop the lookup.

## Limitations

- Windows only.
- Shown on the primary display, centered at the top. Multiple monitors, display changes while running, and
  fullscreen apps are not handled specially.
- The display scale is read once at start-up.
- Version 0.1: expect rough edges.

## License

[MIT](LICENSE). Every dependency is `MIT OR Apache-2.0` licensed; see
[docs/development.md](docs/development.md#dependencies-and-licenses).
