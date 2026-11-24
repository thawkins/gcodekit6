# GTK4 to egui + craft-libs Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the GTK4/libadwaita/sourceview5 front end of GCodeKit5 with an egui/eframe front end built on the `storytold/craft-libs` crates, and move every GitHub Actions workflow, packaging recipe and helper script off the GTK system stack.

**Architecture:** Strangler migration. A new crate `gcodekit5-ui-egui` grows beside the existing `gcodekit5-ui` (GTK) crate; toolkit-free logic currently trapped in `gcodekit5-ui` is first extracted into a new `gcodekit5-app` crate that both front ends consume. The binary selects the front end with a Cargo feature until parity is reached, then the egui front end becomes the default and the GTK crate, its system dependencies and its CI/packaging steps are deleted.

**Tech Stack:** Rust 2024 / MSRV 1.95, egui + eframe + egui_extras + egui_kittest 0.37 (wgpu renderer), craft-libs (git, pinned rev): `craft-ui`, `craft-window`, `craft-menu`, `craft-keymap`, `craft-dialogs`, `craft-platform`, `craft-quit`, `craft-settings`, `craft-i18n`, `craft-svg`, `craft-dxf`, `craft-mesh`, `craft-update` (optional). cargo-nextest, cargo-deny, GitHub Actions.

**Spec:** This document plus the user request of 2026-11-25 ("migrate from gtk4 to a combination of egui and the storytold craft-libs; address the GitHub Actions too"). Existing behaviour reference: `SPEC.md`, `GTK4.md`, `docs/user/`.

---

## Baseline (surveyed 2026-11-25)

| Area | Today | Impact |
|---|---|---|
| `crates/gcodekit5-ui` | 47,817 lines / 92 files; deps `gtk4 0.11`, `libadwaita 0.9`, `sourceview5 0.11`, `glib`/`gio 0.22`, `glow 0.18`, `rfd 0.18`, `gettext-rs`; `build.rs` compiles `po/*.po` with `msgfmt` and `glib-build-tools` gresources | Rewritten |
| Other crates (`core`, `communication`, `camtools`, `designer`, `visualizer`, `settings`, `devicedb`) | 51k lines, zero GTK references | Untouched, except `settings` (gettext-rs) |
| Largest GTK modules | `visualizer/mod.rs` 3413, `machine_control/mod.rs` 3044, `designer_canvas/input.rs` 1543, `gtk_app.rs` 1533, `designer_canvas/rendering.rs` 1260, `tools_manager/mod.rs` 1259 | Biggest port tasks |
| Unsafe sites (AGENTS-LANGUAGE.md table) | 29 OpenGL blocks, 14 `glib::ObjectExt::set_data` blocks, i18n `set_var`, Win32 dialog parent FFI | All disappear with GTK; table must be rewritten |
| Workflows | `code-quality.yml`, `msrv.yml`, `mutation-testing.yml`, `test-coverage.yml`, `release.yml` all `apt-get install libgtk-4-dev libadwaita-1-dev libgtksourceview-5-dev` | Every one changes |
| Packaging | Flatpak on `org.gnome.Platform//50`; AppImage via `linuxdeploy-plugin-gtk`; macOS `scripts/macos-bundle-frameworks.sh`; Windows MSYS2 UCRT64 + ~80 GTK DLLs + generated `wix/gtk-runtime.wxs` | Simplified to a single static-ish binary |
| `deny.toml` | `unknown-git = "deny"`, `allow-git` only `bbqsrc/core2` | Must allow `storytold/craft-libs` |
| craft-libs | `https://github.com/storytold/craft-libs`, latest cached rev `d15bf1f35b17b759870f9e5111b9a5d243f64888` (2026-11-14), egui 0.37, rust-version 1.95, MIT OR Apache-2.0 | Pin by `rev` |

## Decisions to confirm before Task 1

All six defaults below were confirmed by the user on 2026-11-25.

| # | Decision | Default in this plan | Alternative |
|---|---|---|---|
| D1 | eframe renderer | **wgpu** (matches craft-libs/WordCraft, Metal on macOS, Vulkan/DX12 elsewhere); visualizer shaders ported GLSL to WGSL via `egui_wgpu::CallbackTrait` | glow: keeps existing GLSL and `glow` code via `egui_glow` paint callbacks; faster port, but diverges from Craft suite and keeps OpenGL `unsafe` |
| D2 | Translations | Keep `po/*.po` as source of truth; parse at build time into an embedded table served through a `craft_i18n`-compatible lookup; drop `gettext-rs` and the system `libintl`/`msgfmt` dependency | Keep `gettext-rs` (keeps a C dependency and `msgfmt` in CI) |
| D3 | Settings storage | Keep `gcodekit5-settings` types; move file I/O to `craft-settings` (atomic save, backup, migration) with a one-shot migration from the existing JSON config path | Leave settings I/O as is |
| D4 | Windows toolchain | `x86_64-pc-windows-msvc` on `windows-latest` (no MSYS2) | Keep `-gnu` |
| D5 | Flatpak runtime | `org.freedesktop.Platform//25.08` | Keep GNOME runtime (larger, unneeded) |
| D6 | Binary/package name | Keep `gcodekit5` binary and `com.github.thawkins.gcodekit5` app id | Rename |

## Global Constraints

- Edition 2024, `rust-version = "1.95"` inherited by every crate (`msrv.yml` `msrv-verify` job enforces `rust-version.workspace = true`).
- All egui-family crates on one version, the one craft-libs uses at the pinned rev (0.37 at `d15bf1f`). Never mix two egui versions in `Cargo.lock`.
- craft-libs referenced only by `git = "https://github.com/storytold/craft-libs", rev = "<sha>"`, the same `rev` everywhere, declared once in `[workspace.dependencies]`.
- `#![deny(unsafe_code)]` on every new crate root. The egui front end targets zero `unsafe` (D1 = wgpu makes this possible).
- No `.unwrap()`/`.expect()` on user-facing paths; `code-quality.yml` `clippy-unwrap-check` fails on any.
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check` clean after every task.
- Tests live in `crates/<crate>/tests/test_*.rs`, included from the crate's single `tests/all.rs`; `autotests = false`. Run with `cargo nextest run`.
- Tests never write into the repository; snapshot output goes to `target/temp/` unless it is a checked-in reference image under `crates/gcodekit5-ui-egui/tests/snapshots/`.
- Internal units: millimetres as `f32`, UTC datetimes, UTF-8 text; UI shows 2 decimal places.
- No `println!` in application code; `tracing` only.
- Every public fn has a `///` doc comment; every module a `//!` header.
- Do not push, tag or release without an explicit user instruction.

