# Performance measurements

These are local Windows x64 release measurements, not guarantees for every machine or media session.
Raw measurement archives are not kept under `docs/`.

## Test setup

- Date: 2026-10-28, Asia/Jakarta.
- Windows 11 Home build 26300; AMD Ryzen 5 8645HS, 6 cores / 12 logical processors; 15.3 GiB usable RAM.
- Rust 1.97.1, MSVC x64 target, the repository's size-optimized release profile.
- CPU is process CPU time divided by wall time, expressed as a percentage of one core.
- RAM is private working set in MiB. The runner labels these binary units as MB.
- The baseline uses the shelf page, existing activity settings, 20-second idle samples and 10 open/close cycles.

## Latest recorded baseline

The convention-audit run used `scripts/perf.sh --no-build` and passed every baseline gate.
It predates the subsequent reduction of the idle music width cap to 400 DIP.

| Measurement | Result | Limit |
| --- | ---: | ---: |
| Initial private working set | 22.1 MiB | 30 MiB |
| Collapsed idle CPU | 0.078% | 0.1% |
| CPU time per open/close cycle | 131 ms | 400 ms |
| RAM growth over 10 cycles | 0.5 MiB | 3 MiB |
| Handle growth over 10 cycles | -8 | 20 |
| Hover events | 20/20 | exactly 20 |
| Idle CPU after cycles | 0% | 0.1% |

Notch settled after startup and after the animations. Normal pages were restored after the test.
This baseline did not measure music playback, live lyrics, held hover or swiping.

## Settings window

The 0 % idle and 50 MB budget applies to `notch.exe` and is what the baseline above measures. The settings window is a
separate process (`notch-settings.exe`, GPUI) with its own, higher footprint while it is open, and `scripts/perf.sh`
does not run it.

Source: [the spike](superpowers/spikes/2026-10-30-gpui-spike.md), 2026-10-30, Windows 11 Home 10.0.26300, 12 logical
cores, rustc 1.97.1, the workspace release profile. The workload is the spike prototype (one 640 x 420 window, a
sidebar and one toggle), not the finished window, with no input after opening. The spike records no command for the
samples, only the counter: private working set is `\Process(notch-settings)\Working Set - Private`, read 10 s after
the window opened.

| Measurement | Result |
| --- | ---: |
| Release executable size | 3.6 MB |
| Private working set at 10 s | 20.6 MB |
| Working set at 10 s | 49.2 to 49.5 MB |
| Private bytes at 10 s | 48.3 to 49.2 MB |
| Working set, first launch after the build | 86.9 MB |
| Private bytes, first launch after the build | 74.9 MB |
| CPU while open and idle, 10 s sample | 0.6 to 1.1 % of one core |

The private working set of the first launch was not sampled, and the cause of its larger size is not confirmed.

## Earlier measurements

All runs below were recorded on 2026-10-28. They used different revisions and workloads, so they are not controlled
comparisons of individual changes.

| Baseline | Initial idle CPU | Recovery idle CPU | RAM growth | Result |
| --- | ---: | ---: | ---: | --- |
| Continuous scrolling, before gentle snap | 0% | 0% | 2.1 MiB | Passed |
| Gentle snap, extended run | 0.078% | 0.625% | 2.1 MiB | Recovery CPU failed |
| Gentle snap, baseline repeat | 0% | 0.703% | 1.1 MiB | Recovery CPU failed |
| Idle title width | 1.641% | 1.406% | 0.1 MiB | Idle CPU failed |
| Maximized-window hiding | 0% | 0.156% | 2 MiB | Recovery CPU and hover events failed (18/20) |
| Tray options | 0.234% | 0.391% | 5.5 MiB | Idle CPU and RAM growth failed |
| Window recovery | 1.875% | 0.234% | 0.1 MiB | Idle CPU failed |
| Closing bounce | 0% | 0% | 0.6 MiB | Passed |

The closing-bounce baseline measured 17.4 MiB initial RAM and 191 ms CPU per open/close cycle.
The later passing samples do not establish the cause of the earlier failures; performance limits remain unchanged.

The earlier gentle-snap extended run measured normal-page idle at 0.156% CPU / 24.7 MiB, held hover at
1.562% / 26.2 MiB, and ten swipe round trips at 3.213% / 26.2 MiB sampled peak. After swiping, both open and
collapsed stationary samples returned to 0% CPU. Swipe peaks were sampled at the ends of swipes, not continuously.
Music was not playing, and live lyrics comparison was skipped at the user's request.

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
