//! Gerber to G-Code Converter Tool

use crate::ui::gtk::common::text_dropdown::{TextDropDownExt, new_text_dropdown};
use crate::ui::gtk::file_dialog::FileDialogHelpers;
use gtk4::prelude::*;
use gtk4::{
    Align, Box, Button, CheckButton, DropDown, Entry, Label, Orientation, Paned, ScrolledWindow,
    Stack,
};
use libadwaita::prelude::*;
use libadwaita::{ActionRow, PreferencesGroup};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use tracing::warn;

use super::CamToolsView;
use super::common::{
    create_dimension_row, set_paned_initial_fraction, track_measurement_system, write_preset_file,
};
use crate::ui::gtk::help_browser;
use gcodekit5_camtools::gerber::{
    GerberConverter, GerberLayerType, GerberParameters, detect_layer_files,
};
use gcodekit5_core::units;
use gcodekit5_core::{SharedHashMap, shared};
use gcodekit5_settings::SettingsController;

/// Initial board width and height (mm).
pub const BOARD_MM: f64 = 100.0;
/// Initial work origin offsets (mm).
pub const OFFSET_MM: f64 = 0.0;
/// Initial feed rate entry text (mm/min).
pub const FEED_RATE: &str = "100.0";
/// Initial spindle speed entry text (RPM).
pub const SPINDLE_SPEED: &str = "10000.0";
/// Initial cut depth (mm, negative is below the surface).
pub const CUT_DEPTH_MM: f64 = -0.1;
/// Initial safe Z (mm).
pub const SAFE_Z_MM: f64 = 5.0;
/// Initial tool diameter (mm).
pub const TOOL_DIAMETER_MM: f64 = 0.1;
/// Initial isolation width (mm).
pub const ISOLATION_WIDTH_MM: f64 = 0.0;
/// Initial alignment hole diameter (mm).
pub const ALIGNMENT_HOLE_DIAMETER_MM: f64 = 3.175;
/// Initial alignment hole margin (mm).
pub const ALIGNMENT_HOLE_MARGIN_MM: f64 = 5.0;
/// Initial alignment hole depth (mm).
pub const ALIGNMENT_HOLE_DEPTH_MM: f64 = -1.8;
/// Initial layer type id.
pub const LAYER_TYPE: &str = "TopCopper";

/// The layer type of a layer dropdown id (`TopCopper` for an unknown id).
///
/// # Arguments
/// * `id` - The dropdown's id.
pub fn layer_type_from_id(id: &str) -> GerberLayerType {
    match id {
        "TopCopper" => GerberLayerType::TopCopper,
        "BottomCopper" => GerberLayerType::BottomCopper,
        "TopSolderMask" => GerberLayerType::TopSolderMask,
        "BottomSolderMask" => GerberLayerType::BottomSolderMask,
        "TopScreenPrint" => GerberLayerType::TopScreenPrint,
        "BottomScreenPrint" => GerberLayerType::BottomScreenPrint,
        "DrillHoles" => GerberLayerType::DrillHoles,
        "BoardOutline" => GerberLayerType::BoardOutline,
        _ => GerberLayerType::TopCopper,
    }
}

/// The text of the Gerber page's fields, as the page reads them on Generate.
#[derive(Debug, Clone, PartialEq)]
pub struct GerberEntries {
    /// The layer type's dropdown id.
    pub layer_type: String,
    /// Feed rate (mm/min).
    pub feed_rate: String,
    /// Spindle speed (RPM).
    pub spindle_speed: String,
    /// Board width, a length in the measurement system.
    pub board_width: String,
    /// Board height, a length.
    pub board_height: String,
    /// Work origin offset X, a length.
    pub offset_x: String,
    /// Work origin offset Y, a length.
    pub offset_y: String,
    /// Generate alignment holes.
    pub generate_alignment_holes: bool,
    /// Alignment hole diameter, a length.
    pub alignment_hole_diameter: String,
    /// Alignment hole margin, a length.
    pub alignment_hole_margin: String,
    /// Alignment hole depth, a length.
    pub alignment_hole_depth: String,
    /// Cut depth, a length.
    pub cut_depth: String,
    /// Safe Z, a length.
    pub safe_z: String,
    /// Tool diameter, a length.
    pub tool_diameter: String,
    /// Isolation width, a length.
    pub isolation_width: String,
    /// Remove excess copper.
    pub rubout: bool,
    /// Use the board outline for rubout.
    pub use_board_outline: bool,
}

