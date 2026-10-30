# Settings Window, Behavior Matrix, Tray and Google Sign-in Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the notch a separate settings window that edits every setting, add per-context behavior for maximized and full-screen windows, and prepare the Windows 11 tray menu and Google calendar sign-in.

**Architecture:** A Cargo workspace: the existing `notch` package, a new `notch-store` library that owns the `settings.txt` file code, and a new `notch-settings` GPUI binary. The window writes keys through `notch-store` and posts `WM_SETTINGS_CHANGED` to the notch, which re-reads the file and re-applies its fields. Behavior per context becomes a pure function in `src/host/visibility.rs`.

**Tech Stack:** Rust 2024 (1.88), `windows` 0.62.2, GPUI (settings crate only, adopted after Task 1 passes).

**Spec:** `docs/superpowers/specs/2026-10-30-settings-window-design.md`

## Global Constraints

- Branch `settings-window`; never commit to `master`. Commit only when the user asks, so each task ends with a checkpoint, not a commit.
- The notch process: 0 % CPU idle, under 50 MB (`scripts/perf.sh` limits stay as they are); no polling; no UI framework in `notch.exe`.
- Every dependency MIT-compatible (`cargo license`); ask before adding one. GPUI is approved only if Task 1 passes.
- Full-word identifiers; comments one line that says why; tests in `#[cfg(test)] mod tests` at the file's bottom, named as sentences about behavior.
- A task is done when `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass (workspace default members). After host, timer or drawing changes, run `scripts/perf.sh` (tell the user first; it moves the mouse about two minutes).
- Update `README.md` and the covering doc in the same change as any user-visible behavior; update `docs/adding-a-page.md` / `docs/adding-an-activity.md` when `Page` / `Activity` change.
- Boolean keys are `on`/`off`; a missing or unreadable value means the documented default.

## Review Focus

- Two writers (notch and window) changing different keys at once: neither loses the other's key. (Task 2)
- A malformed or half-written `settings.txt` during reload: the notch keeps its current values. (Task 3)
- The window posts a reload while the notch is not running: no crash, no hang. (Task 5)
- Clicking "Settings…" twice: one window, the existing one focused. (Task 4)
- Unknown key values (`notch.maximized=banana`): the documented default applies. (Task 10)
- Reload while the notch is open or hovered: hover and page position are untouched. (Task 3)
- Migration never overwrites a new key the user already set. (Task 10)

---

## Phase 1: Settings window

### Task 1: GPUI spike and go/no-go

**Files:**
- Create: `settings/Cargo.toml`, `settings/src/main.rs` (throwaway), `docs/superpowers/spikes/2026-10-30-gpui-spike.md`
- Modify: `Cargo.toml` (workspace: `members = [".", "settings"]`, `default-members = ["."]`)

**Interfaces:**
- Produces: a measured go/no-go recorded in the spike doc.

- [ ] **Step 1: Convert the root `Cargo.toml` to a workspace**, keeping the `notch` package, profile and `[[example]]` unchanged; add the two members above. Run `cargo build --release` and confirm the notch builds without compiling `settings`.
- [ ] **Step 2: Create `settings` with one GPUI window** (sidebar plus one toggle), pinned to an exact GPUI revision or crates.io version.
- [ ] **Step 3: Measure and record** in the spike doc: release exe size, clean `cargo build --release -p notch-settings` time, `cargo tree -p notch-settings | wc -l`, private working set in MB 10 s after the window opens (Task Manager or `scripts/perf.rs` counters), and the `cargo license` summary.
- [ ] **Step 4: Decide.** Go when working set < 60 MB and every license is permissive. On no-go, stop and record that Tasks 5-7 switch to a native Win32 window drawn with `gfx` and `ui`; ask the user before continuing.
- [ ] **Step 5: Checkpoint:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.

### Task 2: `notch-store` crate with per-key atomic writes

**Files:**
- Create: `store/Cargo.toml` (package `notch-store`, no dependencies), `store/src/lib.rs`
- Modify: `Cargo.toml` (member `store`, dependency `notch-store = { path = "store" }`), `src/host/settings.rs` (becomes `pub use notch_store::Settings;`; move its tests to the crate)

**Interfaces:**
- Produces: `Settings::load() -> Settings`, `Settings::at(path: Option<PathBuf>) -> Settings`, `get_string(&self, key: &str) -> Option<&str>`, `get_bool(&self, key: &str, default: bool) -> bool`, `set_string(&mut self, key: &str, value: &str)`, `set_bool(&mut self, key: &str, value: bool)`, `reload(&mut self)`.
- Behavior: every `set_*` re-reads the file, changes only its key, writes `settings.txt.tmp` then renames over `settings.txt`; `reload` replaces the in-memory map with the file's. All existing callers compile unchanged.

- [ ] **Step 1: Write the failing tests** `two_writers_changing_different_keys_keep_both`, `a_set_keeps_keys_another_process_wrote`, `reload_picks_up_a_changed_file`, `a_missing_file_is_an_empty_store` (assertions on `get_string`).
- [ ] **Step 2:** Run `cargo test -p notch-store`; expect FAIL (crate has no `Settings`).
- [ ] **Step 3: Move `Settings` and `parse` from `src/host/settings.rs` into `store/src/lib.rs`** and implement the re-read, temp file and rename in `save`; keep `Settings::at(None)` meaning "never touches disk".
- [ ] **Step 4:** Run `cargo test` (workspace default) and `cargo test -p notch-store`; expect PASS.
- [ ] **Step 5: Checkpoint:** fmt, clippy, test; update `docs/architecture.md` file map (new `store/`, `settings/`).

### Task 3: Live reload in the notch

**Files:**
- Modify: `src/host/window.rs` (handle `WM_SETTINGS_CHANGED`), `src/host/notch.rs` (`pub fn reload_settings(&mut self)`), `src/page.rs` (`Page::on_settings_changed(&mut self, context: &mut Context)`, default no-op), `src/activity.rs` (same hook on `Activity`), `src/host/notch/snapshots.rs` (tests), `docs/adding-a-page.md`, `docs/adding-an-activity.md`, `.claude/rules/settings-and-tray.md`
- Constant: `pub const WM_SETTINGS_CHANGED: u32 = WM_APP + 30;` in `src/host/window.rs`

**Interfaces:**
- Consumes: `Settings::reload` (Task 2).
- Produces: `Notch::reload_settings` re-reads `notch.hide_when_idle`, `notch.hide_for_fullscreen`, `notch.show_over_maximized`, `notch.lyrics_over_maximized`, `navigation.order`, `navigation.disabled`, `activity.notifications` (start the listener when switched on, hide its presence when off), then calls `on_settings_changed` on every page and started activity; media, timer and calendar re-read their keys in it.

- [ ] **Step 1: Write failing tests** in `snapshots.rs`: `reloading_settings_applies_hide_when_idle`, `reloading_settings_keeps_the_open_page_and_hover`, `a_malformed_settings_file_keeps_the_current_values`.
- [ ] **Step 2:** `cargo test reloading_settings` → FAIL.
- [ ] **Step 3: Implement `reload_settings`**, calling `self.settings.reload()` then re-deriving the fields, then `self.flush()`. A key that fails to parse keeps the field's value.
- [ ] **Step 4:** Implement `on_settings_changed` for `MediaPage` (`media.lyrics`), the timer session (minutes keys) and `CalendarPage` (`calendar.ics_url`, `calendar.refresh_minutes`).
- [ ] **Step 5:** Run the tests → PASS. Update the two docs and the settings rule ("re-read on `WM_SETTINGS_CHANGED`" replaces "read once").
- [ ] **Step 6: Checkpoint:** fmt, clippy, test, then `scripts/perf.sh` (ask first).

### Task 4: Tray "Settings…" and single instance

**Files:**
- Modify: `src/host/tray.rs` (`MenuChoice::OpenSettings`, menu item), `src/host/window.rs` (spawn), `docs/configuration.md`, `README.md` tray list
- Create: `src/host/settings_launcher.rs`

**Interfaces:**
- Produces: `pub fn open_settings_window()` → starts `notch-settings.exe` found in the directory of the running exe (`std::env::current_exe`); logs `could not start settings: {error}` when missing.
- Consumes: the settings program's own single-instance behavior (Task 5).

- [ ] **Step 1: Write the failing test** `the_settings_program_sits_beside_the_notch` for the pure `fn settings_program_path(notch_exe: &Path) -> PathBuf`.
- [ ] **Step 2:** run → FAIL; **Step 3:** implement `settings_program_path` and `open_settings_window` (`CreateProcessW`, no console, do not wait).
- [ ] **Step 4:** Add the tray item and wire `MenuChoice::OpenSettings`; run tests → PASS.
- [ ] **Step 5: Checkpoint:** fmt, clippy, test.

### Task 5: Settings app shell (sidebar, store, notify)

**Files:**
- Create: `settings/src/main.rs`, `settings/src/notify.rs`, `settings/src/sections/mod.rs`, `settings/src/single_instance.rs`
- Modify: `settings/Cargo.toml` (depends on `notch-store`, `windows`, GPUI)

**Interfaces:**
- Produces: `notify::tell_notch_to_reload()` (finds class `NotchWindow` with `FindWindowW`, `PostMessageW(WM_APP + 30)`; does nothing when absent); `single_instance::acquire() -> Option<InstanceGuard>` (named mutex `Local\NotchSettings`; when taken, focuses the existing window and returns `None`); `fn apply(settings: &mut Settings, key: &str, value: &str)` = `set_string` then `tell_notch_to_reload`.
- Window: sidebar with sections General, Pages, Behavior, Notifications, Calendar, Media, Timer, Advanced; follows system light/dark.

- [ ] **Step 1: Write failing tests** `notifying_without_a_running_notch_does_nothing`, `a_second_instance_is_refused_while_the_first_lives`, `applying_a_key_writes_it_to_the_file`.
- [ ] **Step 2:** `cargo test -p notch-settings` → FAIL; **Step 3:** implement the three units and the empty-section window.
- [ ] **Step 4:** run tests → PASS; start the window with `cargo run -p notch-settings` while the notch runs and confirm a toggle posted reaches it (`NOTCH_DEBUG` log shows the reload).
- [ ] **Step 5: Checkpoint:** fmt, clippy (`--workspace --all-targets`), test.

### Task 6: General, Notifications, Media, Timer, Calendar (address), Advanced sections

**Files:**
- Create: `settings/src/sections/{general,notifications,media,timer,calendar,advanced}.rs`, `settings/src/field.rs`

**Interfaces:**
- Produces: `field::toggle(key, default)`, `field::minutes(key, default, range: RangeInclusive<u32>)` (commit on Enter or blur; out-of-range input is rejected and the field reverts), `field::text(key)` (commit on Enter or blur).
- Keys per section come from `docs/configuration.md`: General (`notch.hide_when_idle`, start with Windows via `HKCU\...\Run` as `src/host/startup.rs` does), Notifications (`activity.notifications`, `notifications.show_text`), Media (`media.lyrics`), Timer (`timer.focus_minutes` 5-120, `timer.short_break_minutes`, `timer.long_break_minutes`), Calendar (`calendar.ics_url`, `calendar.refresh_minutes`), Advanced (read-only note of `NOTCH_*` environment variables).

- [ ] **Step 1: Write failing tests** `a_toggle_writes_on_and_off`, `minutes_outside_the_range_are_rejected`, `text_commits_only_on_enter_or_blur` against the pure parts of `field`.
- [ ] **Step 2-4:** run → FAIL; implement; run → PASS.
- [ ] **Step 5:** Run the app, change each section's keys, confirm `settings.txt` and the notch follow.
- [ ] **Step 6: Checkpoint:** fmt, clippy, test.

### Task 7: Pages section (drag reorder and show/hide)

**Files:**
- Modify: `src/host/notch.rs` (publish `pages.known=id:Label,...` at startup), `src/host/navigation.rs` if the text helpers need exposing
- Create: `settings/src/sections/pages.rs`

**Interfaces:**
- Produces: `fn known_pages_text(ids: &[&str], labels: &[&str]) -> String` in the notch; `fn reorder(order: &mut Vec<String>, from: usize, to: usize)` and `fn order_text(order: &[String], disabled: &[String]) -> (String, String)` in the settings crate, producing the `navigation.order` and `navigation.disabled` values `PageList::load` already reads.

- [ ] **Step 1: Write failing tests** `known_pages_text_joins_ids_and_labels`, `reordering_moves_a_page_to_the_new_position`, `switching_a_page_off_adds_it_to_disabled`.
- [ ] **Step 2-4:** run → FAIL; implement; run → PASS.
- [ ] **Step 5:** Drag a page in the window; confirm the notch's page order changes live.
- [ ] **Step 6: Checkpoint:** fmt, clippy, test, `scripts/perf.sh` (ask first).

### Task 8: Build, installer and release

**Files:**
- Modify: `.github/workflows/release.yml`, `.github/workflows/ci.yml`, `installer/notch.iss`, `docs/releasing.md`, `.claude/rules/ci-and-release.md`

- [ ] **Step 1:** CI builds and tests the whole workspace; the release workflow builds `notch` and `notch-settings` in release, fails unless the tag equals both package versions.
- [ ] **Step 2:** The installer ships `notch-settings.exe` beside `notch.exe` and keeps the `AppId`.
- [ ] **Step 3:** Run the installer build locally if Inno Setup is installed; confirm both files install. Otherwise state that it was not run.
- [ ] **Step 4: Checkpoint:** fmt, clippy, test; update the release doc and rule.

### Task 9: Docs and rules for the two-process app

**Files:**
- Modify: `CLAUDE.md`, `README.md`, `docs/architecture.md`, `docs/configuration.md`, `docs/development.md`, `.claude/rules/dependencies-and-license.md`, `.claude/rules/pages-and-host.md`

- [ ] **Step 1:** `CLAUDE.md` and README say the notch process has no UI framework and the budget applies to it; the settings window is a separate process with a higher footprint while open.
- [ ] **Step 2:** The dependency rule names GPUI as allowed in `settings/` only; architecture lists `store/` and `settings/`; configuration documents `pages.known` and how the window edits keys.
- [ ] **Step 3: Checkpoint:** read each edited doc once for stale statements.

---

## Phase 2: Behavior per context

### Task 10: Visibility model and migration (pure)

**Files:**
- Create: `src/host/visibility.rs`
- Modify: `src/host/mod.rs`

**Interfaces:**
- Produces: `pub enum Showing { Hidden, ActivitiesOnly, MusicWithLyrics, Music }`; `Showing::parse(text: &str) -> Option<Showing>` (`hidden|activities|lyrics|music`) and `as_text(self) -> &'static str`; `pub fn showing_for(mode: ScreenMode, maximized: Showing, fullscreen: Showing) -> Option<Showing>` (`None` for `Normal`); `pub fn migrate(settings: &mut Settings)` writing `notch.maximized` and `notch.fullscreen` from the old keys only when the new key is absent, per the spec mapping; `pub fn allows(showing: Showing, has_activity: bool, is_playing: bool, has_lyrics: bool) -> bool`.

