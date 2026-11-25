//! gcodekit5-ui/src/ui/gtk/device_manager/operations.rs
//! Event handlers and operations for the device manager window.

use super::*;
use crate::t;
use gcodekit5_app::device_profiles::{self, ProfileForm, ReportedLimits};
use gcodekit5_devicedb::ControllerType;

impl DeviceManagerWindow {
    pub(crate) fn setup_event_handlers(self: &Rc<Self>) {
        // New device button
        let view = self.clone();
        self.new_btn.connect_clicked(move |_| {
            view.start_create_new();
        });

        // Save button
        let view = self.clone();
        self.save_btn.connect_clicked(move |_| {
            view.save_device();
        });

        // Cancel button
        let view = self.clone();
        self.cancel_btn.connect_clicked(move |_| {
            view.cancel_edit();
        });

        // Delete button
        let view = self.clone();
        self.delete_btn.connect_clicked(move |_| {
            view.delete_device();
        });

        // Sync from Device button
        let view = self.clone();
        self.sync_btn.connect_clicked(move |_| {
            view.sync_from_device();
        });

        // Set Active button
        let view = self.clone();
        self.set_active_btn.connect_clicked(move |_| {
            view.set_as_active();
        });

        // Search
        let view = self.clone();
        self.search_entry
            .connect_search_changed(move |_| view.load_devices());

        // Connection field toggles
        let view = self.clone();
        self.edit_connection_type
            .connect_changed(move |_| view.update_connection_field_sensitivity());

        // Capability field toggles
        let view = self.clone();
        self.edit_has_spindle
            .connect_toggled(move |_| view.update_capabilities_field_sensitivity());
        let view = self.clone();
        self.edit_has_laser
            .connect_toggled(move |_| view.update_capabilities_field_sensitivity());

        // List selection
        let view = self.clone();
        self.devices_list.connect_row_activated(move |_, row| {
            let device_id = row.widget_name();
            if !device_id.is_empty() {
                view.load_device_for_edit(device_id.as_str());
            }
        });
    }

    pub(crate) fn load_devices(&self) {
        // Clear list
        while let Some(child) = self.devices_list.first_child() {
            self.devices_list.remove(&child);
        }

        let mut profiles = self.controller.get_ui_profiles();
        let q = self.search_entry.text().trim().to_lowercase();
        if !q.is_empty() {
            profiles.retain(|p| {
                let hay = format!(
                    "{}\n{}\n{}\n{}\n{}",
                    p.name, p.description, p.device_type, p.controller_type, p.connection_type
                )
                .to_lowercase();
                hay.contains(&q)
            });
        }

        let status = device_status::get_status();
        let connected_port = status.port_name.clone().unwrap_or_default();
        let is_connected = status.is_connected;

        for profile in profiles {
            let row_box = Box::new(Orientation::Vertical, 5);

            // Name Label (Wrapped)
            let name_label = Label::new(Some(&profile.name));
            name_label.add_css_class("title-4");
            name_label.set_halign(Align::Start);
            name_label.set_wrap(true);
            name_label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
            name_label.set_xalign(0.0);
            row_box.append(&name_label);

            // Details and Badge container
            let details_box = Box::new(Orientation::Horizontal, 5);

            // Device Info
            let info = format!("{} - {}", profile.device_type, profile.controller_type);
            let info_label = Label::new(Some(&info));
            info_label.add_css_class("dim-label");
            info_label.set_halign(Align::Start);
            info_label.set_hexpand(true); // Pushes badge to the right
            details_box.append(&info_label);

            // Connected badge (serial-only for now)
            if is_connected
                && profile.connection_type == "Serial"
                && !connected_port.is_empty()
                && profile.port.trim() == connected_port.trim()
            {
                let badge = Label::new(Some("Connected"));
                badge.add_css_class("active-badge");
                badge.set_halign(Align::End);
                badge.set_valign(Align::Center);
                details_box.append(&badge);
            }

            // Active Badge
            if profile.is_active {
                let badge = Label::new(Some("Active"));
                badge.add_css_class("active-badge");
                badge.set_halign(Align::End);
                badge.set_valign(Align::Center);
                details_box.append(&badge);
            }

            row_box.append(&details_box);

            let row = ListBoxRow::new();
            row.set_margin_top(5);
            row.set_margin_bottom(5);
            row.set_margin_start(10);
            row.set_margin_end(10);
            row.set_child(Some(&row_box));
            row.set_widget_name(&profile.id);

            self.devices_list.append(&row);
        }
    }