## Review Focus

1. **Serial streaming while the UI is busy** — a modal dialog, a file dialog or a long redraw must never stall G-code streaming or real-time commands (`!`, `~`, `0x18`). egui repaints only on events; the stream must run off the UI thread and request repaints. Test in Task 8.
2. **Large G-code files (>1M lines)** in the editor and visualizer — the GTK sourceview handled them; a naive `egui::TextEdit` will not. Expect virtualised rendering and no per-frame re-tessellation. Test in Tasks 9 and 10.
3. **Unsaved-changes on quit** — closing the window, Cmd-Q via `craft-quit`, and the OS session ending must all route through the same unsaved-changes prompt (existing `test_unsaved_changes_choice.rs`). Test in Task 7.
4. **Settings from the GTK build** — a user upgrading must keep machine profiles, tool and material libraries, recent files and language choice. Test in Task 3.
5. **HiDPI and mixed-DPI monitors** — canvas hit-testing and jog-pad hit-testing must use egui points, not physical pixels; GTK did the scaling implicitly. Test in Task 11.

---

## File Structure

```
crates/
  gcodekit5-app/                 NEW  toolkit-free application state shared by both front ends
    src/lib.rs                        re-exports
    src/device_status.rs              moved from gcodekit5-ui/src/device_status.rs
    src/console.rs                    moved from gcodekit5-ui/src/ui/console_panel.rs (model part)
    src/device_console_manager.rs     moved
    src/tools_manager_backend.rs      moved
    src/materials_manager_backend.rs  moved
    src/types.rs                      moved (TabbedBoxParams, JigsawPuzzleParams, ...)
    src/command_history.rs            model half of ui/gtk/command_history.rs
    src/machine.rs                    MachineController: owns comms thread, stream state, overrides
    src/i18n.rs                       Translator (D2)
    tests/all.rs, tests/test_*.rs     moved from gcodekit5-ui/tests where they test these modules
  gcodekit5-ui-egui/             NEW  egui front end
    src/lib.rs                        pub fn run(options: LaunchOptions) -> anyhow::Result<()>
    src/app.rs                        GcodeKitApp: eframe::App; owns AppState, Workspace, Keymap
    src/commands.rs                   Command enum + craft_keymap catalog + dispatch
    src/menu.rs                       craft_menu::MenuBar construction
    src/theme.rs                      craft_ui theme + colours (from common/colors.rs, style.css)
    src/workspace.rs                  craft_ui docking layout: panes and their ids
    src/panes/machine_control/*.rs    jog, DRO, overrides, connection, status, direct sender
    src/panes/console.rs
    src/panes/editor/*.rs             G-code editor: virtualised text, highlighting, gutter
    src/panes/visualizer/*.rs         3D view: wgpu callback, camera, nav cube, OSD
    src/panes/designer/*.rs           canvas, toolbox, layers, properties, shape gallery
    src/panes/cam_tools/*.rs          one file per CAM tool page
    src/windows/*.rs                  settings, device manager, tools/materials managers, device info, help, about
    src/dialogs.rs                    craft_dialogs wrapper: open/save/message, unsaved-changes
    tests/all.rs, tests/test_*.rs     egui_kittest tests
    tests/snapshots/                  reference PNGs
  gcodekit5-ui/                  DELETED in Task 16
src/main.rs                      feature-selects front end until Task 15
```

---

## Milestones

| Milestone | Tasks | Exit criterion | Estimate |
|---|---|---|---|
| M0 Foundations | 1-3 | craft-libs builds in workspace and CI; shared crate extracted; GTK app unchanged | 3-4 days |
| M1 Shell | 4-7 | `cargo run --features egui` opens a window with menus, shortcuts, docked empty panes, settings window, quit flow | 1 week |
| M2 Machine | 8 | Connect, jog, stream a file, overrides, console against `scripts/start-grblhal-sim.sh` | 1.5 weeks |
| M3 Editor + Visualizer | 9-11 | Open, edit, save G-code; 3D toolpath view with nav cube | 2 weeks |
| M4 Designer + CAM | 12-13 | Designer canvas parity; all CAM tool pages | 3 weeks |
| M5 Managers + Help | 14 | Device/tools/materials managers, device info, help browser | 1 week |
| M6 Cut-over | 15-16 | egui default; GTK crate, deps, unsafe table entries removed | 2-3 days |
| M7 CI + Packaging | 17-19 | All 5 workflows green without GTK; release builds Linux/macOS/Windows artefacts | 3-4 days |
| M8 Docs + Release | 20 | SPEC/README/QUICKSTART/CHANGELOG/AGENTS updated; release candidate | 1 day |

Total: about 10-11 weeks of one engineer. M2-M5 tasks are independent of each other once M1 lands and can run in parallel sessions (different directories under `src/panes/`).

CI note: Tasks 1-16 keep the existing GTK apt packages in CI (both front ends compile). Task 2 adds the egui system packages alongside. Task 17 removes the GTK packages only after Task 16 deletes the GTK crate.

---

## M0 Foundations

### Task 1: Add craft-libs and egui to the workspace; teach cargo-deny about the git source

**Files:**
- Modify: `Cargo.toml` (`[workspace.dependencies]`)
- Modify: `deny.toml` (`[sources] allow-git`, `[licenses]` if a new licence appears)
- Modify: `.github/dependabot.yml`
- Create: `crates/gcodekit5-ui-egui/Cargo.toml`, `src/lib.rs`, `tests/all.rs`, `tests/test_smoke.rs`
- Modify: `Cargo.toml` `[workspace] members`

