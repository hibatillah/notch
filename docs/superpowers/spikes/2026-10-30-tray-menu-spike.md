# Tray menu look spike

Question: can a custom popup window with a system backdrop and Direct2D drawing reproduce the Windows 11 tray menu look (40 px rows, rounded corners, translucent backdrop), and at what cost? The owner uses the answer to choose between replacing the native menu in `src/host/tray.rs` and keeping it.

Status: built and compiled, not run. Every measurement below is "not measured yet" until the owner runs it.

## What was built

`scripts/tray_spike.rs`, registered as the example `tray_spike` in `Cargo.toml`. It is throwaway code: delete it, its `[[example]]` entry and the two feature flags below once the decision is recorded.

- Window: `WS_POPUP`, `WS_EX_TOOLWINDOW`, `WS_EX_NOACTIVATE`, plus `WS_EX_TOPMOST`. It opens at the cursor without taking focus.
- DWM: `DWMWA_SYSTEMBACKDROP_TYPE = DWMSBT_TRANSIENTWINDOW`, `DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND`, `DWMWA_USE_IMMERSIVE_DARK_MODE` from the `AppsUseLightTheme` registry value, and `DwmExtendFrameIntoClientArea` with margins of -1 so the backdrop is not covered.
- Drawing: an `ID2D1HwndRenderTarget` with premultiplied alpha, cleared to transparent, plus DirectWrite text ("Segoe UI Variable Text", 14 dip). The spike calls the `windows` crate directly because `scripts/` examples cannot reach the `notch` binary crate's `gfx` module.
- Items: 40 dip rows, 240 dip wide, 4 dip padding, labels copied from `tray.rs`: Notifications, Hide when idle, Start with Windows, Settings…, Quit. No icons, check marks, separators, submenus or keyboard navigation.
- Hover highlight: rounded rectangle, white at 10 % alpha (dark theme) or black at 6 % alpha (light theme). These alpha values are guesses, not taken from Windows.
- Dismiss: clicking an item prints its label and exits. A click outside (the mouse is captured with `SetCapture`), Escape, or a change of foreground window also exits. Escape and foreground changes are found by a 50 ms timer, because `WS_EX_NOACTIVATE` means no focus messages arrive. The timer runs only while the menu is open.
- Startup printout: Windows build and revision, system theme, `EnableTransparency` registry value, DPI scale, requested and read-back (`DwmGetWindowAttribute`) backdrop and corner values, time to shown, private working set.
- Private working set uses `K32QueryWorkingSet` and counts non-shared pages, the same method as `scripts/perf.rs`. It measures the whole spike process, including the Rust runtime, Direct2D and DirectWrite, not only the menu.
- Cargo changes: no new dependency. Added `windows` features `Win32_Graphics_Dwm` and `Win32_UI_Controls` (for `MARGINS`).
- One unit test, `item_at` hit-testing; run it with `cargo test --example tray_spike`.

## How to run

The owner must agree first: the spike opens a window over the screen at the cursor position.

```
cargo run --example tray_spike
cargo run --example tray_spike -- --hold-seconds 10
```

The second command opens the menu at the cursor, keeps it open 10 seconds, prints `private working set after hold: N MB`, and exits. Move the pointer over the rows during the hold to see the hover highlight. Do not click or press Escape before the hold ends if you want the printout; dismissing early skips it. The spike does not touch the running notch.

## What the owner records

Run each case and fill the table. Settings > Personalization > Colors > Transparency effects is the toggle for the backdrop rows.

