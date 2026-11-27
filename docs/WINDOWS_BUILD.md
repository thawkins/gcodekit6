# Windows Build and Packaging Guide

## Prerequisites

1. **Rust toolchain** (1.95 or later) with the MSVC target `x86_64-pc-windows-msvc`
2. **Visual Studio Build Tools** (Desktop development with C++ workload)
3. **WiX Toolset v3.14.1** (for the MSI installer)
4. **cargo-wix** (`cargo install cargo-wix`)

No GTK runtime and no MSYS2 are needed. The egui front end links only against system DLLs, and `.cargo/config.toml` links the C runtime statically (`+crt-static`), so the exe needs no VC++ redistributable.

## Build Process

### One command

```powershell
.\scripts\build-windows-release.ps1
```

This builds the release binary, the MSI and the portable zip. Use `-SkipBuild` to reuse an existing binary.

### Step 1: Build the Release Binary

```powershell
cargo build --release --target x86_64-pc-windows-msvc
```

This creates `target\x86_64-pc-windows-msvc\release\gcodekit6.exe`.

### Step 2: Test the Binary Locally

```powershell
.\target\x86_64-pc-windows-msvc\release\gcodekit6.exe
```

### Step 3: Create the MSI Installer

WiX needs a numeric `x.y.z` version, so a prerelease such as `1.2.0-beta.2` is installed as `1.2.0`:

```powershell
cargo wix --package gcodekit6-design --target x86_64-pc-windows-msvc --no-build --nocapture `
    --install-version 1.2.0 -o target\wix\gcodekit6-1.2.0-beta.2-x64.msi
```

The MSI installs `gcodekit6.exe` and `License.rtf`. `wix\main.wxs` is the installer definition.

### Step 4: Create the Portable Zip

The portable zip holds `gcodekit6.exe` and `License.rtf` only. `build-windows-release.ps1` writes it to `target\gcodekit6-<version>-windows-x86_64-portable.zip`.

## Troubleshooting

### WiX Not Found

`cargo wix` finds `candle.exe` and `light.exe` through the `WIX` environment variable or `PATH`. After installing WiX 3.14.1, set:

```powershell
$env:WIX = "${env:ProgramFiles(x86)}\WiX Toolset v3.14\"
```

### The App Shows a Blank Window or Does Not Start

The egui front end draws with wgpu. It needs a working GPU driver with Direct3D 12 or Vulkan. Update the graphics driver and try again.

## Distribution Checklist

Before distributing the MSI:

- [ ] Built with `--release` profile
- [ ] Application starts on a clean Windows 11 system
- [ ] Application starts on a clean Windows 10 system
- [ ] MSI installs to correct location
- [ ] Start menu shortcut works
- [ ] Uninstaller removes all files
- [ ] Version number is correct in installer

## File Size Expectations

- Executable: a single file, no bundled DLLs
- MSI installer and portable zip: about the size of the executable (compressed)

## References

- WiX Toolset: https://wixtoolset.org/
- cargo-wix: https://github.com/volks73/cargo-wix
