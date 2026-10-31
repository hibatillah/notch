# Performance measurements

These are local Windows x64 release measurements, not guarantees for every machine or media session.
Raw measurement archives are not kept under `docs/`.

## Test setup

- Date: 2026-10-31, Asia/Jakarta.
- Windows 11 Home build 26300; AMD Ryzen 5 8645HS, 6 cores / 12 logical processors; 15.3 GiB usable RAM.
- Rust 1.97.1, MSVC x64 target, the repository's size-optimized release profile.
- CPU is process CPU time divided by wall time, expressed as a percentage of one core.
- RAM is private working set in MiB. The runner labels these binary units as MB.
- The baseline uses the shelf page, existing activity settings, 20-second idle samples and 10 open/close cycles.

## Latest recorded baseline

The run used `scripts/perf.sh` (release build) and passed every baseline gate.

| Measurement | Result | Limit |
| --- | ---: | ---: |
| Initial private working set | 23.8 MiB | 30 MiB |
| Collapsed idle CPU | 0% | 0.1% |
| CPU time per open/close cycle | 131 ms | 400 ms |
| RAM growth over 10 cycles | 0.3 MiB | 3 MiB |
| Handle growth over 10 cycles | -21 | 20 |
| Hover events | 20/20 | exactly 20 |
| Idle CPU after cycles | 0% | 0.1% |

Notch settled after startup and after the animations. Normal pages were restored after the test.
This baseline did not measure music playback, live lyrics, held hover or swiping.

## Settings window

The 0 % idle and 50 MB budget applies to `notch.exe` and is what the baseline above measures. The settings window is a
separate process (`notch-settings.exe`, GPUI) with its own, higher footprint while it is open, and `scripts/perf.sh`
does not run it.

Source: one launch of the release `run/notch-settings.exe` on 2026-10-31, same machine as above, with the General page
open and no input. Private working set is `\Process(notch-settings)\Working Set - Private`, read 10 s after the
window opened; CPU is process CPU time over the following 10 s.

| Measurement | Result |
| --- | ---: |
| Release executable size | 9.1 MB |
| Private working set at 10 s | 44.6 MB |
| Working set at 10 s | 76.3 MB |
| Private bytes at 10 s | 86.3 MB |
| CPU while open and idle | 0.94 % of one core |

## Window recovery verification

Before the recovery fix, the main window had lost `WS_EX_TOPMOST` while its drop-target overlay retained it.
Targeted checks deliberately demoted the running window, then tested hover handoff and simulated lock/unlock
notifications. Both restored topmost, opened the notch and preserved foreground focus.
The physical lock/unlock cycle was not tested; see the manual checklist in [testing.md](testing.md).

## Repeat the checks

Run `scripts/perf.sh` from Git Bash for the baseline, or add `--extended` for held hover, swipes and recovery.
For live lyrics, play a track with synced lyrics and use `--lyrics-only`.

The check moves the pointer and replaces the running Notch. Leave the mouse alone during measurement, then run
`scripts/reload.sh` to restore normal pages. See [testing.md](testing.md) for the gates and workload details.
