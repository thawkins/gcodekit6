# egui GTK Visual Restore Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the egui front end look and arrange like the GTK front end shown in `crates/gcodekit5-ui-egui/resources/help_images/`, with icons restored.

**Architecture:** Replace the single docking workspace with GTK's top row of eight page tabs. Each page has a fixed layout composed from the existing panes and windows. Icons come from an embedded catalog: the 23 app SVGs plus Tabler lookalikes for the Adwaita icons. The catalog is served through `craft_ui::icons::SvgIconSet` and egui_extras' SVG loader. An Adwaita-like theme pass restores spacing, framed groups and heading weight.

**Tech Stack:** egui 0.37 (storytold fork), craft-ui (`icons`, `tabs`, `theme`), egui_extras SVG loader, egui_kittest snapshots.

**Spec:** the user's request (2026-11-26): "restore the visuals as much as possible, including putting back the icons that have been lost". The reference screenshots are `help_images/*.png` (Spanish UI). The user's decisions:
- Layout: GTK page tabs, with the managers back as pages.
- Icons: MIT-licensed lookalikes (Tabler Icons 3.31.0, MIT).

The current egui captures are in `target/temp/egui-capture/` (untracked).

## Global Constraints

- Same rules as the migration plan (`2026-11-25-gtk4-to-egui-craft-libs-migration.md`, Global Constraints):
  - one egui version;
  - no `unsafe`;
  - no unwrap/expect on user paths;
  - clippy `-D warnings` and fmt clean;
  - tests go in `tests/all.rs`, run with nextest;
  - tests never write into the repo, except reference snapshots under `crates/gcodekit5-ui-egui/tests/snapshots/`.
- Page order and English labels, exactly: Designer, Visualizer, Machine Control, CAM Tools, Device Manager, Device Config, CNC Tools, Materials.
- Menus as GTK: File, Edit, Machine, Help. Commands now under egui-only menus (View, Tools) move into those four. "View > page" items become page tabs.
- Third-party icons go in `crates/gcodekit5-ui-egui/resources/icons/tabler/`, with Tabler's `LICENSE`, and are listed in the repo's third-party notice. Only the SVGs the code uses are checked in.
- Port no new features. A control GTK shows for a feature egui lacks is left out, and the parity report's Gaps list covers it.
- Machine safety behaviour (guards, enable rules, confirmations) is unchanged. Only appearance and placement move.
- Every new user string goes in `msgids()` and every po file.

## Review Focus

1. Every command reachable before is still reachable (menu, shortcut, button) after the managers become pages and the View/Tools menus go.
2. Saved docking layouts from earlier egui runs must not break startup. Ignore or migrate them.
3. At 1280x800 and at HiDPI, no page clips its primary controls: E-STOP, Stop, Connect.
4. The icon tint follows the light and dark themes, and an icon button keeps its accessible label.
5. Unsaved-changes and quit flows still reach the manager pages' dirty state.

---

### Task V1: Icon catalog

**Files:**
- Create: `crates/gcodekit5-ui-egui/src/icons.rs`
- Create: `resources/icons/tabler/*.svg` + `LICENSE`
- Modify: `Cargo.toml` (egui_extras `svg` feature, same egui version), the third-party notice
- Test: `tests/test_icons.rs`

**Interfaces:**
- `pub enum Icon`: one variant per use, such as `Home`, `Unlock`, `Zero`, `Play` and `JogUp`.
- `Icon::image(self) -> egui::Image<'static>`.
- `pub fn icon_button(ui, icon: Icon, label: &str) -> egui::Response`: icon plus text. The accessible name is the label.
- `pub fn icon_only_button(ui, icon, tooltip: &str) -> Response`.
- `pub fn install(ctx)` installs the SVG loader once.
- Source files: the app's own SVGs from `resources/icons/*.svg` (designer tools, gallery), and the Tabler files from `target/temp/tabler-icons/` (already downloaded with user permission).
- Adwaita -> Tabler mapping:
  - **Navigation and jog:** go-previous/up/down/next -> chevron-left/up/down/right; go-top/bottom -> arrow-bar-to-up/down; go-jump -> arrow-forward; go-home -> home.
  - **Machine and streaming:** edit-clear -> backspace; process-stop -> circle-x; media-playback-start/pause/stop -> player-play/pause/stop; view-refresh -> refresh; lock-open -> lock-open.
  - **Files and editing:** document-save/media-floppy -> device-floppy; folder-open -> folder-open; edit-copy -> copy; user-trash -> trash; edit-find-replace -> replace; list-add -> plus; insert-image -> photo-plus; emblem-documents -> files; bookmark-new -> bookmark-plus.
  - **View:** view-reveal/conceal -> eye/eye-off; view-restore -> arrows-minimize; zoom-in/out -> zoom-in/out; zoom-fit-best -> maximize; view-grid -> layout-grid; selection-mode -> list-check.
  - **Objects:** object-group/ungroup -> layers-union/subtract.
  - **Connection and devices:** network-wired -> network; network-offline -> plug-connected-x; network-transmit-receive -> transfer; mail-send -> send; mail-send-receive -> arrows-exchange; preferences-desktop-display -> device-desktop; input-mouse -> mouse; input-touchpad -> hand-finger.
  - **Dialogs and misc:** window-close -> x; emblem-ok -> circle-check; dialog-question -> help-circle; dialog-error -> alert-circle; camera-photo -> camera; accessories-calculator -> calculator; media-playlist-shuffle -> arrows-shuffle.

