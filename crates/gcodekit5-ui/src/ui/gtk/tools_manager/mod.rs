//! # Tools Manager Panel
//!
//! UI panel for managing the cutting tool library. Provides CRUD
//! operations for tool definitions including geometry, flute count,
//! and recommended speeds and feeds.

mod event_handlers;
mod ui_builders;

use crate::ui::gtk::common::text_dropdown::{TextDropDownExt, new_text_dropdown};
use crate::ui::gtk::file_dialog::FileDialogHelpers;
use gtk4::prelude::*;
use gtk4::{
    Align, Box, Button, DropDown, Entry, Frame, Label, ListBox, ListBoxRow, Orientation, Paned,
    PolicyType, ScrolledWindow, SearchEntry, SelectionMode, SpinButton, Stack, StackSwitcher,
    TextView,
};
use std::cell::RefCell;
use std::rc::Rc;

use crate::t;
use crate::ui::gtk::common::dialog::UnsavedChangesChoice;
use crate::ui::gtk::common::spacing;
use crate::ui::gtk::help_browser;
use crate::ui::gtk::platform;
use crate::ui::tools_manager_backend::ToolsManagerBackend;
use gcodekit5_app::tool_form::{
    COATINGS, ShankChoice, TOOL_MATERIALS, ToolFilter, ToolForm, has_corner_radius, has_tip_angle,
    tool_contents_equal, tool_material_from_filter_id, tool_type_from_filter_id,
};

use gcodekit5_core::data::tools::{ShankType, Tool, ToolCoating, ToolId, ToolMaterial, ToolType};
use gcodekit5_core::{Shared, SharedOption, shared, shared_none};
use gcodekit5_settings::SettingsController;

pub(crate) const ROW_TOOL_ID_KEY: &str = "gcodekit5-tool-id";

#[derive(Clone)]
pub struct ToolsManagerView {
    pub widget: Paned,
    pub(crate) backend: Shared<ToolsManagerBackend>,
    pub(crate) settings_controller: Rc<SettingsController>,

    // Left panel widgets
    pub(crate) tools_list: ListBox,
    pub(crate) search_entry: SearchEntry,
    pub(crate) type_filter: DropDown,
    pub(crate) material_filter: DropDown,
    pub(crate) dia_min: Entry,
    pub(crate) dia_max: Entry,
    // Right panel
    pub(crate) right_panel_stack: Stack,

    // Edit form widgets
    pub(crate) edit_id: Entry,
    pub(crate) edit_number: SpinButton,
    pub(crate) edit_name: Entry,
    pub(crate) edit_tool_type: DropDown,
    pub(crate) edit_material: DropDown,
    pub(crate) edit_coating: DropDown,
    pub(crate) edit_shank: DropDown,

    pub(crate) edit_diameter: Entry,
    pub(crate) edit_length: Entry,
    pub(crate) edit_flute_length: Entry,
    pub(crate) edit_shaft_diameter: Entry,
    pub(crate) edit_flutes: SpinButton,
    pub(crate) edit_corner_radius: Entry,
    pub(crate) edit_tip_angle: Entry,

    pub(crate) edit_manufacturer: Entry,
    pub(crate) edit_part_number: Entry,
    pub(crate) edit_description: TextView,
    pub(crate) edit_notes: TextView,

    pub(crate) error_label: Label,

    // State
    pub(crate) selected_tool: SharedOption<Tool>,
    pub(crate) is_creating: Shared<bool>,
    pub(crate) last_selected_tool_id: SharedOption<String>,

    // Action buttons
    pub(crate) save_btn: Button,
    pub(crate) cancel_btn: Button,
    pub(crate) delete_btn: Button,

    pub(crate) new_btn: Button,
    pub(crate) import_zip_btn: Button,
    pub(crate) import_json_btn: Button,
    pub(crate) export_btn: Button,
    pub(crate) reset_btn: Button,
}