**Interfaces:**
- Produces: workspace deps `egui`, `eframe`, `egui_extras`, `egui_kittest`, `egui-wgpu`, `craft-ui`, `craft-window`, `craft-menu`, `craft-keymap`, `craft-dialogs`, `craft-platform`, `craft-quit`, `craft-settings`, `craft-i18n`, `craft-svg`, `craft-dxf`, `craft-mesh`; crate `gcodekit5-ui-egui` with `pub fn run(options: LaunchOptions) -> anyhow::Result<()>` and `pub struct LaunchOptions { pub files: Vec<std::path::PathBuf> }`.

- [ ] **Step 1:** Add to root `[workspace.dependencies]`: egui-family at `0.37` (`eframe` with `default-features = false, features = ["default_fonts", "wgpu", "accesskit", "wayland", "x11"]`), and each craft crate as `{ git = "https://github.com/storytold/craft-libs", rev = "d15bf1f35b17b759870f9e5111b9a5d243f64888" }` (re-check for a newer rev on the day; record the chosen one in `docs/adr/`).
- [ ] **Step 2:** Create `gcodekit5-ui-egui` with `autotests = false`, `[[test]] name = "all"`, all metadata `.workspace = true` including `rust-version.workspace = true`, `#![deny(unsafe_code)]`.
- [ ] **Step 3: Write the failing test** `tests/test_smoke.rs::test_app_renders_empty_frame`: build `egui_kittest::Harness` around a placeholder `GcodeKitApp::default()` UI fn, `harness.run()`, assert `harness.get_by_label("GCodeKit")` exists (window heading).
- [ ] **Step 4:** Run `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_smoke::)'` — expect FAIL (no app). Implement minimal `GcodeKitApp` showing a heading "GCodeKit". Re-run — expect PASS.
- [ ] **Step 5:** `deny.toml`: append `"https://github.com/storytold/craft-libs"` to `allow-git`. Run `cargo deny check` — expect `advisories ok, bans ok, licenses ok, sources ok`. Add any newly flagged licence (craft-libs vendors krilla/hayro; only crates actually pulled in count) with a comment naming the crate.
- [ ] **Step 6:** `dependabot.yml`: add a group `egui` with patterns `["egui", "eframe", "egui_*", "egui-*", "epaint", "emath", "ecolor"]` so they bump together, and ignore egui-family versions above the one craft-libs uses at the pinned rev (comment: bump together with the craft-libs rev).
- [ ] **Step 7:** `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` — clean.
- [ ] **Step 8: Commit** `feat(ui-egui): add egui + craft-libs workspace deps and empty front-end crate`.

### Task 2: CI installs egui's Linux system packages and runs kittest headless

**Files:**
- Modify: `.github/workflows/code-quality.yml`, `msrv.yml`, `mutation-testing.yml`, `test-coverage.yml` (every `Install system dependencies` step)
- Modify: `.config/nextest.toml`

- [ ] **Step 1:** In every Linux `apt-get install` line add (keep the GTK packages for now): `libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libvulkan1 mesa-vulkan-drivers`. `mesa-vulkan-drivers` provides lavapipe so egui_kittest's wgpu renderer works on runners without a GPU.
- [ ] **Step 2:** `nextest` job in `code-quality.yml`: add env `WGPU_BACKEND: vulkan` and `EGUI_KITTEST_SNAPSHOT_THRESHOLD` left unset (default). On snapshot failure upload `crates/gcodekit5-ui-egui/tests/snapshots/**/*.diff.png` and `*.new.png` with `actions/upload-artifact@v7` (`if: failure()`).
- [ ] **Step 3:** `.config/nextest.toml`: add `[test-groups] gpu = { max-threads = 2 }` and `[[profile.default.overrides]]` + `[[profile.ci.overrides]]` with `filter = 'package(gcodekit5-ui-egui)'`, `test-group = "gpu"` (lavapipe is slow; bounding concurrency avoids 150 s timeouts).
- [ ] **Step 4:** `msrv.yml`: replace `cargo test --workspace` with `cargo nextest run --workspace` + `cargo test --workspace --doc` (AGENTS-LANGUAGE.md: nextest is the runner) and add `taiki-e/install-action@nextest`.
- [ ] **Step 5:** Verify locally with `act` if available, otherwise push the branch to a draft PR only when the user says so; expected: all jobs green, `test_smoke::test_app_renders_empty_frame` listed in the nextest output.
- [ ] **Step 6: Commit** `ci: install egui/wgpu system packages and run kittest headless`.

### Task 3: Extract toolkit-free state into `gcodekit5-app`

**Files:**
- Create: `crates/gcodekit5-app/` (see File Structure)
- Move: `gcodekit5-ui/src/{device_status.rs,types.rs}`, `src/ui/{device_console_manager.rs,tools_manager_backend.rs,materials_manager_backend.rs}`, model half of `src/ui/console_panel.rs` and `src/ui/gtk/command_history.rs`
- Modify: `gcodekit5-ui` to depend on and re-export `gcodekit5-app` (GTK build keeps compiling unchanged)
- Move tests: `test_command_history.rs`, `test_device_console_manager.rs`, `test_materials_manager_backend.rs`, `test_tools_manager_backend.rs`, `test_report_units.rs`, `test_machine_busy_guard.rs`, `test_job_preferences.rs`, `test_unsaved_changes_choice.rs` (model parts) into `crates/gcodekit5-app/tests/`
- Create: `crates/gcodekit5-app/src/i18n.rs` (D2), `crates/gcodekit5-app/build.rs` (po to embedded table)
- Create: `crates/gcodekit5-settings/src/migrate.rs` (D3)

