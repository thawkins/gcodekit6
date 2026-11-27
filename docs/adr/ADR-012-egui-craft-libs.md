# ADR-012: egui and craft-libs Front End

## Status
Accepted

Amended 2026-11-28: the translation build script is `crates/gcodekit6-app/build.rs`.

## Date
2026-11-27

## Context
GCodeKit6 had a GTK4 front end (gtk-rs, libadwaita, GtkSourceView) chosen in [ADR-001](ADR-001-gtk4-ui-framework.md). It made packaging hard: every platform needed the GTK runtime, either installed or bundled (MSYS2 DLLs on Windows, Homebrew frameworks on macOS, a GTK plugin for the AppImage), and the front end's GObject, `Rc<RefCell<T>>` and `glib::idle_add` patterns were hard to test without a display.

## Decision
Replace the GTK4 front end with an egui front end built on the `storytold/craft-libs` crates. [ADR-011](ADR-011-egui-craft-libs-pin.md) records the pinned craft-libs revision.

- **Crates**: `gcodekit6-ui-egui` is the front end (eframe on wgpu, `craft-ui`, `craft-window`, `craft-menu`, `craft-keymap`, `craft-dialogs`, `craft-platform`). `gcodekit6-app` holds the toolkit-free application state (machine controller, console model, device profile forms), so it can be tested without a window. The GTK UI crate and the G-code editor crate are removed.
- **Translations without gettext**: `po/*.po` stays the source. `crates/gcodekit6-app/build.rs` parses the catalogs with `polib` and embeds them in the binary, so there is no `msgfmt` step, no `.mo` files and no `gettext-sys`. `scripts/update-po.sh` still extracts strings with `xgettext` (`--keyword=gettext --keyword=t! --keyword=t --keyword=n_! --keyword=tr! --keyword=trn!:1,2 --keyword=_`).
- **Settings on craft-settings**: settings live in `config.toml`, written atomically with a `.bak` backup. The first run imports the GTK build's `config.json` and data files without changing them.
- **Packaging**: Linux builds need only `libudev-dev pkg-config` and the Wayland and X11 headers, plus a Vulkan or OpenGL driver at run time. macOS and Windows need nothing extra. Windows builds `x86_64-pc-windows-msvc` with a static C runtime.
- **No GTK in the tree**: the `check-no-gtk` step in Code Quality fails if `gtk4`, `libadwaita`, `sourceview5` or `glib-sys` returns.

## Consequences
- Release artefacts are one executable on Windows, a plain app bundle on macOS, and no GTK runtime in the Flatpak, AppImage, `.deb` or `.rpm`.
- UI pages are tested headlessly with `egui_kittest` snapshots.
- The app no longer follows the GNOME theme; it uses its own theme and the desktop's light or dark preference.
- craft-libs is a git-only dependency. Bumping it means updating the pinned revision and the egui and winit patch revisions together (ADR-011).

## Alternatives Considered
| Option | Why not |
|--------|---------|
| Keep GTK4 | Runtime bundling on Windows and macOS, no headless UI tests |
| Iced, Slint | No craft-libs widgets, menus and key map to reuse |
| Tauri | Web runtime and a second language for the UI |

## References
- [ADR-001: GTK4 UI Framework](ADR-001-gtk4-ui-framework.md) (superseded)
- [ADR-011: egui and craft-libs Pin](ADR-011-egui-craft-libs-pin.md)
- https://github.com/storytold/craft-libs