impl ToolsManagerView {
    pub fn new(settings_controller: Rc<SettingsController>) -> Rc<Self> {
        let backend = shared(ToolsManagerBackend::new());

        let widget = Paned::new(Orientation::Horizontal);
        widget.set_hexpand(true);
        widget.set_vexpand(true);

        // LEFT SIDEBAR
        let sidebar = Box::new(Orientation::Vertical, spacing::MEDIUM);
        sidebar.add_css_class("sidebar");
        sidebar.set_width_request(280);
        sidebar.set_margin_top(spacing::MEDIUM);
        sidebar.set_margin_bottom(spacing::MEDIUM);
        sidebar.set_margin_start(spacing::MEDIUM);
        sidebar.set_margin_end(spacing::MEDIUM);

        let header_box = Box::new(Orientation::Horizontal, spacing::MEDIUM);
        header_box.set_margin_start(spacing::SMALL);
        let title = Label::new(Some("CNC Tools"));
        title.add_css_class("title-4");
        title.set_halign(Align::Start);
        header_box.append(&title);

        let spacer = Box::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        header_box.append(&spacer);
        header_box.append(&help_browser::make_help_button("tools_manager"));

        sidebar.append(&header_box);

        // Search
        let search_entry = SearchEntry::new();
        search_entry.set_placeholder_text(Some("Search (name, id, type, Ø)…"));
        sidebar.append(&search_entry);

        // Type filter
        let type_filter = new_text_dropdown();
        type_filter.append(Some("all"), "All Types");
        type_filter.append(Some("endmill_flat"), "Flat End Mill");
        type_filter.append(Some("endmill_ball"), "Ball End Mill");
        type_filter.append(Some("endmill_cr"), "Corner Radius End Mill");
        type_filter.append(Some("vbit"), "V-Bit");
        type_filter.append(Some("drill"), "Drill Bit");
        type_filter.append(Some("spot"), "Spot Drill");
        type_filter.append(Some("engraving"), "Engraving Bit");
        type_filter.append(Some("chamfer"), "Chamfer Tool");
        type_filter.append(Some("specialty"), "Specialty");
        type_filter.set_active_id(Some("all"));
        sidebar.append(&type_filter);

        // Material filter
        let material_filter = new_text_dropdown();
        material_filter.append(Some("all"), "All Materials");
        material_filter.append(Some("HSS"), "HSS");
        material_filter.append(Some("Carbide"), "Carbide");
        material_filter.append(Some("Coated Carbide"), "Coated Carbide");
        material_filter.append(Some("Diamond"), "Diamond");
        material_filter.set_active_id(Some("all"));
        sidebar.append(&material_filter);

        // Extra filters (single-row layout)
        let filters_frame = Frame::new(Some("Filters"));
        let filters_row = Box::new(Orientation::Horizontal, 8);
        filters_row.set_margin_top(8);
        filters_row.set_margin_bottom(8);
        filters_row.set_margin_start(8);
        filters_row.set_margin_end(8);

        let dia_min = Entry::new();
        dia_min.set_placeholder_text(Some("Min Ø (mm)"));
        dia_min.set_width_chars(8);
        let dia_max = Entry::new();
        dia_max.set_placeholder_text(Some("Max Ø (mm)"));
        dia_max.set_width_chars(8);

        filters_row.append(&dia_min);
        filters_row.append(&dia_max);

        filters_frame.set_child(Some(&filters_row));
        sidebar.append(&filters_frame);

        // Tools list
        let scroll = ScrolledWindow::new();
        scroll.set_policy(PolicyType::Never, PolicyType::Automatic);
        scroll.set_vexpand(true);

        let tools_list = ListBox::new();
        tools_list.set_selection_mode(SelectionMode::Single);
        tools_list.add_css_class("boxed-list");
        scroll.set_child(Some(&tools_list));
        sidebar.append(&scroll);

        // Actions (moved to right-hand sidebar)
        let new_btn = Button::with_label("➕ New Tool");
        new_btn.add_css_class("suggested-action");
        new_btn.set_tooltip_text(Some("Create a new custom tool"));

        let actions_frame = Frame::new(Some("Library"));
        let actions_box = Box::new(Orientation::Vertical, 8);
        actions_box.set_margin_top(8);
        actions_box.set_margin_bottom(8);
        actions_box.set_margin_start(8);
        actions_box.set_margin_end(8);

        let import_zip_btn = Button::with_label("Import GTC (.zip)…");
        import_zip_btn.set_tooltip_text(Some("Import tool catalog from a GTC zip package"));
        let import_json_btn = Button::with_label("Import GTC (.json)…");
        import_json_btn.set_tooltip_text(Some("Import tool catalog from a GTC JSON file"));
        let export_btn = Button::with_label("Export Custom Tools…");
        export_btn.set_tooltip_text(Some("Export custom tools to a JSON file"));
        let reset_btn = Button::with_label("Reset Custom Tools…");
        reset_btn.add_css_class("destructive-action");
        reset_btn.set_tooltip_text(Some(
            "Remove all custom/imported tools and delete local storage",
        ));

        actions_box.append(&import_zip_btn);
        actions_box.append(&import_json_btn);
        actions_box.append(&export_btn);
        actions_box.append(&reset_btn);
        actions_frame.set_child(Some(&actions_box));

        widget.set_start_child(Some(&sidebar));

        // RIGHT PANEL (editor + library sidebar)
        let right_panel_stack = Stack::new();
        right_panel_stack.set_hexpand(true);
        right_panel_stack.set_vexpand(true);

        let empty_state = Box::new(Orientation::Vertical, 0);
        empty_state.set_valign(Align::Center);
        empty_state.set_halign(Align::Center);
        empty_state.set_vexpand(true);
        empty_state.set_hexpand(true);

        let empty_label = Label::new(Some("Please select or create a tool to edit"));
        empty_label.add_css_class("title-2");
        empty_label.add_css_class("dim-label");
        empty_state.append(&empty_label);

        right_panel_stack.add_named(&empty_state, Some("empty"));

        // Edit form container
        let main_content = Box::new(Orientation::Vertical, 10);
        main_content.set_margin_top(20);
        main_content.set_margin_bottom(20);
        main_content.set_margin_start(20);
        main_content.set_margin_end(20);

        let action_bar = Box::new(Orientation::Horizontal, 10);
        let save_btn = Button::with_label("💾 Save");
        save_btn.add_css_class("suggested-action");
        save_btn.set_sensitive(false);
        save_btn.set_tooltip_text(Some("Save changes"));

        let cancel_btn = Button::with_label("Cancel");
        cancel_btn.set_sensitive(false);
        cancel_btn.set_tooltip_text(Some("Discard changes"));

        let delete_btn = Button::with_label("🗑️ Delete");
        delete_btn.add_css_class("destructive-action");
        delete_btn.set_sensitive(false);
        delete_btn.set_tooltip_text(Some("Delete this tool"));

        action_bar.append(&save_btn);
        action_bar.append(&cancel_btn);
        action_bar.append(&delete_btn);

        let spacer = Label::new(None);
        spacer.set_hexpand(true);
        action_bar.append(&spacer);

        main_content.append(&action_bar);

        let error_label = Label::new(None);
        error_label.set_halign(Align::Start);
        error_label.set_xalign(0.0);
        error_label.add_css_class("error");
        error_label.set_visible(false);
        main_content.append(&error_label);

        // Tabs
        let stack = Stack::new();
        stack.set_vexpand(true);

        let (
            basic_page,
            edit_id,
            edit_number,
            edit_name,
            edit_tool_type,
            edit_material,
            edit_coating,
            edit_shank,
        ) = Self::create_basic_tab();
        let (
            geometry_page,
            edit_diameter,
            edit_length,
            edit_flute_length,
            edit_shaft_diameter,
            edit_flutes,
            edit_corner_radius,
            edit_tip_angle,
        ) = Self::create_geometry_tab();
        let (mfg_page, edit_manufacturer, edit_part_number, edit_description) =
            Self::create_manufacturer_tab();
        let (notes_page, edit_notes) = Self::create_notes_tab();

        stack.add_titled(&basic_page, Some("basic"), "Basic Info");
        stack.add_titled(&geometry_page, Some("geometry"), "Geometry");
        stack.add_titled(&mfg_page, Some("manufacturer"), "Manufacturer");
        stack.add_titled(&notes_page, Some("notes"), "Notes");

        let switcher = StackSwitcher::new();
        switcher.set_stack(Some(&stack));
        switcher.set_halign(Align::Center);

        main_content.append(&switcher);
        main_content.append(&stack);

        right_panel_stack.add_named(&main_content, Some("edit"));
        right_panel_stack.set_visible_child_name("empty");

        let right_sidebar = Box::new(Orientation::Vertical, spacing::MEDIUM);
        right_sidebar.add_css_class("sidebar");
        right_sidebar.set_width_request(280);
        right_sidebar.set_margin_top(spacing::MEDIUM);
        right_sidebar.set_margin_bottom(spacing::MEDIUM);
        right_sidebar.set_margin_start(spacing::MEDIUM);
        right_sidebar.set_margin_end(spacing::MEDIUM);

        right_sidebar.append(&new_btn);
        right_sidebar.append(&actions_frame);

        let right_container = Box::new(Orientation::Horizontal, 0);
        right_container.set_hexpand(true);
        right_container.set_vexpand(true);
        right_container.append(&right_panel_stack);
        right_container.append(&right_sidebar);

        widget.set_end_child(Some(&right_container));

        let view = Rc::new(Self {
            widget,
            backend: backend.clone(),
            settings_controller,
            tools_list,
            search_entry,
            type_filter,
            material_filter,
            dia_min,
            dia_max,
            right_panel_stack: right_panel_stack.clone(),
            edit_id,
            edit_number,
            edit_name,
            edit_tool_type,
            edit_material,
            edit_coating,
            edit_shank,
            edit_diameter,
            edit_length,
            edit_flute_length,
            edit_shaft_diameter,
            edit_flutes,
            edit_corner_radius,
            edit_tip_angle,
            edit_manufacturer,
            edit_part_number,
            edit_description,
            edit_notes,
            error_label,
            selected_tool: shared_none(),
            is_creating: shared(false),
            last_selected_tool_id: shared_none(),
            save_btn,
            cancel_btn,
            delete_btn,
            new_btn,
            import_zip_btn,
            import_json_btn,
            export_btn,
            reset_btn,
        });

        view.restore_ui_state();
        view.setup_splitter_persistence();
        view.setup_event_handlers();
        view.load_tools();

        view
    }