- [ ] **Step 1: Write failing tests** `parse_rejects_unknown_words_and_the_default_applies`, `fullscreen_hide_on_migrates_to_hidden`, `maximized_show_over_maximized_migrates_to_music`, `maximized_defaults_migrate_to_lyrics`, `migration_never_overwrites_a_new_key`, `music_with_lyrics_needs_lyrics_but_music_does_not`, `activities_only_shows_only_with_an_activity`.
- [ ] **Step 2-4:** run → FAIL; implement; run → PASS.
- [ ] **Step 5: Checkpoint:** fmt, clippy, test.

### Task 11: Wire the model into the host

**Files:**
- Modify: `src/host/notch.rs` (`is_hidden`, `is_suppressed`, `set_screen_mode`; drop `hides_for_apps`, `keeps_lyrics_over_windows`, `keeps_notch_over_windows` in favor of `maximized_showing` and `fullscreen_showing`), `src/host/tray.rs` and `src/host/window.rs` (remove the three old tray items), `src/host/notch/snapshots.rs`

**Interfaces:**
- Consumes: Task 10 functions; Task 3 `reload_settings` re-reads `notch.maximized` and `notch.fullscreen`.
- Behavior: the window is suppressed only when the current context's showing is `Hidden`; otherwise the notch draws what `allows` permits and hides to the strip otherwise. Regular windows keep `notch.hide_when_idle`.

