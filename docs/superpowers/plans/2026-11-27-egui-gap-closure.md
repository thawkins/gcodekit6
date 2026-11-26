# egui Gap Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port every GTK function still missing from the egui front end, so that deleting GTK (migration Task 16) causes no regression.

**Architecture:** Each gap is ported into the existing egui pane that owns it. Toolkit-free logic is reused from `gcodekit5-app`, `gcodekit5-visualizer`, `gcodekit5-designer` and `gcodekit5-camtools`, or moved there. The GTK code in `crates/gcodekit5-ui` is the behavioural reference, and the help screenshots in `crates/gcodekit5-ui-egui/resources/help_images/` are the visual reference.

**Tech Stack:** egui 0.37 (storytold fork), craft-libs, wgpu, egui_kittest.

**Spec:** the user's instruction on 2026-11-26: "All 24, we cannot allow any major regressions". The 24 gaps are the `Gap` rows of `docs/reports/egui-parity.md`, plus the 3D stock removal simulation, which the user asked for separately (visual-restore plan Task V6b).

## Global Constraints

- Every rule from the migration plan's Global Constraints still applies:
  - one egui version;
  - no unsafe;
  - no unwrap or expect on user paths;
  - clippy `-D warnings` and fmt clean;
  - tests go in `tests/all.rs` and run with nextest;
  - tests never write into the repo;
  - mm `f32` internally, 2 decimals in the UI;
  - every user string is in `msgids()` and in every po file.
- Behaviour matches GTK, unless GTK is unsafe. Where GTK is unsafe, the egui version keeps the existing egui guard, and the report says so.
- Anything that moves the machine or writes offsets reuses the panel's existing guards: connected, no job or probe halt, and a fresh Idle report where motion is involved. It fails safe.
- Each task updates its rows in `docs/reports/egui-parity.md` from Gap to Ported, with the test as evidence. It also adds a line to the M6 CHANGELOG entry.
- Each commit passes the crate's tests, including the translation test.

## Review Focus

1. **Keyboard jog:** a key held while focus moves (into a text field, another page, another window) must cancel the jog, like press-and-hold buttons do. Keys must never jog while a text field has focus.
2. **Rotary Zero:** G10 L20 on A/B/C uses the active WCS P number, the same rules as X/Y/Z, and the rotary calibration value.
3. **Replace All in the editor:** refused while a job runs, as editing is, and undoable in one step.
4. **Designer clipboard and boolean ops:** they create new shapes with unique IDs, are undoable, and never alter the shapes they come from unless the operation says so.
5. **Shortcut editor:** a user cannot bind a key that conflicts with E-STOP or soft reset, or bind jog to a bare letter key while typing.

---

### Task G1: Machine Control gaps

GTK reference: `crates/gcodekit5-ui/src/ui/gtk/machine_control/`.

The panes involved are `crates/gcodekit5-ui-egui/src/panes/machine_control/` and the editor pane.

- **A/B/C rotary axes:**
  - the readout shows them in degrees when the machine reports them, as GTK does;
  - each has a Zero button, `G10 L20 P{p} A{calibration}`, using `machine.rotary_calibration` from settings and the work-zero guards.
- **Quick Z-touch and edge probe buttons** call the existing Probe Routines generators and stream path, with the same parameters GTK's quick buttons use.
- **Keyboard jog:** the same keys as GTK (numeric keypad 8/2/4/6 for XY and 9/3 for Z), with the same step or continuous mode as the jog pad. It cancels on key release, focus loss, page switch and window blur, and is inactive while any text field has focus. Ruling: the user asked for all gaps, so GTK's keys are restored, with the egui jog guards.
- **Exact override percentage field:** GTK's "Set %". It sends the realtime override steps needed to reach the target value.
- **Machine limits and buffer readout:** Machine Limits from `$130-$132`, and the Buffer value from the status report's `Bf:` field where the controller sends it. Otherwise "-".
- **Clear editor:** clears the editor. It asks first when the editor has unsaved changes, and is refused while a job runs.

- [ ] Tests for each item, including Review Focus 1 and 2. Commit per item.

### Task G2: Status bar and console

- **Status bar:** elapsed and remaining time while streaming, in GTK's format, plus the feed and spindle readout.
- **Console command history:** kept across sessions, in the same file GTK uses (find it in the GTK console code), with the same size limit.
- [ ] Tests (time format; history round trip in a temp dir). Commit.

### Task G3: Editor Replace

- Replace, Replace All and a match count in the editor find row (`panes/editor`).
- Replace All is a single undo step and is refused while a job runs (Review Focus 3).
- The find row must still fit at 1280 px.
- [ ] Tests. Commit.

### Task G4: Visualizer gaps

Run this after visual-restore Tasks V6 and V6b.

- **2D mode** (top orthographic view, as GTK's 2D toggle).
- **Machine bounds and fit to device:** from the active device profile's work area.
- **Run preview:** play, pause and stop, with a speed choice, animating the tool marker along the toolpath. It sends nothing to the machine.
- **Copy cursor coordinates.**
- **Intensity statistics:** the S-value statistics in GTK's inspector.
- The display toggles V6 left out, if any remain.
- [ ] Tests and a snapshot. Commit per item.

### Task G5: Designer gaps

- **Shape operations:**
  - Mirror X/Y;
  - Align left, right, top, bottom and centre;
  - Cut, copy and paste;
  - Boolean union, difference and intersection.

  Each goes in the context menu and keeps its shortcut. All are undoable, using the `gcodekit5-designer` operations GTK used (Review Focus 4).
- **Grouping keys:** Ctrl+G and Ctrl+Shift+G group and ungroup.
- **Device work area:** shown on the canvas from the active device profile, instead of the fixed 250x250 mm.
- **STL import:** behind the same Settings switch as GTK.
- [ ] Tests. Commit per item.

### Task G6: Preferences

- **Controller page:** has the same fields as GTK's.
- **Keyboard shortcut editor:**
  - built on craft-keymap;
  - saved with the settings;
  - E-STOP and soft reset cannot be rebound to a conflicting key (Review Focus 5).
- [ ] Tests. Commit.

### Task G7: Manual-check list and parity sign-off

- Every row in `docs/reports/egui-parity.md` is Ported, Not applicable, or Manual check needed with a reason.
- No Gap row is left.
- The "Manual checks" list is kept for the user to run before release.
- [ ] Full `timeout 1500 cargo nextest run --workspace --no-fail-fast`. Commit.