    fn persist_tools_sidebar_position(&self, pos: i32) {
        {
            let mut p = self.settings_controller.persistence.borrow_mut();
            p.config_mut().ui.tools_sidebar_position = Some(pos);
            p.persist_or_log();
        }
    }

    fn persist_ui_state(&self) {
        let search = self.search_entry.text().to_string();
        let type_id = self.type_filter.active_id().map(|s| s.to_string());
        let mat_id = self.material_filter.active_id().map(|s| s.to_string());
        let dia_min = self.dia_min.text().to_string();
        let dia_max = self.dia_max.text().to_string();

        let parsed_dia_min = dia_min.parse::<f32>().ok();
        let parsed_dia_max = dia_max.parse::<f32>().ok();
        {
            let mut p = self.settings_controller.persistence.borrow_mut();
            p.config_mut().ui.tools_manager_search = search;
            p.config_mut().ui.tools_manager_type_filter = type_id.unwrap_or_default();
            p.config_mut().ui.tools_manager_material_filter = mat_id.unwrap_or_default();
            p.config_mut().ui.tools_manager_dia_min = parsed_dia_min;
            p.config_mut().ui.tools_manager_dia_max = parsed_dia_max;
            p.config_mut().ui.tools_manager_selected_tool =
                self.last_selected_tool_id.borrow().clone();

            p.persist_or_log();
        }
    }