impl GerberEntries {
    /// The page's initial entries in `system`.
    ///
    /// # Arguments
    /// * `system` - The measurement system the lengths are shown in.
    pub fn defaults(system: units::MeasurementSystem) -> Self {
        let len = |mm: f64| units::format_length(mm as f32, system);
        Self {
            layer_type: LAYER_TYPE.to_string(),
            feed_rate: FEED_RATE.to_string(),
            spindle_speed: SPINDLE_SPEED.to_string(),
            board_width: len(BOARD_MM),
            board_height: len(BOARD_MM),
            offset_x: len(OFFSET_MM),
            offset_y: len(OFFSET_MM),
            generate_alignment_holes: false,
            alignment_hole_diameter: len(ALIGNMENT_HOLE_DIAMETER_MM),
            alignment_hole_margin: len(ALIGNMENT_HOLE_MARGIN_MM),
            alignment_hole_depth: len(ALIGNMENT_HOLE_DEPTH_MM),
            cut_depth: len(CUT_DEPTH_MM),
            safe_z: len(SAFE_Z_MM),
            tool_diameter: len(TOOL_DIAMETER_MM),
            isolation_width: len(ISOLATION_WIDTH_MM),
            rubout: false,
            use_board_outline: false,
        }
    }
}

/// Reads the page's entries into converter parameters; a field that does not parse takes
/// its default value (the page's behaviour).
///
/// # Arguments
/// * `e` - The entries' text.
/// * `system` - The measurement system the lengths are written in.
/// * `directory` - The Gerber project folder, if one was chosen.
/// * `num_axes` - The active device's axis count.
pub fn gerber_parameters(
    e: &GerberEntries,
    system: units::MeasurementSystem,
    directory: Option<String>,
    num_axes: u8,
) -> GerberParameters {
    GerberParameters {
        layer_type: layer_type_from_id(&e.layer_type),
        feed_rate: e.feed_rate.parse().unwrap_or(100.0),
        spindle_speed: e.spindle_speed.parse().unwrap_or(10000.0),
        board_width: units::parse_length(&e.board_width, system).unwrap_or(100.0) as f32,
        board_height: units::parse_length(&e.board_height, system).unwrap_or(100.0) as f32,
        offset_x: units::parse_length(&e.offset_x, system).unwrap_or(0.0) as f32,
        offset_y: units::parse_length(&e.offset_y, system).unwrap_or(0.0) as f32,
        generate_alignment_holes: e.generate_alignment_holes,
        alignment_hole_diameter: units::parse_length(&e.alignment_hole_diameter, system)
            .unwrap_or(3.175) as f32,
        alignment_hole_margin: units::parse_length(&e.alignment_hole_margin, system).unwrap_or(5.0)
            as f32,
        alignment_hole_depth: units::parse_length(&e.alignment_hole_depth, system).unwrap_or(-1.8),
        cut_depth: units::parse_length(&e.cut_depth, system).unwrap_or(-0.1) as f32,
        safe_z: units::parse_length(&e.safe_z, system).unwrap_or(5.0) as f32,
        tool_diameter: units::parse_length(&e.tool_diameter, system).unwrap_or(0.1) as f32,
        isolation_width: units::parse_length(&e.isolation_width, system).unwrap_or(0.0) as f32,
        rubout: e.rubout,
        use_board_outline: e.use_board_outline,
        directory,
        num_axes,
    }
}

