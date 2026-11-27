# Deep-refactor audit: GCodeKit5 workspace

## 1. Summary

| Item | Value |
|---|---|
| Date | 2026-11-27 |
| Audited commit | `2a733ad` (branch `claude/test-race-fixes` = master `bf14920` + test-race fix) |
| Method | 2 read-only passes, 9 auditors (pass 1: uishell, uipanes, designer, app, camvis, corebuild; pass 2: p2fw, p2cam, p2des), plus `cargo clippy` (pedantic lints) and `cargo machete` scans; controller verification of the top findings |
| Baseline (BUILDSTATS.md) | 1390 passed / 0 failed / 3 ignored under `cargo test --workspace --all-targets`; line coverage 78.43% (46144 / 58836) |
| Ledger rows | 94 candidate rows plus 11 deliberate or owner-instruction rows (sec. 5); every row traces to an auditor entry |
| Tickets | 58 (sec. 6): 37 Proposed, 19 Blocked - owner decision, 2 Blocked - characterization required |

Counts per class, after merging duplicates and applying controller corrections:

| Class | Findings | Notes |
|---|---|---|
| DEFECT | 35 | APP-1, UISHELL-1 and UIPANES-1 merged into one. 12 come from pass 1, 23 are new in pass 2 |
| SAFE (behaviour-preserving refactor) | 32 | |
| BLOCKED-BY-PINS | 12 | Includes CAMVIS-10, moved from SAFE because the fix changes output |
| DELIBERATE | 9 groups | sec. 7 |
| PERF (measured) | 0 | UIPANES-5/6 and DESIGNER-6 per-frame costs were never measured, so they are filed as SAFE |

**Pass 2 found plenty.** It turned up 23 new defects, concentrated in the Gerber/CAM generators (P2CAM-1..7) and in designer import/export geometry (P2DES-1..11). Run a third pass over the areas that are still only skimmed before declaring the train complete:

| Still skimmed after pass 2 (from every `## Skimmed`, minus what pass 2 covered) | Priority |
|---|---|
| ui-egui CAM pages: `panes/cam_tools/{gerber/*, probe_routines/*, probe_pane.rs, bitmap_pane.rs, bitmap_engraving/{page,job}.rs, speeds_feeds/*, tools.rs, requests.rs, preset_fields.rs}`. Their validators were never diffed against the domain line by line | High |
| Engravers outside the dither functions: `camtools/src/laser_engraver.rs`, `designer/src/engraving/image_engraver.rs` | High |
| Designer CAM internals: pocket generation in `toolpath/generator.rs`, `try_convert_to_arc`, text outline / `ToolpathBuilder` (~940-1050, 1386-1545, 1690-1840), `designer_state/gcode.rs` outside the cited lines | High |
| egui designer pane: `canvas/{input,overlay,array_dialog,toolpath_preview}.rs`, `toolbox/*`, `layers.rs`, `shape_gallery.rs`, `file_ops.rs`, `properties/{builders.rs,handlers/*}`. Pass 2 only grepped these for patterns | Medium |
| Firmware definition tables: `app/src/firmware_settings/{grbl_definitions,repetier_definitions}.rs` (~800 lines, never checked against firmware docs); the `machine.rs` ack/resend ordering was read but not simulated | Medium |
| Gerber internals: `sketch_to_polylines`, `clean_polyline`, `aperture_outline`, `single_quadrant_center`; tabbed box `apply_slots_to_path` (comments at :660-715 contradict each other), `pack_paths` | Medium |
| camtools services: `speeds_feeds/service.rs` beyond line 300, `speeds_feeds/mod.rs`, `wcs_service.rs`, `cam_presets.rs`, `validator.rs`, `optimizer.rs`, `stats.rs`, `drill_press.rs` 1-60 and 330+ | Medium |
| ui-egui shell: `windows/{settings.rs, firmware_settings, device_info.rs, help/*}`, `panes/visualizer/*`, `panes/editor/*`, `panes/console.rs`, `machine_control/{jog,wcs,overrides,dro,stream,direct_sender,quick_probe,connection}.rs`, `menu.rs`, `shortcuts.rs`, `dialogs.rs`, `app/*.rs` | Medium |
| Designer model: `model/{circle,line,polygon,triangle}.rs`, `parametric_shapes.rs` (sprocket), `model3d.rs`, `tool_library.rs`, `spatial_index.rs`, `selection_manager.rs`, `commands.rs`, `shape_store.rs` | Low |
| core data: `data/{materials,materials_mpi_static,tools,gtc_import}.rs` (logic), `probe.rs`; settings `config.rs` defaults, `build.rs`, i18n plurals; devicedb `model.rs`, `manager.rs` 1-120 | Low |
| visualizer `stock_removal_3d.rs` mesh / greedy-rect internals, `camera.rs`, `update_bounds_with_arc` | Low |
| `.github/workflows/{release,mutation-testing}.yml` bodies, `scripts/*`, `flatpak/`, `wix/` | Low |

## 2. Architecture verdict

The reports **confirm** the hypothesis. Two corrections and one addition follow the table.

| Claim | Verdict | Evidence |
|---|---|---|
| The crate graph is acyclic and layered: core <- domain crates (communication, devicedb, designer, visualizer, settings, camtools) <- app <- ui-egui <- bin | Confirmed by every pass-1 verdict | One minor leak: ui-egui imports `gcodekit5_communication` directly in 4 files, mostly for types; the only I/O call is `list_ports` (APP-9) |
| (a) GTK-era process-global `device_status` store that egui no longer writes | Confirmed by the controller; it is a **safety defect**, not just debt | `git show 26740e3` deleted every writer. The readers (`panes/cam_tools/{mod,drill_press,spoilboard_surfacing}.rs`) see permanent defaults: 3 axes, laser mode off, 200 mm travel (APP-1, APP-2) |
| (b) Large dead GTK-era subsystems kept alive only by tests | Confirmed, and the list is longer | settings dialog stack (COREBUILD-1), `device_console_manager` + listener plumbing + most of `ConsolePanel` (APP-3, P2FW-7), devicedb UI controller (APP-4), visualizer pipeline + scene graph, ~3.5k lines (CAMVIS-2, CAMVIS-3), core command lifecycle (COREBUILD-3). Two more: core error enums (COREBUILD-2) and type aliases (COREBUILD-4a) are dead **with no test pins** and can go in phase 4. Pass 2 added the Repetier mode inside `SerialCommunicator` (P2FW-3), the protocol registry surface (P2FW-4) and the unwired grblHAL decoder (P2FW-2) |
| (c) Business rules duplicated between ui-egui and the domain crates, with drift | Confirmed | CAM validation limits (UIPANES-2: the UI lacks `MAX_PECKS`), designer export check / engrave params (UIPANES-3), CAM dispatch (DESIGNER-11: the preview skips the `laser_params.use_global` handling), two image engravers with different dithering arithmetic (UIPANES-4), the `$13` rule (APP-2), feed-rate math (COREBUILD-5) |
| (d) Monster files | Confirmed with measurements (`wc -l` at 2a733ad) | `app/src/machine.rs` 2366, `designer/src/toolpath/generator.rs` 1877, `ui-egui/src/app.rs` 1834, `designer/src/pocket_operations.rs` 1379. Largest functions (clippy `too_many_lines`, `scan-clippy.txt`): `gcode_gen.rs:286` 555 lines, `model/path.rs:515` 429 lines |

The two corrections:

- The `device_status` store has no live writer at all. The only call sites (`update_commanded_*`) are in `device_console_manager.rs`, which is itself unattached. `rg 'device_status::(set_|update_|begin_stop|end_stop|cancel_stop|set_job_active)' crates src --glob '!**/tests/**'` at 2a733ad returns only those two.
- The "GTK-parity" defence does not cover everything. Several defects are geometry that GTK shared and got wrong too: DESIGNER-3, DESIGNER-4, and P2CAM-3, whose bug is encoded in a fixture. Parity explains why a fixture exists; it does not make its content correct.

The addition: **consumer drift in the egui designer.** The fields `Path.rotation`, `RasterImage.rotation` and `Line.rotation` each mean something different, and only some consumers know which. The result is a canvas that does not show what will be cut, and an SVG export that differs from the design (P2DES-1, P2DES-2, P2DES-3). No test covers `svg_export`, text G-code or path rotation, which is why these defects survived.

## 3. Defects

Most severe first: safety and destructive output, then data loss, then wrong geometry, then the rest. Status values:

- **Controller-verified**: checked in the code by the controller.
- **Re-verified pass 2**: a second auditor re-checked a pass-1 claim.
- **Auditor-only**: one auditor, reading only.