    fn select_row_by_tool_id(&self, list: &ListBox, tool_id: &str) {
        let mut child = list.first_child();
        while let Some(w) = child {
            let next = w.next_sibling();
            if let Ok(row) = w.downcast::<ListBoxRow>() {
                #[allow(unsafe_code)]
                let stored = unsafe {
                    // SAFETY: row.data() retrieves a String previously stored
                    // via set_data() with the same key. Type and key match.
                    row.data::<String>(ROW_TOOL_ID_KEY)
                        .map(|p| p.as_ref().clone())
                };
                if stored.as_deref() == Some(tool_id) {
                    list.select_row(Some(&row));
                    return;
                }
            }
            child = next;
        }
    }

    fn load_tools(&self) {
        while let Some(child) = self.tools_list.first_child() {
            self.tools_list.remove(&child);
        }

        let backend = self.backend.borrow();
        let tools: Vec<&Tool> = backend.get_all_tools();

        let filter = ToolFilter {
            search: self.search_entry.text().to_string(),
            tool_type: self
                .type_filter
                .active_id()
                .and_then(|id| tool_type_from_filter_id(id.as_str())),
            material: self
                .material_filter
                .active_id()
                .and_then(|id| tool_material_from_filter_id(id.as_str())),
            dia_min: self.dia_min.text().to_string(),
            dia_max: self.dia_max.text().to_string(),
        };
        if filter.dia_min_invalid() {
            self.dia_min.add_css_class("error");
        } else {
            self.dia_min.remove_css_class("error");
        }
        if filter.dia_max_invalid() {
            self.dia_max.add_css_class("error");
        } else {
            self.dia_max.remove_css_class("error");
        }
        let tools = filter.apply(tools);

        for tool in tools {
            let row = ListBoxRow::new();
            // SAFETY: set_data stores a String keyed by ROW_TOOL_ID_KEY. The
            // data is valid while the row widget exists in the list.
            #[allow(unsafe_code)]
            unsafe {
                row.set_data(ROW_TOOL_ID_KEY, tool.id.0.clone());
            }

            let row_box = Box::new(Orientation::Vertical, spacing::SMALL);
            row_box.set_margin_top(spacing::BUTTON_INTERNAL);
            row_box.set_margin_bottom(spacing::BUTTON_INTERNAL);
            row_box.set_margin_start(spacing::MEDIUM);
            row_box.set_margin_end(spacing::MEDIUM);

            let name_label = Label::new(Some(&tool.name));
            name_label.add_css_class("title-4");
            name_label.set_halign(Align::Start);
            name_label.set_xalign(0.0);
            name_label.set_wrap(true);
            name_label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
            name_label.set_max_width_chars(36);
            row_box.append(&name_label);

            let coating = tool
                .coating
                .map(|c| c.to_string())
                .unwrap_or("None".to_string());
            let shaft = tool.shaft_diameter.unwrap_or(tool.diameter);
            let info = format!(
                "{} • id:{} • Ø{:.3}mm • shaft Ø{:.3}mm • {}F • FL{:.3}mm • {} / {}",
                tool.tool_type,
                tool.id.0,
                tool.diameter,
                shaft,
                tool.flutes,
                tool.flute_length,
                tool.material,
                coating
            );
            let info_label = Label::new(Some(&info));
            info_label.add_css_class("dim-label");
            info_label.set_halign(Align::Start);
            info_label.set_xalign(0.0);
            info_label.set_wrap(true);
            info_label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
            info_label.set_max_width_chars(44);
            row_box.append(&info_label);

            row.set_child(Some(&row_box));
            self.tools_list.append(&row);
        }

        // Restore selection if possible.
        if self.right_panel_stack.visible_child_name().as_deref() != Some("edit") {
            // IMPORTANT: don't hold a RefCell borrow across select_row(), since that can emit
            // row-selected signals synchronously.
            let selected_tool_id = { self.last_selected_tool_id.borrow().clone() };
            if let Some(tool_id) = selected_tool_id {
                self.select_row_by_tool_id(&self.tools_list, &tool_id);
            }
        }
    }

