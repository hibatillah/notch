---
paths:
  - "build.rs"
  - "assets/**"
  - "installer/**"
  - "src/host/icon.rs"
---

# Branding and build script

`docs/development.md` (branding) and `docs/releasing.md` describe the icon pipeline.

- `assets/branding/logo.png` is the master; `scripts/export-icon.sh` regenerates `logo.ico` from it and the result is
  committed.
- `build.rs` embeds `assets/branding/logo.rc` through the Windows SDK's `rc.exe`, which it finds on `PATH` or under
  `Windows Kits/10`. Keep it dependency-free and have it fail with a message that names the missing tool.
- Load the embedded icon through `host/icon.rs` by the resource name `NOTCH_ICON`; the installer and shortcuts reuse
  the executable's icon.
