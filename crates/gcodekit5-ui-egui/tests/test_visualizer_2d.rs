//! The visualizer's 2D view mode (GTK's 2D / 3D pair): the top view, locked; left drag pans;
//! the nav cube's view grid is hidden; a program loaded in 2D is fitted from the top; 3D
//! returns to the isometric view.

use egui_kittest::kittest::Queryable;
use gcodekit5_ui_egui::commands::Command;
use gcodekit5_ui_egui::panes::visualizer::{StdView, VisualizerPane};
use gcodekit5_ui_egui::workspace::{Pane, PaneContext};

use crate::test_visualizer::{load, pane_harness, sample};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn close(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

#[test]
fn test_2d_button_locks_the_top_view() {
    let mut harness = pane_harness(VisualizerPane::new());
    harness.run();
    assert!(!harness.state().is_2d(), "3D at start, as in GTK");
    assert!(harness.query_by_label("Front View").is_some());
    harness.get_by_label("2D").click();
    harness.run();
    assert!(harness.state().is_2d());
    assert!(close(
        harness.state().camera().angles(),
        StdView::Top.angles()
    ));
    assert!(
        harness.query_by_label("Front View").is_none(),
        "the view grid is hidden in 2D"
    );
    assert!(
        harness
            .query_all_by_label("Fit to Device Working Area")
            .count()
            == 2,
        "the floating column stays"
    );
    // Other views are refused while in 2D.
    let tr = gcodekit5_app::i18n::Translator::new("en");
    let mut requested = Vec::new();
    let mut ctx = PaneContext::new(&tr, &mut requested);
    assert!(harness.state_mut().command(Command::ViewIso, &mut ctx));
    assert!(close(
        harness.state().camera().angles(),
        StdView::Top.angles()
    ));
    harness.get_by_label("3D").click();
    harness.run();
    assert!(!harness.state().is_2d());
    assert!(close(
        harness.state().camera().angles(),
        StdView::IsoSe.angles()
    ));
}

/// A left-button drag from `from` to `to` over the harness.
pub(crate) fn drag<S>(
    harness: &mut egui_kittest::Harness<'_, S>,
    from: egui::Pos2,
    to: egui::Pos2,
) {
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    harness.event(egui::Event::PointerMoved(from));
    harness.step();
    harness.event(press(from, true));
    harness.step();
    for t in 1..=4 {
        harness.event(egui::Event::PointerMoved(
            from + (to - from) * t as f32 / 4.0,
        ));
        harness.step();
    }
    harness.event(press(to, false));
    harness.run();
}

#[test]
fn test_2d_drag_pans_instead_of_orbiting() {
    let mut pane = VisualizerPane::new();
    pane.set_2d(true);
    let mut harness = pane_harness(pane);
    harness.run();
    let before = harness.state().camera().target();
    let centre = harness.state().view_rect().center();
    drag(&mut harness, centre, centre + egui::vec2(40.0, 20.0));
    assert!(close(
        harness.state().camera().angles(),
        StdView::Top.angles()
    ));
    assert_ne!(harness.state().camera().target(), before, "the drag panned");
}

#[test]
fn test_program_loaded_in_2d_is_fitted_from_the_top() -> TestResult {
    let mut pane = VisualizerPane::new();
    pane.set_2d(true);
    let mut harness = pane_harness(pane);
    load(&mut harness, sample())?;
    assert!(close(
        harness.state().camera().angles(),
        StdView::Top.angles()
    ));
    let target = harness.state().camera().target();
    assert!((target.x - 20.0).abs() < 1e-3 && (target.y - 25.0).abs() < 1e-3);
    // Reset View in 2D comes back to the top view.
    harness.get_by_label("Reset View").click();
    harness.run();
    assert!(close(
        harness.state().camera().angles(),
        StdView::Top.angles()
    ));
    Ok(())
}
