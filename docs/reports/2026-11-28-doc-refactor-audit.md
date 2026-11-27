# Documentation refactor audit (2026-11-28)

Scope: every tracked markdown file in the workspace (plus three non-markdown doc files named in
D13/D14). Source code is frozen for the whole train: no `.rs`, `.toml` or `.yml` file changes.
Commit audited: `b0bac4c1` (`git rev-parse --short HEAD`), version `2.0.0-alpha.4`.
Method: four read-only slice audits (root, user, design, misc), then a second pass that re-checked
every "skimmed" item and re-ran the evidence of the ten highest-impact FALSE findings (section 2.1).

Owner decisions taken for this audit (answered by the owner, 2026-11-28):

- **OD-1** canonical repository URL is `https://github.com/thawkins/gcodekit6`. Docs that link
  `github.com/thawkins/gcodekit5` are FALSE.
- **OD-2** dimensions are documented as `f64` mm to match the code. The `f32` rule is dropped from
  `AGENTS.md`, and ADR-008 is updated to match.

Tickets live outside the repo (`target/temp/docaudit/tickets/DNN-*.md`), one PR per ticket. The
ledger is in section 10.

## 1. Baselines

| Measure | Value | Command |
|---|---|---|
| Tracked markdown bytes | 1,461,491 | `git ls-files '*.md' \| xargs wc -c \| tail -1` |
| Tracked markdown files | 154 | `git ls-files '*.md' \| wc -l` |
| `docs/**` markdown bytes | 567,590 | `git ls-files 'docs/*.md' 'docs/**/*.md' \| xargs wc -c \| tail -1` |
| `CHANGELOG.md` | 252,031 B | `wc -c CHANGELOG.md` |

Files agents load automatically (Claude Code reads `AGENTS.md`, and `AGENTS-LANGUAGE.md` through the
`@AGENTS-LANGUAGE.md` include). There is no `CLAUDE.md` (`find . -name CLAUDE.md -not -path './target/*'`
prints nothing).

| File | Bytes | ~Tokens (bytes/4) | Target after D17 | Command |
|---|---:|---:|---:|---|
| AGENTS.md | 27,846 | 6,962 | <= 8,000 B (~2,000 tok) | `wc -c AGENTS.md` |
| AGENTS-LANGUAGE.md | 26,433 | 6,608 | <= 8,000 B (~2,000 tok) | `wc -c AGENTS-LANGUAGE.md` |
| **Per-session total** | 54,279 | **13,570** | <= 16,000 B (~4,000 tok) | |

Relative link and anchor check. The script is in appendix A (GitHub slug rules; `help:` links are
covered by `crates/gcodekit5-ui-egui/tests/test_help.rs` instead). External URLs are listed by the
slice reports and were not fetched.

| Slice | Files | Relative links checked | Broken |
|---|---|---:|---:|
| root | `*.md` at repo root | 77 | 1 (`SPEC.md -> docs/FIRMWARE_CAPABILITIES_DATABASE.md`) |
| user | `docs/user/**`, in-app help `resources/markdown/{en,es}`, `assets/markdown` | 109 | 1 (`docs/user/6AXIS.md -> ../6AXISPLAN.md`) |
| design | `docs/{adr,implementation,analysis,reports,superpowers,bugfixs,howto,archive,audits}` | 35 | 3 (`docs/implementation/designer_features.md` -> `designer_svg_import.md` x2, `SVG_IMPORT_IMPLEMENTATION.md`) |
| misc | other `docs/*.md`, `.github/**/*.md`, `scripts/README.md` | 14 | 3 (`docs/MACOS_CHECKLIST.md` -> `docs/docs/MACOS_*.md`) |

Command: `python3 -I target/temp/docaudit/linkcheck.py` (appendix A).

No tool, CI job or hook lints or link-checks markdown. The only markdown a tool reads is
`CHANGELOG.md`, read by `scripts/release_notes.py` from `.github/workflows/release.yml`. The
in-app help files are compiled in (section 5), and `Cargo.toml:64` sets `readme = "README.md"`.

## 2. Falsehoods (highest impact first)

Each line gives the claim, what is true, and the check that caught it. `SRC=crates/gcodekit5-ui-egui/src`.

1. **Wrong safety keys in the user manual.** `docs/user/90-shortcuts.md`, `03-quick-start.md:48`,
   `10-machine-control.md:88`, `80-streaming.md:17-19,36` and `85-safety-diagnostics.md:18,42` say
   Escape is E-Stop or stops the stream, Space pauses, Ctrl+H homes, Ctrl+R is soft reset, and WASD/Q/Z jog.
   In the code, E-STOP (soft reset) is `Cmd+Shift+K` and is "Never a bare Escape". Feed hold is `!` or `Cmd+Shift+H`, cycle start is `Shift+\``,
   Home is `Cmd+Home`, Unlock is `Cmd+U`, and `Cmd+R`/`F5` is File > Run. Jog uses `6 4 8 2 9 3` (numeric keypad,
   Machine Control only). Stream Pause and Stop have no key. Check: `sed -n 247,375p $SRC/commands.rs`.
   `README.md:404-413` (F11, arrow-key jog) and `SPEC.md` 5.4 (W/A/S/D, Space, Ctrl+L) are FALSE for the same reason.
2. **A whole safety panel that does not exist.** `docs/user/85-safety-diagnostics.md` (interlocks,
   diagnostics panels, "Advanced Features" soft-limit settings) is linked from `docs/user/README.md:44`,
   `60-tool-library.md:63` and `80-streaming.md:38`. Check: `rg -il 'Safety & Diagnostics' $SRC` returns nothing;
   `docs/reports/egui-parity.md:287-288` marks both "Not applicable".
