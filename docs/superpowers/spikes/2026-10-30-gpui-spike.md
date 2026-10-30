# GPUI spike (2026-10-30)

Decides whether the settings window (`notch-settings.exe`) is built with GPUI. Gate: private working set under 60 MB
10 s after the window opens, and every dependency license permissive and MIT-compatible.

## Setup

- GPUI: `gpui = "=0.2.2"` from crates.io (Zed's official crate, repository `zed-industries/zed`), with
  `default-features = false, features = ["windows-manifest"]`.
- Program: `settings/src/main.rs`, one 640 x 420 window with a four-item sidebar and one toggle.
- Machine: Windows 11 Home 10.0.26300, 12 logical cores, rustc 1.97.1.
- Release profile: the workspace `[profile.release]` (`opt-level = "s"`, LTO, `codegen-units = 1`, `panic = "abort"`,
  `strip = true`).
- The root `Cargo.toml` is a workspace (`members = [".", "settings"]`, `default-members = ["."]`); `cargo build
  --release` at the root builds only the notch, and the notch's resolved dependencies are unchanged.

## Measurements

| Measure | Value |
| --- | --- |
| Release exe size | 3.6 MB (3,770,880 bytes) |
| Clean `cargo build --release -p notch-settings` | 199 s (empty target directory, 12 cores) |
| `cargo tree -p notch-settings --prefix none \| sort -u \| wc -l` | 518 lines (410 distinct crates once `(*)` and `(proc-macro)` markers are removed) |
| Private working set at 10 s | 20.6 MB (three runs: 20.6, 20.6, 20.6; one more run: 20.5) |
| Working set at 10 s | 49.2 to 49.5 MB |
| Private bytes (`PrivateMemorySize64`) at 10 s | 48.3 to 49.2 MB |
| Threads | 22 |
| CPU while idle, no input, 10 s sample | 0.6 to 1.1 % of one core |
| Target directory after the build | 1.4 GB |

Private working set is the `\Process(notch-settings)\Working Set - Private` counter, the same quantity
`scripts/perf.rs` reads (non-shared working-set pages). The very first launch after the build read 86.9 MB working set
and 74.9 MB private bytes and held there through 15 s; every later launch read the numbers above. The private working
set of that first launch was not sampled, and the cause (likely first-run shader or driver cache work) is not
confirmed.

## Licenses

`cargo license --filter-platform x86_64-pc-windows-msvc` in `settings/` (441 packages, build dependencies included):

| License | Count |
| --- | --- |
| Apache-2.0 OR MIT | 276 |
| MIT | 82 |
| Unicode-3.0 | 18 |
| Apache-2.0 (gpui and its `gpui_*` crates, `stacksafe`, `stacksafe-macro`, `sync_wrapper`) | 14 |
| Apache-2.0 OR MIT OR Zlib | 9 |
| BSD-3-Clause | 8 |
| MIT OR Unlicense | 7 |
| Apache-2.0 OR ISC OR MIT | 4 |
| BSD-2-Clause | 4 |
| Zlib | 4 |
| Apache-2.0 OR BSD-2-Clause OR MIT, Apache-2.0 OR BSD-3-Clause, ISC | 2 each |
| (Apache-2.0 OR MIT) AND BSD-3-Clause, (Apache-2.0 OR MIT) AND Unicode-3.0, 0BSD OR Apache-2.0 OR MIT, Apache-2.0 AND ISC, Apache-2.0 OR Apache-2.0 WITH LLVM-exception OR MIT, Apache-2.0 OR BSL-1.0, Apache-2.0 OR CC0-1.0, Apache-2.0 OR CC0-1.0 OR MIT-0, Apache-2.0 WITH LLVM-exception | 1 each |

No GPL, LGPL, AGPL or MPL crate is in the Windows build. Without the platform filter the list adds two crates that never
build on Windows: `cbindgen` (MPL-2.0, a build dependency for other platforms) and `r-efi` (Apache-2.0 OR LGPL-2.1+
OR MIT, a UEFI crate, usable under MIT). Apache-2.0-only crates are compatible with an MIT project, but a shipped
binary must carry the Apache-2.0 license text and any NOTICE files of those crates.

## Build note

The default toolchain here is `stable-x86_64-pc-windows-gnu`. With it, `cargo build -p notch-settings` fails on a
host-side `windows-sys v0.61.2`: `error: error calling dlltool 'dlltool.exe': program not found`. The `.cargo/config.toml`
target only moves target code to MSVC; build scripts and proc macros still compile for the GNU host. Building with
`cargo +stable-x86_64-pc-windows-msvc build --release -p notch-settings` works. The notch itself is unaffected.

## Decision

**Go.** Private working set is 20.6 MB against the 60 MB gate (working set 49 MB and private bytes 49 MB are under it
too), and every license in the Windows build is permissive and MIT-compatible.

Open points for the later tasks: the idle window uses 0.6 to 1.1 % of one core with no input, so the settings program
does not idle at 0 % the way the notch does; the builds need the MSVC host toolchain; the installer must ship the
Apache-2.0 license text for the GPUI crates.