| # | ID | Title | Sev | Conf | Evidence | Status |
|---|---|---|---|---|---|---|
| 1 | APP-1 (= UISHELL-1 = UIPANES-1) | Laser-mode `$32` guard, axis count and `$130/$131` centre read a global that production never writes. A laser machine gets an M3 plunge drill program; 2-axis profiles get Z moves | H (safety) | H | `crates/gcodekit5-app/src/device_status.rs:69,73`; readers `crates/gcodekit5-ui-egui/src/panes/cam_tools/mod.rs:564`, `drill_press.rs:106-118,143`, `spoilboard_surfacing.rs:103` | Controller-verified |
| 2 | P2CAM-1 | The Gerber converter ignores G36/G37 regions, aperture macros and `%LPC` polarity with no warning. Rubout hatches the inside of copper pours, milling away copper | H (destructive) | H (absence), M (KiCad macro claim) | `crates/gcodekit5-camtools/src/gerber.rs:545-770`, `:743`, `:759`, `:335`, `:668-672` | Controller-verified (absence) |
| 3 | DESIGNER-2 | Opening a design silently drops shapes it cannot convert; the next save makes the loss permanent. A raster image with a missing file loads empty with only a `warn` | H (data loss) | H | `crates/gcodekit5-designer/src/designer_state/file_io.rs:82-83`, `serialization.rs:777-795,820` | Controller-verified |
| 4 | P2DES-2 | Path and raster `rotation` is applied by G-code and the selection box, but not by the egui painter, SVG export or hit-testing. The canvas does not show what will be cut | H | M-H | `crates/gcodekit5-designer/src/model/path.rs:1079-1120`, `toolpath/generator.rs:582-596`, `crates/gcodekit5-ui-egui/src/panes/designer/canvas/rendering.rs:215-222`, `svg_export.rs:127-134` | Auditor-only |
| 5 | P2DES-3 | Text G-code is rotated with the opposite sign to the canvas, and its baseline and origin differ from the painter's | H | H (sign), M (offset) | `crates/gcodekit5-designer/src/toolpath/generator.rs:955-957,1726-1729`, `crates/gcodekit5-ui-egui/src/panes/designer/canvas/text_outline.rs:85-92` | Auditor-only |
| 6 | P2DES-1 | SVG export rotates every angled line a second time | H | H | `crates/gcodekit5-ui-egui/src/panes/designer/svg_export.rs:88-97` vs `crates/gcodekit5-designer/src/model/line.rs:205-211` | Auditor-only |
| 7 | P2FW-1 | An edited tool number is lost on restart (the field reloads as 0) | M (data loss) | H | `crates/gcodekit5-app/src/tools_manager_backend.rs:15-40,71-73`, `tool_form.rs:185,371` | Controller-verified; **owner decision** |
| 8 | DESIGNER-5 | Contour-parallel and adaptive pocket loops rely on a `debug_assert!` alone to terminate. A tiny stepover hangs the UI in release builds | M | H | `crates/gcodekit5-designer/src/pocket_operations.rs:181`, `:860-878`, `:988-1000` | Controller-verified |
| 9 | P2CAM-5 | The tabbed box accepts kerf, play, feed and power values that give self-intersecting paths, `F0` or `M4 S-n` mid-job | M | H | `crates/gcodekit5-camtools/src/tabbed_box/mod.rs:81-126,197-212,1262-1266` | Auditor-only |
| 10 | P2CAM-3 | Outline and silk layers cut both edges of the stroke, with no tool-radius compensation and a single pass. The fixture encodes this | M | M | `crates/gcodekit5-camtools/src/gerber.rs:803-820`; `crates/gcodekit5-ui-egui/tests/fixtures/cam/gerber_outline.gcode` | Auditor-only; **owner decision** |
| 11 | P2CAM-2 | The "Drill Holes" layer feeds an Excellon file to the Gerber parser and emits an empty "valid" program. Any `.txt` file counts as a drill layer | M | M | `crates/gcodekit5-camtools/src/gerber.rs:177-178,229,464-473,806` | Auditor-only |
| 12 | P2DES-4 | Old-style DXF `POLYLINE` loses its bulges: arcs import as chords | M-H | H | `crates/gcodekit5-designer/src/dxf_parser.rs:912-1067` (no `"42"` arm), `import.rs:668-690` | Auditor-only |
| 13 | P2DES-6 | DXF import silently drops INSERT, MTEXT, HATCH and other entities, never reads BLOCKS, and rejects non-UTF-8 DXF files. SVG `<use>`/`<text>` are ignored | M | H | `crates/gcodekit5-designer/src/dxf_parser.rs:631-638`, `import.rs:453-456,262-318` | Auditor-only |
| 14 | DESIGNER-1 | The adaptive pocket emits the centre-cleanup pass twice per Z pass | M | H | `crates/gcodekit5-designer/src/pocket_operations.rs:1106-1129` | Controller-verified |
| 15 | DESIGNER-4 | The laser collinear merge has no deviation bound (about 0.2 mm sag at r=50); `curve_simplification_tolerance` is never read | M | M | `crates/gcodekit5-designer/src/gcode_gen.rs:156-185,28,86`; `designer_state/gcode.rs:214` | Re-verified pass 2 |
| 16 | APP-6 | The UI offers WebSocket and "Auto Reconnect", but neither is implemented; the Device Manager checkbox is dead twice | M | H | `crates/gcodekit5-app/src/device_profiles.rs:51`, `crates/gcodekit5-communication/src/communication/mod.rs:656-658`, `crates/gcodekit5-ui-egui/src/windows/device_manager/view.rs:329` | Re-verified pass 2; **owner decision** |
| 17 | P2CAM-6 | Probe routines validate `safe_height`/`slow_feed` but most routines never use them. Bore and boss probe at the fast feed. `G21` without a restore switches an inch session to mm | M | H | `crates/gcodekit5-camtools/src/probe_routines.rs:511-516,564,593,676,764,818,959` | Auditor-only |
| 18 | CAMVIS-6 | Gerber outline selection uses two non-equivalent predicates and depends on `read_dir` order; the rubout drops panicked rings silently; KiCad 6+ layer names are not matched | L-M | M | `crates/gcodekit5-camtools/src/gerber.rs:149-186,970-995,938-951` | Re-verified pass 2 |
| 19 | P2DES-5 | SVG `rect` ignores `rx`/`ry`, so an export/import round trip loses corner radii | M | H | `crates/gcodekit5-designer/src/import.rs:330-360` | Auditor-only |
| 20 | P2DES-8 | SVG export anchors text at its centre as if it were the start; the raster box fill is overridden by the inline style | M | H | `crates/gcodekit5-ui-egui/src/panes/designer/svg_export.rs:98-123` | Auditor-only |
| 21 | P2DES-9 | DXF circles, arcs and ellipses import as fixed 64-gons (0.6 mm sag at r=500) | M | H | `crates/gcodekit5-designer/src/import.rs:538-614` | Auditor-only |
| 22 | CAMVIS-1 | The Preferences "Arc Segment Length" option has no effect | M | H | `crates/gcodekit5-ui-egui/src/windows/settings.rs:349,574`; `crates/gcodekit5-visualizer/src/visualizer/visualizer.rs:472,1140`; `crates/gcodekit5-ui-egui/src/panes/visualizer/toolpath.rs:12,167-190` | Re-verified pass 2; **owner decision** |
| 23 | COREBUILD-9 | On Windows release builds the file log sink writes ANSI escapes and pretty multi-line records, beside `current_exe()`. Under Program Files that directory is unwritable, and logging is then silently lost. *Panic-on-reinit: struck (refuted)* | M (Windows) | M | `src/lib.rs:23-57` | Re-verified pass 2 (partly) |
| 24 | P2FW-5 | Three small wrong rules: `$32 >= 1` shows lathe mode as laser; `$30-$32` are filed under "Homing"; text settings are truncated at the first space | L-M | M-H | `crates/gcodekit5-app/src/device_identity.rs:85`, `firmware_settings/mod.rs:61-63`, `crates/gcodekit5-communication/src/firmware/grbl/settings.rs:106-111` | Auditor-only |
| 25 | P2FW-2 | The grblHAL extended error/alarm tables look wrong, and nothing calls the decoder | M (latent) | M (tables from memory), H (no callers) | `crates/gcodekit5-communication/src/firmware/grbl/error_decoder.rs:44-81,100-111` | Auditor-only; **blocked: verify against upstream grblHAL** |
| 26 | APP-7 | Serial writes inherit the 10 ms poll timeout, and one write `TimedOut` ends the connection mid-job. Practically unreachable on Linux; plausible on Windows at low baud | L-M (latent) | M | `crates/gcodekit5-communication/src/communication/serial.rs:25,319,365`; `crates/gcodekit5-app/src/machine.rs:1390` | Re-verified pass 2 (severity lowered) |
| 27 | P2CAM-4 | The jigsaw RNG stops advancing for seeds >= 2^24 (f32 state, `+= 1.0`) | L-M | H | `crates/gcodekit5-camtools/src/jigsaw_puzzle.rs:50-65` | Controller-verified |
| 28 | P2CAM-7 | Speeds-and-feeds limits can contradict each other: `min_rpm` is applied after the max; an arbitrary chip-load floor; the power formula is off by a factor of pi/z | L-M | M | `crates/gcodekit5-camtools/src/speeds_feeds/calculator.rs:446,508,604-626` | Auditor-only; **owner decision** (fixtures) |
| 29 | P2DES-7 | Re-saving a design drops its metadata (created/author/description), the tool start depth and the stock origin | L-M | H | `crates/gcodekit5-designer/src/designer_state/file_io.rs:8,~131,~143` | Auditor-only |
| 30 | P2DES-10 | An SVG without a `viewBox` mis-scales `cm`/`in`/`mm` widths | L-M | M | `crates/gcodekit5-designer/src/import/svg_parse.rs:430-441` | Auditor-only |
| 31 | P2FW-9 | The material form accepts NaN, inf and negative values, which vanish on save | L | H | `crates/gcodekit5-app/src/material_form.rs:~190` vs `tool_form.rs:249-255` | Auditor-only |
| 32 | P2FW-6 | Repetier translation forces a bare axis line to `G1`; any line containing "wait" counts as an ack; a bare `G92` is ignored | L | M | `crates/gcodekit5-communication/src/firmware/repetier/processor.rs:212-216,288,336` | Auditor-only |
| 33 | CAMVIS-4 | The drill press emits a helical G2 without `G17` (low risk: GRBL resets to G17) | L | H | `crates/gcodekit5-camtools/src/drill_press.rs:135-137` | Re-verified pass 2; **owner decision** (fixtures) |
| 34 | DESIGNER-3 | The ramp uses chord length for arcs, so ramps come out up to ~10% **shallower** than requested (pass 1 said "steeper"; refuted) | L | H | `crates/gcodekit5-designer/src/toolpath/generator.rs:782-799,822-823` | Re-verified pass 2 |
| 35 | P2DES-11 | The laser point filter keeps a stale reference after arcs and can drop a closing point (<= 0.1 mm) | L | M | `crates/gcodekit5-designer/src/gcode_gen.rs:128-137,352,402-410` | Auditor-only |

## 4. Product decisions needed

**Decided 2026-11-27 by the owner: option (a) for every decision, D-1 to D-14.** The blocked tickets (T15-T23, T27, T47-T56) are now Approved; fixture edits they imply are sanctioned but must be enumerated in each PR. D-2, D-3, D-4 and D-14 (a) are feature work, scheduled after the cleanup tickets.

| ID | Decision | Options |
|---|---|---|
| D-1 (P2FW-1) | Tool number | (a) persist `number` with `#[serde(default)]`; (b) make it read-only and drop the GTC import numbering |
| D-2 (APP-6) | WebSocket transport and Auto Reconnect | (a) implement both; (b) remove WebSocket from `CONNECTION_TYPES` and remove the checkbox, keeping the serde fields so old files load; (c) mixed |
| D-3 (P2CAM-1b) | Gerber regions, macros and polarity | (a) feature ticket: G36/G37 fills plus macro primitives; (b) warn/refuse only (the T10 warning ships either way) |
| D-4 (P2CAM-2b) | Excellon drill layer | (a) parse Excellon into drill moves; (b) remove the "Drill Holes" layer from the selector |
| D-5 (P2CAM-3) | Outline compensation | (a) offset by tool radius, step-down passes, regenerate `gerber_outline.gcode`; (b) keep GTK-parity output |
| D-6 (CAMVIS-1) | Arc Segment Length | (a) wire it into `toolpath::build` (and choose the default); (b) remove the option from Preferences |
| D-7 (CAMVIS-4) | Drill press `G17` | (a) add the line and regenerate 2 fixtures; (b) leave as is |
| D-8 (UIPANES-4) | Image engraver | (a) camtools `BitmapImageEngraver` (f32 diffusion) becomes canonical and the designer output changes; (b) the designer engraver becomes canonical and the CAM fixtures change; (c) share only a dither module |
| D-9 (APP-2/3/4/5, COREBUILD-1/3/4b, CAMVIS-2/3b/7b, P2FW-4/7/8b, P2CAM-8b, DESIGNER-7/8b) | Delete dead GTK-era code together with the tests that pin it | (a) approve, with every test edit enumerated per ticket (sec. 6, phase 5); (b) keep the dead code |
| D-10 (COREBUILD-1d) | Drop 22 unread `Config` fields (user-visible schema) | (a) drop them, keeping `serde(default)` and tolerating unknown keys; (b) keep them |
| D-11 (COREBUILD-8b) | Root files `package.json`, `package-lock.json`, `devices.json`, `flatpak/com.github.feveal.*` | (a) delete; (b) keep (the flatpak manifest may serve a downstream build) |
| D-12 (P2CAM-6b) | Probe programs restoring the G20/G91 mode | (a) restore the mode and regenerate `probe_*.gcode`; (b) document that probe runs switch to mm |
| D-13 (P2CAM-7) | Speeds-and-feeds formula fixes | (a) fix them and regenerate `speeds_feeds_{clamped,custom}.txt`; (b) keep |
| D-14 (P2CAM-8c) | `FingerStyle::{Springs,Barbs,Snap}` | (a) implement them; (b) remove the variants (only preset JSON can select them) |

## 5. Cleanup ledger

Rules for this table:

- Every count carries the command that produced it, run at `2a733ad`.
- Validation commands use `cargo nextest run -p <crate> -E 'test(<module>::)'`.
- Status is one of `Proposed`, `Blocked - characterization required`, `Blocked - owner decision`, `Rejected (reason)`, `Deliberate - do not fix (reason)`. These use ASCII hyphens for the em dash in the brief.

Crate short names: `ui` = gcodekit5-ui-egui, `app` = gcodekit5-app, `com` = gcodekit5-communication, `des` = gcodekit5-designer, `cam` = gcodekit5-camtools, `vis` = gcodekit5-visualizer, `core`, `set` = gcodekit5-settings, `ddb` = gcodekit5-devicedb.

### 5.1 Defects