3. **Agent files steer agents to tools, scripts and dependencies that do not exist** (AGENTS.md,
   AGENTS-LANGUAGE.md):
   - the ragent tool list and "MUST use codeindex"; Claude Code has no such tools;
   - `scripts/check-test-env-isolation.sh` and `docs/reports/testconsolidation-review.md`;
     `test -e` fails for both;
   - nextest group `tool-rename-guard`; `.config/nextest.toml` has only group `gpu`;
   - `serial_test`; `rg -n serial_test Cargo.toml crates/*/Cargo.toml` returns nothing;
   - `team_create`/`team_wait`;
   - "clippy config sets `unwrap_used = "allow"`". There is no such setting
     (`rg -n unwrap_used -g '*.toml' .` returns nothing). CI denies `.unwrap()` in all targets:
     `.github/workflows/code-quality.yml:48` `cargo clippy --workspace --all-targets -- -D warnings -W clippy::unwrap_used`;
   - "tokio is the workspace runtime" and "use rayon". `rg -n 'tokio|rayon' Cargo.toml crates/*/Cargo.toml`
     returns nothing;
   - MSRV 1.85. `Cargo.toml:18` says `rust-version = "1.95"`;
   - the logging exception lists ragent files (`src/cli.rs`, `src/acp_server.rs`, ...), all missing;
   - the push ritual and doc-location rule name `QUICKSTART.md`, `TUI-QUICKSTART.md` and `PLAN.md`, all missing;
   - the `f32` mm rule (OD-2: code uses `f64`, `crates/gcodekit5-core/src/data/mod.rs:96-100`).
4. **Supported-controller claims.** `README.md:12,319-328`, `SPEC.md` sec. 1/2.2/4.5/8,
   `docs/user/05-supported-controllers.md` ("six firmware types"), `01-introduction.md`, `92-faq.md` and
   `6AXIS.md` claim GRBL, grblHAL, TinyG, g2core, Smoothieware and FluidNC are supported. In
   `crates/gcodekit5-communication/src/firmware/protocol_registry.rs`, `is_supported: true` is set only for
   Repetier (:85) and GRBL (:112). The other profiles are detected but not supported, and Repetier is missing
   from the README. The `ControllerType` enum has 8 values (`crates/gcodekit5-devicedb/src/model.rs:33-43`).
5. **FluidNC settings described as read-only.** `docs/GRBLHAL_FLUIDNC_SUPPORT.md` says "Save
   disabled, tooltip 'Modify config.yaml'". The code treats FluidNC settings as writable with wider ranges
   (`crates/gcodekit5-app/src/firmware_settings/mod.rs:326-372`; `rg -n 'Modify config.yaml' crates` returns nothing).
   The capability API it documents (`FirmwareDetector`, `Capability::`, `gcodekit5_ui::...`) does not exist.
6. **GTK4/libadwaita/OpenGL install and runtime instructions.** `docs/user/01,02,92`,
   `docs/MACOS_QUICK_START.md` (Homebrew GTK, missing `scripts/macos-bundle-frameworks.sh`),
   `docs/bugfixs/WINDOWS_MISSING_DLLS_FIX.md`, and `docs/analysis/UnitConv.md` (Slint). Check:
   `rg -n 'gtk|adwaita' Cargo.toml crates/*/Cargo.toml` returns nothing; `code-quality.yml:43-44` `check-no-gtk`.
7. **Wrong connection, settings and reconnect behaviour.** `docs/user/04-device-setup.md`,
   `70-settings.md`: the connection type is a device-profile field (Device Manager > Connection). There is no
   `ws://` entry. Reconnect is 10 attempts at 1/2/5 s delays (`crates/gcodekit5-app/src/machine/mod.rs:9-16`),
   not a 10 s timeout. The settings tables name rows that do not exist. "Laser device type
   automatically configures `$32=1`" has the direction reversed: the app reads `$32` to pick the device
   type (`crates/gcodekit5-app/src/device_identity.rs:148-149`) and never writes it.
8. **Design file format.** `docs/file_format.md` says the extension is `.gck4`, the version is 1.0, and lists
   ZigZag/Spiral strategies. The app saves `.gckd` (`$SRC/app/designer_files.rs:224`) and opens
   `["gckd","gck5"]` (`$SRC/dialogs.rs:187`). `FILE_FORMAT_VERSION = "1.1"` (`crates/gcodekit5-designer/src/serialization.rs:22`).
   The strategies are Raster/ContourParallel/Adaptive (`pocket_operations.rs:219-227`). Unknown shapes are
   skipped, not rejected. `docs/user/40-designer.md:138` also says `.gck4`.
9. **Architecture docs describe another program.** `ARCHITECTURE.md` types and patterns (`MachineState`,
   `GrblController`, `FirmwareProtocol`, `AppSettings`, ...) return 0 hits for
   `rg -c "(struct|enum|trait|type) <T>\b" crates src`. Its dependency graph says designer depends on visualizer;
   designer depends only on camtools and core (`crates/gcodekit5-designer/Cargo.toml`).
   ADR-009 describes a Tokio design with types that do not exist (`TcpCommunicator`, `BufferedCommunicatorWrapper`, ...).
   `docs/COORDINATE_SYSTEM.md` describes `ViewportTransform` and Cairo; `rg -n 'ViewportTransform|cairo' crates src --type rust`
   returns nothing. `docs/implementation/BUFFERED_COMMUNICATION.md` describes a deleted module.
