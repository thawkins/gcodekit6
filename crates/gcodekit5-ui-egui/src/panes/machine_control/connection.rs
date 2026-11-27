//! The pane's connection section: a status strip (port or host, baud rate, state), the serial
//! port list with a refresh button (or the host and TCP port of a network connection), and
//! the Connect / Disconnect button.
//!
//! Dependencies: `egui`, `gcodekit5-communication` (serial port list),
//! `gcodekit5-settings` (connection settings), `gcodekit5-devicedb` (the active device
//! profile's connection and Auto Reconnect setting).

use gcodekit5_app::i18n::Translator;
use gcodekit5_devicedb::DeviceProfile;
use gcodekit5_settings::{ConnectionSettings, ConnectionType};

use crate::icons::{Icon, icon_only_button};
use crate::machine::MachineStatus;

/// The msgids this section shows.
pub(super) const MSGIDS: &[&str] = &[
    n_!("Connection"),
    n_!("Port:"),
    n_!("Host:"),
    n_!("Baud:"),
    n_!("Connected"),
    n_!("Disconnected"),
    n_!("Connecting…"),
    n_!("Connect"),
    n_!("Disconnect"),
    n_!("Refresh ports"),
    n_!("No ports available"),
    n_!("Error connecting"),
    crate::machine::RECONNECTING,
];

/// What the user asked for in the section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    /// Connect with the form's settings.
    Connect,
    /// Close the connection.
    Disconnect,
}

/// The connection form.
#[derive(Debug)]
pub(super) struct ConnectionForm {
    settings: ConnectionSettings,
    /// The configured connection the form started from; a serial device profile goes back
    /// to its connection type, host and port.
    base: ConnectionSettings,
    /// Whether the connection type, host and port came from a network device profile.
    from_profile: bool,
    /// Serial ports found by the last refresh; `None` before the first one.
    ports: Option<Vec<String>>,
    connecting: bool,
}

impl ConnectionForm {
    /// A form starting from `settings`.
    pub(super) fn new(settings: ConnectionSettings) -> Self {
        Self {
            base: settings.clone(),
            from_profile: false,
            settings,
            ports: None,
            connecting: false,
        }
    }

    /// Takes the connection the active device `profile` describes: its Auto Reconnect
    /// setting always; for a network profile (TCP/IP or WebSocket with a host) also its
    /// connection type, host, port and timeout. A serial profile, or none, keeps the
    /// configured connection (and the serial port chosen in the form).
    ///
    /// # Arguments
    /// * `profile` - The active device profile, if one is set.
    pub(super) fn apply_profile(&mut self, profile: Option<&DeviceProfile>) {
        let network = profile.and_then(|p| {
            let kind = match p.connection_type.as_str() {
                "TCP/IP" => ConnectionType::Tcp,
                "WebSocket" => ConnectionType::WebSocket,
                _ => return None,
            };
            let host = p.tcp_host.trim();
            (!host.is_empty()).then(|| (kind, host.to_string(), p.tcp_port, p.timeout_ms))
        });
        match network {
            Some((kind, host, port, timeout_ms)) => {
                self.settings.connection_type = kind;
                self.settings.port = host;
                self.settings.tcp_port = port;
                self.settings.timeout_ms = timeout_ms;
                self.from_profile = true;
            }
            None if self.from_profile => {
                self.from_profile = false;
                self.settings.connection_type = self.base.connection_type;
                self.settings.port = self.base.port.clone();
                self.settings.tcp_port = self.base.tcp_port;
                self.settings.timeout_ms = self.base.timeout_ms;
            }
            None => {}
        }
        self.settings.auto_reconnect =
            profile.map_or(self.base.auto_reconnect, |p| p.auto_reconnect);
    }

    /// The settings to connect with.
    pub(super) fn settings(&self) -> &ConnectionSettings {
        &self.settings
    }

    /// Marks a connect as started (`true`) or settled (`false`).
    pub(super) fn set_connecting(&mut self, connecting: bool) {
        self.connecting = connecting;
    }

    /// Lists the serial ports again. A failure leaves the list empty and is logged. An
    /// `"Auto"` or empty port becomes the first port found.
    fn refresh_ports(&mut self) {
        let ports: Vec<String> = match gcodekit5_app::list_ports() {
            Ok(ports) => ports.into_iter().map(|p| p.port_name).collect(),
            Err(error) => {
                tracing::warn!(%error, "cannot list serial ports");
                Vec::new()
            }
        };
        let port = self.settings.port.trim();
        if (port.is_empty() || port.eq_ignore_ascii_case("auto"))
            && let Some(first) = ports.first()
        {
            self.settings.port = first.clone();
        }
        self.ports = Some(ports);
    }