    fn start_create_new(&self) {
        if self.is_dirty() {
            let view = self.clone();
            self.show_discard_dialog(move || {
                view.start_create_new_inner();
            });
            return;
        }

        self.start_create_new_inner();
    }

    fn start_create_new_inner(&self) {
        *self.is_creating.borrow_mut() = true;
        *self.selected_tool.borrow_mut() = None;

        self.clear_form();
        self.edit_id.set_sensitive(true);
        self.right_panel_stack.set_visible_child_name("edit");
        self.cancel_btn.set_sensitive(true);
        self.delete_btn.set_sensitive(false);
        self.update_type_dependent_fields();
        self.update_save_state();
    }

    fn load_tool_for_edit(&self, tool_id: &str) {
        let backend = self.backend.borrow();
        if let Some(tool) = backend.get_tool(&ToolId(tool_id.to_string())) {
            *self.is_creating.borrow_mut() = false;
            *self.selected_tool.borrow_mut() = Some(tool.clone());
            match self.last_selected_tool_id.try_borrow_mut() {
                Ok(mut last) => {
                    *last = Some(tool.id.0.clone());
                }
                _ => {
                    tracing::warn!("last_selected_tool_id already borrowed; skipping update");
                }
            }

            self.edit_id.set_text(&tool.id.0);
            self.edit_id.set_sensitive(false);
            self.edit_number.set_value(tool.number as f64);
            self.edit_name.set_text(&tool.name);

            match tool.tool_type {
                ToolType::EndMillFlat => self.edit_tool_type.set_active(Some(0)),
                ToolType::EndMillBall => self.edit_tool_type.set_active(Some(1)),
                ToolType::EndMillCornerRadius => self.edit_tool_type.set_active(Some(2)),
                ToolType::VBit => self.edit_tool_type.set_active(Some(3)),
                ToolType::DrillBit => self.edit_tool_type.set_active(Some(4)),
                ToolType::SpotDrill => self.edit_tool_type.set_active(Some(5)),
                ToolType::EngravingBit => self.edit_tool_type.set_active(Some(6)),
                ToolType::ChamferTool => self.edit_tool_type.set_active(Some(7)),
                ToolType::Specialty => self.edit_tool_type.set_active(Some(8)),
            }

            self.edit_material.set_active(match tool.material {
                ToolMaterial::HSS => Some(0),
                ToolMaterial::Carbide => Some(1),
                ToolMaterial::CoatedCarbide => Some(2),
                ToolMaterial::Diamond => Some(3),
            });

            self.edit_coating.set_active(match tool.coating {
                None => Some(0),
                Some(ToolCoating::TiN) => Some(1),
                Some(ToolCoating::TiAlN) => Some(2),
                Some(ToolCoating::DLC) => Some(3),
                Some(ToolCoating::AlOx) => Some(4),
            });

            self.edit_shank.set_active(match tool.shank {
                ShankType::Straight(_) => Some(0),
                ShankType::Collet => Some(1),
                ShankType::Tapered => Some(2),
            });

            self.edit_diameter
                .set_text(&format!("{:.3}", tool.diameter));
            self.edit_length.set_text(&format!("{:.3}", tool.length));
            self.edit_flute_length
                .set_text(&format!("{:.3}", tool.flute_length));

            let shaft = tool.shaft_diameter.unwrap_or(tool.diameter);
            self.edit_shaft_diameter.set_text(&format!("{shaft:.3}"));
            self.edit_flutes.set_value(tool.flutes as f64);

            self.edit_corner_radius.set_text(
                &tool
                    .corner_radius
                    .map(|v| format!("{v:.3}"))
                    .unwrap_or_default(),
            );
            self.edit_tip_angle.set_text(
                &tool
                    .tip_angle
                    .map(|v| format!("{v:.3}"))
                    .unwrap_or_default(),
            );

            if let Some(ref manufacturer) = tool.manufacturer {
                self.edit_manufacturer.set_text(manufacturer);
            } else {
                self.edit_manufacturer.set_text("");
            }

            if let Some(ref part_number) = tool.part_number {
                self.edit_part_number.set_text(part_number);
            } else {
                self.edit_part_number.set_text("");
            }

            self.edit_description.buffer().set_text(&tool.description);
            self.edit_notes.buffer().set_text(&tool.notes);

            self.right_panel_stack.set_visible_child_name("edit");
            self.cancel_btn.set_sensitive(true);
            self.delete_btn.set_sensitive(true);

            self.update_type_dependent_fields();
            self.update_save_state();
        }
    }