10. **Finished agent plans that read as executable.** `docs/superpowers/plans/*.md` have 78/10/8
    unchecked boxes and 0 checked (`grep -c -- '- \[ \]'`), and each opens with "REQUIRED SUB-SKILL ...
    implement this plan task-by-task". `docs/reports/2026-11-27-deep-refactor-audit.md` ticket headers still
    say Proposed or Blocked, but 58 ticket branches are merged
    (`git log --oneline | grep -ciE 'claude/t[0-9]{2}'`).
11. **Feature chapters and in-app help with labels not in the UI.** `docs/user/10,15,20,30,40,50,60,6AXIS,PROBING,87`
    and in-app topics `machine_control`, `device_console`, `settings`, `designer`, `device_config`,
    `device_manager`, `jigsaw_puzzle`, `tabbed_box_maker`, `gerber`, `visualizer`, `tools_manager`
    (en and es). Each label was checked against `n_!("...")` msgids in `$SRC`. Details are in tickets D03 to D05.
12. **Developer commands that do not run as written.** `DEVELOPMENT.md` `cargo run --release -- --version`: no flag
    exists; every argument is opened as a file (`src/main.rs:16-18`). The VS Code launch config uses
    `--package=gcodekit5`; the package is `gcodekit5-design`. The filter `test(test_grbl_status_parser)` matches nothing.
    The pre-commit description is wrong (`.githooks/pre-commit:14,23,32`). `README.md:531` uses the tracing target
    `gcodekit5::communication`; the target is `gcodekit5_communication`. `CONTRIBUTING.md` says `git rebase origin/main`;
    the default branch is `master`. It also prescribes inline `#[cfg(test)]` tests. `TOOLCHAIN.md` credits a Wild/LLD linker,
    but `rg -n 'wild|lld|linker' .cargo Cargo.toml .github/workflows` returns nothing.
13. **Repository URL (OD-1).** 13 markdown files link `github.com/thawkins/gcodekit5`
    (`rg -l 'github.com/thawkins/gcodekit5([^-]|$)' --glob '*.md' . | grep -v CHANGELOG`). The canonical URL is
    `thawkins/gcodekit6` (`git remote -v`).
14. **Smaller mismatches.**
    - ADR-012 cites `crates/gcodekit5-settings/build.rs`; the build script is `crates/gcodekit5-app/build.rs`.
    - ADR-008 gives `parse_length -> Option<f32>`; it is `Result<f32, String>` (`core/src/units.rs:170`).
    - `docs/MACOS_BUILD.md` names runner `macos-14`; `release.yml:32` uses `macos-latest`. It also invents DMG icon layout.
    - `scripts/README.md` gives the simulator tty as `target/temp/ttyGRBL`; it is `/dev/ttyGRBL`
      (`scripts/start-grblhal-sim.sh:9`).
    - `docs/reports/egui-parity.md` cites 3 tests that no longer exist.
    - `docs/audits/unwrap_audit.csv` is header-only (51 B) but is described as 585 rows.
    - `.github/ISSUE_TEMPLATE/task.md` refers to `PLAN.md`, which does not exist.

### 2.1 Second pass ("loop until dry")

Ten highest-impact FALSE findings: evidence re-run at `b0bac4c1`, and **all reproduced**:

- USER-3 keys (`commands.rs:247-375`);
- USER-4 panel absent;
- ROOT-24 `is_supported` (Repetier :85, GRBL :112 only);
- ROOT-13 `unwrap_used` (no toml setting; `code-quality.yml:48`);
- ROOT-4 (script missing; nextest group `gpu` only; no `serial_test`);
- ROOT-15 (no tokio or rayon);
- MISC-5 FluidNC (`firmware_settings/mod.rs:326-372`);
- MISC-6 `.gckd` (`designer_files.rs:224`, `dialogs.rs:187`);
- DESIGN-23 (78/10/8 unchecked, 0 checked);
- ROOT-32 (`src/main.rs:16-18` has no flag parsing);
- also USER-5 reconnect (`machine/mod.rs:9-16`) and MISC-2 runner (`release.yml:32`).

Skimmed items re-checked:

