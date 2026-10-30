---
paths:
  - ".github/**"
  - "Cargo.toml"
  - "settings/Cargo.toml"
  - "installer/**"
  - "THIRD-PARTY-LICENSES.txt"
---

# CI and release

`docs/releasing.md` describes both workflows.

- `ci.yml` runs on pushes to the default branch only. Tags and pull requests start nothing there.
- `release.yml` runs on `v*` tag pushes only, and fails unless the tag equals `v` plus the version in both `Cargo.toml` and `settings/Cargo.toml`.
- Both workflows use the MSVC toolchain (`stable-x86_64-pc-windows-msvc`) and the `--workspace` flag; the settings crate (GPUI) does not build with the GNU toolchain.
- Read the executable names from `Cargo.toml` and `settings/Cargo.toml` in workflows; never write `notch` into them.
- The installer ships `notch-settings.exe` beside `notch.exe` and `THIRD-PARTY-LICENSES.txt`; regenerate the licenses file with `scripts/third_party_licenses.sh` after any `Cargo.lock` change.
- Keep the `AppId` in `installer/notch.iss` fixed; changing it hides earlier installs from the installer.
- Use the preinstalled `gh` CLI for releases rather than a third-party action.
- Give a workflow the least permission it needs (`contents: read`; `contents: write` only for releases).
