# Issue: 3D Visualizer MVP for toolpath preview

Summary
-------
Build a minimal 3D visualizer that renders parsed G-code toolpaths so users can preview jobs before streaming. The MVP should be integrated into the Slint UI or exposed as an optional web view.

Motivation
----------
- Visual previewing catches issues before they reach the machine and provides confidence when running unfamiliar G-code files.
- A simple visualizer improves UX for many users and is a key differentiator.

Proposed solution
-----------------
- Implement a lightweight visualizer module under `crates/ui` or `crates/visualizer` that:
  - Accepts parsed toolpath segments (G1 moves with XYZ and feedrates).
  - Renders wireframe toolpaths in 3D with pan/zoom/rotate controls.
  - Shows current position and optionally highlights the currently-streamed line.
- For a first pass, use a simple renderer: either Slint's canvas (if sufficient) or embed a small OpenGL/WebGL view via `glow` or `wgpu` depending on existing UI constraints.
- Add a demo UI page/command that loads a sample G-code and displays the visualizer.

Acceptance criteria
-------------------
- A visual preview window renders a simple sample toolpath (e.g., a rectangle and a circle) and supports basic mouse interactions.
- Visualizer is wired to the streaming pipeline in a demo mode so it can highlight the current line as the stream progresses.
- Documentation added explaining integration points and how to extend the visualizer.

Labels
------
- area:ui
- feature
- priority:medium

Estimate
--------
4-8 days for MVP depending on renderer selection and existing Slint capabilities.

Notes / Tradeoffs
-----------------
- Using `wgpu`/`wgpu` provides modern GPU-accelerated rendering but increases dependency complexity.
- A simpler approach is to export a JSON representation of the toolpath and use a browser-based viewer (three.js) as a separate optional component.

Subtasks (PR-sized)
--------------------
- [ ] Choose renderer approach (Slint canvas vs wgpu vs web-based) and document tradeoffs.
- [ ] Implement a minimal renderer that draws wireframe toolpaths and basic camera controls.
- [ ] Add API to accept parsed toolpaths and a demo UI page.
- [ ] Hook visualizer to streamer for demo highlighting of current line.