| Item (slice) | Result | Change |
|---|---|---|
| designer.md "Laser Settings (Object)" (user) | msgid is `Laser Config (Object)` (`$SRC/panes/designer/properties/handlers/laser_override.rs:28`) | New FALSE, added to D03 |
| visualizer.md "Snap to Drawing", "Scroll Bar Display" (user) | Not msgids. Toolbar: Fit to Content, Reset View, Fit to Device Working Area, Top-down 2D view, Rotatable 3D view (`panes/visualizer/sidebar.rs:41-55`) | New FALSE, added to D03 |
| 12-overrides "Range 0% to 200%" (user) | Feed 10-200 %, spindle 50-200 % (`panes/machine_control/overrides.rs:217-219`) | New FALSE, added to D01 |
| 12-overrides "reset to 100% on disconnect" (user) | Not found in `gcodekit5-app/src/machine` | Unverified. D01 STOP: confirm or drop |
| 70-settings "`$32=1` set automatically" (user) | Reversed: app reads `$32` (`device_identity.rs:148-149`); no write in `crates` | New FALSE, added to D05 |
| 10-machine-control step presets (user) | TRUE: metric 0.001-100 mm, imperial 0.0001-1 in (`crates/gcodekit5-app/src/machine/grbl_stream.rs:256-262`, used via `jog_step_choices` at `$SRC/panes/machine_control/jog.rs:639`); the slice's "only default 1.0 mm" was wrong | Keep the table, add the inch row (D01) |
| Editor `Ctrl+F` find (user, 20/90) | No key binding found (`rg -n 'Cmd\+F\|Key::F' $SRC/panes/editor` returns nothing) | D01/D05: verify in app; drop if absent |
| In-app help pins (user) | `tests/test_help.rs` asserts on designer.md heading "Main Actions in 2D Laser Mode", the "Draw primitives (...)" line, image alts "Designer"/"Global Properties", index "GCodeKit5 Help", link "Visualizer", "## Topics"/"## Temas", and the URL `https://github.com/feveal/gcodekit5-design` (:236) | USER-16 heading rename and USER-24 URL change are now **Deliberate - do not fix** (frozen test) |
| GTK4.md "Cavalier Contours" sections (design) | Still true and toolkit-independent: `clean_polyline` (`designer/src/ops.rs:20`, `camtools/src/gerber.rs:71`), `catch_unwind` in 4 files | D08 salvages them before D14 deletes the file |
| Design C-3 "no guard for zero unwrap" (design) | Wrong: CI clippy `-W clippy::unwrap_used -D warnings` is the guard | Dropped from the defects list |
| `.github` PR templates (misc) | Both `.github/PULL_REQUEST_TEMPLATE.md` and `.github/pull_request_template.md` are tracked (`git ls-files .github`) | Confirmed. D07 merges them |
| Rotation sign (misc), es prose, glossary, deep-audit ledger rows, superpowers plan steps, external URLs | Not machine-checkable here, or not needed for the action | No change. Tickets carry STOP conditions |

## 3. Stale sections and files to delete

