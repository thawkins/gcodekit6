This directory contains recommended VS Code Insiders configuration snippets for Rust development and debugging.

Files:
- settings.json -> paste into the Insiders user settings (~/.config/Code - Insiders/User/settings.json or use the Settings UI)
- launch.json -> put in your project's .vscode/launch.json to enable debugging with CodeLLDB
- tasks.json -> put in your project's .vscode/tasks.json to build using Cargo before debugging

Notes:
- Ensure `rustup` and `cargo` are installed (https://rustup.rs/).
- Install `vadimcn.vscode-lldb` extension (CodeLLDB) for debugging.
- On Linux, install `lldb` system package appropriate to your distro (e.g., `lldb` or `lldb-14`).
- The launch config uses the `codelldb` adapter which is provided by the CodeLLDB extension.

See below for content of each file.