# ADR-011: egui and craft-libs Pin

## Status
Accepted

## Date
2026-11-25

## Context
The front end is migrating from GTK4 to egui using the `storytold/craft-libs` crates. craft-libs is a git-only dependency and itself patches egui and winit to forks (its root `[patch.crates-io]`). Cargo patches do not propagate to dependents, so GCodeKit6 must replicate them to keep a single egui version in `Cargo.lock`.

## Decision
- Pin craft-libs at rev `d15bf1f35b17b759870f9e5111b9a5d243f64888` (egui 0.37, rust-version 1.95).
- Use the egui family at `0.37` and replicate craft-libs' `[patch.crates-io]` for egui crates (`storytold/egui` rev `6d518fc799681571963842f95b55a6b68431b15a`) and winit (`storytold/winit` rev `a517b3846ee8457d84ee4975110bfda6797c44dc`).
- Allow the three git sources in `deny.toml` (`craft-libs`, `egui`, `winit`).
- Dependabot groups the egui family and ignores versions above 0.37; bump together with the craft-libs rev.

## Consequences
Reproducible builds with one egui version. Bumping craft-libs requires updating the rev and the patch revs together.

## Alternatives Considered
Using crates.io egui without the fork: rejected, craft-ui relies on fork-only APIs.

## References
- https://github.com/storytold/craft-libs