- [ ] **Step 1: Update the existing screen-mode tests and add** `fullscreen_activities_only_shows_a_banner_but_hides_the_idle_clock`, `maximized_music_shows_without_lyrics`, `maximized_lyrics_hides_music_without_lyrics`, `fullscreen_hidden_suppresses_the_window`.
- [ ] **Step 2-4:** run → FAIL; implement; run → PASS.
- [ ] **Step 5: Checkpoint:** fmt, clippy, test, `scripts/perf.sh` (ask first); update `docs/architecture.md` (hiding section) and `.claude/rules/windows-and-hover.md` if safeguards change.

### Task 12: Behavior section and docs

**Files:**
- Create: `settings/src/sections/behavior.rs`
- Modify: `docs/configuration.md`, `README.md` (maximized and full-screen section, defaults table)

**Interfaces:**
- Produces: dropdowns "Over a maximized window" and "Over a full-screen app" writing `notch.maximized` / `notch.fullscreen` with the four `Showing` words.

- [ ] **Step 1: Write failing test** `the_dropdown_offers_the_four_choices_in_order` for the pure option list.
- [ ] **Step 2-4:** run → FAIL; implement; run → PASS; run the app and change both dropdowns over a real maximized and full-screen window.
- [ ] **Step 5: Checkpoint:** fmt, clippy, test; the README and configuration tables show the new keys and defaults (`lyrics`, `hidden`).

