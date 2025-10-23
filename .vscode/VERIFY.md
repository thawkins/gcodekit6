Quick verification steps

1. Install system lldb if not installed (Fedora example):
   sudo dnf install lldb

2. Open VS Code Insiders and ensure the extensions are enabled:
   - rust-lang.rust-analyzer
   - vadimcn.vscode-lldb
   - serayuzgur.crates

3. Create a new Cargo project and copy `launch.json` and `tasks.json` into its `.vscode/` folder.

4. From VS Code, run the build task (Terminal > Run Build Task or Ctrl+Shift+B) and start the Debug (Run) view and launch "Debug executable (CodeLLDB)".

Notes:
- On some distributions CodeLLDB will use the bundled debug adapter and will still need `lldb` runtime libraries; installing the `lldb` package as above is recommended.
- If debugging fails, open the Debug Console for adapter logs and ensure the built binary path matches `program` in `launch.json`.