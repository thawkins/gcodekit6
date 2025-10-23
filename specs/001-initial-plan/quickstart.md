# quickstart.md

## Quickstart: Build and run (Linux)

1. Install Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2. Install Slint: follow instructions at https://slint-ui.com/
3. From repository root:

```bash
cargo build
cargo test
```

4. Run the application (once scaffolded):

```bash
cargo run -p ui
```

## First Run
- Open Connect dialog and select detected serial port
- Load a `.gcode` file and press Send
- Use Emergency Stop to immediately halt operations