**Interfaces:**
- Produces:
  - `gcodekit5_app::machine::MachineController` — `fn connect(&mut self, cfg: &ConnectionSettings) -> Result<(), MachineError>`, `fn disconnect(&mut self)`, `fn start_stream(&mut self, program: Arc<[String]>) -> Result<(), MachineError>`, `fn pause(&mut self)`, `fn resume(&mut self)`, `fn stop(&mut self)`, `fn jog(&mut self, axis: Axis, distance_mm: f32, feed_mm_min: f32)`, `fn realtime(&mut self, cmd: RealtimeCommand)`, `fn poll(&mut self) -> Vec<MachineEvent>`, `fn set_repaint_hook(&mut self, hook: Box<dyn Fn() + Send + Sync>)`. Owns a worker thread over `gcodekit5-communication`; the UI never blocks on I/O.
  - `gcodekit5_app::i18n::Translator` — `fn new(lang: &str) -> Self`, `fn tr(&self, msgid: &str) -> Cow<'static, str>`, `fn trn(&self, singular: &str, plural: &str, n: u64) -> Cow<'static, str>`; macro `tr!("...")`.
  - `gcodekit5_settings::migrate::import_legacy(dirs: &ConfigDirs) -> Result<MigrationReport, SettingsError>`.

- [ ] **Step 1:** Move the modules with `git mv`; fix paths; GTK crate imports them from `gcodekit5_app`. Run `cargo nextest run -p gcodekit5-ui -p gcodekit5-app` — every previously passing test still passes.
- [ ] **Step 2: Write the failing test** `test_machine_controller::test_stream_does_not_block_caller`: connect to an in-memory loopback transport that delays each `ok` by 50 ms, `start_stream` 100 lines, assert `start_stream` returns in < 10 ms and `poll()` eventually yields `MachineEvent::StreamFinished { lines: 100 }`. Also `test_realtime_bypasses_queue`: during the stream, `realtime(FeedHold)` reaches the transport before the next queued line.
- [ ] **Step 3:** Implement `MachineController` by lifting stream logic out of `ui/gtk/machine_control/{grbl_stream.rs,state.rs,operations.rs,continuous_jog.rs,overrides.rs}` (no GTK types survive). Tests pass.
- [ ] **Step 4: Write the failing tests** `test_i18n::test_po_lookup_de` (`Translator::new("de").tr("Connect")` equals the `msgstr` in `po/de.po`), `test_i18n::test_missing_falls_back_to_msgid`, `test_i18n::test_plural_forms`. Replace `test_translation_catalogs.rs` / `test_i18n_locale_env.rs` expectations accordingly.
- [ ] **Step 5:** Implement `build.rs` parsing `../../po/*.po` (use the `polib` crate; no `msgfmt`) into a generated `phf`/sorted-slice table; implement `Translator`. Tests pass.
- [ ] **Step 6: Write the failing test** `test_settings_migrate::test_import_gtk_config`: copy fixture `tests/fixtures/gtk_config/` (machine profiles, tool library, materials, recent files, `language = "fr"`) into a `target/temp/<unique>` dir, run `import_legacy`, assert every value round-trips and a second run is a no-op (`MigrationReport::AlreadyMigrated`).
- [ ] **Step 7:** Implement with `craft-settings` (atomic save, backup). Tests pass.
- [ ] **Step 8:** `cargo clippy` / `cargo fmt --check` clean; `cargo run` (GTK) still starts and connects to the grblHAL simulator.
- [ ] **Step 9: Commit** `refactor: extract toolkit-free state into gcodekit5-app`.

---

## M1 Shell

### Task 4: App frame, window policy, theme, docking workspace

**Files:**
- Create: `src/app.rs`, `src/theme.rs`, `src/workspace.rs`
- Modify: root `Cargo.toml` (`[features] egui = ["dep:gcodekit5-ui-egui"]`, `gtk = ["dep:gcodekit5-ui"]`, `default = ["gtk"]`), `src/main.rs`
- Test: `tests/test_workspace.rs`

**Interfaces:**
- Consumes: `gcodekit5_app::*`, `craft_window` (frame policy, title bar), `craft_ui::docking`, `craft_ui::theme`, `craft_platform::appearance`.
- Produces: `GcodeKitApp::new(cc: &eframe::CreationContext, state: AppState) -> Self`; `enum PaneId { MachineControl, Console, Editor, Visualizer, Designer, DesignerLayers, DesignerProperties, CamTools }`; `Workspace::default_layout() -> Workspace`; `Workspace::show(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext)`; trait `Pane { fn id(&self) -> PaneId; fn title(&self, tr: &Translator) -> String; fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext); }`.

- [ ] **Step 1: Write failing tests** `test_workspace::test_default_layout_has_all_panes` (every `PaneId` reachable by its translated tab label in a kittest harness) and `test_workspace::test_layout_persists` (serialize, deserialize, equal).
- [ ] **Step 2:** Implement. Window decoration decided by `craft_window` policy; light/dark follows `craft_platform::appearance` with the user override from `UiSettings`. Port colours from `ui/gtk/common/colors.rs` and `style.css` into `theme.rs`.
- [ ] **Step 3:** `src/main.rs`: `#[cfg(feature = "egui")]` calls `gcodekit5_ui_egui::run`, `#[cfg(feature = "gtk")]` the existing path; `compile_error!` if both or neither.
- [ ] **Step 4:** Run tests (PASS); `cargo run --no-default-features --features egui` shows the docked empty panes.
- [ ] **Step 5: Commit** `feat(ui-egui): app frame, theme and docking workspace`.

### Task 5: Commands, keymap and menus

**Files:**
- Create: `src/commands.rs`, `src/menu.rs`
- Test: `tests/test_commands.rs`

**Interfaces:**
- Consumes: `craft_keymap::{Keymap, Chord, CommandRef}`, `craft_menu::{MenuBar, MenuRole, NativeMenu}`.
- Produces: `enum Command` (every action in `ui/gtk/common/accelerators.rs` and the GTK menus: `FileNew`, `FileOpen`, `FileSave`, `FileSaveAs`, `Quit`, `Undo`, `Redo`, `Connect`, `Disconnect`, `StreamStart`, `StreamPause`, `StreamStop`, `FeedHold`, `CycleStart`, `SoftReset`, `Home`, `Unlock`, `JogXPos`...`JogZNeg`, `ZoomFit`, `ViewTop`/`ViewFront`/`ViewIso`, `OpenSettings`, `OpenDeviceManager`, `OpenToolsManager`, `OpenMaterialsManager`, `OpenHelp`, `About`); `fn keymap() -> Keymap`; `fn menu_bar(tr: &Translator) -> MenuBar`; `GcodeKitApp::dispatch(&mut self, cmd: Command)`.

