# Issue: Macro & script storage and execution

Summary
-------
Add persistent storage and execution support for user macros and scripts (reusable G-code snippets, parameterized macros, and small automation scripts). Provide a secure, sandboxed execution model and a UI for managing macros.

Motivation
----------
- Macros allow users to encapsulate repetitive sequences (probe, home, toolchange) and reuse them across jobs. Providing a first-class macro system improves usability and automation.

Proposed solution
-----------------
- Add `crates/core::macros` module with types for `Macro` and `MacroLibrary` and persistence using `crates/utils::storage`.
- Macros support:
  - Named sequences of G-code lines with optional parameters (positional or named).
  - A small templating or parameter substitution mechanism (e.g., `{{probe_depth}}`).
  - Execution API that expands parameters and streams lines through the existing streamer.
- Provide optional script support later (Lua or WASM) behind a feature flag; start with parameterized text macros.

Acceptance criteria
-------------------
- Persistent macro storage in the data directory with CRUD API.
- UI components to list, add, edit, and run macros (hooked into `ui::Api`).
- Tests for parameter substitution and safe execution (ensure no arbitrary code execution beyond allowed commands).

Labels
------
- area:jobs
- feature
- priority:medium

Estimate
--------
2-4 days for basic macro storage and parameter substitution. Additional days for optional script languages (Lua/WASM).

Security notes
--------------
- Avoid executing arbitrary code. For scripting, prefer WASM sandboxing or require explicit opt-in and review. Keep macro substitution conservative (text replacement only) unless a sandboxed runtime is introduced.

Subtasks (PR-sized)
--------------------
- [ ] Design `Macro` and `MacroLibrary` types and storage layout.
- [ ] Implement CRUD persistence using `utils::storage` and add tests.
- [ ] Implement simple parameter substitution and validation.
- [ ] Wire macro execution into streamer with safety checks and a demo UI flow.
