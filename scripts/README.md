# Build Scripts

This directory contains platform-specific build and packaging scripts for GCodeKit5.

## Windows Scripts

### `build-windows-release.ps1`

Builds the Windows release: the MSI installer and the portable zip. No GTK
runtime or MSYS2 is required.

```powershell
.\scripts\build-windows-release.ps1
```

**What it does:**
1. Builds the release binary (`cargo build --release --target x86_64-pc-windows-msvc`)
2. Builds the MSI with `cargo wix --package gcodekit5-design --no-build`
3. Zips `gcodekit5.exe` and `License.rtf` into the portable archive

**Options:**
- `-SkipBuild` - Skip cargo build, use the existing binary

**Output:** `target\wix\gcodekit5-<version>-x64.msi` and
`target\gcodekit5-<version>-windows-x86_64-portable.zip`

---

## macOS Scripts

### `macos-create-dmg.sh`

Creates a DMG installer for macOS.

```bash
./scripts/macos-create-dmg.sh
```

---

## Linux/Unix Scripts

### `start-grblhal-sim.sh`

Starts the grblHAL simulator with a virtual TTY device for testing CNC communication.

```bash
./scripts/start-grblhal-sim.sh
```

**What it does:**
- Creates a virtual serial port at `target/temp/ttyGRBL`
- Starts the grblHAL simulator connected to the virtual port
- Logs step and block output to `target/temp/`
- Allows your application to connect as if to real hardware

**To stop:**
```bash
./scripts/stop-grblhal-sim.sh
# or
killall socat
```

**To connect from your application:**
- Use the path: `~/Projects/gcodekit5/target/temp/ttyGRBL`
- Baud rate: Any (virtual serial, speed doesn't matter)
- The simulator responds to standard GRBL/grblHAL commands

---

### `stop-grblhal-sim.sh`

Stops the grblHAL simulator virtual TTY device.

```bash
./scripts/stop-grblhal-sim.sh
```

---

### `update-po.sh`

Updates translation files (.po) from source code.

```bash
./scripts/update-po.sh
```

**What it does:**
- Extracts translatable strings from Rust source
- Updates `.pot` template
- Updates all `.po` language files

---

## Python Scripts

### `generate_test_stl.py`

Generates test STL files for development and testing.

```bash
python scripts/generate_test_stl.py
```

---

### `release_notes.py`

Builds the GitHub release notes for one version from `CHANGELOG.md`. The release workflow runs it on every tag push and uses the result as the release body.

The notes are every `## [<version>] - ...` section of `CHANGELOG.md` (a version can have several), plus older versions that never had a published release. They end with a downloads table and a link to the full `CHANGELOG.md` at the tag.

**Usage:**
```bash
python3 scripts/release_notes.py 2.0.0-alpha.2 --out target/temp/notes.md --repo thawkins/gcodekit6
```

A tag push fails at this step when `CHANGELOG.md` has no section for its version, so before tagging, give the release its own `## [<version>] - <date>` heading.

## Windows Build Quick Reference

### First Time Setup

Install the Rust MSVC toolchain, WiX Toolset v3.14.1, and `cargo install cargo-wix`.

### Create Release

```powershell
.\scripts\build-windows-release.ps1
```

---

## Documentation

- Windows Build Guide: [docs/WINDOWS_BUILD.md](../docs/WINDOWS_BUILD.md)
- Windows Quick Start: [docs/WINDOWS_QUICK_START.md](../docs/WINDOWS_QUICK_START.md)
- macOS Build Guide: [docs/MACOS_BUILD.md](../docs/MACOS_BUILD.md)

---

## License

All scripts in this directory are part of GCodeKit5 and licensed under MIT OR Apache-2.0.