    fn update_type_dependent_fields(&self) {
        let tt = match self.edit_tool_type.active() {
            Some(0) => ToolType::EndMillFlat,
            Some(1) => ToolType::EndMillBall,
            Some(2) => ToolType::EndMillCornerRadius,
            Some(3) => ToolType::VBit,
            Some(4) => ToolType::DrillBit,
            Some(5) => ToolType::SpotDrill,
            Some(6) => ToolType::EngravingBit,
            Some(7) => ToolType::ChamferTool,
            Some(8) => ToolType::Specialty,
            _ => ToolType::EndMillFlat,
        };

        let show_corner_radius = has_corner_radius(tt);
        let show_tip_angle = has_tip_angle(tt);

        self.edit_corner_radius.set_sensitive(show_corner_radius);
        self.edit_tip_angle.set_sensitive(show_tip_angle);

        if !show_corner_radius {
            self.edit_corner_radius.set_text("");
        }
        if !show_tip_angle {
            self.edit_tip_angle.set_text("");
        }
    }

    fn tool_type_from_active(active: Option<u32>) -> ToolType {
        match active {
            Some(0) => ToolType::EndMillFlat,
            Some(1) => ToolType::EndMillBall,
            Some(2) => ToolType::EndMillCornerRadius,
            Some(3) => ToolType::VBit,
            Some(4) => ToolType::DrillBit,
            Some(5) => ToolType::SpotDrill,
            Some(6) => ToolType::EngravingBit,
            Some(7) => ToolType::ChamferTool,
            Some(8) => ToolType::Specialty,
            _ => ToolType::EndMillFlat,
        }
    }

    fn shank_from_combo(idx: Option<u32>) -> ShankChoice {
        match idx {
            Some(1) => ShankChoice::Collet,
            Some(2) => ShankChoice::Tapered,
            _ => ShankChoice::Derived,
        }
    }

    fn read_text_view(tv: &TextView) -> String {
        let buffer = tv.buffer();
        let start = buffer.start_iter();
        let end = buffer.end_iter();
        buffer.text(&start, &end, true).to_string()
    }

    /// The form's values, as the shared [`ToolForm`].
    fn read_form(&self) -> ToolForm {
        let tool_type = Self::tool_type_from_active(self.edit_tool_type.active());
        let material = TOOL_MATERIALS
            .get(self.edit_material.active().unwrap_or(1) as usize)
            .copied()
            .unwrap_or(ToolMaterial::Carbide);
        let coating = COATINGS
            .get(self.edit_coating.active().unwrap_or(0) as usize)
            .copied()
            .flatten();
        ToolForm {
            id: self.edit_id.text().to_string(),
            number: self.edit_number.value() as u32,
            name: self.edit_name.text().to_string(),
            tool_type,
            material,
            coating,
            shank: Self::shank_from_combo(self.edit_shank.active()),
            diameter: self.edit_diameter.text().to_string(),
            length: self.edit_length.text().to_string(),
            flute_length: self.edit_flute_length.text().to_string(),
            shaft_diameter: self.edit_shaft_diameter.text().to_string(),
            flutes: self.edit_flutes.value() as u32,
            corner_radius: self.edit_corner_radius.text().to_string(),
            tip_angle: self.edit_tip_angle.text().to_string(),
            manufacturer: self.edit_manufacturer.text().to_string(),
            part_number: self.edit_part_number.text().to_string(),
            description: Self::read_text_view(&self.edit_description),
            notes: Self::read_text_view(&self.edit_notes),
        }
    }

    fn build_tool_from_form(&self) -> Result<Tool, String> {
        let base = if *self.is_creating.borrow() {
            None
        } else {
            self.selected_tool.borrow().clone()
        };
        self.read_form()
            .build(base.as_ref())
            .map_err(|msgid| t!(msgid))
    }

    fn is_dirty(&self) -> bool {
        if self.right_panel_stack.visible_child_name().as_deref() != Some("edit") {
            return false;
        }

        let current = match self.build_tool_from_form() {
            Ok(t) => t,
            Err(_) => return true,
        };

        if *self.is_creating.borrow() {
            return true;
        }

        let Some(orig) = self.selected_tool.borrow().clone() else {
            return true;
        };

        !tool_contents_equal(&orig, &current)
    }

