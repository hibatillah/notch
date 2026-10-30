---
paths:
  - "Cargo.toml"
  - "Cargo.lock"
  - "LICENSE"
---

# Dependencies and license

- The project is MIT licensed, so every dependency must be permissive and MIT-compatible.
- After any change to `Cargo.lock`, run `cargo license` (install with `cargo install cargo-license --locked`).
- After any change to `Cargo.lock`, run `scripts/third_party_licenses.sh` and commit `THIRD-PARTY-LICENSES.txt` (the installer ships it).
- Ask before adding a dependency, and before accepting a GPL, LGPL, AGPL or MPL one. The notch is small on purpose.
