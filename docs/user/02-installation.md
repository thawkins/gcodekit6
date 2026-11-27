# Installation

## Prerequisites

- **Rust toolchain** 1.95 or later (install from [rustup.rs](https://rustup.rs))
- A GPU with Vulkan, Metal, DirectX 12 or OpenGL ES 3 support (the UI is drawn with wgpu)

## Linux

### Dependencies

Ubuntu / Debian (the package list the CI build uses):

```bash
sudo apt-get install -y libudev-dev pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libvulkan1 mesa-vulkan-drivers
```

On other distributions, install the equivalent packages.

### Build from Source

```bash
git clone https://github.com/thawkins/gcodekit6.git
cd gcodekit6
cargo build --release
./target/release/gcodekit6
```

### Serial Port Permissions

To access serial devices without root:

```bash
sudo usermod -aG dialout $USER
# Log out and log back in for the change to take effect
```

### Flatpak

Each GitHub release includes a Flatpak bundle, `gcodekit6-<tag>-linux-x86_64.flatpak`, where
`<tag>` is the release tag. Install it with:

```bash
flatpak install --user gcodekit6-<tag>-linux-x86_64.flatpak
```

## macOS

See [macOS Build Guide](../MACOS_BUILD.md) for detailed instructions. The only prerequisite
besides Rust is the Xcode Command Line Tools.

```bash
xcode-select --install
git clone https://github.com/thawkins/gcodekit6.git
cd gcodekit6
cargo build --release
```

## Windows

See [Windows Build Guide](../WINDOWS_BUILD.md) for detailed instructions.

## Verifying Installation

After building, launch the application:

```bash
./target/release/gcodekit6
```

You should see the main GCodeKit6 window with the connection panel, DRO, and console.

## Next Steps

- [Quick Start](03-quick-start.md) — Get running in 5 minutes
- [Device Setup](04-device-setup.md) — Connect your CNC machine
