//! Spoilboard Surfacing Tool

use crate::ui::gtk::file_dialog::FileDialogHelpers;
use gtk4::prelude::*;
use gtk4::{
    Align, Box, Button, CheckButton, Entry, Label, Orientation, Paned, ScrolledWindow, Stack,
};
use libadwaita::prelude::*;
use libadwaita::{ActionRow, PreferencesGroup};
use serde_json;
use std::cell::Cell;
use std::rc::Rc;

use super::CamToolsView;
use super::common::{
    create_dimension_row, load_preset_json, preset_length, save_preset_json,
    set_paned_initial_fraction,
};
use crate::device_status;
use crate::t;
use crate::ui::gtk::help_browser;
use gcodekit5_camtools::spoilboard_surfacing::{
    SpoilboardSurfacingGenerator, SpoilboardSurfacingParameters,
};
use gcodekit5_core::units;
use gcodekit5_settings::SettingsController;

/// Initial spoilboard width (mm).
pub const WIDTH_MM: f64 = 400.0;
/// Initial spoilboard height (mm).
pub const HEIGHT_MM: f64 = 300.0;
/// Initial tool diameter (mm).
pub const TOOL_DIAMETER_MM: f64 = 25.0;
/// Initial cut depth (mm).
pub const CUT_DEPTH_MM: f64 = 0.5;
/// Initial safe Z height (mm).
pub const SAFE_Z_MM: f64 = 5.0;
/// Initial feed rate entry text (mm/min).
pub const FEED_RATE: &str = "1000";
/// Initial spindle speed entry text (RPM).
pub const SPINDLE_SPEED: &str = "18000";
/// Initial stepover entry text (percent of the tool diameter).
pub const STEPOVER_PERCENT: &str = "40";

/// The text of the surfacing page's fields, as the page reads them on Generate.
#[derive(Debug, Clone, PartialEq)]
pub struct SpoilboardSurfacingEntries {
    /// Width, a length in the measurement system.
    pub width: String,
    /// Height, a length.
    pub height: String,
    /// Tool diameter, a length.
    pub tool_diameter: String,
    /// Feed rate (mm/min).
    pub feed_rate: String,
    /// Spindle speed (RPM).
    pub spindle_speed: String,
    /// Cut depth, a length.
    pub cut_depth: String,
    /// Stepover (percent).
    pub stepover_percent: String,
    /// Safe Z height, a length.
    pub safe_z: String,
    /// Home Device Before Start.
    pub home_before: bool,
}

impl SpoilboardSurfacingEntries {
    /// The page's initial entries in `system`.
    ///
    /// # Arguments
    /// * `system` - The measurement system the lengths are shown in.
    pub fn defaults(system: units::MeasurementSystem) -> Self {
        let len = |mm: f64| units::format_length(mm as f32, system);
        Self {
            width: len(WIDTH_MM),
            height: len(HEIGHT_MM),
            tool_diameter: len(TOOL_DIAMETER_MM),
            feed_rate: FEED_RATE.to_string(),
            spindle_speed: SPINDLE_SPEED.to_string(),
            cut_depth: len(CUT_DEPTH_MM),
            stepover_percent: STEPOVER_PERCENT.to_string(),
            safe_z: len(SAFE_Z_MM),
            home_before: false,
        }
    }
}

/// Reads the page's entries into generator parameters; a field that does not
/// parse takes its default value (the page's behaviour).
///
/// # Arguments
/// * `e` - The entries' text.
/// * `system` - The measurement system the lengths are written in.
pub fn spoilboard_surfacing_parameters(
    e: &SpoilboardSurfacingEntries,
    system: units::MeasurementSystem,
) -> SpoilboardSurfacingParameters {
    SpoilboardSurfacingParameters {
        width: units::parse_length(&e.width, system).unwrap_or(400.0) as f64,
        height: units::parse_length(&e.height, system).unwrap_or(300.0) as f64,
        tool_diameter: units::parse_length(&e.tool_diameter, system).unwrap_or(25.0) as f64,
        feed_rate: e.feed_rate.parse().unwrap_or(1000.0),
        spindle_speed: e.spindle_speed.parse().unwrap_or(18000.0),
        cut_depth: units::parse_length(&e.cut_depth, system).unwrap_or(0.5) as f64,
        stepover_percent: e.stepover_percent.parse().unwrap_or(40.0),
        safe_z: units::parse_length(&e.safe_z, system).unwrap_or(5.0) as f64,
        home_before_job: e.home_before,
    }
}