- [ ] **Step 1: Write failing tests**: `test_every_gtk_accelerator_bound` (table of the chords from `accelerators.rs` copied into the test as literals; each maps to the same `Command`), `test_no_keymap_conflicts` (`Keymap::conflicts()` empty), `test_menu_contains_every_command` (except jog commands), `test_ctrl_o_dispatches_file_open` (kittest key press, assert a pending dialog request exists).
- [ ] **Step 2:** Implement; macOS uses `NativeMenu`, other platforms the in-window bar from `craft_menu::layout`.
- [ ] **Step 3:** Tests PASS. **Commit** `feat(ui-egui): command catalog, keymap and menus`.

### Task 6: Non-blocking dialogs and settings window

**Files:**
- Create: `src/dialogs.rs`, `src/windows/settings.rs`, `src/windows/about.rs`
- Port from: `ui/gtk/settings.rs`, `ui/gtk/file_dialog.rs`, `ui/gtk/common/dialog*.rs`, `ui/gtk/config_settings/*` (firmware settings move to Task 14)
- Test: `tests/test_settings_window.rs`, `tests/test_dialogs.rs`

**Interfaces:**
- Consumes: `craft_dialogs` (queued, answered on a later frame), `craft_ui::combo`, `gcodekit5_settings::SettingsManager`.
- Produces: `Dialogs::open_file(&mut self, purpose: FilePurpose, filters: &[FileFilter])`, `Dialogs::save_file(...)`, `Dialogs::confirm(&mut self, req: ConfirmRequest)`, `Dialogs::poll(&mut self) -> Option<DialogAnswer>`; `SettingsWindow::show(&mut self, ctx: &egui::Context, settings: &mut SettingsManager, tr: &Translator) -> SettingsOutcome`.

- [ ] **Step 1: Write failing tests**: `test_settings_window::test_units_toggle_persists` (switch mm to inch, Apply, reload `SettingsManager` from the `target/temp` dir, value persisted), `test_settings_window::test_cancel_discards`, `test_settings_window::test_language_change_requests_restart` (replaces `test_i18n_language_restart.rs`), `test_dialogs::test_request_survives_frames` (request on frame 1, answer injected via the craft-dialogs test hook on frame 3, delivered once).
- [ ] **Step 2:** Implement. Tests PASS. **Commit** `feat(ui-egui): dialogs and settings window`.

### Task 7: Quit and unsaved-changes flow

**Files:**
- Modify: `src/app.rs`
- Test: `tests/test_quit.rs`

**Interfaces:**
- Consumes: `craft_quit`, `gcodekit5_app::UnsavedChangesChoice` (moved in Task 3), `Dialogs::confirm`.
- Produces: `GcodeKitApp::request_quit(&mut self, source: QuitSource)` where `enum QuitSource { WindowClose, MenuQuit, Os }`.

- [ ] **Step 1: Write failing tests** for each `QuitSource` x {editor dirty, designer dirty, streaming in progress, clean}: dirty shows the prompt; `Cancel` keeps the app open (`ViewportCommand::CancelClose` sent); streaming prompts "Stop job and quit?"; clean quits immediately.
- [ ] **Step 2:** Implement via `ctx.input(|i| i.viewport().close_requested())` plus `craft_quit`. Tests PASS. **Commit** `feat(ui-egui): quit flow with unsaved-changes and running-job guard`.

---

## M2 Machine

### Task 8: Machine control, DRO, jog, overrides, console, status bar

**Files:**
- Create: `src/panes/machine_control/{mod.rs,connection.rs,dro.rs,jog.rs,overrides.rs,stream.rs,direct_sender.rs}`, `src/panes/console.rs`, `src/status_bar.rs`
- Port from: `ui/gtk/machine_control/*` (UI parts only), `ui/gtk/device_console.rs`, `ui/console_panel.rs`, `ui/gtk/status_bar.rs`, `ui/gtk/osd_format.rs`
- Test: `tests/test_machine_pane.rs`, `tests/test_console_pane.rs`

**Interfaces:**
- Consumes: `MachineController` (Task 3), `Command` (Task 5).
- Produces: `MachineControlPane::new(controller: Arc<Mutex<MachineController>>) -> Self` implementing `Pane`; `ConsolePane` implementing `Pane`.

- [ ] **Step 1: Write failing tests** against a `MachineController` built on the loopback transport: `test_connect_button_connects`, `test_jog_x_positive_sends_jog` (asserts `$J=G91 G21 X1.00 F1000` written), `test_dro_shows_two_decimals` (status `MPos:1.23456,...` shows `1.23`), `test_feed_override_plus_10`, `test_stream_progress_updates_while_dialog_open` (Review Focus 1: open settings window, advance 20 frames, progress counter still increases), `test_console_filters_status_reports`.
- [ ] **Step 2:** Implement. The controller's repaint hook calls `ctx.request_repaint()`; the pane never locks the controller across an I/O call.
- [ ] **Step 3:** Tests PASS. Manual check: `scripts/start-grblhal-sim.sh`, connect, home, jog, stream `assets/` sample, feed hold, resume, soft reset. **Commit** `feat(ui-egui): machine control, console and status bar`.

---

## M3 Editor + Visualizer

### Task 9: G-code editor

**Files:**
- Create: `src/panes/editor/{mod.rs,buffer.rs,highlight.rs,gutter.rs,find.rs}`
- Port from: `ui/gtk/editor.rs` (sourceview5 replacement)
- Test: `tests/test_editor.rs`