| ID | Preserved behaviour | Evidence | Canonical owner | Pins | Frozen side | Validation | Depends on | Status |
|---|---|---|---|---|---|---|---|---|
| APP-1 (+UISHELL-1, UIPANES-1) | Drill/surfacing params, refusal msgids, 3-axis non-laser output byte-equal | 5 reader sites (`rg -n 'device_status::' crates/gcodekit5-ui-egui/src \| wc -l` at 2a733ad); 0 production writers (`rg 'device_status::(set_\|update_\|begin_stop\|end_stop\|cancel_stop\|set_job_active)' crates src --glob '!**/tests/**'`, only `update_commanded_*` in the unattached console manager) | `DeviceProfile.num_axes` via `Pane::set_device` (`workspace.rs:440,749`); `$32`/`$130-$131` via ui `Machine` | `ui/tests/test_cam_drill_press.rs:214-280` (child-process laser test, global writes :260,:266), `test_cam_spoilboard_surfacing.rs:101`, `test_cam_gerber.rs:287` (set `num_axes` fields) | CAM fixtures | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_drill_press::) + test(test_cam_spoilboard_surfacing::) + test(test_cam_tools::)'` | - | Proposed |
| DESIGNER-2 | Known shapes load unchanged | `file_io.rs:82` `if let Ok` | `DesignerState::load_from_file` + additive load report | `ui/src/panes/designer/file_ops.rs:125`; designer tests calling `load_from_file` | Existing signature (additive API) | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_files::) + test(test_designer_files_app::)'` | - | Proposed |
| P2DES-7 | File format 1.1 loads | `file_io.rs:8,~131,~143` | `DesignerState` keeps `DesignMetadata`; `ToolpathParameters.start_depth` `#[serde(default)]` | designer round-trip tests (`rg -ln DesignFile crates/gcodekit5-designer/tests`) | Old files load | same as DESIGNER-2 | - | Proposed |
| P2DES-7b | - | Non-atomic save claim | - | - | - | - | - | Rejected (the UI path is atomic: `file_ops.rs:166-188` `write_atomic_with`) |
| DESIGNER-1 | Same cuts, one cleanup per Z pass | `pocket_operations.rs:1106-1129` | `PocketGenerator::generate_adaptive_pocket` | `des/tests/test_pocket_holes.rs:47,171,199,274,310` (RapidMove counts) | Tests (STOP if a count encodes the duplicate) | `cargo nextest run -p gcodekit5-designer -E 'test(test_pocket_holes::)'` | - | Proposed |
| DESIGNER-5 | Normal inputs unchanged | `pocket_operations.rs:181` debug_assert; loops :860-878, :988-1000; `holes.rs:35` `MAX_OFFSET_LEVELS` | shared `offset_levels` helper with the cap | `test_pocket_holes.rs`, `test_audit_regressions.rs` | Tests | `cargo nextest run -p gcodekit5-designer -E 'test(test_pocket_holes::) + test(test_audit_regressions::)'` | - | Proposed |
| P2DES-2 | G-code output (the reference) | `path.rs:1079-1120`, `rendering.rs:215-222`, `svg_export.rs:127-134`, `path.rs:96-126` | new `DesignPath::rotated_render()`; `render()` unchanged (the generator rotates itself) | none (`rg -n "Path.*rotation" crates/*/tests` empty) | G-code | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_canvas::) + test(test_designer_snapshots::)'` | - | Proposed |
| P2DES-3 | Unrotated, descender-free, single-line text | `generator.rs:945,955-957,1726-1729`; `text_outline.rs:83-92` | one text layout function in `des::font_manager`/`model/text.rs` | none (`rg -ln "generate_text_toolpath\|build_text_outline" crates/*/tests` empty) | Canvas painter | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_text_tool::)'` | - | Proposed |
| P2DES-1 | Horizontal lines; rect/ellipse/text transforms | `svg_export.rs:88-97` | `svg_export::element` | none (only caller `export.rs:321`) | - | new `ui/tests/test_designer_svg_export.rs` | - | Proposed |
| P2DES-8 | Other SVG elements | `svg_export.rs:98-123` | `svg_export::element` | none | - | same | - | Proposed |
| P2DES-4 | Spline-fit and 3D polyline handling | `dxf_parser.rs:912-1067`; 2 `"42"` arms (`rg -n '"42"' crates/gcodekit5-designer/src/dxf_parser.rs`) | `parse_polyline` | `test_dxf_polyline_splinefit.rs` | Spline-fit tests | `cargo nextest run -p gcodekit5-designer -E 'test(test_dxf_polyline_splinefit::) + test(test_dxf_spline_import::)'` | - | Proposed |
| P2DES-6 | Solids notice | `dxf_parser.rs:631-638`, `import.rs:453-456` | `DxfFile.unsupported_entities` -> UI notice | msgid "Unsupported 3D solids discarded" (`po/`) | msgid catalog | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_import::) + test(test_translations::)'` | - | Proposed |
| P2DES-9 | Topology and start points | `import.rs:538-614` | DXF circle/arc/ellipse as lyon arcs/Beziers | `rg -ln DxfImporter crates/*/tests` | - | `cargo nextest run -p gcodekit5-designer -E 'test(test_import_transforms_units::) + test(test_dxf_z_metadata::)'` | - | Proposed |
| P2DES-5 | Non-rounded rects | `import.rs:330-360` | `element_to_shape` rect arm | SVG import tests | - | `cargo nextest run -p gcodekit5-designer -E 'test(test_import_transforms_units::)'` | - | Proposed |
| P2DES-10 | viewBox files; unitless = mm (documented at `import.rs:209`) | `svg_parse.rs:430-441` | `svg_parse` root sizing | same | Unitless rule | same | - | Proposed |
| DESIGNER-4 | Straight laser geometry | `gcode_gen.rs:156-185`; tolerance written only at `:28,:86,:101`, `designer_state/gcode.rs:214` (`rg -n curve_simplification_tolerance crates`) | `optimize_toolpath_for_laser` honours the tolerance | `des/tests/test_cam_depth_and_gcode.rs:234` | - | `cargo nextest run -p gcodekit5-designer -E 'test(test_cam_depth_and_gcode::)'` | - | Proposed (measure first) |
| P2DES-11 | Same | `gcode_gen.rs:128-137,352,402-410` | same | same | - | same | DESIGNER-4 | Proposed |
| DESIGNER-3 | Linear-only contours | `generator.rs:782-799,822-823` | `ToolpathSegment::arc_length()` | `test_cam_depth_and_gcode.rs:66,367` (max drop only) | - | same | - | Proposed |
| P2CAM-1a (warn) | Plain-stroke boards byte-equal | `rg -n -i "RegionMode\|G36\|G37\|Polarity\|ApertureMacro\|%AM\|LPC" crates/gcodekit5-camtools` = 0 hits | `gerber.rs::parse_gerber_to_sketches` returns an unsupported-construct count | `ui/tests/test_cam_gerber.rs` + `fixtures/cam/gerber_*.gcode`, `fixtures/cam/gerber/` | Fixtures | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_gerber::)'`; `cargo nextest run -p gcodekit5-camtools -E 'test(audit_regressions::)'` | - | Proposed |
| P2CAM-1b (feature) | - | same | same | same | - | - | D-3 | Blocked - owner decision |
| P2CAM-2a | Copper layers | `gerber.rs:177-178,806` | `detect_layer_files`; refuse an empty program | `test_cam_gerber.rs` (no DrillHoles case) | Fixtures | same as P2CAM-1a | - | Proposed |
| P2CAM-2b | - | no Excellon support (`rg -n -i "excellon\|M48" crates` = 0) | - | label table `ui/src/panes/cam_tools/gerber/mod.rs:75` | - | - | D-4 | Blocked - owner decision |
| CAMVIS-6 | Single-outline output byte-equal | `gerber.rs:149-186,970-995,938-951`; union loop x4 (`:810,:836,:915,:996`) | private `is_outline_file`, `merge_sketches`; rubout takes the chosen outline | `test_cam_gerber.rs`, `cam/tests/audit_regressions.rs:570` | Fixtures | same as P2CAM-1a | - | Proposed |
| P2CAM-5 | All valid parameter sets; existing error strings | `tabbed_box/mod.rs:81-126` | `tabbed_box` validator | `ui/tests/test_cam_tabbed_box.rs`, `cam/tests/{generator_fixes,audit_regressions}.rs` | Fixtures, msgs | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_tabbed_box::) + test(test_cam_input_bounds::)'` | - | Proposed |
| P2CAM-4 | Seeds < 2^24 byte-equal | `jigsaw_puzzle.rs:50-65,84-114` | `validate_parameters` rejects seed >= 2^24 - consumption | `fixtures/cam/jigsaw_{default,custom}.txt`, `preset_jigsaw.json` | Fixtures | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_jigsaw::)'` | - | Proposed |
| P2CAM-6a | Emitted G-code byte-equal | `probe_routines.rs:593,676,764,818` | probe validators | `fixtures/cam/probe_*.gcode`, `cam/tests/probe_routines.rs` | Fixtures | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_probe_routines::)'`; `cargo nextest run -p gcodekit5-camtools -E 'test(probe_routines::)'` | - | Proposed |
| P2CAM-6b | - | `probe_routines.rs:511-516` | - | `probe_*.gcode` | - | - | D-12 | Blocked - owner decision |
| UIPANES-2b (MAX_PECKS field rule) | Generator behaviour | domain `cam/src/drill_press.rs:64-110` `MAX_PECKS` vs UI `drill_press.rs:~172-205` | camtools const, UI field rule | `test_cam_drill_press.rs`, `test_cam_input_bounds.rs` | Fixtures | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_drill_press::) + test(test_cam_input_bounds::)'` | UIPANES-2 | Proposed |
| P2FW-5 | `$0=10 (step pulse, usec)` -> `10` | `device_identity.rs:85`, `firmware_settings/mod.rs:61-63`, `grbl/settings.rs:106-111` | same files | `app/tests/test_firmware_settings_model.rs`, `com/tests/grbl_protocol_tests.rs`, `ui/tests/test_firmware_settings.rs` | - | `cargo nextest run -p gcodekit5-app -E 'test(test_firmware_settings_model::)'`; `cargo nextest run -p gcodekit5-communication -E 'test(grbl_protocol_tests::)'` | - | Proposed |
| P2FW-9 | Valid materials | `material_form.rs:~190` | `MaterialForm` (match `ToolForm::required`) | none found (new `app/tests/test_material_form.rs`) | - | `cargo nextest run -p gcodekit5-app -E 'test(test_material_form::)'` | - | Proposed |
| APP-7 | Success path; TCP unchanged | `serial.rs:25,319,365`; `machine.rs:1390` | `RealSerialPort::write` retries `TimedOut` until `timeout_ms` | `com/tests` serial fakes | - | `cargo nextest run -p gcodekit5-communication -E 'test(audit_followup_tests::) + test(test_reply_ledger::)'` | - | Proposed |
| P2FW-6 | Idle `wait` ack (deliberate) | `repetier/processor.rs:212-216,288,336` | `RepetierProcessor` | `com/tests/test_repetier.rs`, `app/tests/test_machine_repetier.rs` | - | `cargo nextest run -p gcodekit5-communication -E 'test(test_repetier::)'`; `cargo nextest run -p gcodekit5-app -E 'test(test_machine_repetier::)'` | - | Proposed |
| COREBUILD-9 | stdout logging, `RUST_LOG` precedence | `src/lib.rs:23-57` | root `init_logging`: `.with_ansi(false)`, per-user dir, `try_init()?` | none (`rg init_logging` = `src/main.rs` only) | - | `cargo check --workspace --target x86_64-pc-windows-gnu --release` if the target is installed, else review | - | Proposed |
| COREBUILD-9b | - | panic-on-reinit | - | - | - | - | - | Rejected (single call site at `src/main.rs:13`; refuted in pass 2) |
| P2FW-1 | `custom_tools.json` loads | `tools_manager_backend.rs:71-73` | `PersistedTool` | `app/tests/test_tools_manager_backend.rs` | - | `cargo nextest run -p gcodekit5-app -E 'test(test_tools_manager_backend::)'` | D-1 | Blocked - owner decision |
| APP-6 | Old profile/config files load | `device_profiles.rs:51`, `com/mod.rs:656`, `view.rs:329` | `device_profiles` / `ConnectionParams` | `rg 'WebSocket\|auto_reconnect' crates/*/tests`, `po/*.po` | Serde fields | `cargo nextest run -p gcodekit5-app -E 'test(test_device_profiles_model::)'`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_device_manager::)'` | D-2 | Blocked - owner decision |
| CAMVIS-1 | Default arc look (5 degrees) | see sec. 3 #22 | `ui` `toolpath::build` | `vis/tests/gcode_parsing_regressions.rs:237-248`, `ui/tests/test_settings_window.rs:264,313` | - | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_settings_window::) + test(test_visualizer::)'` | D-6 | Blocked - owner decision |
| P2FW-2 | GRBL 1.1 texts 1-38, alarms 1-9 | `error_decoder.rs:44-81,100-111`; 0 callers (`rg "decode_error\|decode_alarm\|format_error\|format_alarm" crates src`) | `error_decoder` | `com/tests/test_error_decoder.rs` | - | `cargo nextest run -p gcodekit5-communication -E 'test(test_error_decoder::)'` | upstream grblHAL check | Blocked - characterization required |
| CAMVIS-4 | All other lines | `drill_press.rs:135-137` | `job_preamble::push_modal_header` | `fixtures/cam/drill_press_{default,custom}.gcode` | - | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_drill_press::)'` | D-7, CAMVIS-5 | Blocked - owner decision |
| P2CAM-3 | Copper isolation | `gerber.rs:803-820` | `generate_simple_trace_from_sketches` | `fixtures/cam/gerber_outline.gcode`, `test_cam_gerber.rs:137-139` | - | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_gerber::)'` | D-5 | Blocked - owner decision |
| P2CAM-7 | - | `calculator.rs:446,508,604-626` | calculator | `fixtures/cam/speeds_feeds_{clamped,custom}.txt`, `cam/tests/test_speeds_feeds_calculator.rs`, `ui/tests/test_cam_speeds_feeds.rs` | - | `cargo nextest run -p gcodekit5-camtools -E 'test(test_speeds_feeds_calculator::)'` | D-13 | Blocked - owner decision |

### 5.2 Refactors, dead code, infra