    pub(crate) fn start_create_new(&self) {
        match self.controller.create_new_profile() {
            Ok(id) => {
                self.load_devices();
                self.load_device_for_edit(&id);
            }
            _ => {
                self.load_devices();
            }
        }
    }

    pub(crate) fn load_device_for_edit(&self, device_id: &str) {
        let profiles = self.controller.get_ui_profiles();

        if let Some(profile) = profiles.iter().find(|p| p.id == device_id) {
            *self.selected_device.borrow_mut() = Some(profile.clone());

            // Switch to Edit View
            self.edit_stack.set_visible_child_name("edit_form");

            // Load into form
            self.edit_name.set_text(&profile.name);
            self.edit_description.set_text(&profile.description);

            self.edit_device_type
                .set_active_id(Some(profile.device_type.as_str()));
            self.edit_controller_type
                .set_active_id(Some(profile.controller_type.as_str()));

            self.edit_connection_type
                .set_active_id(Some(profile.connection_type.as_str()));
            self.edit_port.set_text(&profile.port);
            self.edit_baud_rate
                .set_active_id(Some(profile.baud_rate.as_str()));
            self.edit_tcp_host.set_text(profile.tcp_host.trim());
            self.edit_tcp_port.set_text(profile.tcp_port.trim());
            self.edit_timeout.set_text(profile.timeout_ms.trim());
            self.edit_auto_reconnect.set_active(profile.auto_reconnect);

            self.refresh_units_display();

            self.edit_has_spindle.set_active(profile.has_spindle);
            self.edit_has_laser.set_active(profile.has_laser);
            self.edit_has_coolant.set_active(profile.has_coolant);
            self.edit_pen_mode.set_active(profile.pen_mode);
            self.edit_num_axes.set_text(profile.num_axes.trim());
            self.edit_max_s_value.set_text(profile.max_s_value.trim());
            self.edit_spindle_watts
                .set_text(profile.cnc_spindle_watts.trim());
            self.edit_max_spindle_speed_rpm
                .set_text(profile.max_spindle_speed_rpm.trim());
            self.edit_laser_watts.set_text(profile.laser_watts.trim());

            self.update_connection_field_sensitivity();
            self.update_capabilities_field_sensitivity();

            // Enable buttons
            self.save_btn.set_sensitive(true);
            self.cancel_btn.set_sensitive(true);
            self.delete_btn.set_sensitive(true);
            self.set_active_btn.set_sensitive(!profile.is_active);

            // Enable sync button only if a device is connected
            let status = device_status::get_status();
            self.sync_btn.set_sensitive(status.is_connected);
        }
    }

