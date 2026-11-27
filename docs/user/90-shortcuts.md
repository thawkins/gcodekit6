# Keyboard Shortcuts Reference

Keyboard shortcuts can be customized in [Settings](70-settings.md). The E-STOP and Feed Hold
keys are fixed and cannot be changed.

The defaults are listed below. `Ctrl` is `Cmd` on macOS. Commands not listed have no default
key.

## File and Edit

These keys work on every page.

| Shortcut | Action |
|----------|--------|
| **Ctrl+N** | New 2D design |
| **Ctrl+O** | Open file |
| **Ctrl+S** | Save |
| **Ctrl+Shift+S** | Save As |
| **Ctrl+Shift+I** | Import image |
| **Ctrl+R** / **F5** | Run: preview the program in the Visualizer (sends nothing to the machine) |
| **Ctrl+Q** | Quit |
| **Ctrl+Z** | Undo |
| **Ctrl+Y** / **Ctrl+Shift+Z** | Redo |
| **Ctrl+X** | Cut |
| **Ctrl+C** | Copy |
| **Ctrl+V** | Paste |
| **F1** | Documentation |

## Machine

| Shortcut | Action |
|----------|--------|
| **Ctrl+Shift+K** | E-STOP (soft reset). Works on every page, also while a text field has the keyboard |
| **!** or **Ctrl+Shift+H** | Feed Hold. `Ctrl+Shift+H` also works while a text field has the keyboard |
| **Shift+`** | Cycle Start (resume). Machine Control only; this types `~` only on US-style keyboard layouts |
| **Ctrl+Home** | Home |
| **Ctrl+U** | Unlock |

Start Streaming, Pause Streaming and Stop Streaming have no default key. Escape is not bound
to any machine command.

## Jog

Numeric keypad keys. They work only while Machine Control has the keyboard.

| Shortcut | Action |
|----------|--------|
| **6** / **4** | Jog X+ / X- |
| **8** / **2** | Jog Y+ / Y- |
| **9** / **3** | Jog Z+ / Z- |

> Each press jogs one step of the selected step size. In continuous mode, holding the key jogs
> until you release it. See [Machine Control](10-machine-control.md) for step sizes.

## Visualizer

These keys work while the Visualizer has the keyboard.

| Shortcut | Action |
|----------|--------|
| **F** | Zoom to fit |
| **+** / **-** | Zoom in / out |
| **Ctrl+0** | Reset view |

## Designer Canvas

These keys work while the designer canvas has the keyboard.

| Shortcut | Action |
|----------|--------|
| **F** | Zoom to fit |
| **+** or **=** / **-** | Zoom in / out |
| **R** | Reset view |
| **D** | Fit to device working area |
| **Ctrl+I** | Import image |
| **Alt+L** / **Alt+R** | Align left / right |
| **Alt+T** / **Alt+O** | Align top / bottom |
| **Alt+C** / **Alt+V** | Align center X / center Y |
| **Ctrl+G** | Group |
| **Ctrl+Shift+G** | Ungroup |
| **Ctrl+D** | Duplicate |
| **Delete** / **Backspace** | Delete the selected shapes |
| **Escape** | Cancel the polyline being drawn, or deselect all |
| **Enter** | Finish the polyline (Polyline tool) |
