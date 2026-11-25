//! Drill Press Tool

use crate::ui::gtk::file_dialog::FileDialogHelpers;
use gtk4::prelude::*;
use gtk4::{
    Align, Box, Button, CheckButton, Entry, Image, Label, Orientation, Paned, ScrolledWindow,
    Stack, accessible::Property as AccessibleProperty,
};
use libadwaita::prelude::*;
use libadwaita::{ActionRow, PreferencesGroup};
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
use crate::ui::gtk::machine_control::MachineControlView;
use gcodekit5_camtools::drill_press::{DrillPressGenerator, DrillPressParameters};
use gcodekit5_core::units;
use gcodekit5_settings::SettingsController;

/// Initial hole diameter (mm).
pub const HOLE_DIAMETER_MM: f64 = 10.0;
/// Initial tool diameter (mm).
pub const TOOL_DIAMETER_MM: f64 = 6.0;
/// Initial top Z, the material surface (mm).
pub const TOP_Z_MM: f64 = 0.0;
/// Initial bottom Z, the hole's depth (mm).
pub const BOTTOM_Z_MM: f64 = -10.0;
/// Initial peck depth (mm).
pub const PECK_DEPTH_MM: f64 = 2.0;
/// Initial safe Z (mm).
pub const SAFE_Z_MM: f64 = 5.0;
/// Initial plunge rate entry text (mm/min).
pub const PLUNGE_RATE: &str = "100";
/// Initial feed rate entry text (mm/min).
pub const FEED_RATE: &str = "500";
/// Initial spindle speed entry text (RPM).
pub const SPINDLE_SPEED: &str = "10000";

/// The page's initial hole centre (mm): the middle of the machine's travel
/// (GRBL `$130` / `$131`), taking 200 mm for an axis whose travel is unknown.
pub fn default_center() -> (f64, f64) {
    let x_max = device_status::get_grbl_setting_numeric(130).unwrap_or(200.0);
    let y_max = device_status::get_grbl_setting_numeric(131).unwrap_or(200.0);
    (x_max / 2.0, y_max / 2.0)
}

/// The text of the drill press page's fields, as the page reads them on Drill.
#[derive(Debug, Clone, PartialEq)]
pub struct DrillPressEntries {
    /// Hole diameter, a length in the measurement system.
    pub hole_diameter: String,
    /// Tool diameter, a length.
    pub tool_diameter: String,
    /// Top Z, a length.
    pub top_z: String,
    /// Bottom Z, a length.
    pub bottom_z: String,
    /// Peck depth, a length.
    pub peck_depth: String,
    /// Safe Z, a length.
    pub safe_z: String,
    /// Centre X, a length.
    pub x: String,
    /// Centre Y, a length.
    pub y: String,
    /// Plunge rate (mm/min).
    pub plunge_rate: String,
    /// Feed rate (mm/min).
    pub feed_rate: String,
    /// Spindle speed (RPM).
    pub spindle_speed: String,
    /// Home Device Before Start.
    pub home_before: bool,
}

impl DrillPressEntries {
    /// The page's initial entries in `system`, with the hole centred at `center` (mm).
    ///
    /// # Arguments
    /// * `system` - The measurement system the lengths are shown in.
    /// * `center` - The hole centre (mm), see [`default_center`].
    pub fn defaults(system: units::MeasurementSystem, center: (f64, f64)) -> Self {
        let len = |mm: f64| units::format_length(mm as f32, system);
        Self {
            hole_diameter: len(HOLE_DIAMETER_MM),
            tool_diameter: len(TOOL_DIAMETER_MM),
            top_z: len(TOP_Z_MM),
            bottom_z: len(BOTTOM_Z_MM),
            peck_depth: len(PECK_DEPTH_MM),
            safe_z: len(SAFE_Z_MM),
            x: len(center.0),
            y: len(center.1),
            plunge_rate: PLUNGE_RATE.to_string(),
            feed_rate: FEED_RATE.to_string(),
            spindle_speed: SPINDLE_SPEED.to_string(),
            home_before: false,
        }
    }
}