pub struct GerberTool {
    content: Box,
}

struct GerberWidgets {
    layer_type: DropDown,
    feed_rate: Entry,
    spindle_speed: Entry,
    board_width: Entry,
    board_height: Entry,
    offset_x: Entry,
    offset_y: Entry,
    generate_alignment_holes: CheckButton,
    alignment_hole_diameter: Entry,
    alignment_hole_margin: Entry,
    alignment_hole_depth: Entry,
    cut_depth: Entry,
    safe_z: Entry,
    tool_diameter: Entry,
    isolation_width: Entry,
    rubout: CheckButton,
    use_board_outline: CheckButton,
    layer_files: SharedHashMap<GerberLayerType, PathBuf>,
    file_label: Label,
}

impl GerberTool {
    pub fn new<F: Fn(String) + 'static>(
        stack: &Stack,
        settings: Rc<SettingsController>,
        on_generate: Rc<F>,
    ) -> Self {
        let content_box = Box::new(Orientation::Vertical, 0);
        // content_box.set_hexpand(true);
        // content_box.set_vexpand(true);

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
            .label("Gerber to G-Code")
            .css_classes(vec!["title-2"])
            .build();
        title.set_hexpand(true);
        title.set_halign(Align::Start);
        header.append(&title);

        header.append(&help_browser::make_help_button("gerber"));

        content_box.append(&header);

        let paned = Paned::new(Orientation::Horizontal);
        paned.set_hexpand(true);
        paned.set_vexpand(true);

        // Sidebar
        let sidebar = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .min_content_width(300)
            .build();
        let sidebar_box = Box::new(Orientation::Vertical, 12);
        sidebar_box.set_margin_top(12);
        sidebar_box.set_margin_bottom(12);
        sidebar_box.set_margin_start(12);
        sidebar_box.set_margin_end(12);
        sidebar.set_child(Some(&sidebar_box));

        // File Selection
        let file_group = PreferencesGroup::new();
        file_group.set_title("Gerber Project");

        let file_row = ActionRow::builder().title("Select Directory").build();
        let file_btn = Button::with_label("Browse...");
        let file_label = Label::new(Some("No directory selected"));
        file_label.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
        file_label.set_width_chars(20);

        let file_box = Box::new(Orientation::Horizontal, 6);
        file_box.append(&file_label);
        file_box.append(&file_btn);
        file_row.add_suffix(&file_box);
        file_group.add(&file_row);
        sidebar_box.append(&file_group);

        let layer_files = shared(HashMap::new());
        let layer_files_clone = layer_files.clone();
        let file_label_clone = file_label.clone();

        let detect_layers = |path: &std::path::Path| detect_layer_files(path);

        file_btn.connect_clicked(move |_| {
            let dialog = crate::ui::gtk::file_dialog::new_file_dialog("Open Gerber Directory");

            let lf = layer_files_clone.clone();
            let fl = file_label_clone.clone();

            dialog.pick_folder(None::<&gtk4::Window>, move |file| {
                if let Some(path) = file.path() {
                    let map = detect_layers(&path);
                    *lf.borrow_mut() = map;
                    fl.set_text(
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unnamed"),
                    );
                }
            });
        });

        // Parameters
        let params_group = PreferencesGroup::new();
        params_group.set_title("Parameters");

        let layer_type = new_text_dropdown();
        layer_type.append(Some("TopCopper"), "Top Copper");
        layer_type.append(Some("BottomCopper"), "Bottom Copper");
        layer_type.append(Some("TopSolderMask"), "Top Solder Mask");
        layer_type.append(Some("BottomSolderMask"), "Bottom Solder Mask");
        layer_type.append(Some("TopScreenPrint"), "Top Screen Print");
        layer_type.append(Some("BottomScreenPrint"), "Bottom Screen Print");
        layer_type.append(Some("DrillHoles"), "Drill Holes");
        layer_type.append(Some("BoardOutline"), "Board Outline");
        layer_type.set_active_id(Some(LAYER_TYPE));

        let layer_row = ActionRow::builder().title("Layer Type").build();
        layer_row.add_suffix(&layer_type);
        params_group.add(&layer_row);

        let (w_row, board_width, board_width_unit) =
            create_dimension_row("Board Width", BOARD_MM, &settings);
        params_group.add(&w_row);
        let (h_row, board_height, board_height_unit) =
            create_dimension_row("Board Height", BOARD_MM, &settings);
        params_group.add(&h_row);

        let (ox_row, offset_x, offset_x_unit) =
            create_dimension_row("Offset X", OFFSET_MM, &settings);
        params_group.add(&ox_row);
        let (oy_row, offset_y, offset_y_unit) =
            create_dimension_row("Offset Y", OFFSET_MM, &settings);
        params_group.add(&oy_row);

        let feed_rate = Entry::builder()
            .text(FEED_RATE)
            .width_chars(8)
            .valign(Align::Center)
            .build();
        let fr_row = ActionRow::builder().title("Feed Rate").build();
        fr_row.add_suffix(&feed_rate);
        params_group.add(&fr_row);

        let spindle_speed = Entry::builder()
            .text(SPINDLE_SPEED)
            .width_chars(8)
            .valign(Align::Center)
            .build();
        let ss_row = ActionRow::builder().title("Spindle Speed (RPM)").build();
        ss_row.add_suffix(&spindle_speed);
        params_group.add(&ss_row);

        let (cd_row, cut_depth, cut_depth_unit) =
            create_dimension_row("Cut Depth", CUT_DEPTH_MM, &settings);
        params_group.add(&cd_row);

        let (sz_row, safe_z, safe_z_unit) = create_dimension_row("Safe Z", SAFE_Z_MM, &settings);
        params_group.add(&sz_row);

        let (td_row, tool_diameter, tool_diameter_unit) =
            create_dimension_row("Tool Diameter", TOOL_DIAMETER_MM, &settings);
        params_group.add(&td_row);

        let (iw_row, isolation_width, isolation_width_unit) =
            create_dimension_row("Isolation Width", ISOLATION_WIDTH_MM, &settings);
        params_group.add(&iw_row);

        let rubout = CheckButton::builder()
            .active(false)
            .valign(Align::Center)
            .build();
        let rubout_row = ActionRow::builder().title("Remove Excess Copper").build();
        rubout_row.add_suffix(&rubout);
        params_group.add(&rubout_row);

        let use_board_outline = CheckButton::builder()
            .active(false)
            .valign(Align::Center)
            .build();
        let ubo_row = ActionRow::builder()
            .title("Use Board Outline for Rubout")
            .build();
        ubo_row.add_suffix(&use_board_outline);
        params_group.add(&ubo_row);

        sidebar_box.append(&params_group);

        // Alignment Holes
        let align_group = PreferencesGroup::new();
        align_group.set_title("Alignment Holes");

        let generate_alignment_holes = CheckButton::builder()
            .active(false)
            .valign(Align::Center)
            .build();
        let ah_row = ActionRow::builder()
            .title("Generate Alignment Holes")
            .build();
        ah_row.add_suffix(&generate_alignment_holes);
        align_group.add(&ah_row);

        let (ahd_row, alignment_hole_diameter, alignment_hole_diameter_unit) =
            create_dimension_row("Hole Diameter", ALIGNMENT_HOLE_DIAMETER_MM, &settings);
        align_group.add(&ahd_row);

        let (ahm_row, alignment_hole_margin, alignment_hole_margin_unit) =
            create_dimension_row("Margin", ALIGNMENT_HOLE_MARGIN_MM, &settings);
        align_group.add(&ahm_row);

        let (ahz_row, alignment_hole_depth, alignment_hole_depth_unit) =
            create_dimension_row("Drill Depth (Z)", ALIGNMENT_HOLE_DEPTH_MM, &settings);
        ahz_row.set_subtitle("Negative Z for the alignment holes (through the board)");
        align_group.add(&ahz_row);

        sidebar_box.append(&align_group);

        // Keep displayed dimensions consistent with the measurement system used to
        // parse them when generating.
        track_measurement_system(
            &settings,
            vec![
                (board_width.clone(), board_width_unit),
                (board_height.clone(), board_height_unit),
                (offset_x.clone(), offset_x_unit),
                (offset_y.clone(), offset_y_unit),
                (cut_depth.clone(), cut_depth_unit),
                (safe_z.clone(), safe_z_unit),
                (tool_diameter.clone(), tool_diameter_unit),
                (isolation_width.clone(), isolation_width_unit),
                (
                    alignment_hole_diameter.clone(),
                    alignment_hole_diameter_unit,
                ),
                (alignment_hole_margin.clone(), alignment_hole_margin_unit),
                (alignment_hole_depth.clone(), alignment_hole_depth_unit),
            ],
        );

        // Left Panel (Description)
        let left_panel = Box::new(Orientation::Vertical, 12);
        left_panel.add_css_class("sidebar");
        left_panel.set_margin_top(24);
        left_panel.set_margin_bottom(24);
        left_panel.set_margin_start(24);
        left_panel.set_margin_end(24);

        let title_label = Label::builder()
            .label("Gerber to G-Code")
            .css_classes(vec!["title-3"])
            .halign(Align::Start)
            .build();
        left_panel.append(&title_label);

        let desc = Label::builder()
            .label("Convert standard Gerber files (PCB layers) into G-Code for CNC isolation routing. Supports copper clearing (rubout), alignment holes, and custom tool parameters.")
            .css_classes(vec!["body"])
            .wrap(true)
            .halign(Align::Start)
            .build();
        left_panel.append(&desc);

        // Right Panel (Controls + Actions)
        let right_panel = Box::new(Orientation::Vertical, 0);
        sidebar.set_vexpand(true);
        right_panel.append(&sidebar);

        // Actions
        let action_box = Box::new(Orientation::Horizontal, 12);
        action_box.set_margin_top(12);
        action_box.set_margin_bottom(12);
        action_box.set_margin_start(12);
        action_box.set_margin_end(12);
        action_box.set_halign(Align::End);

        let load_btn = Button::with_label("Load");
        let save_btn = Button::with_label("Save");
        let cancel_btn = Button::with_label("Cancel");
        let generate_btn = Button::with_label("Generate G-Code");
        generate_btn.add_css_class("suggested-action");

        action_box.append(&load_btn);
        action_box.append(&save_btn);
        action_box.append(&cancel_btn);
        action_box.append(&generate_btn);
        right_panel.append(&action_box);

        paned.set_start_child(Some(&left_panel));
        paned.set_end_child(Some(&right_panel));
        set_paned_initial_fraction(&paned, 0.40);
        content_box.append(&paned);

        let widgets = Rc::new(GerberWidgets {
            layer_type: layer_type.clone(),
            feed_rate,
            spindle_speed,
            board_width,
            board_height,
            offset_x,
            offset_y,
            generate_alignment_holes,
            alignment_hole_diameter,
            alignment_hole_margin,
            alignment_hole_depth,
            cut_depth,
            safe_z,
            tool_diameter,
            isolation_width,
            rubout,
            use_board_outline,
            layer_files,
            file_label,
        });

        // Update label when layer changes
        let w_layer = widgets.clone();
        layer_type.connect_changed(move |c| {
            if let Some(id) = c.active_id() {
                let layer_type = layer_type_from_id(id.as_str());

                let files = w_layer.layer_files.borrow();
                if let Some(path) = files.get(&layer_type) {
                    w_layer.file_label.set_text(
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unnamed"),
                    );
                } else {
                    w_layer.file_label.set_text("Layer not found in directory");
                }
            }
        });

        // Connect Generate
        let w_gen = widgets.clone();
        let settings_gen = settings.clone();
        let on_gen = on_generate.clone();

        generate_btn.connect_clicked(move |_| {
            let files = w_gen.layer_files.borrow();
            if files.is_empty() {
                CamToolsView::show_error_dialog("Error", "Please select a Gerber directory.");
                return;
            }

            let params = Self::collect_params(&w_gen, &settings_gen);

            let path = match files.get(&params.layer_type) {
                Some(p) => p,
                None => {
                    CamToolsView::show_error_dialog(
                        "Error",
                        "Selected layer not found in directory.",
                    );
                    return;
                }
            };

            warn!("Reading Gerber file: {:?}", path);

            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(e) => {
                    CamToolsView::show_error_dialog(
                        "Error",
                        &format!("Failed to read file: {}", e),
                    );
                    return;
                }
            };

            warn!("Read {} bytes", content.len());
            warn!(
                "Generating G-Code for layer: {:?} with params: {:?}",
                params.layer_type, params
            );

            match GerberConverter::generate(&params, &content) {
                Ok(gcode) => {
                    warn!("Generated {} bytes of G-Code", gcode.len());
                    on_gen(gcode)
                }
                Err(e) => {
                    warn!("Generation failed: {}", e);
                    CamToolsView::show_error_dialog("Generation Failed", &e.to_string())
                }
            }
        });

        // Save
        let w_save = widgets.clone();
        let settings_save = settings.clone();
        save_btn.connect_clicked(move |_| {
            let params = Self::collect_params(&w_save, &settings_save);
            Self::save_params(&params);
        });

        // Load
        let w_load = widgets.clone();
        let settings_load = settings.clone();
        load_btn.connect_clicked(move |_| {
            Self::load_params(&w_load, &settings_load);
        });

        let stack_clone = stack.clone();
        cancel_btn.connect_clicked(move |_| {
            stack_clone.set_visible_child_name("dashboard");
        });

        Self {
            content: content_box,
        }
    }

    pub fn widget(&self) -> &Box {
        &self.content
    }

    fn collect_params(w: &GerberWidgets, settings: &Rc<SettingsController>) -> GerberParameters {
        let system = settings.persistence.borrow().config().ui.measurement_system;
        let entries = GerberEntries {
            // An unset dropdown keeps the default layer.
            layer_type: w
                .layer_type
                .active_id()
                .map_or_else(|| LAYER_TYPE.to_string(), |id| id.to_string()),
            feed_rate: w.feed_rate.text().to_string(),
            spindle_speed: w.spindle_speed.text().to_string(),
            board_width: w.board_width.text().to_string(),
            board_height: w.board_height.text().to_string(),
            offset_x: w.offset_x.text().to_string(),
            offset_y: w.offset_y.text().to_string(),
            generate_alignment_holes: w.generate_alignment_holes.is_active(),
            alignment_hole_diameter: w.alignment_hole_diameter.text().to_string(),
            alignment_hole_margin: w.alignment_hole_margin.text().to_string(),
            alignment_hole_depth: w.alignment_hole_depth.text().to_string(),
            cut_depth: w.cut_depth.text().to_string(),
            safe_z: w.safe_z.text().to_string(),
            tool_diameter: w.tool_diameter.text().to_string(),
            isolation_width: w.isolation_width.text().to_string(),
            rubout: w.rubout.is_active(),
            use_board_outline: w.use_board_outline.is_active(),
        };
        let directory = {
            let files = w.layer_files.borrow();
            // Just grab the parent dir of the first file found, or None
            files
                .values()
                .next()
                .and_then(|p| p.parent().map(|d| d.to_string_lossy().to_string()))
        };
        gerber_parameters(
            &entries,
            system,
            directory,
            crate::device_status::get_active_num_axes(),
        )
    }

    fn save_params(params: &GerberParameters) {
        let dialog = crate::ui::gtk::file_dialog::new_file_dialog("Save Gerber Parameters");
        dialog.set_initial_name(Some("gerber_params.json"));

        let params_clone = params.clone();
        dialog.pick_save(None::<&gtk4::Window>, move |file| {
            if let Some(path) = file.path() {
                match serde_json::to_string_pretty(&params_clone) {
                    Ok(json) => write_preset_file(&path, &json),
                    Err(e) => CamToolsView::show_error_dialog("Save Error", &format!("{}", e)),
                }
            }
        });
    }

    fn load_params(w: &Rc<GerberWidgets>, settings: &Rc<SettingsController>) {
        let dialog = crate::ui::gtk::file_dialog::new_file_dialog("Load Gerber Parameters");

        let w_clone = w.clone();
        let settings_clone = settings.clone();
        dialog.pick_open(None::<&gtk4::Window>, move |file| {
            if let Some(path) = file.path() {
                let loaded = fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|content| {
                        serde_json::from_str::<GerberParameters>(&content)
                            .map_err(|e| e.to_string())
                    });
                match loaded {
                    Ok(params) => Self::apply_params(&w_clone, &params, &settings_clone),
                    Err(e) => {
                        warn!("Failed to load parameters: {}", e);
                        CamToolsView::show_error_dialog(
                            "Load Error",
                            &format!("{}: {}", path.display(), e),
                        );
                    }
                }
            }
        });
    }

    fn apply_params(w: &GerberWidgets, p: &GerberParameters, settings: &Rc<SettingsController>) {
        let system = settings.persistence.borrow().config().ui.measurement_system;

        let layer_id = match p.layer_type {
            GerberLayerType::TopCopper => "TopCopper",
            GerberLayerType::BottomCopper => "BottomCopper",
            GerberLayerType::TopSolderMask => "TopSolderMask",
            GerberLayerType::BottomSolderMask => "BottomSolderMask",
            GerberLayerType::TopScreenPrint => "TopScreenPrint",
            GerberLayerType::BottomScreenPrint => "BottomScreenPrint",
            GerberLayerType::DrillHoles => "DrillHoles",
            GerberLayerType::BoardOutline => "BoardOutline",
        };
        w.layer_type.set_active_id(Some(layer_id));

        w.feed_rate.set_text(&p.feed_rate.to_string());
        w.spindle_speed.set_text(&p.spindle_speed.to_string());
        w.board_width
            .set_text(&units::format_length(p.board_width, system));
        w.board_height
            .set_text(&units::format_length(p.board_height, system));
        w.offset_x
            .set_text(&units::format_length(p.offset_x, system));
        w.offset_y
            .set_text(&units::format_length(p.offset_y, system));
        w.generate_alignment_holes
            .set_active(p.generate_alignment_holes);
        w.alignment_hole_diameter
            .set_text(&units::format_length(p.alignment_hole_diameter, system));
        w.alignment_hole_margin
            .set_text(&units::format_length(p.alignment_hole_margin, system));
        w.alignment_hole_depth
            .set_text(&units::format_length(p.alignment_hole_depth, system));
        w.cut_depth
            .set_text(&units::format_length(p.cut_depth, system));
        w.safe_z.set_text(&units::format_length(p.safe_z, system));
        w.tool_diameter
            .set_text(&units::format_length(p.tool_diameter, system));
        w.isolation_width
            .set_text(&units::format_length(p.isolation_width, system));
        w.rubout.set_active(p.rubout);
        w.use_board_outline.set_active(p.use_board_outline);

        if let Some(dir) = &p.directory {
            let path = PathBuf::from(dir);
            if path.exists() {
                // Re-run detection
                let map = detect_layer_files(&path);
                *w.layer_files.borrow_mut() = map;
                w.file_label.set_text(
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unnamed"),
                );

                // Update label for selected layer
                let files = w.layer_files.borrow();
                if let Some(path) = files.get(&p.layer_type) {
                    w.file_label.set_text(
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unnamed"),
                    );
                } else {
                    w.file_label.set_text("Layer not found in directory");
                }
            }
        }
    }
}

// Drill Press Tool