    /// Asks whether to save the edited tool before `proceed` replaces the form.
    ///
    /// * Save: saves the tool and runs `proceed` only if the save succeeded
    ///   (a validation error stays on screen otherwise).
    /// * Discard: runs `proceed` without saving.
    /// * Cancel (or closing the prompt): does nothing.
    fn show_discard_dialog(&self, proceed: impl FnOnce() + 'static) {
        let Some(window) = self.widget.root().and_downcast::<gtk4::Window>() else {
            return;
        };

        let view = self.clone();
        crate::ui::gtk::common::dialog::show_unsaved_changes(
            &t!("Unsaved changes"),
            &t!("The tool has unsaved changes. Save them before continuing?"),
            Some(&window),
            move |choice| match choice {
                UnsavedChangesChoice::Save => {
                    if view.save_tool() {
                        proceed();
                    }
                }
                UnsavedChangesChoice::Discard => proceed(),
                UnsavedChangesChoice::Cancel => {}
            },
        );
    }

    fn update_save_state(&self) {
        // Clear previous error
        self.error_label.set_visible(false);
        self.error_label.set_text("");

        let visible_edit = self.right_panel_stack.visible_child_name().as_deref() == Some("edit");
        if !visible_edit {
            self.save_btn.set_sensitive(false);
            return;
        }

        let dirty = self.is_dirty();
        let valid = self.build_tool_from_form().is_ok();

        self.save_btn.set_sensitive(dirty && valid);
    }

    fn cancel_edit_with_confirm(&self) {
        if self.is_dirty() {
            let view = self.clone();
            self.show_discard_dialog(move || {
                view.cancel_edit();
            });
            return;
        }
        self.cancel_edit();
    }

    /// Saves the tool in the form, showing any validation error in the form.
    ///
    /// # Returns
    /// `true` when the tool was saved.
    pub(crate) fn save_tool(&self) -> bool {
        self.error_label.set_visible(false);
        self.error_label.set_text("");

        let tool = match self.build_tool_from_form() {
            Ok(t) => t,
            Err(e) => {
                self.error_label.set_text(&e);
                self.error_label.set_visible(true);
                return false;
            }
        };

        // Validate uniqueness on create
        if *self.is_creating.borrow() {
            let backend = self.backend.borrow();
            if backend.get_tool(&tool.id).is_some() {
                self.error_label
                    .set_text("Tool ID already exists. Please choose a unique ID.");
                self.error_label.set_visible(true);
                return false;
            }
        }

        // A tool that could not be written is not saved: keep the form as it
        // is (still unsaved) and show why.
        let saved = self.backend.borrow_mut().save_tool(tool.clone());
        if let Err(e) = saved {
            tracing::warn!("Failed to save tool: {e}");
            self.error_label
                .set_text(&format!("{} {e}", t!("Could not save the tool:")));
            self.error_label.set_visible(true);
            return false;
        }

        *self.last_selected_tool_id.borrow_mut() = Some(tool.id.0.clone());
        self.persist_ui_state();

        self.load_tools();
        self.load_tool_for_edit(&tool.id.0);
        self.update_save_state();
        true
    }

    fn delete_tool(&self) {
        if let Some(ref tool) = *self.selected_tool.borrow() {
            let tool_id = tool.id.clone();

            if let Some(window) = self.widget.root().and_downcast::<gtk4::Window>() {
                let backend = self.backend.clone();
                let view = self.clone();

                crate::ui::gtk::common::dialog::show_question(
                    "Delete Tool?",
                    &format!(
                        "Are you sure you want to delete tool '{}' (id: {})?\n\nThis action cannot be undone.",
                        tool.name, tool.id.0
                    ),
                    Some(&window),
                    move |response| {
                        if response != gtk4::ResponseType::Yes {
                            return;
                        }
                        let removed = backend.borrow_mut().remove_tool(&tool_id);
                        match removed {
                            Ok(_) => {
                                view.load_tools();
                                view.cancel_edit();
                            }
                            // The tool was not deleted on disk, so keep it and
                            // its form, and say why.
                            Err(e) => {
                                tracing::warn!("Failed to delete tool: {e}");
                                crate::ui::gtk::common::dialog::show_error(
                                    &t!("Delete Failed"),
                                    &format!("{} {e}", t!("Could not delete the tool:")),
                                    view.widget.root().and_downcast::<gtk4::Window>().as_ref(),
                                );
                            }
                        }
                    },
                );
            }
        }
    }

    fn cancel_edit(&self) {
        *self.is_creating.borrow_mut() = false;
        *self.selected_tool.borrow_mut() = None;
        self.clear_form();
        self.right_panel_stack.set_visible_child_name("empty");
        self.save_btn.set_sensitive(false);
        self.cancel_btn.set_sensitive(false);
        self.delete_btn.set_sensitive(false);
    }