| Check | Result |
| --- | --- |
| Windows build number (from the printout) | not measured yet |
| Backdrop visible, Transparency effects ON, dark theme | not measured yet |
| Backdrop visible, Transparency effects ON, light theme | not measured yet |
| Backdrop visible, Transparency effects OFF, dark theme | not measured yet |
| Backdrop visible, Transparency effects OFF, light theme | not measured yet |
| Rounded corners present | not measured yet |
| Read-back backdrop and corner values match the requested ones | not measured yet |
| Text and highlight legible in both themes | not measured yet |
| Private working set at startup (MB, printout) | not measured yet |
| Private working set after the hold (MB, printout) | not measured yet |
| Flicker on open, on hover or on dismiss | not measured yet |
| Focus stolen from the previous window (does typing still go to it?) | not measured yet |
| DPI 100 %: row height and text sharpness | not measured yet |
| DPI 150 %: row height and text sharpness | not measured yet |
| Menu position near a screen edge or the taskbar | not measured yet |

Also record whether the result looks like the Windows 11 tray menu next to the native one (same theme).

## Known limits of the approach

Verified from this repo:

- The native menu follows the system theme through undocumented `uxtheme.dll` ordinals 135 and 136 (`follow_system_theme` in `src/host/tray.rs`). It has no 40 px rows and no rounded or translucent look today.
- `TrackPopupMenu` handles keyboard navigation, dismissal and the foreground-window handshake. A custom window must reimplement them. The spike has no arrow-key or Enter handling, and `WS_EX_NOACTIVATE` means it receives no keyboard input, so Escape is read with `GetAsyncKeyState`.
- `.claude/rules/performance.md` forbids polling. The spike's 50 ms timer runs only while the menu is open, but a production version needs an event-driven replacement. I cannot confirm what that costs.
- The same rules budget 50 MB for the notch and note that Direct2D path geometry costs about 10 MB the first time it is used. The spike uses `FillRoundedRectangle` and `DrawText`, not path geometry. Whether those add a comparable fixed cost is what the private working set rows measure; I cannot confirm it without a run.
- A production menu would live in the notch process, which already uses Direct2D and DirectWrite (`src/gfx`). The spike is a separate process, so its number includes costs the notch already pays. I cannot confirm how much.

Not verified (no run, no network lookup):

- The first Windows build that honours `DWMWA_SYSTEMBACKDROP_TYPE`. I recall Windows 11 22H2 (build 22621) from Microsoft documentation, but I cannot confirm this here. The printout records the owner's build and the read-back values show whether the setting was accepted.
- Whether `DWMSBT_TRANSIENTWINDOW` falls back to a solid color when Transparency effects is off. I cannot confirm this; the OFF rows test it.
- Whether an `ID2D1HwndRenderTarget` with premultiplied alpha lets the backdrop show through the cleared area, or a different swap chain (DirectComposition or DXGI) is needed. I cannot confirm this. If the text draws but the backdrop is missing, this is the first suspect.
- Whether the Windows 11 menu uses these exact attributes, the same corner radius, or a border and shadow the spike does not draw. I cannot confirm this.
- Screen-reader and UI Automation behavior. The spike exposes no accessibility tree; the native menu does. I cannot confirm the impact.
- Multiple monitors with different DPI. The spike sizes the window from the DPI of the window after creation at the cursor, which may be wrong when the cursor is on a second monitor with different scaling.

## Decision

Fill the table, then record the choice at the bottom of this file and delete `scripts/tray_spike.rs`, its `[[example]]` entry and the two feature flags.

| Option | Choose it when |
| --- | --- |
| Write a follow-up plan to replace the native menu | The backdrop and rounded corners show with Transparency effects ON in both themes; the look with it OFF is acceptable; there is no focus stealing or flicker; the private working set growth keeps the notch under its 50 MB budget with room to spare; the owner accepts reimplementing keyboard navigation, outside-click and focus-loss dismissal, DPI handling and accessibility. |
| Keep the native menu | The backdrop does not show, flickers or steals focus; the extra private working set puts the notch near 50 MB; or the owner values keyboard navigation and accessibility handled by the system over the look. |

If the result is mixed (for example the backdrop works but memory is borderline), state that in the decision line. Keeping the native menu is the default when no row decides it.

Decision: not made yet.