/// Reads the page's entries into generator parameters.
///
/// # Arguments
/// * `e` - The entries' text.
/// * `system` - The measurement system the lengths are written in.
///
/// # Errors
/// The names of the fields whose text is not a length or a number.
pub fn drill_press_parameters(
    e: &DrillPressEntries,
    system: units::MeasurementSystem,
) -> Result<DrillPressParameters, Vec<&'static str>> {
    let mut invalid: Vec<&'static str> = Vec::new();
    let mut len = |text: &str, name: &'static str| -> f64 {
        match units::parse_length(text, system) {
            Ok(v) => v as f64,
            Err(_) => {
                invalid.push(name);
                0.0
            }
        }
    };
    let hole_diameter = len(&e.hole_diameter, "Hole Diameter");
    let tool_diameter = len(&e.tool_diameter, "Tool Diameter");
    let top_z = len(&e.top_z, "Top Z");
    let bottom_z = len(&e.bottom_z, "Bottom Z");
    let peck_depth = len(&e.peck_depth, "Peck Depth");
    let safe_z = len(&e.safe_z, "Safe Z");
    let x = len(&e.x, "Center X");
    let y = len(&e.y, "Center Y");
    let mut num = |text: &str, name: &'static str| -> f64 {
        match text.trim().parse::<f64>() {
            Ok(v) => v,
            Err(_) => {
                invalid.push(name);
                0.0
            }
        }
    };
    let plunge_rate = num(&e.plunge_rate, "Plunge Rate");
    let feed_rate = num(&e.feed_rate, "Feed Rate");
    let spindle_speed = num(&e.spindle_speed, "Spindle Speed");
    if !invalid.is_empty() {
        return Err(invalid);
    }
    Ok(DrillPressParameters {
        hole_diameter,
        tool_diameter,
        top_z,
        bottom_z,
        peck_depth,
        plunge_rate,
        feed_rate,
        spindle_speed,
        safe_z,
        x,
        y,
        home_before_job: e.home_before,
    })
}

struct DrillPressWidgets {
    hole_diameter: Entry,
    tool_diameter: Entry,
    top_z: Entry,
    bottom_z: Entry,
    peck_depth: Entry,
    plunge_rate: Entry,
    feed_rate: Entry,
    spindle_speed: Entry,
    safe_z: Entry,
    x: Entry,
    y: Entry,
    home_before: CheckButton,
}

pub struct DrillPressTool {
    content: Box,
}

impl DrillPressTool {
    pub fn new<F: Fn(String) + 'static>(
        stack: &Stack,
        settings: Rc<SettingsController>,
        machine_control: Option<MachineControlView>,
        _on_generate: Rc<F>,
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
            .label("Drill Press")
            .css_classes(vec!["title-2"])
            .build();
        title.set_hexpand(true);
        title.set_halign(Align::Start);
        header.append(&title);
        header.append(&help_browser::make_help_button("drill_press"));
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
            .label("Drill Press")
            .css_classes(vec!["title-3"])
            .halign(Align::Start)
            .build();
        sidebar.append(&title_label);

        let desc = Label::builder()
            .label("Emulate a drill press on your CNC. Supports standard drilling, peck drilling for chip clearing, and helical interpolation for holes larger than the tool.")
            .css_classes(vec!["body"])
            .wrap(true)
            .halign(Align::Start)
            .build();
        sidebar.append(&desc);

        // Sidebar Actions
        let sidebar_actions = Box::new(Orientation::Horizontal, 12);
        sidebar_actions.set_margin_top(24);
        sidebar_actions.set_halign(Align::Center);

        let drill_btn = Button::with_label("Drill");
        drill_btn.add_css_class("suggested-action");
        drill_btn.set_width_request(112);
        drill_btn.set_height_request(80);

        // eStop
        let estop_btn = Button::new();
        let estop_content = Box::new(Orientation::Vertical, 2);
        estop_content.set_halign(Align::Center);
        estop_content.set_valign(Align::Center);
        let estop_picture = Image::from_resource("/com/gcodekit5/images/eStop2.png");
        estop_picture.set_pixel_size(64);
        let estop_text = Label::new(Some(&t!("E-STOP")));
        estop_text.add_css_class("title-4");
        estop_content.append(&estop_picture);
        estop_content.append(&estop_text);
        estop_btn.set_child(Some(&estop_content));
        estop_btn.set_tooltip_text(Some(&t!("Emergency stop (soft reset Ctrl-X)")));
        estop_btn.update_property(&[AccessibleProperty::Label(&t!("Emergency stop"))]);