| ID | Preserved behaviour | Evidence | Canonical owner | Pins | Frozen side | Validation | Depends on | Status |
|---|---|---|---|---|---|---|---|---|
| UIPANES-4 | One side's output changes (to be decided) | `cam/src/laser_engraver.rs:355-400` (f32) vs `des/src/engraving/image_engraver.rs:285-318` (i16) | per D-8 | `cam/tests/{generator_fixes,homing_safety}.rs`, `ui/tests/test_cam_bitmap_engraving.rs`, `fixtures/cam/bitmap_engraving_*.gcode` | - | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_bitmap_engraving::)'` | D-8 | Blocked - owner decision |
| COREBUILD-6 | Resolved dep graph (single version each) | 11 machete hits (`target/temp/refactor/scan-machete.txt`, re-verified by grep); craft-svg/dxf/mesh/i18n unused; 9+ crates pin inheritable deps | root `[workspace.dependencies]` | none | All tests | `cargo build --workspace --all-targets`; `cargo machete` -> no findings | - | Proposed |
| COREBUILD-7 | Same test coverage in CI | `.github/workflows/test-coverage.yml:17-21` `if: false`; `--lib` selects 8 inline tests (`grep -rn "#\[test\]" crates/*/src src \| wc -l` at 2a733ad); duplicate clippy jobs in `code-quality.yml` | workflows | none (`rg "test-isolation\|clippy-strict\|unwrap-audit" . -g '!target'` docs only) | `check-no-gtk`, nextest `gpu` group, release dry-run | `actionlint` if available; CI run on the PR | - | Proposed |
| COREBUILD-8a | - | 11 tracked files under `legacy/` (`git ls-files legacy \| wc -l` at 2a733ad), not compiled | delete | `LEGACY_CODE.md` (docs) | Docs (doc pass) | `cargo build --workspace` | - | Proposed |
| COREBUILD-8b | - | `package.json`, `package-lock.json`, `devices.json`, `flatpak/com.github.feveal.*` | - | none | - | - | D-11 | Blocked - owner decision |
| COREBUILD-2 | `Error`, `Result`, `Error::other`, `Error::Io` Display | 4 enums, 0 users outside `core/src/error.rs` and the `lib.rs:33` re-export (`rg -nw "ControllerError\|GcodeError\|ConnectionError\|FirmwareError" --glob '*.rs' . -g '!target'`) | `core::Error { Io, Other }` | none in tests | All tests | `cargo nextest run -p gcodekit5-core`; `cargo build --workspace --all-targets` | - | Proposed |
| COREBUILD-4a | Thread-safe alias set | 14 aliases with no user (`for s in ...; rg -nw $s --glob '*.rs' crates src \| grep -v core/src/types`) | `core/src/types/aliases.rs` | `lib.rs:41-46` re-exports | All tests | `cargo nextest run -p gcodekit5-core -E 'test(test_type_aliases::)'` | - | Proposed |
| COREBUILD-4b | - | Rc `Shared*`/`Ui*` used only by the dead settings controller; test-only `shared_none`, `thread_safe_deque`, `thread_safe_map` | delete | `core/tests/test_type_aliases.rs:18,45,56` | - | same | COREBUILD-1, D-9 | Blocked - owner decision |
| COREBUILD-5 | Byte-identical G-code; display precision (2 dp metric / 3 dp inch, deliberate) | 42 `25.4` sites (`rg -c '25\.4' --glob '*.rs' crates src -g '!**/tests/**'` summed, at 2a733ad); 5 private `MM_PER_INCH` consts | `core::units::MM_PER_INCH` + `FeedRateUnits` conversions | `core/tests/{test_units,audit_regression_tests}.rs`, `ui` jog tests, `com/tests/grbl_protocol_tests.rs` | All tests | `cargo nextest run -p gcodekit5-core -E 'test(test_units::)'`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_machine_jog::)'` | - | Proposed |
| UIPANES-2 | Identical accept/reject sets and msgids | jigsaw, tabbed box, drill, surfacing limits diffed (uipanes report) | camtools `pub const` next to the validators | `ui/tests/test_cam_{input_bounds,jigsaw,tabbed_box,drill_press}.rs` | All tests | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_input_bounds::) + test(test_cam_jigsaw::) + test(test_cam_tabbed_box::)'` | - | Proposed |
| UIPANES-3 | Same refusals and program text | `ui/src/panes/designer/export.rs:185-197,243-280,424-455` vs `des/src/designer_state/gcode.rs:62-100,620-645` | `DesignerState` (visibility predicate, `RasterImage::engraving_params`) | `ui/tests/test_designer_files.rs:300-352`, `test_designer_editor_handoff.rs`, `test_designer_properties_cam.rs` | All tests | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_files::) + test(test_designer_properties_cam::)'` | - | Proposed |
| DESIGNER-11 | G-code byte-identical | `des/src/designer_state/gcode.rs:226-556` vs `ui/src/panes/designer/canvas/toolpath_preview.rs:92-185` | `des` `shape_toolpaths` + `effective_cnc_object` | `des/tests/{test_cam_depth_and_gcode,test_default_properties_inheritance}.rs`, `ui/tests/test_designer_toolpath.rs` | All tests | `cargo nextest run -p gcodekit5-designer`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_toolpath::)'` | - | Proposed |
| UISHELL-2 | Every msgid literal; notice text | `ui/src/windows/tools_manager/mod.rs:324-347`, `materials_manager/mod.rs:202-225`, `device_manager/mod.rs:153-170` | `LibraryNotice` in `windows/manager_ui.rs` | `ui/tests/test_{tools,materials,device}_manager.rs`, `test_translations.rs` | All tests | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_tools_manager::) + test(test_materials_manager::) + test(test_device_manager::) + test(test_translations::)'` | - | Proposed |
| P2FW-8a | - | 13 zero-caller fns (`rg -nw <fn> crates src` per fn) in `app/src/{materials,tools}_manager_backend.rs` | shared `CustomLibraryFile` | none | All tests | `cargo nextest run -p gcodekit5-app -E 'test(test_materials_manager_backend::) + test(test_tools_manager_backend::)'` | - | Proposed |
| P2FW-8b | - | test-only `string_to_category`, `string_to_tool_type`, `search_tools`, `search_materials`, `add_tool` | delete | `app/tests/test_{tools,materials}_manager_backend.rs` | - | same | D-9 | Blocked - owner decision |
| UIPANES-5 | Output hides on any input change | `ui/src/panes/cam_tools/mod.rs:293-318` | `PageInput` enum, compared with `==` | `test_cam_drill_press.rs`, `test_cam_tools.rs` | All tests | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_tools::) + test(test_cam_drill_press::)'` | APP-1 | Proposed |
| UIPANES-6 | Panel content | `ui/src/panes/designer/properties/update.rs:143-176` | `Summary` holds `&Shape` | `properties/handlers/*` read `summary.single` | All tests | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_properties::) + test(test_designer_properties_shapes::)'` | - | Proposed |
| DESIGNER-6 | Identical output | `des/src/model/path.rs:1081-1092,1165-1176,1245-1257`; `pocket_operations.rs:258-407` (comment block) | delete | none | All tests | `cargo nextest run -p gcodekit5-designer` | - | Proposed |
| P2DES-12 | - | `DxfFile::bounds()` fake extents; `model/gear.rs:1-12` duplicate docblock | delete the fake field or parse EXTMIN | none (`rg -n "\.dimensions" crates/gcodekit5-ui-egui/src` empty) | All tests | same | - | Proposed |
| DESIGNER-8a | Identical | `des/src/gcode_gen.rs:43-70` | `with_line_numbers` builds on `new` | none | All tests | same | - | Proposed |
| DESIGNER-8b | - | `_units` unused; 15 call sites (`rg -c 'ToolpathToGcode::(new\|with_line_numbers)\(' crates` summed, at 2a733ad) | drop the param | `des/tests/test_cam_depth_and_gcode.rs:142,158,165,208,255` | - | same | D-9 | Blocked - owner decision |
| DESIGNER-10 | Empty result on panic | 10 `catch_unwind` sites (`rg -c catch_unwind crates/gcodekit5-designer/src` summed, at 2a733ad) | `safe_parallel_offset` that logs | none | All tests | same | - | Proposed |
| DESIGNER-9 | Tolerances per call site (0.005 vs 0.01) | `ops.rs:527`, `holes.rs:46,110,155`, `pocket_operations.rs:793`, `generator.rs:1866`, `model/polygon.rs:147`, `model/path.rs:47` | new `des/src/geometry.rs` | `test_pocket_holes.rs`, `test_audit_regressions.rs`, ops tests | All tests | same | - | Proposed |
| CAMVIS-5 | Exact fixture bytes; end-of-program and precision drift kept | 6 identical `G21`/`G90` header copies (`rg -n 'G21 ; Set units' crates/gcodekit5-camtools/src`) | `job_preamble::push_modal_header` | `fixtures/cam/*.gcode`, `cam/tests/{homing_safety,generator_fixes,audit_regressions}.rs` | Fixtures | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_)'`; `cargo nextest run -p gcodekit5-camtools` | - | Proposed |
| CAMVIS-9 | Both M5 spellings | `laser_engraver.rs:571-640` vs `:752-805` | one scan routine plus a verbose-M5 flag | `bitmap_engraving_{basic,custom}.gcode` | Fixtures | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_bitmap_engraving::)'` | CAMVIS-5 | Proposed |
| CAMVIS-7a | - | `ArcExpanderConfig::segment_length` never read; no-op stick-out branch `speeds_feeds/service.rs:221-226`; false doc `comment_processor.rs:10` | delete / fix the doc | none | All tests | `cargo nextest run -p gcodekit5-camtools` | - | Proposed |
| CAMVIS-7b | - | `ArcExpander`, `CommentProcessor` test-only | delete | `cam/tests/audit_regressions.rs:461`, `generator_fixes.rs:6,341` | - | same | D-9 | Blocked - owner decision |
| P2CAM-8a | Byte-identical output | unused `_tab_width`/`_tab_height` (`jigsaw_puzzle.rs:200-240`); `BoxType::NoSides`/`NoLeftRight` identical (`tabbed_box/mod.rs:957-961`) | delete the params | none | Fixtures | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_jigsaw::) + test(test_cam_tabbed_box::)'` | - | Proposed |
| P2CAM-8b | - | `SpeedsFeedsToolService`/`MaterialService` test-only | delete | `cam/tests/test_speeds_feeds_service.rs` | - | `cargo nextest run -p gcodekit5-camtools` | D-9 | Blocked - owner decision |
| P2CAM-8c | - | `FingerStyle::{Springs,Barbs,Snap}` render as Rectangular | - | preset JSON | - | - | D-14 | Blocked - owner decision |
| CAMVIS-3a | parse output | 11 unused `_` consts (`vis/src/visualizer/visualizer.rs:12-28`), commented blocks `:383-431`, `:545-566` | delete | none | All tests | `cargo nextest run -p gcodekit5-visualizer` | - | Proposed |
| CAMVIS-3b | `parse_gcode`, `commands()`, colour mapping | view-state methods with 0 external callers (camvis report, per-method `rg`) | slim `Visualizer` | `vis/tests/{gcode_parsing_regressions,audit_followup_regressions}.rs` (`get_cutting_bounds`, `clear_dirty`, `is_dirty`, `set_arc_segment_length`, `ToolpathCache`) | - | same | D-9, CAMVIS-2 | Blocked - owner decision |
| P2FW-3 | GRBL path of `send`/`receive` | Repetier fields/branches in `com/src/communication/mod.rs:391-396,513-576,714-798,849-867`, 0 callers outside the file | `app/src/machine.rs` Repetier link | none (`rg -n SerialCommunicator crates/*/tests` = GRBL-path only) | All tests | `cargo nextest run -p gcodekit5-communication`; `cargo nextest run -p gcodekit5-app -E 'test(test_machine_repetier::)'` | - | Proposed |
| APP-9 | Public paths | ui imports `gcodekit5_communication` in 4 files; uppercase copy on every send; duplicate TX debug loop `com/mod.rs:~708-716` | `app` re-exports | `rg 'gcodekit5_communication' crates/gcodekit5-ui-egui/tests` | All tests | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_machine_pane::) + test(test_firmware_settings::)'` | P2FW-3 | Proposed |
| UISHELL-5 / APP-9c / P2FW-10a / DESIGNER-13a | Code unchanged (comments only) | 672 "gtk" hits in ui-egui src (`rg -ci gtk crates/gcodekit5-ui-egui/src` summed), 94 in app src (same command), at 2a733ad; false file refs `ui/src/{resources.rs:2-3,theme.rs:4-6,dialogs.rs:9-10,status_bar.rs:10}`; Spanish comments in des/com | same files | none (comments) | Code | `cargo build --workspace`; `git diff --stat` shows only comment lines | - | Proposed |
| P2FW-10b | `work_zero_support` | `app/src/work_zero.rs:24-37` duplicates `work_zero_support(c) == Reported` | `work_zero_support` | `app/tests/test_work_zero.rs:123-138` | - | `cargo nextest run -p gcodekit5-app -E 'test(test_work_zero::)'` | D-9 | Blocked - owner decision |
| UISHELL-4 | Frame order (jog_allowed after Home/Unlock), `begin_frame`/`end_frame` bracket | `ui/src/panes/machine_control/mod.rs:920-1079` | private helpers + `apply_jog_actions` | `ui/tests/test_machine_{control_layout,pane,jog,rotary_jog,quick_probe,repetier_pane}.rs`, `test_hidpi.rs` | All tests | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_machine_)'` | - | Proposed |
| UISHELL-3 | Dispatch order; `handle_answer` precedence; exhaustive `match cmd` | `ui/src/app.rs:344-424,471-583,1107-1231,1310-1409` | `Managers`, `Flows` sub-structs | public accessors only (fields private) | All tests | `cargo nextest run -p gcodekit5-ui-egui` | - | Proposed |
| APP-8 | `gcodekit5_app::machine::*` paths, event order, `worker_text::ALL` | `app/src/machine.rs` 2366 lines (`wc -l` at 2a733ad) | `machine/{mod,types,controller,worker,status_tracker,repetier,grbl_stream}.rs` | public items only (`rg 'src/machine' crates/*/tests` = none) | All tests | `cargo nextest run -p gcodekit5-app` | - | Proposed |
| DESIGNER-12 | Identical output | clippy `too_many_lines`: `gcode_gen.rs:286` 555 lines; others in the designer report | same files, extracted helpers | behaviour tests | All tests | `cargo nextest run -p gcodekit5-designer`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_)'` | DESIGNER-1/5, DESIGNER-11 | Proposed |
| CAMVIS-8 | Identical `GCodeCommand` stream and mesh floats; path `visualizer::stock_removal_3d` | `parse_gcode` 356 lines (`vis/src/visualizer/visualizer.rs:586-941`); `stock_removal_3d.rs` 1283 lines | split + `pub use` | `vis/tests/*`, `ui/tests/test_visualizer_stock.rs:18` | All tests | `cargo nextest run -p gcodekit5-visualizer`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_visualizer)'` | CAMVIS-2, CAMVIS-3b | Proposed |
| APP-2 | `parse_numeric_prefix` (used by `device_profiles.rs:484`) | statics table (app report); 0 production writers | ui `Machine`, app `MachineController` | `app/tests/test_report_units.rs`, `test_machine_busy_guard.rs` | Production (dead) | `cargo nextest run -p gcodekit5-app` | APP-1, APP-8, D-9 | Blocked - owner decision |
| APP-3 | - | `rg -l 'DeviceConsoleManager\|get_console_manager\|ConsoleListener\|DeviceMessageType' crates` = module, lib.rs, test | `console.rs` | `app/tests/test_device_console_manager.rs`; `com/src/lib.rs:12` re-exports; `app/src/test_support.rs:304-306` | - | `cargo nextest run -p gcodekit5-app`; `cargo nextest run -p gcodekit5-communication` | D-9 | Blocked - owner decision |
| P2FW-7 | `ConsolePanel::{new,add_message,add_command,clear,message_count,messages}` | `app/src/console.rs` | same | `test_device_console_manager.rs` | - | `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_console_pane::)'` | APP-3 | Blocked - owner decision |
| APP-4 | - | `rg 'DeviceUiController\|DeviceProfileUiModel' crates` = 3 files | `app::device_profiles::DeviceProfileForm` | `ddb/tests/ui_integration_tests.rs` | - | `cargo nextest run -p gcodekit5-devicedb`; `cargo nextest run -p gcodekit5-app -E 'test(test_device_profiles_model::)'` | D-9 | Blocked - owner decision |
| APP-5 | Live: `jog_step_choices`, `jog_value_text`, `prepare_program`, `MAX_LINE_LENGTH`, `realtime_label`, `jog_writes_zero` | 6 test-only fns in `app/src/machine/grbl_stream.rs`; `CommandHistory::{load,save,get_config_path}` | delete | `app/tests/test_grbl_stream.rs:5-8`, `test_command_history.rs` | - | `cargo nextest run -p gcodekit5-app -E 'test(test_grbl_stream::) + test(test_command_history::)'` | D-9 | Blocked - owner decision |
| P2FW-4 | `detect_protocol` order; `announces_identity` | live = `machine.rs:2007,2010` only | registry: `id` + signatures + `announces_identity` | `com/tests/test_protocol_registry.rs`, `grbl_protocol_tests.rs:56-70` | - | `cargo nextest run -p gcodekit5-communication -E 'test(test_protocol_registry::) + test(grbl_protocol_tests::)'` | D-9 | Blocked - owner decision |
| COREBUILD-1 | egui Preferences; `SettingsManager::{open,apply,load,save,detached,config_dirs}`; `migrate.rs`; `Config` serde shape | `set/src/{persistence.rs,controller.rs,view_model.rs}`; 0 hits outside set (`rg -nw "SettingsPersistence\|SettingsController\|SettingsDialog\|..." --glob '*.rs' crates src \| grep -v gcodekit5-settings/`) | `manager.rs` + `migrate.rs` + `config.rs`; i18n moves to app | `set/tests/{config_regression_tests,test_settings_dialog_pages,test_settings_keyboard_shortcuts,test_settings_manager_store}.rs` | Config/Manager cases | `cargo nextest run -p gcodekit5-settings`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_settings_window::) + test(test_translations::)'` | D-9 | Blocked - owner decision |
| COREBUILD-1d | Old files load (`serde(default)`) | 22 unread `Config` fields (python field scan, corebuild report) | `config.rs` | `set/tests/test_settings_migrate.rs:68`, `fixtures/gtk_config/config.json` | - | `cargo nextest run -p gcodekit5-settings` | COREBUILD-1, D-10 | Blocked - owner decision |
| COREBUILD-3 | `GcodeCommand::{new,with_sequence}`, `line`, `CommandNumberGenerator` | `core/src/gcode/command.rs` lifecycle API; 0 production callers | delete | `core/benches/core_bench.rs:22,47,61-64`, `vis/src/lib.rs:12-14` | - | `cargo build --workspace --all-targets`; `cargo nextest run -p gcodekit5-core` | CAMVIS-2 (same PR), D-9 | Blocked - owner decision |
| CAMVIS-2 | `core::gcode::{words,arc}`; `Visualizer::parse_gcode` | cluster A 1761 lines, cluster B 1738 lines (`wc -l` of the listed files at 2a733ad) | `core::gcode`, `Visualizer::parse_gcode` | `vis/tests/{gcode_parsing_regressions,audit_followup_regressions}.rs`, `vis/benches/pipeline_bench.rs`, `core/benches/core_bench.rs:6` | Live test cases (move them, do not delete) | `cargo nextest run -p gcodekit5-visualizer`; `cargo bench --no-run --workspace` | D-9 | Blocked - owner decision |
| DESIGNER-7 | - | ~90 `pub` items with one `rg -w` hit (designer report; trait/serde reach not filtered) | delete per module after `rg -w` | `font_manager::get_font` pinned by `ui/tests/test_designer_text_tool.rs:158` (keep) | - | `cargo nextest run -p gcodekit5-designer`; `cargo build --workspace --all-targets` | D-9 | Blocked - owner decision |
| CAMVIS-10 | Repetier line bytes | `com/src/firmware/repetier/processor.rs:385-397` vs `core/src/gcode/words.rs:30-53` (`G1(c)X10` -> `G1X10` vs `G1 X10`) | core `strip_comments` | `com/tests/test_repetier.rs` | - | `cargo nextest run -p gcodekit5-communication -E 'test(test_repetier::)'` | characterization test | Blocked - characterization required |

