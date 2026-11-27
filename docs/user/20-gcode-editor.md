# G-Code Editor

## Overview

The G-Code Editor provides a full-featured text editor for viewing and modifying G-code files.

## Opening Files

- **File → Open** or **Ctrl+O** to open a G-code file
- The file dialog lists `.gcode`, `.nc`, `.gc` and `.tap` files
- Large files (more than 50,000 lines) open read-only to stay fast; tick **Edit anyway** to edit them

## Syntax Highlighting

G-code elements are color-coded for readability:

| Element | Examples |
|---------|----------|
| **G-codes** | G0, G1, G2, G3, G28, G54 |
| **M-codes** | M3, M5, M8, M9, M30 |
| **Coordinates** | X, Y, Z, A, B, C values |
| **Parameters** | F (feed), S (speed), T (tool), P, R |
| **Comments** | Lines starting with `;` or text in `(parentheses)` |

## Search and Replace

The find row sits above the text:

- **Search...**: enter text; matching ignores case (`x10` finds `X10`)
- **Previous Match** / **Next Match**: step through matches, wrapping around
- **Matches:** shows the total number of matches
- **Replace with ...**: enter the replacement, then **Replace** (the current match, then on to the next) or **Replace All**
- **Clear editor** empties the editor after asking

Replace is disabled while a job runs or while a large file is shown read-only.

## Execution Tracking

While streaming a program to the machine:

- The line in progress becomes the **current line**, highlighted in the line-number gutter
- The editor **auto-scrolls** to keep the current line visible
- The text is read-only while the job runs

## Saving Files

- **Ctrl+S** — Save to the current file
- **Ctrl+Shift+S** — Save As with a new filename
- Unsaved changes are marked with `*` after the file name under the text; you are asked to save before opening another file or quitting

## See Also

- [Streaming G-Code](80-streaming.md) — Sending programs to the machine
- [Keyboard Shortcuts](90-shortcuts.md) — Editor shortcut reference