        estop_btn.add_css_class("estop-big");
        estop_btn.set_width_request(112);
        estop_btn.set_height_request(80);

        sidebar_actions.append(&drill_btn);
        sidebar_actions.append(&estop_btn);
        sidebar.append(&sidebar_actions);

        // Offline Message
        let offline_msg = Label::builder()
            .label("Device connection required to use Drill Press")
            .css_classes(vec!["title-3", "error"])
            .wrap(true)
            .justify(gtk4::Justification::Center)
            .halign(Align::Center)
            .valign(Align::Center)
            .vexpand(true)
            .build();
        sidebar.append(&offline_msg);

        // Initial visibility
        let connected = device_status::get_status().is_connected;
        sidebar_actions.set_visible(connected);
        offline_msg.set_visible(!connected);

        // Connection status polling: weak refs only, stopped when the page is destroyed.
        let sidebar_actions_weak = sidebar_actions.downgrade();
        super::common::poll_connection_status(&offline_msg, move |offline_msg, connected| {
            if let Some(actions) = sidebar_actions_weak.upgrade() {
                actions.set_visible(connected);
            }
            offline_msg.set_visible(!connected);
        });

        // Content Area
        let right_panel = Box::new(Orientation::Vertical, 0);
        let scroll_content = Box::new(Orientation::Vertical, 0);
        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .child(&scroll_content)
            .build();

        // Create widgets
        let (hole_dia_row, hole_diameter, hole_dia_unit) =
            create_dimension_row("Hole Diameter:", HOLE_DIAMETER_MM, &settings);
        let (tool_dia_row, tool_diameter, tool_dia_unit) =
            create_dimension_row("Tool Diameter:", TOOL_DIAMETER_MM, &settings);
        let (top_z_row, top_z, top_z_unit) =
            create_dimension_row("Top Z (Surface):", TOP_Z_MM, &settings);
        let (bottom_z_row, bottom_z, bottom_z_unit) =
            create_dimension_row("Bottom Z (Depth):", BOTTOM_Z_MM, &settings);
        let (peck_row, peck_depth, peck_unit) =
            create_dimension_row("Peck Depth (0 for none):", PECK_DEPTH_MM, &settings);
        let (safe_z_row, safe_z, safe_z_unit) =
            create_dimension_row("Safe Z:", SAFE_Z_MM, &settings);

        // Default X/Y to device center if available
        let (center_x, center_y) = default_center();

        let (x_row, x, x_unit) = create_dimension_row("Center X:", center_x, &settings);
        let (y_row, y, y_unit) = create_dimension_row("Center Y:", center_y, &settings);

        let plunge_rate = Entry::builder()
            .text(PLUNGE_RATE)
            .valign(Align::Center)
            .build();
        let feed_rate = Entry::builder()
            .text(FEED_RATE)
            .valign(Align::Center)
            .build();
        let spindle_speed = Entry::builder()
            .text(SPINDLE_SPEED)
            .valign(Align::Center)
            .build();

        let home_before = CheckButton::builder()
            .active(false)
            .valign(Align::Center)
            .build();

        // Groups
        let hole_group = PreferencesGroup::builder().title("Hole Geometry").build();
        hole_group.add(&hole_dia_row);
        hole_group.add(&tool_dia_row);
        hole_group.add(&x_row);
        hole_group.add(&y_row);
        scroll_content.append(&hole_group);

        let depth_group = PreferencesGroup::builder()
            .title("Depth and Pecking")
            .build();
        depth_group.add(&top_z_row);
        depth_group.add(&bottom_z_row);
        depth_group.add(&peck_row);
        scroll_content.append(&depth_group);

        let machine_group = PreferencesGroup::builder()
            .title("Machine Settings")
            .build();
        machine_group.add(&Self::create_row("Plunge Rate (mm/min):", &plunge_rate));
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
        action_box.append(&load_btn);
        action_box.append(&save_btn);
        action_box.append(&cancel_btn);
        right_panel.append(&action_box);

