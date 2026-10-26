# Workflow

- Replace the running notch with `scripts/reload.sh`, or let the user's `scripts/watch.sh` rebuild it; confirm a
  rebuild by reading the tail of its output. The user closes an old copy themselves while developing.
- A change is done when `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` all pass.
  After touching the host, animation, timers or drawing, `scripts/perf.sh` must pass too; tell the user first, because
  it moves the mouse for about two minutes and replaces the running notch.
- Mouse-driven checks (`scripts/perf.sh`) and screenshots need an unlocked screen; while it is locked (a `LogonUI`
  process runs) screenshots come out black. Verify through the `NOTCH_DEBUG` log or unit tests then. A check that
  opens windows over the screen or moves the mouse is run on purpose, after telling the user.
- When the `Page` trait or anything pages use changes, update `docs/adding-a-page.md` in the same change; its example is
  a real page that compiled. Do the same for the `Activity` trait and `docs/adding-an-activity.md`.
- When behavior a user can see changes, update `README.md` and the doc that covers it in the same change.
- Commit only when the user asks.