### 5.3 Deliberate and owner-instruction rows

| ID | Status |
|---|---|
| UISHELL-6, UIPANES-7, APP-10, DESIGNER-13b, DESIGNER-14, COREBUILD-10, P2FW-11, P2CAM-9, P2DES-13 | Deliberate - do not fix (reasons in sec. 7) |
| COREBUILD-8c (`EEPROM.DAT`, `scripts/EEPROM.DAT`) | Deliberate - do not fix (owner instruction: never in any ticket) |
| AGENTS.md references `scripts/check-*.sh` guards and a `tool-rename-guard` nextest group that do not exist (`ls scripts/check*` fails at 2a733ad) | Deliberate - do not fix here (doc drift; belongs to a later doc-refactor) |

## 6. Ticket train

Common rules for every ticket:

- **Gate G** (every ticket):
  - `cargo fmt --check` -> exit 0.
  - `cargo clippy --workspace --all-targets` -> no warnings.
  - The ticket's own `cargo nextest run ...` commands -> all pass.
  - The full `cargo nextest run --workspace --no-fail-fast` -> all pass.
  - Capture the output in `target/temp/` and do not re-run to re-read it.
- **Never in any ticket**: `EEPROM.DAT` files, pushing or merging without an explicit owner instruction, `unsafe`, `.unwrap()` on user paths, tests that write into the repo, new top-level test binaries. A new test file is added as a `#[path]` line in that crate's `tests/all.rs`.
- **Fixtures** in `crates/gcodekit5-ui-egui/tests/fixtures/cam` are frozen. Only a phase-2 ticket may edit them, and only the files it lists.
- **Counts** in a ticket are recomputed with the stated command at the ticket's base commit; they are never copied as literals.

### Phase 1: bug batches (each fix adds a locking test; existing tests and fixtures frozen)

**T01 - Feed device state to CAM tools instead of the `device_status` global** (Effort M, Proposed)
- Ledger: APP-1 (= UISHELL-1, UIPANES-1).
- Preserves: 3-axis, non-laser output byte-equal; refusal msgids; `DrillPressParams::with_center`.
- In scope:
  - `crates/gcodekit5-ui-egui/src/panes/cam_tools/{mod,shared,drill_press,spoilboard_surfacing}.rs`, `tabbed_box/mod.rs`, `bitmap_engraving/mod.rs`, `gerber/mod.rs` (`num_axes` init);
  - `crates/gcodekit5-ui-egui/src/machine.rs` (capture `$32` like `$13`);
  - `Pane::set_device` on `SharedCamTools`.
- Out of scope: deleting `device_status.rs` (T47); `UIPANES-5` input key (T34).
- Owner: `DeviceProfile.num_axes` via `Pane::set_device`; ui `Machine` for `$32` and `$130-$132`. Frozen: CAM fixtures.
- Pins: `ui/tests/test_cam_drill_press.rs:214-280`. **Sanctioned test edit:** the child-process laser test (`laser_mode_child`, which writes the global at :260 and :266) goes back in-process and drives laser mode through the machine/profile. No other test edits.
- Locking tests:
  - a 2-axis profile makes Drill Press and Surfacing refuse;
  - a fake transport answering `$$` with `$32=1` makes Drill Press refuse;
  - no `gcodekit5_app::device_status` call remains in `crates/gcodekit5-ui-egui/src` (`rg -n 'device_status::' crates/gcodekit5-ui-egui/src` -> 0 lines).
- Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_drill_press::) + test(test_cam_spoilboard_surfacing::) + test(test_cam_tools::) + test(test_cam_gerber::)'` -> pass.
- STOP if: any `fixtures/cam/*.gcode` diff appears, or a test other than `test_cam_drill_press` needs editing.
- Prevention: Ownership recorded (doc comment on `Pane::set_device`: device facts flow through panes, never statics).

**T02 - Design file round trip: no silent shape loss, keep metadata** (Effort S-M, Proposed)
- Ledger: DESIGNER-2, P2DES-7.
- Preserves: known shapes load unchanged; file format 1.1 loads; `load_from_file` signature (add `load_from_file_report`).
- In scope: `crates/gcodekit5-designer/src/designer_state/{file_io,mod}.rs`, `serialization.rs`, `crates/gcodekit5-ui-egui/src/panes/designer/file_ops.rs`.
- Out of scope: non-atomic `save_to_file` (Rejected); hide/lock flags (documented as not saved).
- Owner: `DesignerState`. Frozen: existing tests.
- Pins: designer tests calling `load_from_file`; `test_designer_files.rs`.
- Locking tests:
  - a file with an unknown `shape_type` reports the skip and the UI notice names it;
  - a missing raster file is reported;
  - metadata (`created`, `author`, `description`), `start_depth` and stock origin survive load -> save -> load.
- Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_files::) + test(test_designer_files_app::)'`; `cargo nextest run -p gcodekit5-designer`.
- STOP if: an existing test asserts that unknown shapes vanish silently.
- Prevention: Guard added (locking test).

**T03 - Pocket generator: single centre cleanup, bounded offset loops** (Effort S, Proposed)
- Ledger: DESIGNER-1, DESIGNER-5.
- Preserves: cuts for normal inputs.
- In scope: `crates/gcodekit5-designer/src/pocket_operations.rs`, `pocket_operations/holes.rs` (share `MAX_OFFSET_LEVELS`).
- Out of scope: the commented-out `generate_raster_cleanup` block (T35); function splits (T44).
- Owner: `PocketGenerator`. Frozen: tests.
- Pins: `des/tests/test_pocket_holes.rs:47,274,310` (RapidMove counts).
- Locking tests: an adaptive pocket emits exactly one cleanup per Z pass; `stepover = 1e-6` and `step_down = 1e-6` return within the cap in a release-mode-equivalent path (no `debug_assert` reliance).
- Commands: G + `cargo nextest run -p gcodekit5-designer -E 'test(test_pocket_holes::) + test(test_audit_regressions::)'`.
- STOP if: a RapidMove count assertion fails. That count encodes the duplicate; move the edit to phase 2 with the line enumerated.
- Prevention: Guard added.

**T04 - Path and raster rotation honoured by painter, SVG export and hit-testing** (Effort M, Proposed)
- Ledger: P2DES-2.
- Preserves: G-code output; `Path::render()` unchanged (the generator and `ops::perform_offset` rotate it themselves).
- In scope: `crates/gcodekit5-designer/src/model/{path,mod}.rs` (add `rotated_render`, raster bounds), `crates/gcodekit5-ui-egui/src/panes/designer/{canvas/rendering.rs,svg_export.rs}`.
- Out of scope: changing `render()` globally (double-rotates G-code).
- Owner: `DesignPath::rotated_render`. Frozen: G-code.
- Pins: none known.
- Locking tests: a rotated path's painted outline, SVG output and `distance_to_point` match the G-code geometry within 0.01 mm.
- Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_canvas::) + test(test_designer_snapshots::)'`; `cargo nextest run -p gcodekit5-designer`.
- STOP if: a snapshot under `crates/gcodekit5-ui-egui/tests/snapshots` changes for an unrotated shape.
- Prevention: Guard added.

**T05 - One text layout for canvas, bounds and G-code** (Effort M, Proposed)
- Ledger: P2DES-3.
- Preserves: unrotated, descender-free, single-line text with `line_gap = 0`.
- In scope: `crates/gcodekit5-designer/src/{toolpath/generator.rs (text section),model/text.rs,font_manager.rs}`, `crates/gcodekit5-ui-egui/src/panes/designer/canvas/text_outline.rs`.
- Out of scope: multi-line text UI.
- Owner: one designer layout function. Frozen: the canvas painter (rotation is CCW-positive like every other shape).
- Pins: none (`rg -ln "generate_text_toolpath|build_text_outline" crates/*/tests` empty). First `rg -l` the fixtures for text programs.
- Locking test: rotated "gyp" text, where G-code glyph origins equal painter origins within 0.01 mm.
- Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_text_tool::)'`; `cargo nextest run -p gcodekit5-designer`.
- STOP if: a fixture contains text G-code.
- Prevention: Guard added.

**T06 - SVG export: lines not double-rotated, text anchored, raster box filled** (Effort S, Proposed)
- Ledger: P2DES-1, P2DES-8.
- Preserves: rect, ellipse and text rotation transforms.
- In scope: `crates/gcodekit5-ui-egui/src/panes/designer/svg_export.rs`; new `crates/gcodekit5-ui-egui/tests/test_designer_svg_export.rs`.
- Out of scope: path rotation (T04).
- Owner: `svg_export::element`. Frozen: other elements' output.
- Pins: none.
- Locking tests: a 45-degree line exports endpoints only; text has `text-anchor:middle`.
- Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_svg_export::)'`.
- STOP if: none expected.
- Prevention: Guard added.

**T07 - DXF import: POLYLINE bulges, unsupported-entity report, non-UTF-8, true arcs** (Effort M, Proposed)
- Ledger: P2DES-4, P2DES-6, P2DES-9.
- Preserves: spline-fit/3D polylines; LWPOLYLINE handling.
- In scope: `crates/gcodekit5-designer/src/{dxf_parser.rs,import.rs}`, `crates/gcodekit5-ui-egui/src/panes/designer/file_ops.rs` (notice), `po/*`.
- Out of scope: BLOCKS/INSERT expansion (report only).
- Owner: `parse_polyline`, `DxfFile.unsupported_entities`. Frozen: existing DXF tests.
- Pins:
  - msgid "Unsupported 3D solids discarded". Generalising it is a sanctioned msgid change: update every `po/*.po` and pass `test_translations`.
  - `test_dxf_*`.
- Locking tests:
  - a bulged R12 POLYLINE imports arcs;
  - a CP1252 DXF loads;
  - INSERT and MTEXT are counted;
  - a DXF circle of r=500 has sag <= 0.01 mm.
- Commands: G + `cargo nextest run -p gcodekit5-designer -E 'test(test_dxf_polyline_splinefit::) + test(test_dxf_spline_import::) + test(test_dxf_z_metadata::) + test(test_import_transforms_units::)'`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_import::) + test(test_translations::)'`.
- STOP if: an existing DXF test's vertex count changes.
- Prevention: Guard added.

**T08 - SVG import: rect corner radii and no-viewBox units** (Effort S, Proposed)
- Ledger: P2DES-5, P2DES-10.
- Preserves: viewBox files; unitless = mm (documented).
- In scope: `crates/gcodekit5-designer/src/{import.rs,import/svg_parse.rs}`.
- Out of scope: `<use>`/`<text>` support.
- Owner: `element_to_shape`, `svg_parse`. Frozen: existing tests.
- Pins: `test_import_transforms_units.rs`.
- Locking tests: `rect rx=2` round-trips with `corner_radius`; `width="100mm"` without a viewBox scales at 25.4/96 mm per user unit.
- Commands: G + `cargo nextest run -p gcodekit5-designer -E 'test(test_import_transforms_units::)'`.
- STOP if: an existing assertion relies on cm/in-per-user-unit scaling.
- Prevention: Guard added.

