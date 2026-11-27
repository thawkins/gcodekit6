# Tool Library & Materials

## Tool Library

The **CNC Tools** page stores your cutting tools. It lists standard tools and your own custom tools, searchable by name, ID, type and diameter, and filterable by type, material and diameter.

### Tool Properties

The edit form has four tabs: Basic Info, Geometry, Manufacturer and Notes. Its fields include:

| Field | Description |
|-------|-------------|
| **Tool Number:** | Reference number for the tool |
| **Name:** | Descriptive label |
| **Tool Type:** | The kind of tool (end mill, drill, V-bit, ...) |
| **Tool Material:** | What the tool is made of |
| **Coating:** | Tool coating |
| **Diameter (mm):** | Cutting diameter |
| **Shaft Diameter (mm):** | Shank diameter |
| **Flute Length (mm):** | Length of the cutting flutes |
| **Length (mm):** | Overall length |
| **Number of Flutes:** | Number of cutting flutes |
| **Corner Radius (mm):** | Only for corner radius end mills |
| **Tip Angle (deg):** | Used for drills and V-bits |
| **Manufacturer:** | Maker of the tool |
| **Part Number:** | Maker's part number |

### Managing Tools

- **New Tool** — Create a new custom tool and enter its properties
- **Edit Tool** — Select a tool from the list, modify its values, then **Save**
- **Delete** — Remove a custom tool (standard tools cannot be deleted)
- **Import GTC (.zip)...** / **Import GTC (.json)...** — Import a tool catalog
- **Export Custom Tools...** — Export your custom tools to a JSON file
- **Reset Custom Tools...** — Remove all custom and imported tools

Custom tools are saved in `custom_tools.json` in the settings folder and are available across sessions.

### Using Tools

The [Speeds and Feeds Calculator](50-camtools.md) picks its tool from this library. The [Designer](40-designer.md) and the other [CAM Tools](50-camtools.md) take the tool diameter as a field of their own.

## Materials Database

The **Materials Manager** stores material properties, used by the [Speeds and Feeds Calculator](50-camtools.md).

### Material Properties

The edit form has five tabs: Basic Info, Properties, Machining, Safety and Notes. Its fields include:

| Field | Description |
|-------|-------------|
| **Name:**, **Category:**, **Subcategory:** | What the material is (e.g., "Hardwood", "Alloy") |
| **Density (kg/m3):** | Density |
| **Machinability (1-10):** | How easy the material is to cut |
| **Tensile Strength (MPa):** | Strength |
| **Melting Point (deg C):** | Melting point |
| **Chip Type:** | The chips the material forms |
| **Heat Sensitivity:** | How sensitive the material is to heat |
| **Abrasiveness:** | How abrasive the material is to tools |
| **Surface Finish:** | The finish to expect |
| **Dust Hazard:**, **Fume Hazard:** | Safety hazards |
| **Coolant Required** | Whether coolant is needed |

### Managing Materials

- **New Material** — Define a new custom material
- **Edit Material** — Modify an existing material, then **Save**
- **Delete** — Remove a custom material

### Built-in Materials

GCodeKit5 includes a set of standard materials. Saving a standard material stores a custom copy; deleting that copy brings the standard material back.

## See Also

- [CAM Tools](50-camtools.md) — Using tools in G-code generation
- [Designer](40-designer.md) — Toolpath generation from designs
- [Settings](70-settings.md) — Application configuration
