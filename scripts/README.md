# Build Scripts

This directory contains platform-specific build and packaging scripts for GCodeKit6.

## Windows Scripts

### `build-windows-release.ps1`

Builds the Windows release: the MSI installer and the portable zip. No GTK
runtime or MSYS2 is required.

```powershell
.\scripts\build-windows-release.ps1
```

**What it does:**
1. Builds the release binary (`cargo build --release --target x86_64-pc-windows-msvc`)
2. Builds the MSI with `cargo wix --package gcodekit6-design --no-build`
3. Zips `gcodekit6.exe` and `License.rtf` into the portable archive

**Options:**
- `-SkipBuild` - Skip cargo build, use the existing binary

**Output:** `target\wix\gcodekit6-<version>-x64.msi` and
`target\gcodekit6-<version>-windows-x86_64-portable.zip`

Prerequisites and details: [docs/WINDOWS_BUILD.md](../docs/WINDOWS_BUILD.md).

---

## macOS Scripts

### `macos-create-dmg.sh`

Creates a DMG installer for macOS. Run it from the folder that holds
`GCodeKit.app`.

```bash
./scripts/macos-create-dmg.sh [VERSION] [ARCH]
```

- `VERSION` - default `1.0.0`
- `ARCH` - default `macos-arm64`

**Output:** `gcodekit6-<VERSION>-<ARCH>.dmg` in the current folder. See
[docs/MACOS_BUILD.md](../docs/MACOS_BUILD.md).

---

## Linux/Unix Scripts

### `start-grblhal-sim.sh`

Starts the grblHAL simulator with a virtual TTY device for testing CNC communication.
Creating the device in `/dev` needs sudo. `grblHAL_sim` must be on `PATH` or in the
invoking user's `~/.local/bin`, and `socat` must be installed.

```bash
sudo ./scripts/start-grblhal-sim.sh
```

**What it does:**
- Creates a virtual serial port at `/dev/ttyGRBL`
- Starts the grblHAL simulator connected to the virtual port
- Logs step and block output to `target/temp/grblhal-step.out` and
  `target/temp/grblhal-block.out`
- Allows your application to connect as if to real hardware

**To stop:**
```bash
sudo ./scripts/stop-grblhal-sim.sh
# or
sudo killall socat
```

**To connect from your application:**
- Use the path: `/dev/ttyGRBL`
- Baud rate: Any (virtual serial, speed doesn't matter)
- The simulator responds to standard GRBL/grblHAL commands

---

### `stop-grblhal-sim.sh`

Stops the grblHAL simulator (`killall socat`) and removes `/dev/ttyGRBL`.

```bash
sudo ./scripts/stop-grblhal-sim.sh
```

---

### `test-grblhal-sim.sh`

Sends `?`, `$$`, `$I` and `$G` to the running simulator at `/dev/ttyGRBL` and
prints the replies.

```bash
./scripts/test-grblhal-sim.sh
```

---

### `EEPROM.DAT`

Simulator EEPROM image; do not edit.

---

### `monthly-dependency-review.sh`

Runs the monthly dependency review: `cargo outdated`, `cargo audit` and
`cargo tree --duplicates`. Needs `cargo install cargo-outdated cargo-audit`.

```bash
./scripts/monthly-dependency-review.sh [--update]
```

- `--update` - also run `cargo update` (semver-compatible updates)

**Output:** `target/temp/dependency-review-YYYY-MM.md`. See
[docs/dependency_management.md](../docs/dependency_management.md).

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

## Documentation

- Windows Build Guide: [docs/WINDOWS_BUILD.md](../docs/WINDOWS_BUILD.md)
- macOS Build Guide: [docs/MACOS_BUILD.md](../docs/MACOS_BUILD.md)

---

## License

All scripts in this directory are part of GCodeKit6 and licensed under MIT OR Apache-2.0.