**T09 - Laser merge deviation bound, point filter, arc-length ramp** (Effort S-M, Proposed)
- Ledger: DESIGNER-4, P2DES-11, DESIGNER-3.
- Preserves: straight laser geometry; linear-only ramps.
- In scope: `crates/gcodekit5-designer/src/{gcode_gen.rs,toolpath/generator.rs,toolpath/segment.rs}`.
- Out of scope: other match-arm dedupe.
- Owner: `optimize_toolpath_for_laser`, `ToolpathSegment::arc_length`. Frozen: tests.
- Pins: `des/tests/test_cam_depth_and_gcode.rs:66,234,367`.
- Step 1: measure the sag on a synthetic r=50 arc and record it in the PR.
- Locking tests: merged chord sag <= `curve_simplification_tolerance`; the ramp on a circle descends `step_down` per lap with slope within 1% of requested.
- Commands: G + `cargo nextest run -p gcodekit5-designer -E 'test(test_cam_depth_and_gcode::)'`.
- STOP if: the segment count asserted at `:234` changes. That assertion encodes the bug; enumerate it and move the edit to phase 2.
- Prevention: Guard added; CHANGELOG notes the curved-laser output change.

**T10 - Gerber input safety: warn on unsupported constructs, no empty drill programs, deterministic outline** (Effort M, Proposed)
- Ledger: P2CAM-1a, P2CAM-2a, CAMVIS-6.
- Preserves: every `fixtures/cam/gerber_*.gcode` byte-equal.
- In scope: `crates/gcodekit5-camtools/src/gerber.rs`, `crates/gcodekit5-ui-egui/src/panes/cam_tools/gerber/mod.rs` (show the warning).
- Out of scope: region/macro support (D-3), Excellon parsing or layer removal (D-4), outline compensation (T20).
- Owner: `parse_gerber_to_sketches`, `detect_layer_files`. Frozen: fixtures.
- Pins: `test_cam_gerber.rs`, `cam/tests/audit_regressions.rs:570`.
- Locking tests:
  - a G36/G37 file yields a warning count > 0;
  - an Excellon or `.txt` file does not produce a G-code program;
  - two outline-named files pick the same file in the UI and in rubout, sorted by name;
  - a rubout ring panic logs `warn`.
- Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_gerber::)'`; `cargo nextest run -p gcodekit5-camtools -E 'test(audit_regressions::)'`.
- STOP if: any gerber fixture diff.
- Prevention: Guard added.

**T11 - camtools parameter validation gaps** (Effort S, Proposed)
- Ledger: P2CAM-5, P2CAM-4, P2CAM-6a, UIPANES-2b.
- Preserves: all fixtures; existing error strings; seeds < 2^24 output.
- In scope: `crates/gcodekit5-camtools/src/{tabbed_box/mod.rs,jigsaw_puzzle.rs,probe_routines.rs,drill_press.rs}`; ui pages `cam_tools/{tabbed_box/mod.rs,jigsaw.rs,probe_routines/*,drill_press.rs}`.
- Out of scope: probe mode restore (D-12); slow re-probe feature.
- Owner: camtools validators (UI mirrors them until T30). Frozen: fixtures.
- Pins: `cam/tests/{generator_fixes,audit_regressions,probe_routines}.rs`, `ui/tests/test_cam_{tabbed_box,jigsaw,probe_routines,drill_press,input_bounds}.rs`.
- Locking tests:
  - negative `feed_rate`, `laser_power` or excessive `burn` are refused;
  - seed `1 << 24` is refused;
  - edge/corner/bore/boss accept a zero `safe_height`;
  - a peck depth past `MAX_PECKS` errors at the field.
- Commands: G + `cargo nextest run -p gcodekit5-camtools`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_tabbed_box::) + test(test_cam_jigsaw::) + test(test_cam_probe_routines::) + test(test_cam_drill_press::) + test(test_cam_input_bounds::)'`.
- STOP if: a fixture or preset JSON diff appears.
- Prevention: Guard added.

**T12 - Firmware settings and form rules** (Effort S, Proposed)
- Ledger: P2FW-5, P2FW-9.
- Preserves: `$0=10 (step pulse, usec)` -> `10`; `$22` bitmask rule.
- In scope: `crates/gcodekit5-app/src/{device_identity.rs,firmware_settings/mod.rs,material_form.rs}`, `crates/gcodekit5-communication/src/firmware/grbl/settings.rs`.
- Out of scope: definition tables (pass 3).
- Owner: same files. Frozen: tests.
- Pins: `app/tests/test_firmware_settings_model.rs`, `ui/tests/test_firmware_settings.rs`, `com/tests/grbl_protocol_tests.rs`.
- Locking tests (new `app/tests/test_material_form.rs`):
  - `$32=2` is not laser;
  - `$30-$32` fall under Spindle;
  - `$70=my ssid` keeps the space;
  - "nan", "inf" and negative material values are refused.
- Commands: G + `cargo nextest run -p gcodekit5-app -E 'test(test_firmware_settings_model::) + test(test_material_form::)'`; `cargo nextest run -p gcodekit5-communication -E 'test(grbl_protocol_tests::)'`.
- STOP if: a test asserts the Homing category for `$30`.
- Prevention: Guard added.

**T13 - Transport robustness: serial write timeout, Repetier translation** (Effort S, Proposed)
- Ledger: APP-7, P2FW-6.
- Preserves: TCP path; the Repetier idle `wait` ack; `acks_due` resend rule.
- In scope: `crates/gcodekit5-communication/src/communication/serial.rs`, `firmware/repetier/processor.rs`.
- Out of scope: `strip_comments` drift (T58); dead Repetier mode (T39).
- Owner: `RealSerialPort::write`, `RepetierProcessor`. Frozen: tests.
- Pins: `com/tests/test_repetier.rs`, `app/tests/test_machine_repetier.rs`.
- Locking tests:
  - a fake `ReadWrite` timing out twice then succeeding completes the write;
  - a bare `X10` after `G0` is not forced to `G1`;
  - `"waiting for heater"` is not an ack;
  - bare `G92` resets the stand-in WCO.
- Commands: G + `cargo nextest run -p gcodekit5-communication -E 'test(test_repetier::) + test(audit_followup_tests::)'`; `cargo nextest run -p gcodekit5-app -E 'test(test_machine_repetier::)'`.
- STOP if: a Repetier test asserts `G1` insertion for bare axis lines.
- Prevention: Guard added.

**T14 - Root logging on Windows** (Effort S, Proposed)
- Ledger: COREBUILD-9.
- Preserves: stdout pretty logging; `RUST_LOG` precedence.
- In scope: `src/lib.rs`, root `Cargo.toml` (`tracing-subscriber` features).
- Out of scope: log rotation policy beyond the per-user dir.
- Owner: `init_logging`. Frozen: non-Windows behaviour.
- Pins: none.
- Change: `.with_ansi(false)`, compact format, per-user data dir, `try_init()?`.
- Commands: G + `cargo build -p gcodekit5-design`; `cargo check -p gcodekit5-design --release --target x86_64-pc-windows-gnu` if that target is installed.
- STOP if: the Windows arm cannot be compiled anywhere; then ship behind review with that stated.
- Prevention: No guard justified (platform-gated init code, no test harness).

**T15 - Tool number persistence** (Effort S, Blocked - owner decision D-1)
- Ledger: P2FW-1. In scope: `crates/gcodekit5-app/src/{tools_manager_backend.rs,tool_form.rs}`, `crates/gcodekit5-ui-egui/src/windows/tools_manager/view.rs`. Pins: `app/tests/test_tools_manager_backend.rs`. Locking test: edited number survives save/load (option a) or the field is not editable (b). Commands: G + `cargo nextest run -p gcodekit5-app -E 'test(test_tools_manager_backend::)'`. Prevention: Guard added.

**T16 - WebSocket and Auto Reconnect** (Effort S remove / L implement, Blocked - owner decision D-2)
- Ledger: APP-6. In scope: `crates/gcodekit5-app/src/{device_profiles.rs,machine.rs}`, `crates/gcodekit5-communication/src/communication/{mod.rs,tcp.rs}`, `crates/gcodekit5-ui-egui/src/windows/device_manager/view.rs`, `po/*`. Preserves: old profile/config files load (serde fields kept). Pins: `rg 'WebSocket|auto_reconnect' crates/*/tests`. Commands: G + `cargo nextest run -p gcodekit5-app -E 'test(test_device_profiles_model::)'`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_device_manager::) + test(test_translations::)'`. Prevention: Guard added (test that each offered connection type connects or is absent).

**T17 - Arc Segment Length preference** (Effort S, Blocked - owner decision D-6)
- Ledger: CAMVIS-1. In scope: `crates/gcodekit5-ui-egui/src/{panes/visualizer/toolpath.rs,windows/settings.rs}`. Pins: `ui/tests/test_settings_window.rs:264,313` (kept for option a; sanctioned edit for option b). Visualizer field/setter removal belongs to T51. Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_settings_window::) + test(test_visualizer::)'`. Prevention: Guard added.

**T18 - grblHAL error/alarm tables** (Effort S, Blocked - characterization required)
- Ledger: P2FW-2. Prerequisite: compare `error_decoder.rs:44-81,100-111` with upstream grblHAL `status_code_t`/`alarm_code_t` and cite the source revision in the PR. Then either correct the arms or return "Unknown code" for 39+ / 10+, and wire the GRBL 1.1 texts into console formatting (`ui/src/panes/console.rs:127`). Pins: `com/tests/test_error_decoder.rs` (edit only the extended-code assertions, enumerated in the PR). Commands: G + `cargo nextest run -p gcodekit5-communication -E 'test(test_error_decoder::)'`. STOP if: no authoritative upstream source is available. Prevention: Guard added.

### Phase 2: correctness fixes whose fixtures encode the bug (each fixture edit enumerated)

**T19 - Drill press emits G17** (Effort S, Blocked - owner decision D-7; after T37)
- Ledger: CAMVIS-4. In scope: `crates/gcodekit5-camtools/src/drill_press.rs`. **Fixture edits (exactly):** `crates/gcodekit5-ui-egui/tests/fixtures/cam/drill_press_default.gcode` and `drill_press_custom.gcode`, each gaining one `G17 ; XY plane selection` line after `G90`; no other byte changes (`git diff --stat` shows 2 files, +1 line each). Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_drill_press::) + test(test_cam_presets::)'`. STOP if: any other fixture changes. Prevention: Guard added (the fixture itself).

**T20 - Gerber board outline with tool compensation** (Effort M, Blocked - owner decision D-5)
- Ledger: P2CAM-3. In scope: `crates/gcodekit5-camtools/src/gerber.rs` (`generate_simple_trace_from_sketches`). **Fixture edit:** `fixtures/cam/gerber_outline.gcode` regenerated; the PR shows the old two-loop body vs the new single offset loop with step-down passes. **Test edit:** `test_cam_gerber.rs:137-139` if it asserts loop count. Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_gerber::)'`. STOP if: `gerber_default` or `gerber_custom` change. Prevention: Guard added (fixture plus a geometry assertion: the loop is offset by `tool_diameter/2`).

**T21 - Speeds-and-feeds limit order and power formula** (Effort S, Blocked - owner decision D-13)
- Ledger: P2CAM-7. In scope: `crates/gcodekit5-camtools/src/speeds_feeds/calculator.rs`. **Fixture edits:** `fixtures/cam/speeds_feeds_clamped.txt`, `speeds_feeds_custom.txt`; the PR lists every changed number with its formula. Pins: `cam/tests/test_speeds_feeds_calculator.rs`, `ui/tests/test_cam_speeds_feeds.rs`. Commands: G + `cargo nextest run -p gcodekit5-camtools -E 'test(test_speeds_feeds_calculator::)'`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_speeds_feeds::)'`. Prevention: Guard added (max RPM never exceeded).

**T22 - Probe programs restore the modal state** (Effort S, Blocked - owner decision D-12)
- Ledger: P2CAM-6b. In scope: `crates/gcodekit5-camtools/src/probe_routines.rs` header/footer. **Fixture edits:** every `fixtures/cam/probe_*.gcode` (list them with `ls crates/gcodekit5-ui-egui/tests/fixtures/cam/probe_*.gcode` at the base commit), with only the header/footer lines changed. Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_probe_)'`; `cargo nextest run -p gcodekit5-camtools -E 'test(probe_routines::)'`. STOP if: any body line changes. Prevention: Guard added.

**T23 - One image engraver** (Effort L, Blocked - owner decision D-8)
- Ledger: UIPANES-4. In scope: `crates/gcodekit5-camtools/src/laser_engraver.rs`, `crates/gcodekit5-designer/src/engraving/image_engraver.rs`, designer callers. **Fixture edits:** per the option chosen, either the designer raster expectations or `fixtures/cam/bitmap_engraving_*.gcode`, enumerated in the PR. Pins: `cam/tests/{generator_fixes,homing_safety}.rs`, `ui/tests/test_cam_bitmap_engraving.rs`. Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_bitmap_engraving::) + test(test_designer_)'`; `cargo nextest run -p gcodekit5-camtools`. Prevention: Ownership recorded (single engraver module).

### Phase 3: infra and dependency diet

**T24 - Remove unused dependencies, finish workspace inheritance** (Effort S, Proposed)
- Ledger: COREBUILD-6.
- Preserves: one resolved version per crate in `Cargo.lock`.
- In scope: every `Cargo.toml`.
- Out of scope: `mutants` dev-dep (used); `Cargo.lock` hand edits.
- Owner: root `[workspace.dependencies]`. Frozen: all tests.
- Changes:
  - drop the machete hits listed in `target/temp/refactor/scan-machete.txt` (re-run `cargo machete` at the base commit for the live list);
  - drop the designer `csgrs` dev-dep duplicate and the root `tracing-subscriber` `json` feature;
  - drop `craft-svg`, `craft-dxf`, `craft-mesh`, `craft-i18n` and the `unused_workspace_dependencies = "allow"` lint;
  - move pinned serde, serde_json, tracing, anyhow, chrono, thiserror, uuid, tempfile and glam to `{ workspace = true }`.