    /// Whether a connect is under way.
    pub(super) fn connecting(&self) -> bool {
        self.connecting
    }

    /// Draws the status strip (port or host, baud rate, state) at the top of the
    /// sidebar.
    pub(super) fn show_strip(&self, ui: &mut egui::Ui, tr: &Translator, status: &MachineStatus) {
        let network = !matches!(self.settings.connection_type, ConnectionType::Serial);
        ui.horizontal_wrapped(|ui| {
            if network {
                ui.weak(format!(
                    "{} {}:{}",
                    tr.tr("Host:"),
                    self.settings.port,
                    self.settings.tcp_port
                ));
            } else {
                ui.weak(format!("{} {}", tr.tr("Port:"), self.settings.port));
                ui.weak(format!("{} {}", tr.tr("Baud:"), self.settings.baud_rate));
            }
            ui.label(state_text(tr, status, self.connecting));
        });
    }

    /// Draws the section: the port list (or host and TCP port), then Connect or Disconnect
    /// with the refresh button beside it.
    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        tr: &Translator,
        status: &MachineStatus,
    ) -> Option<Action> {
        let network = !matches!(self.settings.connection_type, ConnectionType::Serial);
        crate::theme::section_heading(ui, &tr.tr("Connection"));
        // While Auto Reconnect retries, the connection is still the user's to close.
        let open = status.connected || self.connecting || status.reconnecting.is_some();
        let editable = !open;
        ui.add_enabled_ui(editable, |ui| {
            if network {
                self.show_host(ui, tr);
            } else {
                self.show_ports(ui, tr);
            }
        });
        let mut action = None;
        // Right to left: the refresh button takes its own width first and Connect fills
        // exactly what is left. A fixed allowance for the refresh button made the row wider
        // than the sidebar, and the resizable sidebar grew to fit it on every frame.
        // Inside `horizontal`, so the row keeps one widget's height.
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !network {
                    ui.add_enabled_ui(editable, |ui| {
                        if icon_only_button(ui, Icon::Refresh, &tr.tr("Refresh ports")).clicked() {
                            self.refresh_ports();
                        }
                    });
                }
                let size = egui::vec2(ui.available_width(), 0.0);
                let button = if open {
                    let label = tr.tr("Disconnect");
                    egui::Button::image_and_text(Icon::Disconnected.themed(ui), label)
                        .min_size(size)
                } else {
                    crate::theme::primary_icon_button(Icon::Network.themed(ui), tr.tr("Connect"))
                        .min_size(size)
                };
                if ui.add(button).clicked() {
                    action = Some(if open {
                        Action::Disconnect
                    } else {
                        Action::Connect
                    });
                }
            });
        });
        action
    }

    /// The host and TCP port of a network connection.
    fn show_host(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        ui.horizontal_wrapped(|ui| {
            let host = ui.label(tr.tr("Host:"));
            ui.add(egui::TextEdit::singleline(&mut self.settings.port).desired_width(90.0))
                .labelled_by(host.id);
            let port = ui.label(tr.tr("Port:"));
            ui.add(egui::DragValue::new(&mut self.settings.tcp_port).range(1..=65535))
                .labelled_by(port.id);
        });
    }

    /// The serial port list, as wide as the column; named "Port:" for assistive technology
    /// (the strip above shows the port).
    fn show_ports(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        if self.ports.is_none() {
            self.refresh_ports();
        }
        let ports = self.ports.as_deref().unwrap_or_default();
        let selected = if ports.is_empty() && self.settings.port.trim().is_empty() {
            tr.tr("No ports available").into_owned()
        } else {
            self.settings.port.clone()
        };
        let response = egui::ComboBox::from_id_salt("machine-control-port")
            .selected_text(selected)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for port in ports {
                    ui.selectable_value(&mut self.settings.port, port.clone(), port);
                }
            })
            .response;
        let name = tr.tr("Port:");
        let enabled = ui.is_enabled();
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, enabled, &name));
    }
}

/// The connection state as text: Connected, Connecting..., Reconnecting... (n/m) or
/// Disconnected.
pub(super) fn state_text(
    tr: &Translator,
    status: &MachineStatus,
    connecting: bool,
) -> std::borrow::Cow<'static, str> {
    if status.connected {
        tr.tr("Connected")
    } else if let Some(text) = crate::machine::reconnecting_text(status, tr) {
        text.into()
    } else if connecting {
        tr.tr("Connecting…")
    } else {
        tr.tr("Disconnected")
    }
}
