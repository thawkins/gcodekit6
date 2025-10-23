# Issue: G-code arc expander (G2/G3 -> G1 approximation)

Summary
-------
Implement a preprocessing stage that expands circular arc commands (G2/G3) into a series of linear moves (G1) for devices or firmwares that do not support arcs natively.

Motivation
----------
- Some devices and firmware versions lack native support for arc commands or have buggy arc implementations. Expanding arcs into small linear segments increases compatibility and allows consistent behavior across devices.

Proposed solution
-----------------
- Create an `arc_expander` module in `crates/core::preprocessors` that accepts a G-code line with G2/G3 and outputs a sequence of G1 lines approximating the arc.
- Provide configuration options: segment_length (mm), max_segments, tolerance, and whether to preserve feedrates and spindle commands.
- Implement both IJK and R parameter handling for arc centers.
- Add unit tests for simple quarter-circle, full-circle, and eccentric arcs; include numerical tolerances in assertions.

Acceptance criteria
-------------------
- `arc_expander` is available as a pluggable preprocessor in the streaming pipeline.
- Tests show acceptable geometric error below configured tolerance for representative arcs.
- Performance benchmarks for large arcs (e.g., 10k segments) are added to ensure CPU/memory usage is reasonable.

Labels
------
- area:preprocessor
- feature
- priority:medium

Estimate
--------
3-6 days (math implementation, parameter parsing, tests, and benchmarks)

Notes / Edge cases
------------------
- Handle G-code state carry-over (modal commands like feed, units, plane selection).
- Provide a streaming-friendly API so large arc expansions can be produced incrementally rather than all at once to avoid large memory spikes.

Subtasks (PR-sized)
--------------------
- [ ] Implement basic arc math and parameter parsing for IJK and R forms.
- [ ] Add configuration options (segment_length, tolerance) and incremental generator API.
- [ ] Add unit tests for canonical arcs and edge cases.
- [ ] Add benchmark harness and a streaming integration test.