    fn clear_form(&self) {
        self.edit_id.set_text("");
        self.edit_number.set_value(1.0);
        self.edit_name.set_text("");
        self.edit_tool_type.set_active(Some(0));
        self.edit_material.set_active(Some(1));
        self.edit_coating.set_active(Some(0));
        self.edit_shank.set_active(Some(0));

        self.edit_diameter.set_text("6.350");
        self.edit_length.set_text("50.000");
        self.edit_flute_length.set_text("20.000");
        self.edit_shaft_diameter.set_text("6.350");
        self.edit_flutes.set_value(2.0);
        self.edit_corner_radius.set_text("");
        self.edit_tip_angle.set_text("");

        self.edit_manufacturer.set_text("");
        self.edit_part_number.set_text("");
        self.edit_description.buffer().set_text("");
        self.edit_notes.buffer().set_text("");

        self.error_label.set_text("");
        self.error_label.set_visible(false);

        self.update_type_dependent_fields();
    }

    fn import_gtc_zip(&self) {
        let Some(window) = self.widget.root().and_downcast::<gtk4::Window>() else {
            return;
        };

        let dialog = crate::ui::gtk::file_dialog::open_dialog("Import GTC Package");

        let backend = self.backend.clone();
        let view = self.clone();

        dialog.pick_open(Some(&window), move |file| {
            if let Some(path) = file.path() {
                let mut backend = backend.borrow_mut();
                match backend.import_gtc_package(path) {
                    Ok(result) => {
                        view.load_tools();
                        view.show_info_dialog(
                            "Import Complete",
                            &format!(
                                "Imported: {}\nSkipped: {}\nErrors: {}",
                                result.imported_tools.len(),
                                result.skipped_tools,
                                result.errors.len()
                            ),
                        );
                    }
                    Err(e) => {
                        view.show_error_dialog("Import Failed", &e.to_string());
                    }
                }
            }
        });
    }

    fn import_gtc_json(&self) {
        let Some(window) = self.widget.root().and_downcast::<gtk4::Window>() else {
            return;
        };

        let dialog = crate::ui::gtk::file_dialog::open_dialog("Import GTC JSON");

        let backend = self.backend.clone();
        let view = self.clone();

        dialog.pick_open(Some(&window), move |file| {
            if let Some(path) = file.path() {
                let mut backend = backend.borrow_mut();
                match backend.import_gtc_json(path) {
                    Ok(result) => {
                        view.load_tools();
                        view.show_info_dialog(
                            "Import Complete",
                            &format!(
                                "Imported: {}\nSkipped: {}\nErrors: {}",
                                result.imported_tools.len(),
                                result.skipped_tools,
                                result.errors.len()
                            ),
                        );
                    }
                    Err(e) => {
                        view.show_error_dialog("Import Failed", &e.to_string());
                    }
                }
            }
        });
    }

    fn export_custom_tools(&self) {
        let backend = self.backend.clone();
        let view = self.clone();

        let dialog = rfd::FileDialog::new()
            .set_title("Export Custom Tools")
            .add_filter("JSON", &["json"]);

        // Use the platform helper for proper parent window handling
        if let Some(path) = platform::save_file_with_parent(dialog) {
            let backend = backend.borrow();
            match backend.export_custom_tools(path) {
                Ok(_) => view.show_info_dialog("Export Complete", "Custom tools exported."),
                Err(e) => view.show_error_dialog("Export Failed", &e.to_string()),
            }
        }
    }

    fn reset_custom_tools(&self) {
        let Some(window) = self.widget.root().and_downcast::<gtk4::Window>() else {
            return;
        };

        let backend = self.backend.clone();
        let view = self.clone();

        crate::ui::gtk::common::dialog::show_question(
            "Reset custom tools?",
            "This will delete all custom/imported tools and remove local custom tools storage.\n\nContinue?",
            Some(&window),
            move |resp| {
                if resp == gtk4::ResponseType::Yes {
                    let mut backend = backend.borrow_mut();
                    match backend.reset_custom_tools() {
                        Ok(_) => {
                            view.cancel_edit();
                            view.load_tools();
                            view.show_info_dialog("Reset Complete", "Custom tools removed.");
                        }
                        Err(e) => view.show_error_dialog("Reset Failed", &e.to_string()),
                    }
                }
            },
        );
    }

    fn show_info_dialog(&self, title: &str, message: &str) {
        if let Some(window) = self.widget.root().and_downcast::<gtk4::Window>() {
            crate::ui::gtk::common::dialog::show_info(title, message, Some(&window));
        }
    }

    fn show_error_dialog(&self, title: &str, message: &str) {
        let parent = crate::ui::gtk::file_dialog::parent_window(&self.widget);
        crate::ui::gtk::file_dialog::show_error_dialog(title, message, parent.as_ref());
    }
}
