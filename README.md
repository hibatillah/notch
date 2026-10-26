# Notch

A tiny notch for the top of your Windows screen, written in Rust. It sits at the top edge as a slim black pill, and opens when you hover over it. While music plays, it shows the track and scrolls the synced lyrics inline, line by line. It also keeps a file shelf, a calendar, a focus timer and system stats, and shows short messages (volume, notifications, Caps Lock) in the pill. It uses no CPU while idle.

It draws straight to the screen with Direct2D: no UI framework, no web view. That keeps it small, fast, and at 0 % CPU while idle. Every page and every pill message is its own module, so you can switch them off, remove them, or write your own.

![The open notch on the media page, playing a song with its synced lyrics beside the controls](docs/images/notch-lyrics.png)

## What it includes

Pages (what you see when you open the notch; the time sits in the header of every one):

- Media player with synced lyrics
- File shelf: drop files to keep them for a while
- Calendar: the week and the events of a day you pick
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
- Caps Lock on and off
- Windows notifications: the pill opens into a banner with the app's logo for a few seconds, between its idle and open sizes; point at it to keep it on screen

### Maximized and full-screen windows

Over a **maximized** window the notch hides to a thin strip at the very top edge of the screen (like **Hide when idle**), and wakes when the pointer waits there a little. Volume, notifications and the other short messages still show. The lyric line stays if you tick **Show lyrics over maximized windows**. Over a **full-screen** window (a game, a video, F11 in a browser) the notch is not on screen at all, and nothing shows until you leave it. **Hide for full-screen apps** in the tray menu turns both off.

### Status marks

While the notch is open, small marks beside the time show what is going on: the wave while music plays, a ring while a focus timer runs, a crossed speaker while the sound is muted, and an arrow while Caps Lock is on.

### Extensible

Nothing here is fixed. The tray menu switches activities on and off, and the settings file orders and removes pages (the **Edit pages** view that does it in the notch is switched off for now; see [docs/configuration.md](docs/configuration.md)). To go further, write your own page or activity: each is one folder or file and one line in a registry, with no other code to touch. See [docs/adding-a-page.md](docs/adding-a-page.md) and [docs/adding-an-activity.md](docs/adding-an-activity.md).

### Limitations

- Windows only, on the primary display, centered at the top. Multiple monitors and display changes while running get no special handling; only the window in front on the primary display is looked at. The display scale is read once at start-up.
- Microphone and camera show that an app has the device **open**, not that it is recording. An app that keeps the microphone open while you are muted (Google Meet in a browser) keeps the orange dot. The microphone follows the default recording device at start-up. The camera is found through the Windows Frame Server log, so it misses some older DirectShow apps and a camera already on at start-up. This was built on Windows 11 build 26300; other versions are untested.
- Notifications show a few seconds after Windows shows their own banner, and only as a banner: there is no list of past ones, and the banner has no buttons (the buttons of the notification itself are not available to it). The text of a message can be left out with `notifications.show_text=off` in the settings file.
- Calendar reads an iCal address, not a Google sign-in. A daylight-saving event can show an hour off.
- The file shelf keeps the paths of files, not copies. Dragging a file out copies it and never moves or deletes the original.
- The open notch is 86 px high for every page, so the calendar lists two events at a time.
- Brightness follows the laptop panel only.
- Version 0.1: expect rough edges.

## How to use it

- **Hover** over the pill to open it; move away to close it.
- **Pages:** scroll the wheel (or swipe two fingers on a touchpad) or click a dot. Pages loop, and the notch opens on the page you left.
- **Media:** click the buttons; the speech-bubble button turns lyrics on and off.
- **Shelf:** drag files onto the notch (it turns blue when it is ready). Double-click a file to open it, drag it out to take it off the shelf into another app, and right-click it or press the cross on its corner to remove it.
- **Calendar:** click a day of the week to list its events.
- **Calendar:** copy your calendar's iCal address, then right-click the tray icon and choose **Use calendar link from clipboard**. For Google Calendar, use "Secret address in iCal format" and treat it like a password.
- **Tray menu** (right-click the tray icon; the notch itself has no menu): switch activities on and off, set the calendar link, **Hide when idle**, **Hide for full-screen apps**, **Show lyrics over maximized windows**, **Start with Windows**, and **Quit**. On Windows 11 the icon may sit under the `^` overflow menu.

