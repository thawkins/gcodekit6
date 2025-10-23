# Issue: GRBL character-counted streaming mode

Summary
-------
Implement support for GRBL-style character-counted streaming (N<i>nnn</i> and checksum acknowledgements) in the streamer layer. This mode is required for compatibility with some GRBL firmware versions that expect line numbering and checksums and that require accurate character-count accounting for the input buffer.

Motivation
----------
- Many open-source CNC firmwares (notably GRBL) support a character-counted streaming protocol that requires the host to send lines with `N<line_number> <gcode> *<checksum>` and manage acknowledgements accordingly.
- Without this, certain machines will reject or mis-order streamed commands.

Proposed solution
-----------------
- Add a GRBL framing option to the streamer implementation.
- When enabled, serialize outgoing lines with line numbers and checksums, and maintain an internal character count of unacknowledged characters.
- Implement parsing of firmware acknowledgements (`ok`, `error`, `Resend: n`, or `C<n>` style). Support resending lines when asked.
- Provide a mock/fake transport in tests to assert correct behavior (character accounting, resend on `Resend: n`, pause/resume during resend).

Acceptance criteria
-------------------
- Streamer can be configured in `grbl` mode via a new enum/option in the streamer config.
- Unit tests demonstrate correct checksum calculation and character counting across multiple line sends.
- Integration test with a simulated GRBL server (mock) demonstrates resend flow and correct recovery from `Resend:` messages.

Labels
------
- area:streamer
- feature
- priority:high

Estimate
--------
2-4 days (core implementation, tests, and robustness around resends)

Notes / Edge cases
------------------
- Consider line number wrap-around and large job numbers.
- Provide a toggle to disable GRBL framing for non-GRBL devices.
- Ensure this implementation doesn't affect non-GRBL devices; keep behavior opt-in.

Subtasks (PR-sized)
--------------------
- [ ] Add GRBL mode option to streamer config and feature-guard specifics if needed.
- [ ] Implement checksum/line-number serialization and character-counting logic.
- [ ] Implement ack parsing including `ok`, `error`, and `Resend:` flows.
- [ ] Create a mock GRBL server test verifying resend and recovery logic.
- [ ] Update docs and add examples showing how to enable GRBL mode.
