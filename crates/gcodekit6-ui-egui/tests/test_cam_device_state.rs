//! Device facts reach the CAM pages through the workspace, not a process-wide static: the
//! active profile's axis count makes the Z-cutting pages refuse on a 2-axis machine.

use egui_kittest::kittest::Queryable;
use gcodekit6_devicedb::DeviceProfile;
use gcodekit6_ui_egui::panes::cam_tools::CamTool;

use crate::test_cam_drill_press::{open_tool, pane};
use crate::test_designer_files_app::frames;

/// Opens `tool` with a 2-axis profile active and presses Generate.
fn generate_on_two_axes(tool: CamTool) -> Option<String> {
    let mut h = open_tool(tool);
    let profile = DeviceProfile {
        num_axes: 2,
        ..DeviceProfile::default()
    };
    h.state_mut().workspace_mut().set_device(Some(&profile));
    frames(&mut h, 2);
    h.get_by_label("Generate").scroll_to_me();
    frames(&mut h, 2);
    h.get_by_label("Generate").click();
    frames(&mut h, 2);
    pane(&h).output().map(str::to_string)
}

#[test]
fn test_cam_device_state_two_axes_refuse_drill_press() {
    assert_eq!(generate_on_two_axes(CamTool::DrillPress), None);
}

#[test]
fn test_cam_device_state_two_axes_refuse_surfacing() {
    assert_eq!(generate_on_two_axes(CamTool::SpoilboardSurfacing), None);
}

/// The default 3-axis profile still generates, so the refusals above are the axis check.
#[test]
fn test_cam_device_state_three_axes_generate() {
    let mut h = open_tool(CamTool::DrillPress);
    h.state_mut()
        .workspace_mut()
        .set_device(Some(&DeviceProfile::default()));
    frames(&mut h, 2);
    h.get_by_label("Generate").click();
    frames(&mut h, 2);
    assert!(pane(&h).output().is_some());
}
