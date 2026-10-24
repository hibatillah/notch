# Notch

A small "dynamic island" for the top of your Windows screen, written in Rust. It sits at the top edge as a slim
black pill, and opens when you hover over it to show a page: a clock, or the media player with album art, controls
and synced lyrics. It draws directly with Direct2D, with no UI framework and no web view, so it stays small and uses
no CPU while idle.

## What it includes

Three pages out of the box. Pages are separate modules, so you can add your own or remove these.

| Page      | Collapsed                                                 | Open                                                                                                                    |
| --------- | --------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| **Clock** | the time                                                  | the time and date                                                                                                       |
| **Media** | the app icon, title and artist, or the current lyric line | album art, title, artist, seek bar, time left, play/pause, previous, next, shuffle, repeat, and scrolling synced lyrics |
| **Timer** | while running: a progress ring and the time left, beside the other page | focus timer: the time, pause, +5 min and stop; start, and the length of a session |

The timer runs focus sessions (25 minutes by default, four to a round) and breaks (5 minutes, 15 after the fourth
session). A ring and the minutes left show in the collapsed pill (seconds in the last ten minutes), and when a phase
ends the notch shows an alert and plays the Windows notification sound; open the notch to start the break or skip it.
The timer keeps real time, so it survives restarting the notch. Break lengths can be set by hand in the settings file
(see [docs/configuration.md](docs/configuration.md)).

The media page works with any player that reports to the Windows media controls (Spotify, browsers, and so on). It
has been tested with Spotify. A tray icon lets you quit.

Activities show up in the collapsed pill while something happens, and each can be switched off from the tray menu:

| Activity              | Shows                                                           |
| --------------------- | --------------------------------------------------------------- |
| **Volume**            | a bar with the value, or "Muted"                                |
| **Brightness**        | a bar with the value (laptop screens)                           |
| **Charger / battery** | "Charging", "On battery", and "Low battery" at 20 % and 10 %    |
| **Microphone**        | "Microphone on", then an orange dot while an app has it open    |
| **Camera**            | "Camera on", then a green dot while an app has it open          |
| **Copied**            | "Copied" when something is copied (never reads what you copied) |

Limits of the microphone and camera activities:

- They show that an app has the device **open**, not that it is recording or sending. An app that keeps the device
  open while you are muted (Google Meet in a browser keeps the microphone open) keeps the orange dot.
- The microphone follows the default recording device at the time the notch starts.
- The camera is found through the Windows Frame Server log, so it misses apps that reach the camera another way (some
  older DirectShow programs) and a camera that is already on when the notch starts. This relies on how Windows 11
  build 26300 behaves; other Windows versions are untested.

## Device requirements

- Windows 10 or 11, 64-bit (developed and tested on Windows 11; Windows 10 is untested)
- For the media page: a player that shows up in the Windows media controls
- Internet access only if you want lyrics (see [Privacy](#privacy))

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
- **Pages:** when more than one page has something to show, dots appear at the bottom of the open notch. Scroll the
  mouse wheel (or swipe two fingers on a touchpad) or click a dot to move between them; the notch opens on the page you
  left.
- **Media page:** click the buttons, or click or drag the progress bar to jump to a point in the song. The
  speech-bubble button turns the lyrics on and off, and your choice is remembered.
- **Quit:** right-click the tray icon (on Windows 11 it may be under the `^` overflow menu) or the notch, and choose
  **Quit**.

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

Nothing is sent anywhere except lyric lookups. When a new track starts, the media page asks
[LRCLIB](https://lrclib.net) for synced lyrics, which sends the track's artist, title, album and length to that
service. There are no accounts, no analytics and no other network use. Settings stay on your computer in
`%APPDATA%\notch\settings.txt`.

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
