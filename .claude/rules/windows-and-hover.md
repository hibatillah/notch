---
paths:
  - "src/host/**"
  - "src/gfx/**"
  - "Cargo.toml"
---

# Windows specifics

- `windows = 0.62.2` and `windows-numerics = 0.3` move together; `Matrix3x2` and `Vector2` come from
  `windows_numerics`. `.cargo/config.toml` sets the `x86_64-pc-windows-msvc` target because the GNU toolchain has no
  linker here.
- The window is layered with per-pixel alpha and resizes with the pill, which can lose mouse tracking. Hover code
  keeps three safeguards: `TrackMouseEvent` re-armed on every move, `WM_MOUSELEAVE` checked against the cursor and
  window rect, and a 120 ms hover-watch timer while open. `WM_MOUSELEAVE` is defined as `0x02A3` in `host` because
  the `windows` crate does not export it.
- The pill is black and flush with the top edge. Concave "flare" fillets join its sides to the top of the screen, like
  a MacBook notch, and the window region includes their width.
- Enable `windows` crate features one API at a time in `Cargo.toml`.
