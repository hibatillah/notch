# Workflow

- Replace the running notch with `scripts/reload.sh`. The user closes it themselves while developing.
- A change is done when `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` all pass.
  After touching the host, animation, timers or drawing, `scripts/perf.sh` must pass too; tell the user first, because
  it moves the mouse for about two minutes and replaces the running notch.
- Mouse-driven checks (`scripts/perf.sh`) and screenshots need an unlocked screen; while it is locked (a `LogonUI`
  process runs) screenshots come out black. Verify through the `NOTCH_DEBUG` log or unit tests then.
- When the `Page` trait or anything pages use changes, update `docs/adding-a-page.md`; its example is a real page
  that compiled. Do the same for the `Activity` trait and `docs/adding-an-activity.md`.
- Commit only when the user asks.