**Interfaces:**
- Produces: `EditorPane` implementing `Pane`; `EditorBuffer { fn load(path: &Path) -> Result<Self>; fn text(&self) -> &str; fn is_dirty(&self) -> bool; fn current_line(&self) -> usize }`; `fn highlight(line: &str) -> egui::text::LayoutJob` (G/M words, coordinates, comments, line numbers coloured from `theme.rs`).

- [ ] **Step 1: Write failing tests**: `test_open_save_round_trip`, `test_dirty_flag`, `test_highlight_classes` (`G1 X10 ; cut` yields 4 spans with expected colours), `test_find_next`, `test_current_line_follows_stream` (streaming line 500 scrolls it into view), `test_large_file_frame_time` (Review Focus 2: load 1,000,000 generated lines into `target/temp`, one frame renders in < 50 ms in release, `#[ignore]` in debug with a `--run-ignored` CI line in the nextest job).
- [ ] **Step 2:** Implement with `egui::TextEdit` + `layouter` for small files and a virtualised read-only view (`ScrollArea::show_rows`) above 50,000 lines with an "Edit anyway" switch. Tests PASS. **Commit** `feat(ui-egui): G-code editor`.

### Task 10: 3D visualizer (wgpu)

**Files:**
- Create: `src/panes/visualizer/{mod.rs,camera.rs,pipeline.rs,shaders/toolpath.wgsl,shaders/grid.wgsl,nav_cube.rs,osd.rs}`
- Port from: `ui/gtk/visualizer/mod.rs`, `ui/gtk/renderer_3d.rs`, `ui/gtk/shaders.rs`, `ui/gtk/nav_cube.rs`
- Test: `tests/test_visualizer.rs`, `tests/snapshots/visualizer_*.png`

**Interfaces:**
- Consumes: `gcodekit5_visualizer` toolpath model (unchanged), `egui_wgpu::CallbackTrait`, `glam`.
- Produces: `VisualizerPane` implementing `Pane`; `Camera { fn orbit(&mut self, dx: f32, dy: f32); fn pan(...); fn zoom(...); fn fit(&mut self, bounds: Aabb); fn view(&mut self, v: StdView); fn view_proj(&self, aspect: f32) -> glam::Mat4 }`.

- [ ] **Step 1: Write failing tests**: camera math unit tests (`test_fit_contains_bounds`, `test_view_top_looks_down_z`), `test_nav_cube_click_front_sets_view` (kittest click), snapshot tests `visualizer_iso_sample.png`, `visualizer_top_sample.png` for a fixed sample program.
- [ ] **Step 2:** Port the GLSL in `shaders.rs` to WGSL; upload vertex buffers once per program load (not per frame); progress-colour by streamed line via a uniform. Tests PASS. **Commit** `feat(ui-egui): wgpu 3D visualizer and nav cube`.

### Task 11: HiDPI correctness pass for editor/visualizer input

**Files:**
- Test: `tests/test_hidpi.rs`

- [ ] **Step 1: Write failing tests** with kittest `pixels_per_point` of `1.0`, `1.5`, `2.0`: nav cube face hits, editor gutter click selects the right line, jog pad button hit areas — identical outcomes at every scale (Review Focus 5).
- [ ] **Step 2:** Fix any point/pixel mixing. Tests PASS. **Commit** `test(ui-egui): HiDPI hit-testing`.

---

## M4 Designer + CAM

### Task 12: Designer canvas, toolbox, layers, properties, shape gallery

**Files:**
- Create: `src/panes/designer/{mod.rs,canvas/{mod.rs,input.rs,rendering.rs,editing.rs,toolpath_preview.rs,array_dialog.rs},toolbox.rs,layers.rs,properties/{mod.rs,builders.rs,update.rs,handlers/*.rs},shape_gallery.rs,file_ops.rs}`
- Port from: `ui/gtk/designer*`, `ui/gtk/designer_canvas/*`, `ui/gtk/designer_properties/*`, `ui/gtk/fast_shape_gallery.rs`, `ui/gtk/designer/file_ops.rs`
- Use: `craft-svg` and `craft-dxf` for import where they replace hand-rolled readers in `gcodekit5-designer` only if behaviour-identical (separate commit, existing designer tests must pass unchanged)
- Test: `tests/test_designer_*.rs`, `tests/snapshots/designer_*.png`

**Interfaces:**
- Consumes: `gcodekit5_designer` model (unchanged), `egui::Painter` (shapes are 2D: no GPU callback needed), `lyon` tessellation already in the designer crate.
- Produces: `DesignerPane`, `LayersPane`, `PropertiesPane` implementing `Pane`; `CanvasView { fn screen_to_world(&self, p: egui::Pos2) -> glam::Vec2; fn world_to_screen(&self, p: glam::Vec2) -> egui::Pos2 }`.

- [ ] **Step 1: Write failing tests** mirroring every interaction in `designer_canvas/input.rs`: draw rectangle by drag, select, multi-select with shift, move, resize by handle, rotate, delete, undo/redo, snap-to-grid, zoom-at-cursor keeps the world point under the cursor fixed, array dialog creates N copies, property edit updates shape, layer visibility hides shape, import SVG, import DXF, export G-code via toolpath preview. One snapshot per tool.
- [ ] **Step 2:** Implement incrementally (one commit per sub-area: canvas+input, toolbox, layers, properties, gallery, file ops). Tests PASS after each. 

### Task 13: CAM tool pages

**Files:**
- Create: `src/panes/cam_tools/{mod.rs,common.rs,bitmap_engraving.rs,drill_press.rs,gerber.rs,jigsaw.rs,probe_routines.rs,speeds_feeds.rs,spoilboard_grid.rs,spoilboard_surfacing.rs,tabbed_box.rs}`
- Port from: `ui/gtk/cam_tools/*`
- Test: `tests/test_cam_<tool>.rs` (one per tool)

**Interfaces:**
- Consumes: `gcodekit5_camtools` generators and `gcodekit5_app::types::*Params` (unchanged).
- Produces: `CamToolsPane` implementing `Pane`; per tool `fn show(ui: &mut egui::Ui, params: &mut <Tool>Params, tr: &Translator) -> CamAction` where `enum CamAction { None, Generate, SendToEditor, Preview }`.