        paned.set_start_child(Some(&sidebar));
        paned.set_end_child(Some(&right_panel));
        set_paned_initial_fraction(&paned, 0.40);

        content_box.append(&paned);

        let widgets = Rc::new(DrillPressWidgets {
            hole_diameter,
            tool_diameter,
            top_z,
            bottom_z,
            peck_depth,
            plunge_rate,
            feed_rate,
            spindle_speed,
            safe_z,
            x,
            y,
            home_before,
        });

        // Unit update listener
        {
            let w = widgets.clone();
            let hole_dia_unit = hole_dia_unit.clone();
            let tool_dia_unit = tool_dia_unit.clone();
            let top_z_unit = top_z_unit.clone();
            let bottom_z_unit = bottom_z_unit.clone();
            let peck_unit = peck_unit.clone();
            let safe_z_unit = safe_z_unit.clone();
            let x_unit = x_unit.clone();
            let y_unit = y_unit.clone();

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

                        update_entry(&w.hole_diameter, &hole_dia_unit);
                        update_entry(&w.tool_diameter, &tool_dia_unit);
                        update_entry(&w.top_z, &top_z_unit);
                        update_entry(&w.bottom_z, &bottom_z_unit);
                        update_entry(&w.peck_depth, &peck_unit);
                        update_entry(&w.safe_z, &safe_z_unit);
                        update_entry(&w.x, &x_unit);
                        update_entry(&w.y, &y_unit);

                        last_system.set(new_system);
                    }
                }
            });
        }

        // Drill button
        let w_run = widgets.clone();
        let settings_run = settings.clone();
        let mc_run = machine_control.clone();
        drill_btn.connect_clicked(move |_| {
            let status = device_status::get_status();
            if !status.is_connected {
                CamToolsView::show_error_dialog(
                    "Device Offline",
                    "Please connect to a CNC machine before running the drill press tool.",
                );
                return;
            }

            // Check if laser mode is enabled ($32=1)
            if device_status::get_grbl_setting_numeric(32).unwrap_or(0.0) > 0.5 {
                CamToolsView::show_error_dialog(
                    "Laser Mode Enabled",
                    "The device is currently in Laser Mode ($32=1). Please switch to CNC mode to use the Drill Press tool.",
                );
                return;
            }

            let home_before = w_run.home_before.is_active();
            let system = settings_run
                .persistence
                .borrow()
                .config()
                .ui
                .measurement_system;

            // Drill press requires Z axis
            if device_status::get_active_num_axes() < 3 {
                CamToolsView::show_error_dialog(
                    "Insufficient Axes",
                    "The Drill Press tool requires at least 3 axes (X, Y, Z). The active device has fewer than 3 axes configured.",
                );
                return;
            }

            // This tool sends G-code straight to the machine, so invalid input must
            // abort instead of silently falling back to a default value.
            let entries = DrillPressEntries {
                hole_diameter: w_run.hole_diameter.text().to_string(),
                tool_diameter: w_run.tool_diameter.text().to_string(),
                top_z: w_run.top_z.text().to_string(),
                bottom_z: w_run.bottom_z.text().to_string(),
                peck_depth: w_run.peck_depth.text().to_string(),
                safe_z: w_run.safe_z.text().to_string(),
                x: w_run.x.text().to_string(),
                y: w_run.y.text().to_string(),
                plunge_rate: w_run.plunge_rate.text().to_string(),
                feed_rate: w_run.feed_rate.text().to_string(),
                spindle_speed: w_run.spindle_speed.text().to_string(),
                home_before,
            };
            let params = match drill_press_parameters(&entries, system) {
                Ok(params) => params,
                Err(invalid) => {
                    CamToolsView::show_error_dialog(
                        "Invalid Parameters",
                        &format!("Please correct: {}", invalid.join(", ")),
                    );
                    return;
                }
            };

            let generator = DrillPressGenerator::new(params);
            match generator.generate() {
                Ok(gcode) => {

                    if let Some(mc) = mc_run.as_ref() {
                        mc.start_job(&gcode);
                    } else {
                        CamToolsView::show_error_dialog(
                            "Execution Failed",
                            "Machine control interface not available.",
                        );
                    }
                }
                Err(e) => {
                    CamToolsView::show_error_dialog(
                        "Generation Failed",
                        &format!("Failed to generate drill press toolpath: {}", e),
                    );
                }
            }
        });

        // eStop button
        let mc_estop = machine_control.clone();
        estop_btn.connect_clicked(move |_| {
            if let Some(mc) = mc_estop.as_ref() {
                mc.emergency_stop();
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

    fn save_params(w: &DrillPressWidgets, settings: &Rc<SettingsController>) {
        let dialog = crate::ui::gtk::file_dialog::new_file_dialog("Save Parameters");
        dialog.set_initial_name(Some("drill_params.json"));

        let system = settings.persistence.borrow().config().ui.measurement_system;
        let length = |name: &str, e: &Entry| preset_length(name, e, system).map(f64::from);
        let number = |name: &str, e: &Entry| {
            e.text()
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("{name}: \"{}\" is not a number", e.text()))
        };
        let params = (|| -> Result<DrillPressParameters, String> {
            Ok(DrillPressParameters {
                hole_diameter: length("Hole diameter", &w.hole_diameter)?,
                tool_diameter: length("Tool diameter", &w.tool_diameter)?,
                top_z: length("Top Z", &w.top_z)?,
                bottom_z: length("Bottom Z", &w.bottom_z)?,
                peck_depth: length("Peck depth", &w.peck_depth)?,
                plunge_rate: number("Plunge rate", &w.plunge_rate)?,
                feed_rate: number("Feed rate", &w.feed_rate)?,
                spindle_speed: number("Spindle speed", &w.spindle_speed)?,
                safe_z: length("Safe Z", &w.safe_z)?,
                x: length("X", &w.x)?,
                y: length("Y", &w.y)?,
                home_before_job: w.home_before.is_active(),
            })
        })();
        let params = match params {
            Ok(params) => params,
            Err(e) => {
                CamToolsView::show_error_dialog(&t!("Save Error"), &e);
                return;
            }
        };

        dialog.pick_save(None::<&gtk4::Window>, move |file| {
            if let Some(path) = file.path() {
                save_preset_json(&path, &params);
            }
        });
    }

    fn load_params(w: &DrillPressWidgets, settings: &Rc<SettingsController>) {
        let dialog = crate::ui::gtk::file_dialog::new_file_dialog("Load Parameters");

        let home_before_clone = w.home_before.clone();
        let w_clone = Rc::new((
            w.hole_diameter.clone(),
            w.tool_diameter.clone(),
            w.top_z.clone(),
            w.bottom_z.clone(),
            w.peck_depth.clone(),
            w.plunge_rate.clone(),
            w.feed_rate.clone(),
            w.spindle_speed.clone(),
            w.safe_z.clone(),
            w.x.clone(),
            w.y.clone(),
        ));
        let settings_clone = settings.clone();

        dialog.pick_open(None::<&gtk4::Window>, move |file| {
            if let Some(path) = file.path()
                && let Some(params) = load_preset_json::<DrillPressParameters>(&path)
            {
                Self::apply_params(&w_clone, &params, &settings_clone);
                home_before_clone.set_active(params.home_before_job);
            }
        });
    }

    fn apply_params(
        w: &(
            Entry,
            Entry,
            Entry,
            Entry,
            Entry,
            Entry,
            Entry,
            Entry,
            Entry,
            Entry,
            Entry,
        ),
        p: &DrillPressParameters,
        settings: &Rc<SettingsController>,
    ) {
        let system = settings.persistence.borrow().config().ui.measurement_system;
        w.0.set_text(&units::format_length(p.hole_diameter as f32, system));
        w.1.set_text(&units::format_length(p.tool_diameter as f32, system));
        w.2.set_text(&units::format_length(p.top_z as f32, system));
        w.3.set_text(&units::format_length(p.bottom_z as f32, system));
        w.4.set_text(&units::format_length(p.peck_depth as f32, system));
        w.5.set_text(&p.plunge_rate.to_string());
        w.6.set_text(&p.feed_rate.to_string());
        w.7.set_text(&p.spindle_speed.to_string());
        w.8.set_text(&units::format_length(p.safe_z as f32, system));
        w.9.set_text(&units::format_length(p.x as f32, system));
        w.10.set_text(&units::format_length(p.y as f32, system));
    }
}
