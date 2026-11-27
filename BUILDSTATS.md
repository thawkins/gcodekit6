# Build Statistics

- **Date**: 2026-11-27
- **Version**: 2.0.0-alpha.3
- **Host**: TimWorkFed01.local, Linux 7.3.4 (Fedora 45), 8 CPUs
- **Toolchain**: cargo 1.102.0-nightly (ae8f06eb4 2026-11-21), rustc 1.102.0-nightly (6c709eb42 2026-11-25)

## Build phases

| Step | Command | Time | Disk consumed | Disk free before | Disk free after |
|------|---------|------|---------------|------------------|-----------------|
| clean | `cargo clean` | 6s | freed 52.90 GiB | 631.35 GiB | 684.24 GiB |
| release | `cargo build --release --workspace` | 7m 9s | 0.37 GiB | 684.26 GiB | 683.88 GiB |
| debug | `cargo build --workspace` | 1m 0s | 0.66 GiB | 683.88 GiB | 683.22 GiB |
| tests | `cargo test --workspace --all-targets` | 55s | freed 0.01 GiB | 676.38 GiB | 676.38 GiB |
| coverage | `cargo llvm-cov --workspace --all-targets` | 1m 33s | 1.25 GiB | 676.38 GiB | 675.13 GiB |

Notes:

- The release and debug builds are from a clean tree.
- The tests and coverage rows are a re-run, so they are not from a clean tree:
  - The first test run (3m 2s) found 5 tests in `gcodekit5-ui-egui` that fail under `cargo test`, which runs every test of a binary in one process:
    - `test_machine_jog::test_inverted_z_jogs_negative` raced on a process-wide jog-inversion table;
    - four `test_cam_drill_press` tests raced with the laser-mode test's `$32=1`.
  - The first coverage run did not compile, because a source file was being edited while it ran.
  - After both fixes the two steps were re-run. The test build was then mostly incremental, and coverage reused dependencies already compiled into `target/llvm-cov-target`, so both times are lower than a cold run.
  - The fix checks were built in a separate target directory, which accounts for the 6.84 GiB drop in free disk space between the debug and tests rows.

## Tests

`cargo test --workspace --all-targets` ran 21 test binaries:

- 1390 passed;
- 0 failed;
- 3 ignored.

`--all-targets` leaves out doc tests. CI runs the suite with `cargo nextest`, which runs each test in its own process.

## Coverage

Overall line coverage: **46144 / 58836 lines = 78.43%**.

How it was measured:

- `cargo llvm-cov --workspace --all-targets`;
- files under `tests/`, `benches/`, `examples/` and `target/` are excluded;
- lines are grouped by the crate directory under `crates/`.

| Crate | Lines covered | Lines | Coverage |
|-------|---------------|-------|----------|
| gcodekit5 (bin, `src/`) | 0 | 24 | 0.00% |
| gcodekit5-app | 4513 | 5052 | 89.33% |
| gcodekit5-camtools | 4014 | 5062 | 79.30% |
| gcodekit5-communication | 970 | 1707 | 56.82% |
| gcodekit5-core | 1255 | 2153 | 58.29% |
| gcodekit5-designer | 9140 | 14609 | 62.56% |
| gcodekit5-devicedb | 321 | 481 | 66.74% |
| gcodekit5-settings | 1320 | 1629 | 81.03% |
| gcodekit5-ui-egui | 22874 | 24729 | 92.50% |
| gcodekit5-visualizer | 1737 | 3390 | 51.24% |