- [ ] Test: every `Icon` resolves to a non-empty SVG in the catalog, and the SVG decodes through egui's loader in a kittest harness. `icon_button` exposes its label to accesskit.
- [ ] Implement, wire `install` at app start, commit `feat(ui-egui): embedded icon catalog`.

### Task V2: Page shell, menus and status bar

**Files:** `workspace.rs` (or a new `pages.rs`), `app.rs`, `menu.rs`, `commands.rs`, `status_bar.rs`, `app/machine_windows.rs`, `app/library_windows.rs`.

- A top tab row of equal-width tabs across the window, in the GTK order. The active tab is filled. Switching pages keeps each pane's state.
- Page contents:
  - **Designer:** toolbox | canvas | inspector over the objects list.
  - **Visualizer:** sidebar | view.
  - **Machine Control:** left sidebar | centre controls | editor above the console.
  - **CAM Tools:** the CAM pane.
  - **Device Manager**, **Device Config** (Firmware Settings), **CNC Tools** (Tools manager) and **Materials:** the former windows, shown as pages.

  The G-code editor is no longer a separate tab; it lives in the Machine Control right column, as in GTK.
- Status bar as GTK: eStop button | connection state | active profile `<name> | <firmware> | <type>`.
- Menus: File, Edit, Machine, Help, with every former View/Tools command re-homed (Review Focus 1).
- [ ] Tests:
  - each page is reachable by its tab label;
  - each manager's dirty state still reaches the quit prompt;
  - no command lost: every `Command` in the catalog is reachable from a menu item, shortcut or page control;
  - an old saved docking layout is ignored and startup still succeeds.
- [ ] Commit `feat(ui-egui): GTK page tabs replace the docking workspace`.

### Task V3: Adwaita-like theme

**Files:** `theme.rs`.

- Spacing and margins: GTK pages have 12 px outer margins and 6 px between rows.
- Bold section headings. Rounded framed groups (Designer inspector, Machine Type).
- Button fill and rounding like Adwaita dark and light. Accent blue `#3584e4` for primary buttons such as Connect and checked boxes; destructive red for Stop and Zero All.
- [ ] Snapshot test of a sample of widgets in dark and light. Commit.

### Task V4: Machine Control page

Match `help_images/machine_control.png`:
- **Left column:** port/baud/state line; Connection (port combo, Connect primary button with icon, refresh icon); Machine State (large state text, feed/spindle/buffer); Home/Unlock with icons; Work Coordinates G54-G59 grid of 3x2 with icons; Job Control (Send, Stop, Pause, Resume with icons; Stop destructive).
- **Centre column:** Current Feed & Spindle override rows (-10 -1 reset +1 +10, spindle stop); large WX/WY/WZ monospace readout with Zero icon buttons; Zero All Axes (destructive) and Go to Work Zero + Include Z; Machine Coordinates line; Step/Jog Feed row; jog pad with chevron icons; the E-STOP button showing `images/eStop2.png`.
- **Right column:** G-code editor above the device console, with icon buttons for clear, copy and help.
- Keep every guard and enable rule exactly as now.
- [ ] Snapshot test at 1440x900 dark. Existing machine-control tests still pass. Commit.

### Task V5: Designer page

Match `help_images/designer.png`:
- **Left:** a 3-column icon grid of the toolbox using the app SVGs (select, grab, rectangle, circle, line, ellipse, triangle, polygon, polyline, text, gear, sprocket); Quick shapes with the Shape Gallery button (gallery cards get their SVG icons); Machine Type group; Tool and Stock settings buttons; Generate G-Code / Frame group; View controls (Show Grid, Grid Spacing, Snap, threshold).
- **Canvas:** floating icon toolbar at the top centre; status line overlay at the bottom (zoom, center, cursor, units).
- **Right:** Inspector (Size, Laser/Cut properties in framed groups, Hide toggle) above Objects (list rows with icon, number, kind, name; icon buttons for move to top, up, down and bottom).
- [ ] Snapshot test. Existing designer tests still pass. Commit.

### Task V6: Visualizer page

Match `help_images/visualizer.png`:
- **Left sidebar:** icon toolbar (fit, centre, top view, Hide); collapsible Toolpath / Guides / Inspector sections with the existing toggles.
- **View:** the navigation grid at top right restyled as GTK (T/L/F/R/B, corner icons, - Fit +); bottom status overlay.
- Controls for gaps (stock simulation, run preview) stay out.
- [ ] Snapshot test. Commit.

### Task V7: CAM Tools and manager pages, final pass

- Restyle the CAM tool cards and pages, and the four manager pages, with the theme and icons.
- Compare each page with the matching help image and record the remaining differences in `docs/reports/egui-parity.md` (Visual section).
- [ ] Snapshot per page. Full `cargo nextest run -p gcodekit5-ui-egui -p gcodekit5-app`. Commit.
