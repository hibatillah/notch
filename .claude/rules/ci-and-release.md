---
paths:
  - ".github/**"
  - "Cargo.toml"
  - "installer/**"
---

# CI and release

`docs/releasing.md` describes both workflows.

- `ci.yml` runs on pushes to the default branch only. Tags and pull requests start nothing there.
- `release.yml` runs on `v*` tag pushes only, and fails unless the tag equals `v` plus the `Cargo.toml` version.
- Read the executable name from `Cargo.toml` in workflows; never write `notch` into them.
- Keep the `AppId` in `installer/notch.iss` fixed; changing it hides earlier installs from the installer.
- Use the preinstalled `gh` CLI for releases rather than a third-party action.
- Give a workflow the least permission it needs (`contents: read`; `contents: write` only for releases).