---

## Phase 3: Windows 11 tray menu (spike, then its own plan)

### Task 13: Tray look spike

**Files:**
- Create: `docs/superpowers/spikes/2026-10-30-tray-menu-spike.md`, a throwaway example `scripts/tray_spike.rs`

- [ ] **Step 1:** A popup window (`WS_POPUP`, `WS_EX_TOOLWINDOW`, `WS_EX_NOACTIVATE`) with `DWMWA_SYSTEMBACKDROP_TYPE = DWMSBT_TRANSIENTWINDOW`, rounded corners (`DWMWA_WINDOW_CORNER_PREFERENCE`), a 40 px item height, drawn through Direct2D.
- [ ] **Step 2:** Run on the owner's Windows 11 machine; record whether the backdrop shows with transparency effects on and off, and the extra private working set.
- [ ] **Step 3:** Decide: write a follow-up plan to replace `host/tray.rs`'s menu, or keep the native menu. Delete the example when the decision is recorded.

## Phase 4: Google sign-in (research, then its own plan)

### Task 14: Sign-in research

**Files:**
- Create: `docs/superpowers/spikes/2026-10-30-google-signin-research.md`

- [ ] **Step 1:** From Google's primary docs, record: the scope needed to read events (read-only), whether it is sensitive and what verification a public app needs, the test-mode token lifetime, the desktop flow (loopback redirect, PKCE) and what the owner must create (Cloud project, OAuth client).
- [ ] **Step 2:** Record where the refresh token is stored (Windows Credential Manager) and what the README privacy section must change.
- [ ] **Step 3:** Decide with the owner, then write a follow-up plan. Keep the pasted iCal address as a fallback.

## Self-review

- **Spec coverage:** four projects map to Phases 1-4; window layout (Task 5), instant/commit-on-Enter (Task 6), shared store and notify (Tasks 2, 3, 5), page list (Task 7), matrix and migration (Tasks 10-12), tray and sign-in spikes (Tasks 13-14).
- **Open by design:** Phases 3 and 4 end in a decision and a follow-up plan, because their implementation depends on a measurement and on Google's policy.
