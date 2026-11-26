# macOS Build and Distribution Guide

## Overview

GCodeKit5 builds on macOS 11 or later. The release workflow builds the Apple Silicon (ARM64) app. You can also build for Intel Macs locally.

The egui front end uses only system frameworks. There is nothing to install with Homebrew and nothing to bundle into the app.

## GitHub Actions Workflow

The workflow in `.github/workflows/release.yml` builds `aarch64-apple-darwin` on `macos-14`:

1. Builds the release binary with `cargo build --release --target aarch64-apple-darwin`.
2. Creates `GCodeKit.app` with the binary, an `AppIcon.icns` icon and an `Info.plist`. `LSMinimumSystemVersion` is `11.0`.
3. Creates the DMG with `scripts/macos-create-dmg.sh`.
4. Uploads the DMG to the GitHub Release.

## Building Locally

### Prerequisites

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Add the Intel target if cross-compiling on Apple Silicon
rustup target add x86_64-apple-darwin
```

Rust 1.95 or later is required. Xcode Command Line Tools provide the linker (`xcode-select --install`).

### Build for Current Architecture

```bash
# Debug build
cargo build

# Release build
cargo build --release
```

### Build for Specific Architecture

```bash
# Apple Silicon (ARM64)
cargo build --release --target aarch64-apple-darwin

# Intel (x86_64)
cargo build --release --target x86_64-apple-darwin
```

## Create App Bundle and DMG

The release workflow builds the bundle in this layout:

```
GCodeKit.app/
└── Contents/
    ├── MacOS/
    │   └── gcodekit5 (executable)
    ├── Resources/
    │   └── AppIcon.icns
    └── Info.plist
```

To build it by hand:

```bash
# 1. Build the release binary
cargo build --release --target aarch64-apple-darwin

# 2. Create the bundle folders and copy the binary
mkdir -p "GCodeKit.app/Contents/MacOS" "GCodeKit.app/Contents/Resources"
cp target/aarch64-apple-darwin/release/gcodekit5 "GCodeKit.app/Contents/MacOS/"

# 3. Add AppIcon.icns and Info.plist (see the "Build (macOS)" step in release.yml)

# 4. Create the DMG
bash scripts/macos-create-dmg.sh "v1.2.0" "macos-arm64"
```

## DMG Distribution

The `scripts/macos-create-dmg.sh` script creates a DMG that:

- Includes a symbolic link to the Applications folder
- Sets icon view with appropriate sizing
- Positions app and Applications folder icons
- Uses UDZO compression for smaller file size
- Creates files matching the pattern: `gcodekit5-<version>-<arch>.dmg`

## Signing and Notarization (Future)

For production releases, consider adding:

1. **Code Signing:**
   ```bash
   codesign -s - --deep --force --verify --verbose=4 "GCodeKit.app"
   ```

2. **Notarization (required for distribution):**
   - Requires Apple Developer account
   - Command-line tools available via `xcrun notarytool`

## Troubleshooting

### App Does Not Start

Check that the binary links only system libraries:

```bash
otool -L "GCodeKit.app/Contents/MacOS/gcodekit5"
```

Every entry should be under `/usr/lib` or `/System/Library`.

### Build Failures on M1/M2 Macs

Ensure you have the correct target installed:
```bash
rustup target add aarch64-apple-darwin
```

## CI/CD Pipeline

The GitHub Actions workflow:

1. Triggers on version tags (e.g., `v1.2.0`), or by hand with `workflow_dispatch` for a dry run that builds every artefact without publishing
2. Builds the macOS ARM64 app, plus the Linux and Windows packages
3. Creates the DMG for distribution
4. Uploads to GitHub Releases on a tag push
5. Includes the changelog from `CHANGELOG.md`

To trigger a release build:

```bash
git tag -a v1.2.0 -m "Release 1.2.0"
git push origin v1.2.0
```

## Performance Optimization

Built binaries include:
- Release optimizations (`opt-level = 3`)
- Link-time optimization (LTO)
- Single codegen unit for maximum optimization

This results in smaller, faster binaries suitable for distribution.