## Device requirements

- Windows 10 or 11, 64-bit (tested on Windows 11)
- For the media page: a player that appears in the Windows media controls
- Internet only for lyrics and a calendar link (see [Privacy](#privacy))

## Download

Download `notch-<version>-windows-x64-setup.exe` from the [latest release](https://github.com/hibatillah/notch/releases/latest) and run it. The installer is not code-signed, so Windows may show a SmartScreen warning. The notch starts with Windows unless you untick that in the installer or in the tray menu (**Start with Windows**). Or clone the repository and build it yourself; see [Development](#development).

### Updating

Notch has no automatic update. It does not look for new versions, because the only things it connects to the internet for are lyrics and a calendar link you set yourself (see [Privacy](#privacy)). To update, download the newer `-setup.exe` from the [releases](https://github.com/hibatillah/notch/releases) page and run it. The installer sees that Notch is already installed, says which version is installed and which one it would put in its place, and asks whether to continue. If you say yes it closes the running notch, replaces it in place and starts it again. You do not uninstall the old version first, and your settings stay.

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

While developing, run `scripts/watch.sh` from Git Bash to rebuild and relaunch on every save, or `scripts/reload.sh` for a single release build. It builds, starts the new build and closes the previous one without a gap (a running `.exe` cannot be overwritten on Windows).

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

Measured by `scripts/perf.sh` on the author's machine (Windows 11, 60 Hz display), on an earlier build: the shelf previews, the Caps Lock hook, the window hooks and the new layout came after, so run it for your own numbers:

|                                              | Result                                 |
| -------------------------------------------- | -------------------------------------- |
| Collapsed and idle                           | 0 % CPU, about 15 MB of private memory |
| One open-and-close                           | about 80 ms of CPU                     |
| Memory growth after 10 open-and-close cycles | under 1 MB                             |
| Playing music with synced lyrics             | up to about 35 MB of private memory    |

Run `scripts/perf.sh` to measure your own machine. See [docs/testing.md](docs/testing.md).

## Privacy

Notch is a local program. You download it and it runs on your computer, with no account and no analytics. It reads what is playing, your notifications (their text and app), the clipboard state and your shelf files only on your machine, and never reads what you copy. It sees which window is in front only to tell whether it is maximized or full screen. For Caps Lock it installs a keyboard hook that looks at that one key and keeps nothing it sees. Settings stay in `%APPDATA%\notch\settings.txt`, and a "Start with Windows" entry is added under your user's `Run` key unless you untick it.

It connects out for two things only:

- **Lyrics:** when a track starts, it asks [LRCLIB](https://lrclib.net) for synced lyrics, sending the artist, title, album and length. The player itself (Spotify, a browser) is only read through Windows, never contacted. Set `NOTCH_LYRICS=off` to stop lookups; the lyrics button only hides them.
- **Calendar:** if you set a calendar link (for example Google Calendar's iCal address), it downloads that file every 15 minutes. The link sits in the settings file, so treat that file like a password.

## Acknowledgements

Notch is heavily inspired by Apple's Dynamic Island, and by the apps that bring the same idea to screens that have no hardware notch: [NotchNook](https://lo.cafe/notchnook) for the Mac and [Nusa](https://rama2555.github.io/nusa/) for Windows. They showed what a notch overlay can hold and how it should feel: the media player, the file shelf, the calendar and the short messages that open the pill.

Notch itself is written independently, from scratch in Rust. It draws with Direct2D and uses no UI framework, and every page and every pill message is its own module, so you can switch one off, remove it or write your own. It is open source under the MIT license: clone or fork it and change your copy (see [Contributing](#contributing)).

Notch is not affiliated with or endorsed by Apple, NotchNook, Nusa or their developers. Dynamic Island is a trademark of Apple Inc., and the other names belong to their owners.

## License

[MIT](LICENSE)
