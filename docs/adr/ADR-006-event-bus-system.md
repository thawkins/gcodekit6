# ADR-006: Unified Event Bus System

## Status
Rejected (2026-11): never implemented; superseded by ADR-012; listener plumbing removed in audit T48.

Amended 2026-11-28: cut the unimplemented design to this summary. The full proposal is in git history.

**Date**: 2026-01-30
**Related**: ADR-004 (Interior Mutability), ADR-005 (Error Handling), ADR-012 (egui and craft-libs Front End)

## Context

In January 2026 the GTK front end used several event patterns at once: broadcast channels
(`EventDispatcher`, `MessageDispatcher`), listener traits (`CommunicatorListener`,
`ControllerListener`), closure callbacks behind `Arc<Mutex<_>>`, GTK signals, and `RefCell`
callbacks. Event flow was hard to trace and components were tightly coupled.

## Decision (proposed, not adopted)

Add one typed, asynchronous `EventBus` in `gcodekit6-core`: a root `AppEvent` enum (connection,
machine, file, UI, communication, settings, error), a subscriber trait with filters, publish
helpers, and a bridge to GTK signals. Existing listeners would have been migrated to it in phases.

## Why it was rejected

The bus was never built. The egui front end (ADR-012) removed the need for it:

- egui is immediate mode, so there are no widget signals or UI callbacks to connect.
- `MachineController` (`crates/gcodekit6-app/src/machine/`) owns one worker thread. Commands go
  to it and `MachineEvent`s come back over two `std::sync::mpsc` channels; the app collects the
  events with `MachineController::poll` each frame.
- The listener types above were removed in audit T48
  (`docs/reports/2026-11-27-deep-refactor-audit.md`). None of `EventDispatcher`,
  `MessageDispatcher`, `CommunicatorListener` or `EventBus` exists in the code, and the workspace
  has no async runtime.