- [ ] **Step 1: Write failing tests** per tool: default params render; invalid input (negative depth, zero tool diameter) shows inline error and disables Generate; Generate produces G-code byte-identical to the GTK path for the same params (fixtures captured from the GTK build before deletion into `crates/gcodekit5-ui-egui/tests/fixtures/cam/*.gcode`).
- [ ] **Step 2:** Implement; one commit per tool. Tests PASS.

---

## M5 Managers + Help

### Task 14: Device manager, firmware settings, tools/materials managers, device info, help browser

**Files:**
- Create: `src/windows/{device_manager.rs,firmware_settings.rs,tools_manager.rs,materials_manager.rs,device_info.rs,help.rs}`
- Port from: `ui/gtk/device_manager/*`, `ui/gtk/config_settings/{grbl_settings.rs,repetier_settings.rs,operations.rs}`, `ui/gtk/tools_manager/*`, `ui/gtk/materials_manager.rs`, `ui/gtk/device_info.rs`, `ui/gtk/help_browser.rs`
- Resources: `crates/gcodekit5-ui/resources/{markdown,help_images,icons,images}` move to `crates/gcodekit5-ui-egui/resources/` and are embedded with `include_bytes!`/`include_str!` (replaces gresources)
- Test: `tests/test_<window>.rs`

- [ ] **Step 1: Write failing tests**: filter in firmware settings (replaces `test_config_setting_filter.rs`), write `$110=5000` sends exactly that line, tools CRUD round-trip, materials CRUD round-trip, device profile add/edit/delete, help opens topic by id and renders headings and images (`egui_commonmark` or `craft-ui` markdown if present at the pinned rev).
- [ ] **Step 2:** Implement. Tests PASS. **Commit** per window.

---

## M6 Cut-over

### Task 15: Make egui the default front end

**Files:**
- Modify: root `Cargo.toml` (`default = ["egui"]`), `README.md` run instructions

- [ ] **Step 1:** Parity checklist: walk `docs/user/` and `SPEC.md` feature list; every row has an egui test or a recorded manual check in `docs/reports/egui-parity.md`.
- [ ] **Step 2:** `cargo nextest run --workspace --no-fail-fast 2>&1 | tee target/temp/nextest-cutover.log` — all pass.
- [ ] **Step 3: Commit** `feat: egui front end is the default`.

### Task 16: Delete the GTK front end

**Files:**
- Delete: `crates/gcodekit5-ui/` (after moving any remaining non-GTK test into `gcodekit5-app`), `src/main.rs` feature switch, `glib_log_filter`, `test_glib_log_filter.rs`, `test_ui_leaks.rs` (GTK-specific), `slint_legacy_tests` feature
- Modify: `crates/gcodekit5-settings/Cargo.toml` and `src/persistence.rs:16-19` (drop the `gettextrs::gettext` macros and `gettext-rs`; use `Translator`), root `Cargo.toml` members, `Cargo.lock`
- Modify: `src/platform.rs:36` (comment refers to a glib/gtk event loop)
- Modify: `AGENTS-LANGUAGE.md` unsafe-sites table (remove all four GTK rows; table becomes empty or lists only what remains)
- Modify: `rustfmt.toml`/`clippy.toml` only if they reference GTK paths
- Optionally rename `gcodekit5-ui-egui` to `gcodekit5-ui` in a separate commit (pure rename)

- [ ] **Step 1:** `cargo tree -i gtk4`, `cargo tree -i glib`, `cargo tree -i gettext-sys` — each reports "did not match any packages".
- [ ] **Step 2:** `rg -n 'unsafe' crates src --glob '*.rs'` — only `#![deny(unsafe_code)]` lines remain (or documented survivors).
- [ ] **Step 3:** Full clippy, fmt, nextest, `cargo test --doc`, `cargo deny check` — clean.
- [ ] **Step 4: Commit** `chore: remove GTK4 front end`.

---

## M7 CI + Packaging

### Task 17: Remove GTK from every workflow

**Files:** `.github/workflows/{code-quality,msrv,mutation-testing,test-coverage}.yml`