| File(s) | Bytes | Still covered by | Ticket |
|---|---:|---|---|
| AGENTS-OLD.md | 13,709 | AGENTS.md | D13 |
| 6AXISPLAN, PROBEPLAN, FEEDSSPEEDS (shipped in 0.55) | 65,992 | docs/user/6AXIS.md, PROBING.md, in-app `speeds_feeds_calculator` | D13 |
| UIFIXPLAN, UIFIXSUM, LEGACY_CODE, CODEBASE_IMPROVEMENTS, REMEDIATION_PLAN, REMEDIATION_QUICK_START, EXECUTION_ROADMAP, ANALYSIS_INDEX, IMPROVEMENTS_SUMMARY.txt | ~190,000 | docs/reports/2026-11-27-deep-refactor-audit.md | D13 |
| TESTPLAN.md, TEST_SUITE_SUMMARY.md | 59,903 | docs/reports/egui-parity.md "Manual checks", docs/howto/HOWTO.UAT.md | D13 |
| RELEASE.md | 1,618 | CHANGELOG.md | D13 |
| DEVHOW.md, TOOLCHAIN.md (folded first) | 7,367 | CONTRIBUTING.md, DEVELOPMENT.md | D07 |
| docs/implementation/{3D_INTEGRATION_COMPLETE,3D_TESTING_PLAN,TASK5_*,TASK6_*,BUFFERED_COMMUNICATION,USER,aspect-ratio-lock,designer_features} | 46,628 | docs/user/40-designer.md, ADR-009 (amended), `reply_ledger.rs` module docs | D14 |
| docs/analysis/* (3), docs/howto/DrillPressCAMToolDesign.md | 15,221 | ADR-008, docs/user/50-camtools.md | D14 |
| docs/superpowers/plans/* (3) | 59,264 | ADR-012, docs/reports/egui-parity.md, CHANGELOG | D14 |
| docs/bugfixs/* (4; 2axisdevices user part moved first) | 23,445 | docs/user/04-device-setup.md, git history | D04 then D14 |
| docs/archive/GTK4.md (Cavalier section salvaged first) | 43,446 | ARCHITECTURE.md geometry note | D08 then D14 |
| docs/audits/* (report + header-only csv) | 3,521 | CI clippy `unwrap_used` gate | D14 |
| docs/MACOS_{QUICK_START,CHANGE_SUMMARY,CHECKLIST,IMPLEMENTATION_SUMMARY}, WINDOWS_QUICK_START | 21,963 | docs/MACOS_BUILD.md, docs/WINDOWS_BUILD.md | D15 |
| docs/PR_ARCHIVE_* (2), REVIEW.CAMTOOLS.md, dependency_audit.md, assets/markdown/joglayout.md | 17,009 | none needed (Slint/GTK era), docs/dependency_management.md | D15 |

Sections to delete inside surviving files:

- `README.md:630-905` "Recent Updates" (a CHANGELOG copy);
- `SPEC.md:7-575` and `:1995-2124` (release blocks copied from CHANGELOG);
- `SPEC.md` sections 1-11 (a 2024 UGS-port wish list), cut to the as-built parts;
- AGENTS.md "Available Tools", "File Reading Best Practices" and "Team Workflow";
- ADR-006 body (cut to a stub with status `Rejected`).

## 4. Duplication map

| Fact | Copies today | Canonical home | Others become |
|---|---|---|---|
| Keyboard shortcuts | README, SPEC 5.4, docs/user/90 + 03/10/80/85/92, 70-settings | `$SRC/commands.rs` `Command::spec` (rendered in Preferences > Shortcuts); doc mirror docs/user/90-shortcuts.md | One link to 90 |
| Release history | CHANGELOG, README "Recent Updates", SPEC release blocks, RELEASE.md | CHANGELOG.md | Deleted |
| Build/test/lint commands | README (twice), DEVELOPMENT, CONTRIBUTING, TOOLCHAIN, AGENTS-LANGUAGE | DEVELOPMENT.md, with the CI gates block copied from `code-quality.yml` | README keeps build+run; others link |
| Linux build packages | README:437-441, DEVELOPMENT, TOOLCHAIN, SPEC:25, docs/user/02 | `.github/actions/install-system-deps/action.yml:10` | DEVELOPMENT.md copy; others link |
| Test layout rules | AGENTS.md "Test Layout" + AGENTS-LANGUAGE "Test Organization" | AGENTS-LANGUAGE.md "Test Organization" | AGENTS.md links |
| `cargo fmt` rule | 5 places in AGENTS-LANGUAGE | one 3-line rule | deleted |
| unsafe/unwrap rules | AGENTS.md General Preferences, AGENTS-LANGUAGE Core Code Rules, ADR-005 | AGENTS-LANGUAGE Core Code Rules | deleted or linked |
| Windows build | WINDOWS_BUILD, WINDOWS_QUICK_START, scripts/README "Quick Reference" | docs/WINDOWS_BUILD.md | deleted or linked |
| Tools/materials fields | docs/user/60, in-app tools_manager/materials_manager | in-app help (en) | 60 summarises and links |
| PR template | two `.github` templates | `.github/pull_request_template.md` | merged |

Replicated invariants (deliberate, must stay identical) and their sync checks:

| Invariant | Copies | Sync check |
|---|---|---|
| craft-libs rev `d15bf1f...` | Cargo.toml (8x), ADR-011, dependabot.yml comment | `rg -o 'd15bf1f[0-9a-f]*' Cargo.toml docs/adr .github \| sort \| uniq -c` |
| xgettext keyword list | scripts/update-po.sh, docs/HOWTO.i18n.md | `diff <(grep -o -- '--keyword=[^ ]*' scripts/update-po.sh \| sort) <(grep -o -- '--keyword=[^ ]*' docs/HOWTO.i18n.md \| sort)` (empty) |
| en/es in-app help topic set and structure | `resources/markdown/en` vs `es` | `diff <(ls crates/gcodekit5-ui-egui/resources/markdown/en) <(ls crates/gcodekit5-ui-egui/resources/markdown/es)` (empty); per topic `diff <(grep -c '^#' en/X.md) <(grep -c '^#' es/X.md)` |
| Default shortcut table | `commands.rs` and docs/user/90-shortcuts.md | `rg -o '&\["[^]]*\]' crates/gcodekit5-ui-egui/src/commands.rs` reviewed against the 90 table (manual; no script, source frozen) |
| Units rule | AGENTS.md "Units" and ADR-008 (OD-2: f64) | `grep -n 'f32\|f64' AGENTS.md docs/adr/ADR-008-units-system.md` (both say f64 for dimensions) |

## 5. Generated files and the in-app help pin

- **Generated markdown: none.** No generator marker in any tracked `.md`
  (`rg -n -i 'generated by|do not edit' --glob '*.md' .` gives prose dates only). `CHANGELOG.md` is an
  *input* to a generator: `scripts/release_notes.py` parses `^## \[(?P<ver>[^\]]+)\]` and stops at the
  first `## [Unreleased]` below a version. The 9 `## [2.0.0-alpha.1]` headings and 51 dated
  `## [Unreleased]` headings are load-bearing and must not be restructured. Historical entries are history,
  not falsehoods.
- **In-app help is compiled in.** `crates/gcodekit5-ui-egui/src/resources.rs:25,35` `include_str!`s
  `resources/markdown/{en,es}/<topic>.md` for 20 topic ids. These files can be **edited but not deleted, renamed or
  added to** without a source change, which is out of scope. `test_help_embeds_every_resource_file` asserts that the
  `markdown/{en,es}` and `help_images` file sets equal the embedded sets. `tests/test_help.rs` also pins content (section
  2.1): the designer.md heading "Main Actions in 2D Laser Mode", the "Draw primitives (rectangles, circles,
  lines, ellipses, polylines, triangles, polygons, pinions, gears)" line, the image alts, "GCodeKit5 Help",
  "## Topics"/"## Temas", and the URL `https://github.com/feveal/gcodekit5-design` in index.md. Every
  in-app-help ticket runs `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_help::)'`.
- Other name pins: `README.md` (Cargo.toml:64 `readme`), `CONTRIBUTING.md`, `AGENTS.md`,
  `.github/pull_request_template.md`, `.github/ISSUE_TEMPLATE/*.md`, `docs/user/70-settings.md` (cited by
  `$SRC/windows/settings.rs:13`), `scripts/EEPROM.DAT` (simulator image; untouched).

## 6. Compression candidates

| File | Bytes now | Target | What to cut | Operative tokens that must survive |
|---|---:|---:|---|---|
| AGENTS.md | 27,846 (~6.96k tok) | <= 8,000 (~2.0k tok) | ADHD block 5.5 KB to the numbered rules (~1.8 KB); Test Layout (moved to AGENTS-LANGUAGE); Documentation Standards duplicate; General Preferences duplicates | "push to remote", "push release to remote", `Version: <version_number>`, "NEVER push without explicit instruction", "Do not tag releases", "DO NOT use git checkout to rewind files", `target/temp/`, no `/tmp`, CHANGELOG Keep a Changelog + semver, `docs/` location rule + root exceptions, "no pre-existing failure", `stop adhd mode`/`normal mode`, `@AGENTS-LANGUAGE.md`, the greeting line |
| AGENTS-LANGUAGE.md | 26,433 (~6.61k tok) | <= 8,000 (~2.0k tok) | `cargo fmt` stated 5x to 1x; "Idiomatic Rust Practices" (~10 KB generic) to <= 6 project lines; duplicate test text | `cargo fmt`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings -W clippy::unwrap_used`, `cargo nextest run -p <crate> -E 'test(test_foo::)'`, `cargo test --doc`, `timeout 1500`, 150 s nextest kill, `autotests = false`, `[[test]] name = "all"`, `#[path = "test_x.rs"] mod test_x;`, `crate::global_env_lock()` (never twice), `.config/nextest.toml` `[test-groups]` (`gpu`), `#![deny(unsafe_code)]`, `#![allow(unsafe_code)]` test exception, `.expect("why")`, `rust-version 1.95`, edition 2024, `std::sync::mpsc` worker thread, `tracing` |
| ADR-006 | 19,767 | ~2,000 | everything but Context, Decision summary, Status `Rejected` | ADR number, title |
| README.md | 49,016 | ~22,000 | Recent Updates, duplicate build blocks, dev status logs | badges, controller table (derived), build + run, links |
| SPEC.md | 85,128 | ~30,000 | release blocks, aspirational sections 1-11 | header version, as-built crate map, "Audit Follow-up (November 2026)" |

The D17 behavioural delta is stated in the ticket. Every rule that is true and enforced survives
(checked by an operative-token diff). Removed are only rules that are false, rules for a
different project, and repetitions.

## 7. Gaps (found by walking a journey)

- **Clone, build, first change.** No single place states the real CI gates. `code-quality.yml` runs:
  - `cargo fmt --all -- --check`;
  - clippy with `unwrap_used`;
  - nextest `--profile ci`;
  - `cargo test --workspace --doc`;
  - `cargo deny check`;
  - `cargo machete`;
  - `check-no-gtk`.

  Fix in D07 (DEVELOPMENT.md).
- **Using a CAM tool from the help button.**
  - The Drill Press help topic is 15/21 bytes (heading only, `wc -c`), but it is live (`$SRC/panes/cam_tools/tools.rs:60`).
  - The `cam_tools`/`index` topics do not link `help:gerber`.
  - Laser overscan (2.5 mm, `bitmap_engraving/mod.rs:42`) is undocumented.
  - es designer lacks "Notes about Corner and Chamfer".

  Fix in D18.
- **Finding the manual.** docs/user/README.md TOC omits 6AXIS.md and PROBING.md. Fix in D05.
- **Setting a 2-axis device.** Axis-count behaviour is only in `docs/bugfixs/2axisdevices.md`. It moves to
  04-device-setup in D04.
- **Lock aspect ratio and STL import** are not in docs/user/40-designer.md. Add one line each in D05.

## 8. Code and config defects found (reported, not fixed; source frozen)

| ID | Where | Defect | Evidence |
|---|---|---|---|
| C-1 | `.github/dependabot.yml:74-77` | `npm` ecosystem with no `package.json` (deleted b13c64cf); labels `rust`, `github-actions`, `javascript`, `dependencies` undefined | `ls package.json`; `gh label list` |
| C-2 | `crates/gcodekit5-communication/src/communication/mod.rs:185-189` | `Communicator` doc says implementations exist for Serial, TCP, WebSocket. Only `NoOpCommunicator`/`SerialCommunicator` implement it; TCP/WS are port backends | `rg -n 'impl Communicator for' crates` |
| C-3 | `crates/gcodekit5-ui-egui/src/panes/console.rs:3-27` | Module docs describe "the GTK front end", "GTK's command_history.json" | `rg -n -i gtk crates/gcodekit5-ui-egui/src/panes/console.rs` |
| C-4 | `crates/gcodekit5-designer/src/serialization.rs:3` | Module doc says `.gck4 (GCodeKit4)`; app writes `.gckd` | `rg -n gck4 crates --type rust` |
| C-5 | `Cargo.toml:21` | `repository = "https://github.com/thawkins/gcodekit5"`, but OD-1 says `gcodekit6` | `grep -n repository Cargo.toml` |
| C-6 | `crates/gcodekit5-designer/src/gcode_converter.rs:82`, `crates/gcodekit5-designer/src/model/mod.rs:668` | Inline `#[cfg(test)]` modules against the AGENTS-LANGUAGE test rule | `sed -n 82p ...` |
| C-7 | `.githooks/pre-commit:32` | Runs `cargo test --lib`; CI and docs use `cargo nextest` | `grep -n cargo .githooks/pre-commit` |
| C-8 | `src/main.rs:16-18` | No `--version`/`--help`: any argument is opened as a file (docs are fixed instead; adding a flag is the owner's call) | read |
| C-9 | `.github/ISSUE_TEMPLATE/{feature,task}.md` | Labels `feature`/`task` undefined, so GitHub drops them | `gh label list` |
| C-10 | `crates/gcodekit5-app/src/firmware_settings/mod.rs:326-372` | FluidNC `$` settings are offered as writable; if FluidNC rejects runtime writes this is a product bug (not verified on hardware) | read |
| C-11 | `crates/gcodekit5-ui-egui/tests/test_help.rs:236` | The test pins a third-party fork URL as the help "Project website", which blocks OD-1 alignment of in-app help | read |
| C-12 | `crates/gcodekit5-core/src/units.rs:107,170` | Under OD-2 (f64 dimensions) the formatting/parsing helpers still take and return `f32`; docs will state this split, the code is unchanged | read |

## 9. Owner decisions

| ID | Question | Decision | Effect |
|---|---|---|---|
| OD-1 | Canonical repo URL | **Decided: `github.com/thawkins/gcodekit6`** | D04, D06, D07, D10, D15 fixed the URLs (Done). C-5 and C-11 are code/config follow-ups |
| OD-2 | Dimension type | **Decided: document `f64`** (match code); drop the f32 rule from AGENTS.md; update ADR-008 | D02 and D09 applied it (Done) |
| OD-3 (new) | In-app "Project website" points at `feveal/gcodekit5-design` | Pinned by a frozen test (C-11): **Deliberate - do not fix** in this train | none |

## 10. Cleanup train ledger

Status vocabulary: Proposed / Blocked - owner decision OD-n / Deliberate - do not fix / Done (#PR). No row is Blocked.
Drain finished 2026-11-28: every ticket merged; the link check (`python3 -I target/temp/docaudit/linkcheck.py`, appendix A) reports 0 broken links in all four slices.
`V0` means the shared validation `git diff --stat origin/master -- . ':!*.md'` prints nothing.
`LC` means `python3 -I target/temp/docaudit/linkcheck.py` reports 0 broken for the slice.
`HT` means `cargo nextest run -p gcodekit5-ui-egui -E 'test(test_help::)'` passes.

| ID | Surviving claim / reader behaviour | Evidence | Canonical home | Pins | Frozen boundary | Validation | Dependency | Status |
|---|---|---|---|---|---|---|---|---|
| L01 | User finds the correct E-STOP, feed hold and jog keys | `commands.rs:247-375` | docs/user/90-shortcuts.md mirrors `commands.rs` | 70-settings links 90 | commands.rs | V0, LC, `rg -n -i 'escape\|wasd\|space' docs/user/{03,10,80,85,90}*.md` returns no key claims | - | Done (D01 #86) |
| L02 | Safety page lists only real controls (E-STOP, feed hold, jog clamp) | parity rows 96/101/286-288 | docs/user/85-safety-diagnostics.md (filename kept) | README:44, 60:63, 80:38 | - | V0, LC | - | Done (D01 #86) |
| L03 | Override ranges feed 10-200 %, spindle 50-200 % | `overrides.rs:217-219` | docs/user/12-overrides.md | - | - | V0 | - | Done (D01 #86) |
| L04 | Visualizer mouse/keys/toggles match UI | `commands.rs` VIS keys; sidebar msgids | docs/user/30-visualizer.md | - | - | V0 | - | Done (D01 #86) |
| L05 | Agents use only real tools, scripts and deps | ROOT-1..5,13,15,16,20 | AGENTS.md / AGENTS-LANGUAGE.md | auto-load names | code-quality.yml | V0, `rg -n 'codeindex\|check-test-env\|tool-rename-guard\|tokio\|rayon\|unwrap_used = \|1\.85\|QUICKSTART\|PLAN\.md\|team_create' AGENTS*.md` returns nothing | - | Done (D02 #84) |
| L06 | Dimensions are f64 mm | `core/src/data/mod.rs:96-100` | ADR-008 (AGENTS links) | - | units.rs | V0 | OD-2 decided | Done (D02 #84, D09 #96) |
| L07 | Greeting canary lines | AGENTS.md:175, AGENTS-LANGUAGE.md:3 | AGENTS*.md | session canary | - | - | - | Deliberate - do not fix (owner's canary) |
| L08 | In-app help labels match msgids (11 topics x en/es) | USER-12..22, 2.1 | resources/markdown | include_str, test_help pins | resources.rs, test_help.rs | V0, HT | - | Done (D03 #91) |
| L09 | designer.md heading "Main Actions in 2D Laser Mode" | test_help.rs | in-app designer.md | frozen test | test_help.rs | HT | - | Deliberate - do not fix |
| L10 | In-app "Project website" URL | test_help.rs:236 | in-app index.md | frozen test | test_help.rs | HT | OD-3 | Deliberate - do not fix |
| L11 | Install/connection chapters say egui/wgpu deps, gcodekit6 URL, MSRV 1.95, real controller support | USER-1,2,5,11; ROOT-24 | docs/user/{README,01,02,04,05,06,87,92} | README TOC | - | V0, LC, `rg -n -i 'gtk\|adwaita\|opengl\|gcodekit5\.git\|1\.88' docs/user` returns nothing | OD-1 decided | Done (D04 #87) |
| L12 | Feature chapters describe only implemented UI | USER-6..10, 2.1 | docs/user/{15,20,40,50,60,70,6AXIS,PROBING} | 70 cited by settings.rs:13 | - | V0, LC | - | Done (D05 #89) |
| L13 | README capability, keys, commands, URL true; no CHANGELOG copy | ROOT-24..28 | README.md | Cargo.toml `readme` | - | V0, LC | OD-1 decided | Done (D06 #85) |
| L14 | Contributor commands run as written; CI gates in one place | ROOT-32,33,35,44; MISC-8,10 | DEVELOPMENT.md, CONTRIBUTING.md | forge names | .githooks, workflows | V0, LC | OD-1 decided | Done (D07 #88) |
| L15 | Architecture types and graph come from the code | ROOT-29; MISC-6,7; GTK4.md salvage | ARCHITECTURE.md, COORDINATE_SYSTEM.md, file_format.md | README/DEVELOPMENT links | - | V0, LC | - | Done (D08 #93) |
| L16 | ADR statuses and named types match the code | DESIGN-1..10,12 | docs/adr | ADR filenames (Cargo.toml, deny.toml, dependabot cite ADR-011) | - | V0, LC | OD-2 decided | Done (D09 #96) |
| L17 | Ops docs (macOS, scripts, dependencies, grblHAL/FluidNC) true | MISC-2,4,5,11,13,16 | docs/MACOS_BUILD.md, scripts/README.md, docs/dependency_management.md, docs/GRBLHAL_FLUIDNC_SUPPORT.md | DEVELOPMENT links | scripts/*, dependabot.yml | V0, LC | OD-1 decided | Done (D10 #92) |
| L18 | Reports reflect HEAD (parity test refs, ticket statuses, UAT script) | DESIGN-21,22,28 | docs/reports, docs/howto/HOWTO.UAT.md | CHANGELOG:34 | - | V0, LC | - | Done (D11 #90) |
| L19 | SPEC is an as-built spec | ROOT-30,31 | SPEC.md | AGENTS push ritual | - | V0, LC | - | Done (D12 #94) |
| L20 | Stale root plans removed | ROOT-23,36-39 | git history | inbound links listed in D13 | - | V0 (except `IMPROVEMENTS_SUMMARY.txt`), LC | D05, D12 | Done (D13 #101) |
| L21 | Stale docs subdirs removed | DESIGN-13..20,23-27,29,30 | git history | inbound listed in D14 | - | V0 (except `docs/audits/unwrap_audit.csv`), LC | D04, D08 | Done (D14 #100) |
| L22 | Stale docs/ root files removed | MISC-1,3,4,12,14,15; USER-26 | MACOS_BUILD, WINDOWS_BUILD | inbound listed in D15 | - | V0, LC | D10 | Done (D15 #99) |
| L23 | Bare counts become commands; CHANGELOG gets a title | ROOT-41,43 | STATS.md, BUILDSTATS.md, CHANGELOG.md | release_notes.py | scripts/release_notes.py | V0, `python3 -I scripts/release_notes.py 2.0.0-alpha.4 --out target/temp/rn.md` exits 0, output identical before/after | - | Done (D16 #97) |
| L24 | CHANGELOG `## [x.y.z]` and dated `[Unreleased]` headings | release_notes.py | CHANGELOG.md | release.yml | - | `grep -c '^## \[' CHANGELOG.md` unchanged | - | Deliberate - do not fix |
| L25 | Agent files cost <= ~4k tokens with all true rules intact | 6 | AGENTS*.md | auto-load | - | V0, `wc -c`, operative-token diff | D02 | Done (D17 #98) |
| L26 | Drill Press help topic, Gerber links, overscan, es parity | USER-23,24,25,28 | resources/markdown | include_str, test_help | - | V0, HT | D03 | Done (D18 #102) |
| L27 | Copilot reads AGENTS.md pointer | MISC-9 | `.github/copilot-instructions.md` | GitHub path | - | V0 | - | Done (D19 #95) |
| L28 | Historical CHANGELOG entries (GTK, `.cargo` notes, `docs/bugfixes/` typo) | ROOT-42 | CHANGELOG.md | - | - | - | - | Deliberate - do not fix |

Prevention: no guard is added in this train, because CI/test files are frozen. Two follow-ups are recorded
for the owner: an offline relative-link check job (appendix A logic), and an en/es help-structure parity
test in `test_help.rs`.

## Appendix A: link checker (`target/temp/docaudit/linkcheck.py`)

```python
import re,subprocess,os,collections
files=subprocess.run(['git','ls-files','*.md'],capture_output=True,text=True).stdout.split()
def slug(h):
    h=h.strip().lower(); h=re.sub(r'[^\w\- ]','',h); return h.replace(' ','-')
def anchors(p):
    out=set(); cnt=collections.Counter()
    t=re.sub(r'```.*?```','',open(p,encoding='utf-8').read(),flags=re.S)
    for m in re.finditer(r'^#{1,6}\s+(.*?)\s*#*\s*$',t,re.M):
        s=slug(m.group(1)); n=cnt[s]; cnt[s]+=1; out.add(s if n==0 else f"{s}-{n}")
    return out
def slice_of(f):
    if '/' not in f: return 'root'
    if f.startswith(('docs/user/','crates/gcodekit5-ui-egui/resources/markdown','assets/')): return 'user'
    if re.match(r'docs/(adr|implementation|analysis|reports|superpowers|bugfixs|howto|archive|audits)/',f): return 'design'
    return 'misc'
chk=collections.Counter(); bad=collections.defaultdict(list)
for f in files:
    t=re.sub(r'```.*?```','',open(f,encoding='utf-8',errors='replace').read(),flags=re.S)
    for m in re.finditer(r'\]\(([^)\s]+)(?:\s+"[^"]*")?\)',t):
        u=m.group(1)
        if re.match(r'[a-z]+:',u): continue
        s=slice_of(f); chk[s]+=1
        path,_,anc=u.partition('#')
        tgt=os.path.normpath(os.path.join(os.path.dirname(f),path)) if path else f
        if not os.path.exists(tgt): bad[s].append(f"{f} -> {u}"); continue
        if anc and tgt.endswith('.md') and anc.lower() not in anchors(tgt): bad[s].append(f"{f} -> {u} (anchor)")
for s in ['root','user','design','misc']:
    print(s,chk[s],len(bad[s])); [print('   ',b) for b in bad[s]]
```
