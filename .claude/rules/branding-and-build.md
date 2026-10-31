---
paths:
  - "build.rs"
  - "settings/build.rs"
  - "build_support/**"
  - "assets/**"
  - "installer/**"
  - "src/host/icon.rs"
---

# Branding and build script

`docs/development.md` (branding) and `docs/releasing.md` describe the icon pipeline.

- `assets/branding/logo.png` is the master; `scripts/export-icon.sh` regenerates `logo.ico` from it and the result is
  committed.
- `build.rs` embeds `assets/branding/logo.rc`, and `settings/build.rs` embeds `assets/branding/settings.rc` (icon id 1,
  the one GPUI loads for the window and taskbar). Both call `build_support/icon_resource.rs`, which runs the Windows
  SDK's `rc.exe` (found on `PATH` or under `Windows Kits/10`). Keep it dependency-free and have it fail with a message
  that names the missing tool.
- Load the embedded icon through `host/icon.rs` by the resource name `NOTCH_ICON`; the installer and shortcuts reuse
  the executable's icon.
