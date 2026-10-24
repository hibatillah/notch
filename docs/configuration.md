# Configuration

There are three ways to configure the notch, from what a user can change to what a developer changes in code.

## Which pages are on

Without any setting, every registered page is on. To use only some of them, list their ids, separated by commas:

- **Environment variable:** `NOTCH_PAGES=clock,media`
- **Settings file:** a line `pages=clock,media` in `%APPDATA%\notch\settings.txt`

The environment variable wins when both are set. Page ids are the values returned by each page's `id()`; the built-in
ones are `clock` and `media`.

- Pages keep their registration order, whatever order you list them in.
- Ids that match no page are ignored. If none match, every page stays on.
- Keep `clock` on: it is the fallback shown when no other page has anything to show.

Example, to run without the media page:

```
set NOTCH_PAGES=clock
notch.exe
```

To change which pages are *compiled in*, edit `src/pages/mod.rs`; see [adding-a-page.md](adding-a-page.md).

## Settings file

`%APPDATA%\notch\settings.txt` is a plain text file with one `key=value` per line. The notch writes it when you
change something; you can also edit it by hand while the notch is not running.

| Key | Values | Meaning |
| --- | --- | --- |
| `pages` | comma-separated page ids | which pages are on (see above) |
| `media.lyrics` | `on`, `off` | show lyrics on the media page (the speech-bubble button changes this) |

Pages add their own keys, always starting with the page id. A page reads and writes them through
`Context::settings`.

## Environment variables

| Variable | Effect |
| --- | --- |
| `NOTCH_PAGES` | comma-separated page ids to switch on (see above) |
| `NOTCH_LYRICS=off` | never look up lyrics, so nothing about your music leaves the computer |
| `NOTCH_DEBUG=<file>` | append a timestamped log of what the notch is doing to this file |
| `NOTCH_TOP=<pixels>` | move the notch down by this many pixels, to compare it with another notch app |

## In the code

Things a developer tunes by editing source:

| What | Where |
| --- | --- |
| which pages exist and their order (the order breaks priority ties) | `src/pages/mod.rs` |
| when a page is shown | the page's `priority()` |
| how big a page is | the page's `sizes()` |
| corner radius, spring stiffness, window size | `src/host/notch.rs` |
| text styles | `const FontStyle`s in each page |
| the limits of the performance test | `scripts/perf.sh` |

## Start with Windows

This is not built in. To start the notch at sign-in, put a shortcut to `notch.exe` in the folder opened by
pressing Win+R and running `shell:startup`.
