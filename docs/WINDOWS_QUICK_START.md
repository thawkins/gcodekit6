# Windows Build Quick Start

## One-Command Build

From PowerShell in the project root:

```powershell
.\scripts\build-windows-release.ps1
```

This will:
1. Build the release binary for `x86_64-pc-windows-msvc`
2. Create the MSI installer
3. Create the portable zip

## Options

```powershell
# Skip the cargo build (use existing binary)
.\scripts\build-windows-release.ps1 -SkipBuild
```

## Prerequisites

1. **Rust toolchain** (MSVC target)
2. **WiX Toolset** v3.14.1
3. **cargo-wix**: `cargo install cargo-wix`

No GTK runtime and no MSYS2 are needed.

## Output

```
target\wix\gcodekit5-<version>-x64.msi
target\gcodekit5-<version>-windows-x86_64-portable.zip
```

## Troubleshooting

### WiX Not Found

Install WiX 3.14.1 from https://wixtoolset.org/ and set the `WIX` environment variable to its folder (for example `C:\Program Files (x86)\WiX Toolset v3.14\`), or put its `bin` folder in your PATH.

### The App Does Not Start

The egui front end needs a GPU driver with Direct3D 12 or Vulkan. Update the graphics driver.

## Distribution

Before distributing the MSI:

1. Test on clean Windows 11 VM
2. Test on clean Windows 10 VM
3. Verify all functionality works

## See Also

- [docs/WINDOWS_BUILD.md](WINDOWS_BUILD.md) - Complete build guide
- [scripts/build-windows-release.ps1](../scripts/build-windows-release.ps1) - Build script
- [wix/main.wxs](../wix/main.wxs) - WiX installer configuration