- Commands: G + `cargo build --workspace --all-targets`; `cargo machete` -> "didn't find any unused dependencies"; `cargo tree -d` -> no new duplicates vs base.
- STOP if: a removal breaks a feature-gated build.
- Prevention: Guard added (`cargo machete` step in `.github/workflows/code-quality.yml`).

**T25 - CI: remove no-op and duplicate jobs** (Effort S, Proposed)
- Ledger: COREBUILD-7.
- Preserves: the `check-no-gtk` step, nextest `gpu` group, release dry-run gating, test coverage executed.
- In scope: `.github/workflows/{code-quality,msrv,test-coverage}.yml`, a new composite action for the apt step.
- Out of scope: `release.yml` logic.
- Changes:
  - merge the two clippy jobs;
  - delete `unwrap-audit`, `test-isolation` and the `if: false` coverage job;
  - MSRV becomes `cargo check --workspace --all-targets`;
  - add `cargo bench --no-run --workspace`.
- Commands: G + `actionlint` (if installed) -> clean; CI run on the PR -> green.
- STOP if: a removed job is a required status check in branch protection (ask the owner).
- Prevention: No guard justified (workflow files).

**T26 - Delete `legacy/`** (Effort S, Proposed)
- Ledger: COREBUILD-8a. In scope: `git rm -r legacy`. Out of scope: `EEPROM.DAT` (owner instruction), `LEGACY_CODE.md` (doc pass), D-11 files. Commands: G + `cargo build --workspace`; `rg -l legacy --glob '*.toml' --glob '*.rs' .` -> no build reference. Prevention: No guard justified.

**T27 - Delete stale root files** (Effort S, Blocked - owner decision D-11)
- Ledger: COREBUILD-8b. In scope: whichever of `package.json`, `package-lock.json`, `devices.json`, `flatpak/com.github.feveal.*` the owner approves. Commands: G. Prevention: No guard justified.

### Phase 4: behaviour-preserving refactors (suite passes unchanged; no test edits)

**T28 - Delete unused core error enums and zero-reference aliases** (Effort S, Proposed)
- Ledger: COREBUILD-2, COREBUILD-4a. Preserves: `core::{Error, Result}`, `Error::other`, `Error::Io` Display. In scope: `crates/gcodekit5-core/src/{error.rs,lib.rs,types/aliases.rs}`. Out of scope: Rc aliases and test-only aliases (T54). Verify each name with `rg -nw <name> --glob '*.rs' crates src` -> definition only. Commands: G + `cargo nextest run -p gcodekit5-core`; `cargo build --workspace --all-targets`. STOP if: any test references a removed name. Prevention: No guard justified.

**T29 - One `MM_PER_INCH` and feed-rate conversion** (Effort S-M, Proposed)
- Ledger: COREBUILD-5. Preserves: byte-identical output; display precision. In scope: `crates/gcodekit5-core/src/units.rs`, the 5 private consts (`rg -n 'const MM_PER_INCH' crates`), `ui/src/panes/machine_control/jog.rs:177-193`. Out of scope: length formatting call sites. Commands: G + `cargo nextest run -p gcodekit5-core -E 'test(test_units::)'`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_machine_jog::)'`; `cargo nextest run -p gcodekit5-communication -E 'test(grbl_protocol_tests::)'`. Prevention: Ownership recorded (doc on `core::units`).

**T30 - camtools exports validation limits; UI pages import them** (Effort S, Proposed; after T11)
- Ledger: UIPANES-2. Preserves: accept/reject sets and msgids. In scope: `crates/gcodekit5-camtools/src/{drill_press,jigsaw_puzzle,tabbed_box/mod,spoilboard_surfacing}.rs` (`pub const`), the four UI pages. Out of scope: moving msgids into the domain. Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_input_bounds::) + test(test_cam_jigsaw::) + test(test_cam_tabbed_box::) + test(test_cam_drill_press::)'`. Prevention: Ownership recorded.

**T31 - Designer pane uses domain export check and engraving params** (Effort S-M, Proposed)
- Ledger: UIPANES-3. Preserves: refusals and program text. In scope: `crates/gcodekit5-ui-egui/src/panes/designer/export.rs`, `crates/gcodekit5-designer/src/designer_state/gcode.rs`, `model` (`RasterImage::engraving_params`). Pins: `ui/tests/test_designer_files.rs:300-352`. STOP if: the item-3 differential G-code run (copy export with and without the pane's setters) differs. Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_files::) + test(test_designer_editor_handoff::) + test(test_designer_properties_cam::)'`. Prevention: Ownership recorded.

**T32 - CAM dispatch owned by the designer crate** (Effort M, Proposed)
- Ledger: DESIGNER-11. Preserves: G-code byte-identical; preview test hooks (`panic_builds`, `build_delay`, `started`) stay in egui. In scope: `crates/gcodekit5-designer/src/designer_state/gcode.rs`, `crates/gcodekit5-ui-egui/src/panes/designer/canvas/toolpath_preview.rs`. Note: adopting `laser_params.use_global` in the preview changes preview feed/power only. Commands: G + `cargo nextest run -p gcodekit5-designer`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_toolpath::) + test(test_designer_properties_cam::)'`. STOP if: any G-code assertion changes. Prevention: Ownership recorded.

**T33 - Shared library notice and custom-library file handling** (Effort S, Proposed)
- Ledger: UISHELL-2, P2FW-8a. Preserves: every `n_!` msgid literal; notice text. In scope: `crates/gcodekit5-ui-egui/src/windows/{manager_ui.rs,tools_manager/mod.rs,materials_manager/mod.rs,device_manager/mod.rs}`, `crates/gcodekit5-app/src/{tools,materials}_manager_backend.rs` (delete only fns with 0 `rg -nw` hits incl. tests; share `CustomLibraryFile`). Out of scope: test-only fns (T52). Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_tools_manager::) + test(test_materials_manager::) + test(test_device_manager::) + test(test_translations::)'`; `cargo nextest run -p gcodekit5-app -E 'test(test_tools_manager_backend::) + test(test_materials_manager_backend::)'`. Prevention: No guard justified.

**T34 - Per-frame work in CAM output key and designer summary** (Effort S, Proposed; after T01)
- Ledger: UIPANES-5, UIPANES-6. Preserves: output hides on any input change; panel content. In scope: `crates/gcodekit5-ui-egui/src/panes/cam_tools/mod.rs`, `panes/designer/properties/{update,mod}.rs`, `properties/handlers/*` (mechanical `summary.single` edits). Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_tools::) + test(test_cam_drill_press::) + test(test_designer_properties)'`. Prevention: No guard justified (unmeasured; structure only).

**T35 - Designer hygiene: debug loops, dead block, constructor dedupe, logged offset** (Effort S, Proposed)
- Ledger: DESIGNER-6, P2DES-12, DESIGNER-8a, DESIGNER-10. Preserves: identical output; empty result on an offset panic (now logged at warn). In scope: `crates/gcodekit5-designer/src/{model/path.rs,pocket_operations.rs,gcode_gen.rs,ops.rs,pocket_operations/holes.rs,dxf_parser.rs,model/gear.rs}`. Out of scope: dropping `_units` (T56). Commands: G + `cargo nextest run -p gcodekit5-designer`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_)'`. Prevention: No guard justified.

**T36 - Designer geometry helpers** (Effort M, Proposed)
- Ledger: DESIGNER-9. Preserves: per-call-site tolerances (0.005 vs 0.01). In scope: new `crates/gcodekit5-designer/src/geometry.rs`; point-in-polygon, point-segment distance, `prepare_polygon`/`to_pline`. Out of scope: unifying flattener tolerances. Commands: G + `cargo nextest run -p gcodekit5-designer -E 'test(test_pocket_holes::) + test(test_audit_regressions::) + test(test_chamfer_dimensions::)'`. STOP if: any vertex-count assertion changes. Prevention: Ownership recorded.

**T37 - camtools shared modal header and emitter hygiene** (Effort S, Proposed)
- Ledger: CAMVIS-5, CAMVIS-9, CAMVIS-7a, P2CAM-8a. Preserves: exact bytes of every `fixtures/cam/*` file; end-of-program and precision drift kept as parameters. In scope: `crates/gcodekit5-camtools/src/{job_preamble,spoilboard_grid,spoilboard_surfacing,laser_engraver,drill_press,tabbed_box/mod,gerber,arc_expander,comment_processor,speeds_feeds/service,jigsaw_puzzle}.rs`. Out of scope: gerber/probe adopting `job_preamble` (deliberate). Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_cam_)'`; `cargo nextest run -p gcodekit5-camtools`. STOP if: any fixture diff. Prevention: Ownership recorded (`job_preamble` doc lists its real users).

**T38 - Visualizer dead source** (Effort S, Proposed)
- Ledger: CAMVIS-3a. In scope: `crates/gcodekit5-visualizer/src/visualizer/visualizer.rs` (underscore consts, commented blocks, `safe_to_i32` if only referenced from them). Commands: G + `cargo nextest run -p gcodekit5-visualizer`. Prevention: No guard justified.

**T39 - Communication: remove the dead Repetier mode, fix the layering leak** (Effort S-M, Proposed)
- Ledger: P2FW-3, APP-9. Preserves: the GRBL `send`/`receive` path and `reply_ledger`; `RepetierProcessor`. In scope: `crates/gcodekit5-communication/src/communication/mod.rs`, `firmware/mod.rs` docs, `crates/gcodekit5-app/src/lib.rs` (re-export `ControllerProtocol`, status types, `list_ports`), the 4 ui files. Remove the ui -> communication edge only if `rg 'gcodekit5_communication' crates/gcodekit5-ui-egui/tests` is empty; otherwise keep it and note why. Commands: G + `cargo nextest run -p gcodekit5-communication`; `cargo nextest run -p gcodekit5-app -E 'test(test_machine_repetier::) + test(test_machine_controller::)'`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_machine_) + test(test_firmware_settings::)'`. Prevention: Ownership recorded.

**T40 - Comments that lie, GTK references and Spanish** (Effort S, Proposed)
- Ledger: UISHELL-5 (comment part), APP-9 (comment part), P2FW-10a, DESIGNER-13a. Preserves: code. In scope: comment lines only, in the files named by those findings. Rewrite parity notes as the rule ("matches the original behaviour: X") and drop references to deleted files. Out of scope: msgids, identifiers. Commands: G + `cargo build --workspace`; `git diff -U0 | rg '^[+-]' | rg -v '^\+\+\+|^---' | rg -v '^\s*[+-]\s*(//|///|//!)'` -> empty (comment-only diff). Prevention: No guard justified.

**T41 - Split Machine Control `show_controls`** (Effort S, Proposed)
- Ledger: UISHELL-4. Preserves: frame order (jog_allowed read after Home/Unlock), `jog.begin_frame/end_frame` bracket, the exhaustive `run` list. In scope: `crates/gcodekit5-ui-egui/src/panes/machine_control/mod.rs`. Commands: G + `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_machine_) + test(test_hidpi::)'`. Prevention: No guard justified.

**T42 - `GcodeKitApp`: extract `Managers`** (Effort M, Proposed) and **T43 - `GcodeKitApp`: extract `Flows` and text-field focus** (Effort M, Proposed; after T42)
- Ledger: UISHELL-3. Preserves: dispatch order (`dispatch_design` first); `handle_answer` precedence; the flat exhaustive `match cmd` (no `_ =>`). In scope: `crates/gcodekit5-ui-egui/src/app.rs`, `app/*.rs`. Out of scope: public accessors used by tests. Commands: G + `cargo nextest run -p gcodekit5-ui-egui`. Prevention: No guard justified.

**T44 - Split `app/src/machine.rs` into a module directory** (Effort M, Proposed; before T47)
- Ledger: APP-8. Preserves: every `gcodekit5_app::machine::*` path, event order, `worker_text::ALL`. In scope: `crates/gcodekit5-app/src/machine.rs` -> `machine/{mod,types,controller,worker,status_tracker,repetier}.rs` (`grbl_stream.rs` already exists). Commands: G + `cargo nextest run -p gcodekit5-app`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_machine_)'`. STOP if: a test needs an import change. Prevention: No guard justified.

**T45 - Decompose pocket generators** (Effort M, Proposed; after T03)
- Ledger: DESIGNER-12 (pocket part). In scope: `crates/gcodekit5-designer/src/pocket_operations.rs` (the shared Z-pass/loop helper for adaptive and contour-parallel; `generate_raster_pocket`). Commands: G + `cargo nextest run -p gcodekit5-designer`. Prevention: No guard justified.

**T46 - Decompose designer generator and serializer monsters** (Effort L, Proposed; after T32)
- Ledger: DESIGNER-12 (rest). In scope: `crates/gcodekit5-designer/src/{gcode_gen.rs (generate_body_continuing),toolpath/generator.rs (generate_rectangle_contour, create_multipass_toolpaths),designer_state/gcode.rs (emit_shapes_gcode),serialization.rs (from_drawing_object),import.rs (convert_entities_to_shapes),dxf_parser.rs (parse_polyline)}`. Commands: G + `cargo nextest run -p gcodekit5-designer`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_designer_)'`. Prevention: No guard justified.

### Phase 5: structural (dead-subsystem deletions that edit tests; file splits last)

Rules for every phase-5 ticket:

- Each is **Blocked - owner decision D-9** until the owner approves deleting tests that pin dead code.
- Each lists its sanctioned test edits. Any test edit not listed is a STOP.
- Before deleting a symbol, prove it is dead with `rg -nw <symbol> --glob '*.rs' crates src` and also search for trait-impl and serde reach (`rg 'impl .* for|#\[serde'` on the defining type).
- Where a guard test is rewritten rather than deleted, show non-vacuity: revert the guarded fix locally, show the rewritten test fails, and paste that output in the PR.

