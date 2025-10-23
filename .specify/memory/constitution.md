<!--
Sync Impact Report:
Version: 1.0.0-alpha (new constitution)
Modified principles: N/A (initial creation)
Added sections: All sections are new
Removed sections: N/A
Templates requiring updates:
✅ Updated: All templates inherit constitution compliance
⚠ Pending: None - initial constitution establishment
Follow-up TODOs: None
-->

# GCodeKit6 Constitution

## Core Principles

### I. Rust-Native Safety & Performance (NON-NEGOTIABLE)
All code MUST be written in Rust edition 2021 or greater with safety as the primary concern. Memory safety violations, data races, and undefined behavior are NEVER acceptable. Performance optimizations MUST NOT compromise safety. Use `cargo clippy` and address ALL warnings before code review. Unsafe code blocks require explicit justification and safety documentation.

**Rationale**: Fabrication machine control demands absolute reliability - a software crash can damage expensive equipment or cause safety hazards.

### II. Test-Driven Development (NON-NEGOTIABLE)
ALL tests MUST be organized under a `tests/` directory and must import from the public crate (e.g., `use gcodekit6::`). Historically this required a single top-level `tests/` folder at the repository root; however, to better support idiomatic Rust workspace layouts and crate-level isolation, crate-local integration tests located in `crates/*/tests/` are ALSO permitted.

Constraints when using crate-local `crates/*/tests/`:
- Tests MUST still be public-integration style (use the public crate surface) and not be inline unit tests inside implementation files.
- CI must run all integration tests across the workspace (top-level and crate-local). Test discovery rules and CI configuration MUST be documented in the repository CI configuration.
- The project remains governed by the rule that tests are written before implementation (TDD): write tests first, verify they fail, then implement behavior.

Use `#[test]` for sync and `#[tokio::test]` for async tests. All test runs have a 10-minute timeout to prevent hanging.

**Rationale**: Machine control software requires extensive testing - untested code controlling physical devices poses safety and financial risks.

### III. Slint UI Framework Consistency
User interface MUST be implemented using the Slint framework exclusively. UI components MUST be modular, reusable, and follow Slint best practices. All UI interactions MUST be responsive and provide clear feedback for fabrication operations (connection status, job progress, error states).

**Rationale**: Consistent UI framework ensures maintainable, cross-platform desktop application with professional appearance suitable for industrial environments.

### IV. Real-Time Communication Excellence
Serial communication with fabrication devices (GRBL, TinyG, G2core, Smoothieware) MUST be reliable, responsive, and fault-tolerant. Communication errors MUST be handled gracefully with clear user feedback. Buffer management MUST prevent command loss or device state corruption. Support emergency stop functionality at all times.

**Rationale**: Real-time control of fabrication machines requires bulletproof communication - delays or errors can ruin projects or cause safety incidents.

### V. Documentation & Code Quality Standards
ALL functions MUST have docblock documentation describing purpose, arguments, and return values. ALL modules MUST have file-level docblocks describing purpose and dependencies. Use `tracing` for structured logging - NO `println!` in production code. Follow 4-space indentation, 100-character line width, snake_case for functions/variables, PascalCase for types.

**Rationale**: Machine control software has long lifecycles and multiple maintainers - comprehensive documentation ensures knowledge transfer and reduces bugs.

## Technology & Build Standards

### Mandatory Technology Stack
- **Language**: Rust edition 2021 or greater
- **UI Framework**: Slint (desktop native)
- **Build System**: Cargo with standard project structure
- **Testing**: Tests in `tests/` folder, NOT inline
- **Logging**: `tracing` crate with structured logging
- **Error Handling**: `Result<T, E>` with `?` operator, `anyhow::Result` for main functions, `thiserror` for custom errors

### Build Requirements
- `cargo build` for debug builds (timeout: 10 minutes minimum)
- `cargo build --release` for optimized builds (timeout: 10 minutes minimum)
- `cargo test` with 10-minute timeout (`timeout 600 cargo test`)
- `cargo clippy` MUST pass with zero warnings
- `cargo fmt` MUST pass (automatically enforced)

### Documentation Structure
- **Root Documentation**: `README.md`, `CHANGELOG.md`, `SPEC.md`, `AGENTS.md`, `STATS.md`, `PLAN.md` remain in project root
- **All Other Documentation**: MUST be in `docs/` folder (implementation guides, architecture docs, user guides, API references)
- **Temporary Files**: Use `target/temp/` directory (ensure in `.gitignore`)

## Development Workflow

### Version Control & Release Management
- **Versioning**: Semantic versioning (major.minor.patch-prerelease)
- **Development Suffix**: "-alpha" appended during development
- **Changelog**: Update `CHANGELOG.md` before each push following Keep a Changelog format
- **GitHub Integration**: Use `gh` CLI for all repository operations
- **Issue Templates**: BUG, FEATURE, TASK, CHANGE (initialize once only)

### Issue Handling Process
1. **Analyze First**: Comment in issue with analysis and proposed fix plan
2. **Implement**: Execute the proposed solution
3. **Wait for Confirmation**: Do NOT close until reporter confirms fix works

### Quality Gates
- **Code Review**: All changes require review and constitution compliance verification
- **Error Handling**: Use `Result` types consistently, implement `thiserror` for custom errors
- **Performance**: Optimize for real-time responsiveness in machine communication
- **Security**: Validate all user inputs, especially G-code and device commands

## Governance

### Amendment Process
Constitution amendments require:
1. Documentation of proposed changes with rationale
2. Impact assessment on existing codebase
3. Migration plan for breaking changes
4. Version bump following semantic versioning rules (MAJOR: breaking governance changes, MINOR: new principles/sections, PATCH: clarifications/typos)

### Compliance & Enforcement
- ALL pull requests MUST verify constitution compliance before merge
- Complexity violations MUST be explicitly justified in code review
- Cognitive complexity MUST remain ≤30 per function
- Missing documentation triggers automatic review failure
- Use AGENTS.md for runtime development guidance and detailed technical standards

### Priority & Task Management
When asked "what's next", present top 9 unimplemented tasks by task number, accept selection, and execute. Focus on implementation over feature suggestions unless explicitly requested.

---
### Amendment: 2025-10-23 — Allow crate-local integration tests

Summary: To reconcile constitution requirements with idiomatic Rust workspace layouts and to reduce heavy refactoring of existing test suites, the Governance Council ratified an amendment on 2025-10-23 to permit crate-local integration tests under `crates/*/tests/` in addition to a repository-level `tests/` directory. This preserves the testing principles (tests import from public crates and remain integration-style) while enabling modular development and faster iteration.

Rationale: Rust workspaces commonly place integration tests inside crate-level `tests/` directories. Enforcing a single top-level `tests/` causes significant refactor overhead and harms developer ergonomics.

Migration plan:
- CI: Ensure the project's CI is configured to run all integration tests across crates and the root `tests/` folder.
- Tests: Existing crate-local tests are acceptable; maintainers should ensure tests only use public APIs.
- Documentation: Update CONTRIBUTING.md and the project README to explain test layout and how to run tests locally.

**Version**: 1.1.0-alpha | **Ratified**: 2025-10-23 | **Last Amended**: 2025-10-23
