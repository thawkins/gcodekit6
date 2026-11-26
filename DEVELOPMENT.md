# Developer Setup Guide

This guide will help you set up a development environment for GCodeKit5.

## Prerequisites

### Required Software

| Software | Minimum Version | Purpose |
|----------|-----------------|---------|
| **Rust** | 1.95+ | Compiler and cargo |
| **pkg-config** | - | Library discovery (Linux) |
| **Git** | 2.0+ | Version control |

### Platform-Specific Setup

#### Linux (Fedora/RHEL)

```bash
# Install development tools
sudo dnf groupinstall "Development Tools"

# Install the build dependencies (serial ports, Wayland and X11 for egui)
sudo dnf install systemd-devel pkgconf-pkg-config \
    libxkbcommon-devel wayland-devel libX11-devel \
    libXcursor-devel libXrandr-devel libXi-devel

# Runtime graphics driver for wgpu
sudo dnf install vulkan-loader mesa-vulkan-drivers

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

#### Linux (Ubuntu/Debian)

```bash
# Install development tools
sudo apt update
sudo apt install build-essential

# Install the build dependencies (serial ports, Wayland and X11 for egui)
sudo apt install libudev-dev pkg-config libxkbcommon-dev \
    libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev

# Runtime graphics driver for wgpu
sudo apt install libvulkan1 mesa-vulkan-drivers

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

#### macOS

Nothing needs installing besides Rust and the Xcode Command Line Tools.

```bash
# Install the Xcode Command Line Tools (linker)
xcode-select --install

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

See [docs/MACOS_BUILD.md](docs/MACOS_BUILD.md) for detailed macOS instructions.

#### Windows

See [docs/WINDOWS_BUILD.md](docs/WINDOWS_BUILD.md) for Windows setup (Rust MSVC toolchain, no extra libraries).

## Clone and Build

### 1. Clone the Repository

```bash
git clone https://github.com/thawkins/gcodekit5.git
cd gcodekit5
```

### 2. Configure Git Hooks

```bash
# Enable pre-commit hooks for code quality
git config core.hooksPath .githooks
```

### 3. Build the Project

```bash
# Debug build (faster compilation, includes debug symbols)
cargo build

# Release build (optimized, slower to compile)
cargo build --release
```

**Note**: First build downloads dependencies and may take 5-10 minutes. Subsequent builds are much faster due to incremental compilation.

### 4. Verify the Build

```bash
# Check that the binary runs
cargo run --release -- --version
```

## Running the Application

### Development Mode

```bash
# Run with debug logging
RUST_LOG=debug cargo run

# Run with trace logging (very verbose)
RUST_LOG=trace cargo run

# Run specific module logging
RUST_LOG=gcodekit5_communication=debug cargo run
```

### Release Mode

```bash
cargo run --release
```

### Using the Development Container

If you have Podman installed, you can use the development container:

```bash
# Build and enter the container
podman build -t gcodekit5-dev .devcontainer/
podman run -it --rm \
    --userns=keep-id \
    --security-opt label=disable \
    -v "$(pwd):/workspace:Z" \
    gcodekit5-dev

# Inside container
cargo build
cargo run
```

Or open in VS Code with the Dev Containers extension (set `"dev.containers.dockerPath": "podman"` in settings).

## Running Tests

### All Tests

The workspace runs tests with [cargo-nextest](https://nexte.st) (`cargo install cargo-nextest`).

```bash
# Run all tests
cargo nextest run --workspace

# Show output of passing tests too
cargo nextest run --workspace --success-output immediate

# Run with a timeout (a full run is slow)
timeout 1500 cargo nextest run --workspace
```

### Specific Tests

```bash
# Run tests for a specific crate
cargo nextest run -p gcodekit5-core

# Run tests matching a name
cargo nextest run -E 'test(test_grbl_status_parser)'

# Run one test file (each crate has one `all` test binary)
cargo nextest run -p gcodekit5-core -E 'test(test_foo::)'
```

### Doc Tests

nextest does not run doc tests:

```bash
cargo test --doc
```

## Code Quality

### Formatting

```bash
# Format all code
cargo fmt

# Check formatting without changes
cargo fmt --check
```

### Linting

```bash
# Run Clippy linter
cargo clippy

# Clippy with all warnings as errors
cargo clippy -- -D warnings