    pub(crate) fn update_connection_field_sensitivity(&self) {
        let conn = self
            .edit_connection_type
            .active_id()
            .map(|s| s.to_string())
            .or_else(|| {
                self.edit_connection_type
                    .active_text()
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "Serial".to_string());

        match conn.as_str() {
            "Serial" => {
                self.edit_port.set_sensitive(true);
                self.edit_baud_rate.set_sensitive(true);
                self.edit_tcp_host.set_sensitive(false);
                self.edit_tcp_port.set_sensitive(false);
            }
            "TCP/IP" | "WebSocket" => {
                self.edit_port.set_sensitive(false);
                self.edit_baud_rate.set_sensitive(false);
                self.edit_tcp_host.set_sensitive(true);
                self.edit_tcp_port.set_sensitive(true);
            }
            _ => {}
        }
    }

    pub(crate) fn update_capabilities_field_sensitivity(&self) {
        let has_spindle = self.edit_has_spindle.is_active();
        self.edit_spindle_watts.set_sensitive(has_spindle);
        self.edit_max_spindle_speed_rpm.set_sensitive(has_spindle);

        let has_laser = self.edit_has_laser.is_active();
        self.edit_laser_watts.set_sensitive(has_laser);
    }

    pub(crate) fn show_error_dialog(&self, title: &str, details: &str) {
        let parent = crate::ui::gtk::file_dialog::parent_window(&self.widget);
        crate::ui::gtk::file_dialog::show_error_dialog(title, details, parent.as_ref());
    }

    pub(crate) fn save_device(&self) {
        // Collect model first, then drop the borrow
        let model_opt = self.selected_device.borrow().clone();
        let Some(model) = model_opt else {
            return;
        };
        let text = |entry: &Entry| entry.text().to_string();
        let device_type = self
            .edit_device_type
            .active_text()
            .and_then(|t| device_profiles::device_type_from_label(&t))
            .unwrap_or_default();
        let controller_type = self
            .edit_controller_type
            .active_id()
            .and_then(|id| ControllerType::from_name(&id))
            .unwrap_or_default();
        let connection_type = self
            .edit_connection_type
            .active_text()
            .map_or_else(|| "Serial".to_string(), |t| t.to_string());
        let baud_rate = self
            .edit_baud_rate
            .active_text()
            .and_then(|t| t.parse().ok())
            .unwrap_or(115_200);
        let form = ProfileForm {
            name: text(&self.edit_name),
            description: text(&self.edit_description),
            device_type,
            controller_type,
            connection_type,
            port: text(&self.edit_port),
            baud_rate,
            tcp_host: text(&self.edit_tcp_host),
            tcp_port: text(&self.edit_tcp_port),
            timeout_ms: text(&self.edit_timeout),
            auto_reconnect: self.edit_auto_reconnect.is_active(),
            x_min: text(&self.edit_x_min),
            x_max: text(&self.edit_x_max),
            y_min: text(&self.edit_y_min),
            y_max: text(&self.edit_y_max),
            z_min: text(&self.edit_z_min),
            z_max: text(&self.edit_z_max),
            has_spindle: self.edit_has_spindle.is_active(),
            has_laser: self.edit_has_laser.is_active(),
            has_coolant: self.edit_has_coolant.is_active(),
            pen_mode: self.edit_pen_mode.is_active(),
            num_axes: text(&self.edit_num_axes),
            max_feed_rate: text(&self.edit_max_feed_rate),
            max_s_value: text(&self.edit_max_s_value),
            spindle_watts: text(&self.edit_spindle_watts),
            max_spindle_speed_rpm: text(&self.edit_max_spindle_speed_rpm),
            laser_watts: text(&self.edit_laser_watts),
        };

        // The shared form rules (gcodekit5-app) check every field, including whether the
        // controller is supported, before anything is saved.
        let units = *self.current_units.borrow();
        let feed_units = *self.current_feed_units.borrow();
        let base = self.controller.get_profile(&model.id).unwrap_or_default();
        let mut profile = match form.apply(&base, units, feed_units) {
            Ok(profile) => profile,
            Err(e) => {
                self.show_error_dialog(&t!(e.title), &e.fill(&t!(e.detail)));
                return;
            }
        };
        profile.id = model.id;

        if let Err(e) = self.controller.save_profile(profile) {
            error!("Failed to save device profile: {}", e);
            self.show_error_dialog("Failed to save device", &e.to_string());
            return;
        }

        self.load_devices();
        self.cancel_edit();
    }

    pub(crate) fn delete_device(self: &Rc<Self>) {
        let Some(model) = self.selected_device.borrow().clone() else {
            return;
        };

        let Some(window) = self.widget.root().and_downcast::<gtk4::Window>() else {
            return;
        };

        let view = self.clone();
        crate::ui::gtk::common::dialog::show_question(
            "Delete device?",
            &format!(
                "Delete \u{201c}{}\u{201d}? This cannot be undone.",
                model.name
            ),
            Some(&window),
            move |resp| {
                if resp == ResponseType::Yes {
                    if let Err(e) = view.controller.delete_profile(&model.id) {
                        tracing::warn!(error = %e, "Failed to delete device profile");
                        view.show_error_dialog("Delete failed", &e.to_string());
                    }
                    view.load_devices();
                    view.cancel_edit();
                }
            },
        );
    }

    pub(crate) fn set_as_active(&self) {
        let id_opt = self.selected_device.borrow().as_ref().map(|d| d.id.clone());
        if let Some(id) = id_opt {
            // Only switch the panel's axis count once the active device is saved,
            // so the two cannot disagree.
            if let Err(e) = self.controller.set_active_profile(&id) {
                tracing::warn!(error = %e, "Failed to set the active device profile");
                self.show_error_dialog("Set active failed", &e.to_string());
                return;
            }
            if let Some(profile) = self.selected_device.borrow().as_ref() {
                let num: u8 = profile.num_axes.trim().parse().unwrap_or(3);
                crate::device_status::set_active_num_axes(num);
            }
            self.load_devices();
            self.cancel_edit();
        }
    }

    pub(crate) fn cancel_edit(&self) {
        *self.selected_device.borrow_mut() = None;
        self.edit_stack.set_visible_child_name("placeholder");
        self.save_btn.set_sensitive(false);
        self.cancel_btn.set_sensitive(false);
        self.delete_btn.set_sensitive(false);
        self.sync_btn.set_sensitive(false);
        self.set_active_btn.set_sensitive(false);
    }

    pub(crate) fn sync_from_device(&self) {
        let status = device_status::get_status();

        if !status.is_connected {
            self.show_error_dialog("Not Connected", "Please connect to a device first.");
            return;
        }

        let model_opt = self.selected_device.borrow().clone();
        let Some(mut model) = model_opt else {
            return;
        };

        // The values the dump gives a profile ($130-$132 travel, $30 spindle speed, $32
        // laser mode) are read by the shared rule in gcodekit5-app.
        let limits = ReportedLimits::from_settings(&status.grbl_settings);
        if let Some(x) = limits.x_max {
            model.x_max = format!("{:.2}", x);
        }
        if let Some(y) = limits.y_max {
            model.y_max = format!("{:.2}", y);
        }
        if let Some(z) = limits.z_max {
            model.z_max = format!("{:.2}", z);
        }
        if let Some(rpm) = limits.max_rpm {
            model.max_spindle_speed_rpm = rpm.to_string();
            model.max_s_value = rpm.to_string();
            if rpm > 0 {
                model.has_spindle = true;
            }
        }
        if let Some(laser) = limits.laser {
            model.has_laser = laser;
        }

        // Reload the updated model into the form
        *self.selected_device.borrow_mut() = Some(model.clone());
        self.edit_name.set_text(&model.name);
        self.edit_description.set_text(&model.description);

        self.refresh_units_display();

        self.edit_has_spindle.set_active(model.has_spindle);
        self.edit_has_laser.set_active(model.has_laser);
        self.edit_max_spindle_speed_rpm
            .set_text(&model.max_spindle_speed_rpm);
        self.edit_max_s_value.set_text(&model.max_s_value);

        self.update_capabilities_field_sensitivity();

        // Show success message
        let Some(window) = self.widget.root().and_downcast::<gtk4::Window>() else {
            return;
        };

        crate::ui::gtk::common::dialog::show_info(
            "Device Information Updated",
            "Device dimensions, spindle speed, and capabilities have been updated from the connected device. Click Save to apply changes.",
            Some(&window),
        );
    }
}
