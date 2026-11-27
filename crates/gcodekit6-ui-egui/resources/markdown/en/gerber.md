# Gerber to G-Code

The Gerber to G-Code tool allows you to convert standard Gerber files (commonly used for PCB manufacturing) into G-Code for CNC isolation routing.

## Usage

1.  **Select Directory**: Click "Browse..." to select the folder containing your Gerber files. The tool will automatically detect and map common layer files (e.g., Top Copper, Drill Holes).
2.  **Layer Type**: Select the layer you want to process from the dropdown. The detected file for that layer will be shown below. The **Board Outline** layer cuts around the outside of the board edge, offset by the tool radius.
3.  **Parameters**:
    *   **Board Width/Height**: Dimensions of your PCB stock.
    *   **Offset X/Y**: Shift the origin of the G-Code.
    *   **Feed Rate**: Cutting speed in mm/min.
    *   **Spindle Speed**: Spindle RPM.
    *   **Cut Depth**: Z-depth for the cut (negative for cutting into material).
    *   **Outline Step Down**: For the Board Outline layer, the most Z drops per pass on the way down to Cut Depth (default 0.5 mm).
    *   **Safe Z**: Retract height for rapid moves.
    *   **Tool Diameter**: Diameter of your engraving bit (V-bit or end mill).
    *   **Isolation Width**: Additional width to clear around traces.
    *   **Remove Excess Copper**: (Rubout) If checked, generates toolpaths to remove all copper not part of the traces.
    *   **Use Board Outline for Rubout**: Limits the rubout to the board outline file. Without it, or when no outline file is found, the rubout covers the copper bounds.
4.  **Alignment Holes**:
    *   **Generate Alignment Holes**: Adds drilling operations for alignment pins.
    *   **Hole Diameter**: Diameter of the alignment holes.
    *   **Margin**: Distance from the board edge to the alignment holes.
    *   **Drill Depth (Z)**: Depth of the alignment holes and of the Drill Holes layer (negative, through the board).

## Drill Holes

The Drill Holes layer reads the Excellon drill file (`.drl`, `.xln`, `.exc`, a name with "drill", or a `.txt` that starts with `M48`) and drills every hole to **Drill Depth (Z)** at the feed rate. The drills are used smallest first; before each one the spindle stops and the program pauses (`M0`) with a comment naming the drill to load. Resume once it is fitted. Slots are cut with the drill from end to end. The holes use the same Offset X/Y as the copper layers, so they line up with the isolation cut.

## Output

Click **Generate** to create the toolpath, then **Send to Editor** to load it into the G-Code Editor, or **Preview** to load it and show it in the Visualizer.