# Fix auto-fixable issues
cargo clippy --fix
```

### Pre-Commit Checks

The git hooks run automatically, but you can run them manually:

```bash
# Run the same checks as pre-commit
cargo fmt --check && cargo clippy && cargo nextest run --workspace
```

## Project Structure

```
gcodekit5/
├── Cargo.toml              # Workspace root
├── src/                    # Main binary
│   └── main.rs
├── crates/                 # Library crates
│   ├── gcodekit5-core/     # Core types, traits, events
│   ├── gcodekit5-communication/  # Serial, firmware protocols
│   ├── gcodekit5-visualizer/     # 2D/3D rendering
│   ├── gcodekit5-designer/       # CAD/CAM tools
│   ├── gcodekit5-camtools/       # Box maker, puzzle, etc.
│   ├── gcodekit5-devicedb/       # Device profiles
│   ├── gcodekit5-settings/       # App configuration
│   ├── gcodekit5-app/            # Toolkit-free application state
│   └── gcodekit5-ui-egui/        # egui + craft-libs front end
├── docs/                   # Documentation
├── assets/                 # Icons, images
├── flatpak/                # Flatpak packaging
└── scripts/                # Build/utility scripts
```

## Debugging Tips

### Enable Rust Backtraces

```bash
RUST_BACKTRACE=1 cargo run
RUST_BACKTRACE=full cargo run  # Full backtrace with source locations
```

### Debug Logging Levels

```bash
# Levels: error, warn, info, debug, trace
RUST_LOG=debug cargo run

# Multiple modules with different levels
RUST_LOG=gcodekit5=debug,gcodekit5_communication=trace cargo run

# All modules at debug except noisy ones
RUST_LOG=debug,hyper=warn cargo run
```

### VS Code Debugging

1. Install the "CodeLLDB" extension
2. Create/update `.vscode/launch.json`:

```json
{
    "version": "0.2.0",
    "configurations": [
        {
            "type": "lldb",
            "request": "launch",
            "name": "Debug GCodeKit5",
            "cargo": {
                "args": ["build", "--bin=gcodekit5", "--package=gcodekit5"],
                "filter": {
                    "name": "gcodekit5",
                    "kind": "bin"
                }
            },
            "args": [],
            "cwd": "${workspaceFolder}",
            "env": {
                "RUST_LOG": "debug",
                "RUST_BACKTRACE": "1"
            }
        }
    ]
}
```

3. Set breakpoints and press F5 to debug

### GDB/LLDB Command Line

```bash
# Build debug binary
cargo build

# Debug with GDB
gdb target/debug/gcodekit5

# Debug with LLDB
lldb target/debug/gcodekit5
```

### egui Inspection

The egui front end has no inspector. To check a page's layout without a display, the front
end's tests use `egui_kittest` and write snapshot images (see `crates/gcodekit5-ui-egui/tests`).

## Common Issues

### Build Fails: Missing System Libraries (Linux)

**Error**: `The system library `libudev` required by crate `libudev-sys` was not found` or a
missing `xkbcommon` or `wayland` library

**Solution**: Install the Linux build dependencies (see Prerequisites above)

```bash
# Fedora
sudo dnf install systemd-devel libxkbcommon-devel wayland-devel

# Ubuntu
sudo apt install libudev-dev libxkbcommon-dev libwayland-dev
```

### Build Fails: pkg-config Not Found

**Error**: `pkg-config: command not found`

**Solution**:
```bash
# Fedora
sudo dnf install pkg-config

# Ubuntu
sudo apt install pkg-config

# macOS
brew install pkg-config
```

### Tests Timeout

**Error**: Tests hang or timeout

**Solution**: nextest kills a single test after 150 s (see `.config/nextest.toml`). Run one crate at a time:
```bash
timeout 1500 cargo nextest run -p gcodekit5-core
```

### Serial Port Permission Denied (Linux)

**Error**: `Permission denied` when connecting to CNC controller

**Solution**: Add your user to the dialout group:
```bash
sudo usermod -aG dialout $USER
# Log out and back in for changes to take effect
```

### Window Does Not Open / Blank Window (Linux)

**Error**: The app starts but shows no window, or logs a wgpu adapter error

**Solution**: Install a Vulkan or OpenGL driver for wgpu:
```bash
# Ubuntu
sudo apt install libvulkan1 mesa-vulkan-drivers
```

### Flatpak Build Fails

**Error**: Flatpak manifest errors

**Solution**: Install flatpak-builder:
```bash
# Fedora
sudo dnf install flatpak-builder

# Ubuntu
sudo apt install flatpak-builder
```

## Getting Help

- **Issues**: [GitHub Issues](https://github.com/thawkins/gcodekit5/issues)
- **Documentation**: See `docs/` folder
- **Architecture**: See [docs/adr/](docs/adr/) for design decisions
- **Coordinate System**: See [docs/COORDINATE_SYSTEM.md](docs/COORDINATE_SYSTEM.md)

## Next Steps

1. Read the [Architecture Decision Records](docs/adr/) to understand design choices
2. Explore the crate structure in `crates/`
3. Run the application and explore the UI
4. Pick an issue labeled "good first issue" to start contributing