- [ ] **Step 1:** In every `Install system dependencies` step replace the package list with exactly: `libudev-dev pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libvulkan1 mesa-vulkan-drivers` (plus `libdbus-1-dev` if `craft-dialogs`' xdg-portal path needs it — confirm with a clean container build). Drop `gettext` (D2). Jobs that build only library crates (`mutation-testing` core crates, `test-coverage` coverage job) need only `libudev-dev pkg-config`.
- [ ] **Step 2:** Bump cache keys (`-cargo-clippy-v2-`, etc.) so GTK-era `target/` caches are not restored.
- [ ] **Step 3:** `test-coverage.yml` `test-isolation` job: replace `cargo test --workspace --lib` with `cargo nextest run --workspace --lib` twice (keeps intent, matches the runner rule).
- [ ] **Step 4:** Add a `check-no-gtk` step to `code-quality.yml`: `! cargo tree --workspace -e normal | grep -E '^.*(gtk4|libadwaita|sourceview5|glib-sys) '` — fails if GTK creeps back.
- [ ] **Step 5:** Expected after push (user-authorised): Code Quality, MSRV, Test Coverage green; Mutation Testing via `workflow_dispatch` green. **Commit** `ci: drop GTK system packages`.

### Task 18: Release workflow and packaging without GTK

**Files:**
- Modify: `.github/workflows/release.yml`
- Modify: `flatpak/com.github.thawkins.gcodekit5.yml` (and the `feveal` variant, or delete it if unused — ask)
- Modify: `wix/main.wxs` (remove `GtkSchemasData`, `GtkRuntimeFiles` refs and comments)
- Delete: `scripts/macos-bundle-frameworks.sh`, `scripts/windows-bundle-gtk.ps1`, `scripts/check-dll-dependencies.ps1`; update `scripts/build-windows-release.ps1`, `scripts/README.md`
- Modify: root `Cargo.toml` `[package.metadata.deb]` (`depends = "$auto"` stays; verify it no longer lists GTK) and `generate-rpm` metadata if present

- [ ] **Step 1 Linux:** remove the GTK/pango/cairo/gdk-pixbuf/sourceview/rsvg packages; keep Task 17's list plus `flatpak flatpak-builder libfuse2`. Flatpak: install `org.freedesktop.Platform//25.08` + `org.freedesktop.Sdk//25.08` + `org.freedesktop.Sdk.Extension.rust-stable//25.08` (D5). AppImage: drop `--plugin gtk` and the `linuxdeploy-plugin-gtk.sh` download; keep linuxdeploy for desktop/icon integration.
- [ ] **Step 2 macOS:** delete the `brew install gtk4 ...` step and `PKG_CONFIG_PATH`; delete the `macos-bundle-frameworks.sh` call; add `CFBundleIconFile` with an `.icns` generated from the PNG via `iconutil` in the step; keep `macos-create-dmg.sh`. Add `LSMinimumSystemVersion` matching wgpu's Metal floor (11.0).
- [ ] **Step 3 Windows:** delete the MSYS2 setup, `Set Windows Environment`, the DLL-copy loop and the `Generate WiX Component Groups` step; target `x86_64-pc-windows-msvc` (D4) with plain `cargo build --release --target x86_64-pc-windows-msvc`; `cargo wix --package gcodekit5-design --no-build` (fix the stale `--package gcodekit5`: the package is `gcodekit5-design`); portable zip contains just `gcodekit5.exe` + `License.rtf`.
- [ ] **Step 4:** Add a `release-dry-run` `workflow_dispatch` trigger to `release.yml` that runs the build matrix and uploads artifacts without the `softprops/action-gh-release` steps (guard them with `startsWith(github.ref, 'refs/tags/')`, which they already have). Expected: all three matrix legs green; artefacts: `.deb`, `.rpm`, `.flatpak`, `.AppImage`, `.dmg`, `.msi`, `-portable.zip`.
- [ ] **Step 5:** Smoke-test each artefact on a clean VM (or ask the user to): app launches, connects to the simulator, opens a file.
- [ ] **Step 6: Commit** `ci(release): build and package without GTK`.

### Task 19: Supporting repo config

**Files:** `.github/dependabot.yml`, `deny.toml`, `.gitignore`, `.config/nextest.toml`, `.github/PULL_REQUEST_TEMPLATE.md` / `pull_request_template.md`

- [ ] **Step 1:** Dependabot: remove any GTK-specific ignore/group entries (none today); confirm the `egui` group from Task 1; dependabot cannot bump a git `rev`, so add a monthly manual step to `scripts/monthly-dependency-review.sh`: "check storytold/craft-libs for a new rev; bump rev and egui-family together".
- [ ] **Step 2:** `deny.toml`: remove `bans` skip entries that only existed for glib/gtk duplicates; run `cargo deny check` clean.
- [ ] **Step 3:** `.gitignore`: add `crates/gcodekit5-ui-egui/tests/snapshots/**/*.new.png` and `*.diff.png`.
- [ ] **Step 4:** PR templates: replace any "tested on GTK" checklist line with "kittest snapshots updated if UI changed".
- [ ] **Step 5: Commit** `chore: repo config for egui front end`.

---

## M8 Docs + Release

### Task 20: Documentation and release candidate

**Files:** `SPEC.md`, `README.md`, `QUICKSTART.md`, `STATS.md`, `CHANGELOG.md`, `ARCHITECTURE.md`, `DEVELOPMENT.md`, `TOOLCHAIN.md`, `docs/MACOS_BUILD.md`, `docs/WINDOWS_BUILD.md`, `docs/HOWTO.i18n.md`, `GTK4.md` (archive to `docs/archive/`), `AGENTS-LANGUAGE.md` (unsafe table, Technology Stack), new `docs/adr/NNNN-egui-craft-libs.md`

- [ ] **Step 1:** Build-from-source instructions list only `libudev-dev pkg-config` + Wayland/X11 headers on Linux, nothing extra on macOS/Windows.
- [ ] **Step 2:** `docs/HOWTO.i18n.md`: `po/` still the source, no `msgfmt` needed; `scripts/update-po.sh` still uses `xgettext --keyword=tr --keyword=trn:1,2`.
- [ ] **Step 3:** CHANGELOG `## [Unreleased] ### Changed`: "UI rewritten on egui + craft-libs; GTK4, libadwaita and GtkSourceView are no longer required". Version bump to `2.0.0-alpha.1` (breaking: toolkit, config location if D3) only when the user says "push release to remote".
- [ ] **Step 4: Commit** `docs: egui + craft-libs migration`.

---

## Workflow change matrix (quick reference)

| Workflow / file | Task 2 (add) | Task 17/18 (remove GTK) | Other |
|---|---|---|---|
| `code-quality.yml` clippy-unwrap-check, clippy-strict | egui apt pkgs | drop GTK pkgs, new cache key | `check-no-gtk` step |
| `code-quality.yml` nextest | egui apt pkgs, lavapipe, snapshot diff upload | drop GTK + gettext | large-file ignored test run in release |
| `code-quality.yml` fmt-check, unwrap-audit | — | — | unchanged |
| `code-quality.yml` dependency-audit | — | — | `deny.toml` allow-git craft-libs (Task 1) |
| `msrv.yml` | egui apt pkgs; nextest instead of `cargo test` | drop GTK pkgs | `msrv-verify` covers new crates automatically |
| `mutation-testing.yml` | — | reduce to `libudev-dev pkg-config` | unchanged crate list |
| `test-coverage.yml` (coverage disabled, isolation, bench) | egui apt pkgs | drop GTK pkgs; isolation via nextest | — |
| `release.yml` | — | Linux/macOS/Windows rewrite, flatpak runtime, AppImage plugin, MSVC, WiX | `workflow_dispatch` dry run |
| `dependabot.yml` | egui group | — | craft-libs rev bumped manually |
| `flatpak/*.yml`, `wix/main.wxs`, `scripts/*` | — | Task 18 | — |