struct SpoilboardSurfacingWidgets {
    width: Entry,
    height: Entry,
    tool_diameter: Entry,
    feed_rate: Entry,
    spindle_speed: Entry,
    cut_depth: Entry,
    stepover_percent: Entry,
    safe_z: Entry,
    home_before: CheckButton,
}

pub struct SpoilboardSurfacingTool {
    content: Box,
}

impl SpoilboardSurfacingTool {
    pub fn new<F: Fn(String) + 'static>(
        stack: &Stack,
        settings: Rc<SettingsController>,
        on_generate: Rc<F>,
    ) -> Self {
        let content_box = Box::new(Orientation::Vertical, 0);

        // Header
        let header = Box::new(Orientation::Horizontal, 12);
        header.set_margin_top(12);
        header.set_margin_bottom(12);
        header.set_margin_start(12);
        header.set_margin_end(12);

        let back_btn = Button::builder().icon_name("go-previous-symbolic").build();
        let stack_clone = stack.clone();
        back_btn.connect_clicked(move |_| {
            stack_clone.set_visible_child_name("dashboard");
        });
        header.append(&back_btn);

        let title = Label::builder()
            .label("Spoilboard Surfacing")
            .css_classes(vec!["title-2"])
            .build();
        title.set_hexpand(true);
        title.set_halign(Align::Start);
        header.append(&title);
        header.append(&help_browser::make_help_button("spoilboard_surfacing"));
        content_box.append(&header);

        // Paned Layout
        let paned = Paned::new(Orientation::Horizontal);
        paned.set_hexpand(true);
        paned.set_vexpand(true);

        // Sidebar (40%)
        let sidebar = Box::new(Orientation::Vertical, 12);
        sidebar.add_css_class("sidebar");
        sidebar.set_margin_top(24);
        sidebar.set_margin_bottom(24);
        sidebar.set_margin_start(24);
        sidebar.set_margin_end(24);

        let title_label = Label::builder()
            .label("Spoilboard Surfacing")
            .css_classes(vec!["title-3"])
            .halign(Align::Start)
            .build();
        sidebar.append(&title_label);

        let desc = Label::builder()
            .label("Generate G-code for surfacing your CNC spoilboard to ensure a flat, level work surface.")
            .css_classes(vec!["body"])
            .wrap(true)
            .halign(Align::Start)
            .build();
        sidebar.append(&desc);

        // Content Area
        let right_panel = Box::new(Orientation::Vertical, 0);
        let scroll_content = Box::new(Orientation::Vertical, 0);
        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .child(&scroll_content)
            .build();

        // Create widgets
        let (width_row, width, width_unit) = create_dimension_row("Width:", WIDTH_MM, &settings);
        let (height_row, height, height_unit) =
            create_dimension_row("Height:", HEIGHT_MM, &settings);
        let (tool_diameter_row, tool_diameter, tool_diameter_unit) =
            create_dimension_row("Tool Diameter:", TOOL_DIAMETER_MM, &settings);
        let feed_rate = Entry::builder()
            .text(FEED_RATE)
            .valign(Align::Center)
            .build();
        let spindle_speed = Entry::builder()
            .text(SPINDLE_SPEED)
            .valign(Align::Center)
            .build();
        let (cut_depth_row, cut_depth, cut_depth_unit) =
            create_dimension_row("Cut Depth:", CUT_DEPTH_MM, &settings);
        let stepover_percent = Entry::builder()
            .text(STEPOVER_PERCENT)
            .valign(Align::Center)
            .build();
        let (safe_z_row, safe_z, safe_z_unit) =
            create_dimension_row("Safe Z Height:", SAFE_Z_MM, &settings);
        let home_before = CheckButton::builder()
            .active(false)
            .valign(Align::Center)
            .build();

        // Groups
        let dim_group = PreferencesGroup::builder()
            .title("Spoilboard Dimensions")
            .build();
        dim_group.add(&width_row);
        dim_group.add(&height_row);
        scroll_content.append(&dim_group);

        let tool_group = PreferencesGroup::builder().title("Tool Settings").build();
        tool_group.add(&tool_diameter_row);
        tool_group.add(&cut_depth_row);
        tool_group.add(&Self::create_row("Stepover (%):", &stepover_percent));
        scroll_content.append(&tool_group);

        let machine_group = PreferencesGroup::builder()
            .title("Machine Settings")
            .build();
        machine_group.add(&Self::create_row("Feed Rate (mm/min):", &feed_rate));
        machine_group.add(&Self::create_row("Spindle Speed (RPM):", &spindle_speed));
        machine_group.add(&safe_z_row);

        let home_row = ActionRow::builder()
            .title("Home Device Before Start")
            .build();
        home_row.add_suffix(&home_before);
        machine_group.add(&home_row);

        scroll_content.append(&machine_group);

        right_panel.append(&scrolled);

        // Action Buttons
        let action_box = Box::new(Orientation::Horizontal, 12);
        action_box.set_margin_top(12);
        action_box.set_margin_bottom(12);
        action_box.set_margin_end(12);
        action_box.set_halign(Align::End);

        let load_btn = Button::with_label("Load");
        let save_btn = Button::with_label("Save");
        let cancel_btn = Button::with_label("Cancel");
        let generate_btn = Button::with_label("Generate");
        generate_btn.add_css_class("suggested-action");
        action_box.append(&load_btn);
        action_box.append(&save_btn);
        action_box.append(&cancel_btn);
        action_box.append(&generate_btn);
        right_panel.append(&action_box);

        paned.set_start_child(Some(&sidebar));
        paned.set_end_child(Some(&right_panel));
        // Initial ratio only; do not fight user resizing.
        set_paned_initial_fraction(&paned, 0.40);

        content_box.append(&paned);

        let widgets = Rc::new(SpoilboardSurfacingWidgets {
            width,
            height,
            tool_diameter,
            feed_rate,
            spindle_speed,
            cut_depth,
            stepover_percent,
            safe_z,
            home_before,
        });

        // Unit update listener
        {
            let w = widgets.clone();
            let width_unit = width_unit.clone();
            let height_unit = height_unit.clone();
            let tool_diameter_unit = tool_diameter_unit.clone();
            let cut_depth_unit = cut_depth_unit.clone();
            let safe_z_unit = safe_z_unit.clone();

            let last_system = Rc::new(Cell::new(
                settings.persistence.borrow().config().ui.measurement_system,
            ));

            settings.on_setting_changed(move |key, value| {
                if key == "measurement_system" {
                    // Listeners fire when the setting is edited, before it is saved to
                    // persistence, so the new system must be taken from `value`.
                    let Ok(new_system) = value.parse::<units::MeasurementSystem>() else {
                        return;
                    };
                    let old_system = last_system.get();

                    if new_system != old_system {
                        let unit_label = units::get_unit_label(new_system);

                        let update_entry = |entry: &Entry, label: &Label| {
                            if let Ok(val_mm) = units::parse_length(&entry.text(), old_system) {
                                entry.set_text(&units::format_length(val_mm, new_system));
                            }
                            label.set_text(unit_label);
                        };

                        update_entry(&w.width, &width_unit);
                        update_entry(&w.height, &height_unit);
                        update_entry(&w.tool_diameter, &tool_diameter_unit);
                        update_entry(&w.cut_depth, &cut_depth_unit);
                        update_entry(&w.safe_z, &safe_z_unit);

                        last_system.set(new_system);
                    }
                }
            });
        }

        // Generate button
        let w_gen = widgets.clone();
        let settings_gen = settings.clone();
        generate_btn.connect_clicked(move |_| {
            let home_before = w_gen.home_before.is_active();
            let system = settings_gen
                .persistence
                .borrow()
                .config()
                .ui
                .measurement_system;

            // Spoilboard surfacing requires Z axis
            if device_status::get_active_num_axes() < 3 {
                CamToolsView::show_error_dialog(
                    "Insufficient Axes",
                    "Spoilboard Surfacing requires at least 3 axes (X, Y, Z). The active device has fewer than 3 axes configured.",
                );
                return;
            }

            let entries = SpoilboardSurfacingEntries {
                width: w_gen.width.text().to_string(),
                height: w_gen.height.text().to_string(),
                tool_diameter: w_gen.tool_diameter.text().to_string(),
                feed_rate: w_gen.feed_rate.text().to_string(),
                spindle_speed: w_gen.spindle_speed.text().to_string(),
                cut_depth: w_gen.cut_depth.text().to_string(),
                stepover_percent: w_gen.stepover_percent.text().to_string(),
                safe_z: w_gen.safe_z.text().to_string(),
                home_before,
            };
            let params = spoilboard_surfacing_parameters(&entries, system);

            let generator = SpoilboardSurfacingGenerator::new(params);
            match generator.generate() {
                Ok(gcode) => {
                    on_generate(gcode);
                }
                Err(e) => {
                    CamToolsView::show_error_dialog(
                        "Generation Failed",
                        &format!("Failed to generate surfacing toolpath: {}", e),
                    );
                }
            }
        });

        // Save/Load/Cancel
        let w_save = widgets.clone();
        let settings_save = settings.clone();
        save_btn.connect_clicked(move |_| {
            Self::save_params(&w_save, &settings_save);
        });

        let w_load = widgets.clone();
        let settings_load = settings.clone();
        load_btn.connect_clicked(move |_| {
            Self::load_params(&w_load, &settings_load);
        });

        let stack_clone_cancel = stack.clone();
        cancel_btn.connect_clicked(move |_| {
            stack_clone_cancel.set_visible_child_name("dashboard");
        });

        Self {
            content: content_box,
        }
    }

    pub fn widget(&self) -> &Box {
        &self.content
    }

    fn create_row(title: &str, widget: &impl IsA<gtk4::Widget>) -> ActionRow {
        let row = ActionRow::builder().title(title).build();
        row.add_suffix(widget);
        row
    }

    fn save_params(w: &SpoilboardSurfacingWidgets, settings: &Rc<SettingsController>) {
        let dialog = crate::ui::gtk::file_dialog::new_file_dialog("Save Parameters");
        dialog.set_initial_name(Some("surfacing_params.json"));

        let system = settings.persistence.borrow().config().ui.measurement_system;
        let values = (|| -> Result<_, String> {
            Ok((
                preset_length("width", &w.width, system)?,
                preset_length("height", &w.height, system)?,
                preset_length("tool diameter", &w.tool_diameter, system)?,
                w.feed_rate.text().to_string(),
                w.spindle_speed.text().to_string(),
                preset_length("cut depth", &w.cut_depth, system)?,
                w.stepover_percent.text().to_string(),
                preset_length("safe z", &w.safe_z, system)?,
            ))
        })();
        let w_clone = match values {
            Ok(values) => Rc::new(values),
            Err(e) => {
                CamToolsView::show_error_dialog(&t!("Save Error"), &e);
                return;
            }
        };

        dialog.pick_save(None::<&gtk4::Window>, move |file| {
            if let Some(path) = file.path() {
                let json = serde_json::json!({
                    "width": w_clone.0,
                    "height": w_clone.1,
                    "tool_diameter": w_clone.2,
                    "feed_rate": w_clone.3,
                    "spindle_speed": w_clone.4,
                    "cut_depth": w_clone.5,
                    "stepover_percent": w_clone.6,
                    "safe_z": w_clone.7,
                });
                save_preset_json(&path, &json);
            }
        });
    }

    fn load_params(w: &SpoilboardSurfacingWidgets, settings: &Rc<SettingsController>) {
        let dialog = crate::ui::gtk::file_dialog::new_file_dialog("Load Parameters");

        let w_clone = Rc::new((
            w.width.clone(),
            w.height.clone(),
            w.tool_diameter.clone(),
            w.feed_rate.clone(),
            w.spindle_speed.clone(),
            w.cut_depth.clone(),
            w.stepover_percent.clone(),
            w.safe_z.clone(),
        ));
        let settings_clone = settings.clone();

        dialog.pick_open(None::<&gtk4::Window>, move |file| {
            if let Some(path) = file.path()
                && let Some(params) = load_preset_json::<serde_json::Value>(&path)
            {
                {
                    let system = settings_clone
                        .persistence
                        .borrow()
                        .config()
                        .ui
                        .measurement_system;
                    if let Some(v) = params.get("width").and_then(|v| v.as_f64()) {
                        w_clone.0.set_text(&units::format_length(v as f32, system));
                    }
                    if let Some(v) = params.get("height").and_then(|v| v.as_f64()) {
                        w_clone.1.set_text(&units::format_length(v as f32, system));
                    }
                    if let Some(v) = params.get("tool_diameter").and_then(|v| v.as_f64()) {
                        w_clone.2.set_text(&units::format_length(v as f32, system));
                    }
                    if let Some(v) = params.get("feed_rate").and_then(|v| v.as_str()) {
                        w_clone.3.set_text(v);
                    }
                    if let Some(v) = params.get("spindle_speed").and_then(|v| v.as_str()) {
                        w_clone.4.set_text(v);
                    }
                    if let Some(v) = params.get("cut_depth").and_then(|v| v.as_f64()) {
                        w_clone.5.set_text(&units::format_length(v as f32, system));
                    }
                    if let Some(v) = params.get("stepover_percent").and_then(|v| v.as_str()) {
                        w_clone.6.set_text(v);
                    }
                    if let Some(v) = params.get("safe_z").and_then(|v| v.as_f64()) {
                        w_clone.7.set_text(&units::format_length(v as f32, system));
                    }
                }
            }
        });
    }
}

// Spoilboard Grid Tool