**T47 - Retire `device_status.rs`** (Effort S; after T01 and T44)
- Ledger: APP-2.
- Preserves: `parse_numeric_prefix`, moved to `device_profiles.rs`.
- Sanctioned test edits:
  - delete `crates/gcodekit5-app/tests/test_report_units.rs` and `test_machine_busy_guard.rs`, and their `tests/all.rs` lines;
  - before deleting, port any case covering live code (`parse_numeric_prefix`, `MachineController::is_streaming`) to `test_device_profiles_model.rs` / `test_machine_controller.rs`, and prove the ported cases are non-vacuous.
- Also drop `once_cell` from `crates/gcodekit5-app/Cargo.toml`.
- Commands: G + `cargo nextest run -p gcodekit5-app`; `rg -n device_status crates src` -> no code hits.
- Prevention: Guard added (`rg -n 'static .*(RwLock|Mutex|Atomic|Lazy|OnceLock)' crates/gcodekit5-app/src` reviewed in the PR; record the rule "no process-global device state" in the app crate docs).

**T48 - Remove `device_console_manager`, listener plumbing and dead `ConsolePanel` members** (Effort M)
- Ledger: APP-3, P2FW-7.
- Sanctioned test edits: delete `crates/gcodekit5-app/tests/test_device_console_manager.rs` and its `all.rs` line; trim `app/src/test_support.rs:304-306` listener methods.
- Removes: `Communicator::add_listener`/`remove_listener`, `CommunicatorEvent`, `notify_listeners`, the `com/src/lib.rs:12` re-exports.
- Commands: G + `cargo nextest run -p gcodekit5-app`; `cargo nextest run -p gcodekit5-communication`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_console_pane::)'`.

**T49 - Remove the devicedb UI controller** (Effort S)
- Ledger: APP-4.
- Sanctioned test edits: delete `crates/gcodekit5-devicedb/tests/ui_integration_tests.rs` (2 tests). Move any assertion not already covered to `crates/gcodekit5-app/tests/test_device_profiles_model.rs`, with a non-vacuity proof. If `tests/all.rs` would become empty, keep it with a placeholder module only if `cargo nextest` requires it.
- Commands: G + `cargo nextest run -p gcodekit5-devicedb`; `cargo nextest run -p gcodekit5-app -E 'test(test_device_profiles_model::)'`.

**T50 - Delete visualizer pipeline (cluster A) and the core command lifecycle** (Effort M)
- Ledger: CAMVIS-2 (cluster A), COREBUILD-3.
- Preserves: `core::gcode::{words,arc}`, `GcodeCommand::{new,with_sequence}`, `CommandNumberGenerator`.
- Sanctioned test edits:
  - in `crates/gcodekit5-visualizer/tests/{gcode_parsing_regressions,audit_followup_regressions}.rs`, delete only cases that import `GcodeParser`, `ProcessorPipeline`, `ArcExpander` (vis), `PatternRemover` or stream readers;
  - **move**, do not delete, the cases covering `words`, `arc_sweep` and `Visualizer::parse_gcode` (list them in the PR by test name);
  - delete `crates/gcodekit5-visualizer/benches/pipeline_bench.rs`;
  - edit `crates/gcodekit5-core/benches/core_bench.rs:22,47,61-64`;
  - trim `vis/src/lib.rs:12-14`.
- Commands: G + `cargo nextest run -p gcodekit5-visualizer`; `cargo nextest run -p gcodekit5-core`; `cargo bench --no-run --workspace`.
- STOP if: a moved case fails.

**T51 - Delete visualizer scene graph (cluster B) and GTK view state** (Effort M; after T50)
- Ledger: CAMVIS-2 (cluster B), CAMVIS-3b, CAMVIS-1 field/setter (only once D-6 is decided).
- Sanctioned test edits: remove assertions on `get_cutting_bounds`, `clear_dirty`, `is_dirty`, `set_arc_segment_length` and `ToolpathCache` in `vis/tests/{gcode_parsing_regressions,audit_followup_regressions}.rs` (including `:237-248`); keep every `parse_gcode`/`commands()` assertion.
- Commands: G + `cargo nextest run -p gcodekit5-visualizer`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_visualizer)'`.

**T52 - Remove test-only exports in app and camtools** (Effort S)
- Ledger: APP-5, P2FW-8b, P2FW-10b, CAMVIS-7b, P2CAM-8b; `QuitSource::Os` and `last_dialog_answer` from UISHELL-5 if `rg -n 'QuitSource::Os|last_dialog_answer' crates` shows only tests.
- Sanctioned test edits:
  - `app/tests/test_grbl_stream.rs:5-8` imports and the cases for the 6 removed fns;
  - `app/tests/test_command_history.rs` cases for `load`/`save`/`get_config_path`;
  - `app/tests/test_{tools,materials}_manager_backend.rs` cases for the 5 test-only fns;
  - `app/tests/test_work_zero.rs:123-138` re-pointed at `work_zero_support`, with a non-vacuity proof;
  - `cam/tests/audit_regressions.rs:461`, `generator_fixes.rs:6,341`;
  - delete `cam/tests/test_speeds_feeds_service.rs`;
  - `ui/tests/test_commands.rs:645` if `last_dialog_answer` goes.
- Commands: G + `cargo nextest run -p gcodekit5-app`; `cargo nextest run -p gcodekit5-camtools`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_commands::)'`.

**T53 - Shrink the protocol registry** (Effort S-M)
- Ledger: P2FW-4.
- Preserves: `detect_protocol` order and `announces_identity`.
- Sanctioned test edits: rewrite `crates/gcodekit5-communication/tests/test_protocol_registry.rs` to cover the kept fields only, with a non-vacuity proof (break the detection order locally and show the failure); `grbl_protocol_tests.rs:56-70` unchanged.
- Commands: G + `cargo nextest run -p gcodekit5-communication -E 'test(test_protocol_registry::) + test(grbl_protocol_tests::)'`.

**T54 - Delete the GTK settings-dialog stack; move i18n to app** (Effort M)
- Ledger: COREBUILD-1 (a, b, c, e), COREBUILD-4b.
- Preserves: egui Preferences; `SettingsManager` API; `migrate.rs`; `Config` serde shape.
- Sanctioned test edits:
  - in `crates/gcodekit5-settings/tests/{config_regression_tests,test_settings_dialog_pages,test_settings_keyboard_shortcuts,test_settings_manager_store}.rs`, delete the `test_gtk_*` and dialog-type cases;
  - rewrite `SettingsPersistence::load_from_dirs` uses (`test_settings_manager_store.rs:58`, `test_settings_keyboard_shortcuts.rs:86`) against `SettingsManager`, with non-vacuity proofs;
  - replace `config_regression_tests.rs:135` with a test of the live craft-settings quarantine path;
  - delete the matching cases in `crates/gcodekit5-core/tests/test_type_aliases.rs`.
- Commands: G + `cargo nextest run -p gcodekit5-settings`; `cargo nextest run -p gcodekit5-core -E 'test(test_type_aliases::)'`; `cargo nextest run -p gcodekit5-app -E 'test(test_i18n::) + test(test_translation_catalogs::)'`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_settings_window::) + test(test_translations::)'`.

**T55 - Drop unread `Config` fields** (Effort S; Blocked - owner decision D-10; after T54)
- Ledger: COREBUILD-1d. Preserves: old `config.toml` and imported `config.json` files still load. Sanctioned test edit: `set/tests/test_settings_migrate.rs:68` if it reads `recent_files`. Commands: G + `cargo nextest run -p gcodekit5-settings`.

**T56 - Delete uncalled designer `pub` items; drop `ToolpathToGcode` `_units`** (Effort M)
- Ledger: DESIGNER-7, DESIGNER-8b.
- Before deleting each item: `rg -w <name> crates src` shows the definition only, and trait-impl/serde reach is excluded. `font_manager::get_font` stays (pinned).
- Sanctioned test edits: `crates/gcodekit5-designer/tests/test_cam_depth_and_gcode.rs:142,158,165,208,255` (drop the units argument) and every other `ToolpathToGcode::(new|with_line_numbers)` call (`rg -n 'ToolpathToGcode::(new|with_line_numbers)\(' crates` at the base commit).
- Commands: G + `cargo nextest run -p gcodekit5-designer`; `cargo build --workspace --all-targets`.

**T57 - Split visualizer `parse_gcode` and `stock_removal_3d.rs`** (Effort M, Proposed; after T50, T51)
- Ledger: CAMVIS-8. Behaviour-preserving split, placed last because it depends on T50 and T51. No test edits; the module path `visualizer::stock_removal_3d` is kept via `pub use`. Commands: G + `cargo nextest run -p gcodekit5-visualizer`; `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_visualizer_stock::) + test(test_visualizer)'`. STOP if: any test import changes. Prevention: No guard justified.

**T58 - Repetier comment stripping via core** (Effort S, Blocked - characterization required)
- Ledger: CAMVIS-10. First add characterization tests in `com/tests/test_repetier.rs` for `G1(c)X10` and `(a;b) G1` on the current behaviour, then decide whether the core semantics are acceptable on Repetier/Marlin firmware. Commands: G + `cargo nextest run -p gcodekit5-communication -E 'test(test_repetier::)'`. Prevention: Guard added (characterization tests).

Tickets: 58 (T01-T58; T42 and T43 are separate PRs). Proposed: 37. Blocked - owner decision: 19. Blocked - characterization required: 2.

## 7. Deliberate - do not fix

| Item | Reason |
|---|---|
| UISHELL-6: `custom_tools.json`, `devices.json` names and locations; `OpenFirmwareSettings`/`OpenDeviceInfo` both open `Page::DeviceConfig` | Existing users' libraries load unchanged; harmless menu alias |
| UISHELL-3/4: exhaustive `match cmd` in `dispatch`; the 38-arm `run` list | AGENTS core rule: no wildcard fallbacks |
| UIPANES-7: UI rules stricter than the domain (surfacing height > 0, jigsaw cap 2000 mm, `gated(strict, ..)`); GTK-format preset JSON | Documented choices; presets byte-compared in `test_cam_presets.rs` |
| APP-10: `GRBL_RX_BUFFER_BYTES = 127`; `Ok(0)` for dropped Repetier `!`/`~`; `on_ack` ignoring an `ok` on an empty window; `drop_line` sequence hand-off; `halt` completing only on `Hold:0`/`Alarm` | GRBL buffer semantics and streaming invariants |
| DESIGNER-13b: GTK parity values in comments (e.g. `designer_state/shapes.rs:212` Ctrl+D 10 mm) | They document deliberate values |
| DESIGNER-14: DXF `BAD_NUMBERS` thread-local | Single-threaded parse; it keeps `parse_or` simple |
| COREBUILD-10: `parse_length -> Err(String)`; DRO precision 2 dp / 3 dp; `SettingsManager::save` re-opening the store; i18n `CURRENT` atomic; partial-file tolerance | Stable API; matches GTK output; preserves unknown keys and comments |
| COREBUILD-7: `check-no-gtk` CI step, nextest `gpu` group, release dry-run gating | Intended guards |
| COREBUILD-8c: `EEPROM.DAT`, `scripts/EEPROM.DAT` | Owner instruction: never touched by any ticket |
| CAMVIS-5 note / P2CAM-9: gerber and probe zero the work offset (`G10 L20`) and do not use `job_preamble` | Their workflow sets work zero from the current position |
| P2CAM-9: `G53 G0 Z` drawn at `max(current_z, max_z_seen)`; arcs with no axis word skipped; R-form and G18/G19 axis order; G38 drawn as untinted rapids | Correct per GRBL, or documented unknowns |
| P2FW-11: `parse_axis_values` returns `None` on a bad token; `extract_field` skips the state field; A/B/C unscaled; spindle parsed as `u32`; `check_value` rejects exponents and negatives; `settings_blocked` lets an unknown state through; Repetier idle `wait` ack and `acks_due + 1` after a resend; `ShankChoice::Derived` truncation | Fail-safe parsing and firmware semantics |
| P2DES-13: header comments out feed/spindle; rect/ellipse/text reflection maths; `mirror_arc_angles` and extrusion handling; `bspline_point` at `t_max`; gear root = base circle for >= 42 teeth; rotated-rect `resize` simplification | Verified correct, or a known simplification |
| P2DES-10 part: unitless SVG = mm | Historical behaviour, documented at `import.rs:209` |
| Mixed f32/f64 in drill/surfacing/grid/probe params | Changing them alters preset JSON and fixtures |

## 8. Lessons for the next audit

1. **`rg -w` "dead" claims miss reach.** Trait-impl methods, serde-reached fields, `lib.rs` re-exports and macro call sites all look unreferenced. DESIGNER-7's ~90 items were not filtered for this, and pass 2 found that DESIGNER-4's "never set" missed a call site (`designer_state/gcode.rs:214`). Require the reach check in the brief.
2. **The `device_status` pattern.** A process-global store whose writers were deleted in a migration is invisible to the compiler and to tests that poke the global themselves. Grep for writers, not readers. The test forced into a child process (`test_cam_drill_press::laser_mode_child`) was the clue.
3. **Tests that pin dead code look like coverage.** Example: `config_regression_tests.rs:135` proves a backup on a path production never calls. Coverage numbers (78.43%) include these lines. Ask "which production caller does this test protect?"
4. **Fixtures encode bugs as well as parity.** Example: `gerber_outline.gcode` (P2CAM-3). Byte-equality proves stability, not correctness. Review the geometry of every fixture once.
5. **Pass 2 found more than pass 1 in the generator and import code.** These are the areas that pass 1 skimmed or that no consumer test exercises: `svg_export`, text G-code, Gerber regions. Seed the next brief with "areas with no test importing them".
6. **Severity needs a second reader.** Pass 2 reversed DESIGNER-3's direction ("steeper" was wrong), lowered APP-7's reachability, refuted part of COREBUILD-9 and downgraded CAMVIS-4. Keep a re-verification step for every H/M defect.
7. **Docs claim guards that do not exist.** AGENTS.md cites `scripts/check-test-env-isolation.sh` and a `tool-rename-guard` group, and neither exists. Auditors should `ls` before trusting a doc-described guard.
