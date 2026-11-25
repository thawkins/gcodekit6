//! The pane's connection section: a status strip (port or host, baud rate, state), the serial
//! port list with a refresh button (or the host and TCP port of a network connection), and
//! the Connect / Disconnect button.
//!
//! Dependencies: `egui`, `gcodekit5-communication` (serial port list),
//! `gcodekit5-settings` (connection settings).

use gcodekit5_app::i18n::Translator;
use gcodekit5_settings::{ConnectionSettings, ConnectionType};

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
    /// Serial ports found by the last refresh; `None` before the first one.
    ports: Option<Vec<String>>,
    connecting: bool,
}

impl ConnectionForm {
    /// A form starting from `settings`.
    pub(super) fn new(settings: ConnectionSettings) -> Self {
        Self {
            settings,
            ports: None,
            connecting: false,
        }
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
    /// `"Auto"` or empty port becomes the first port found, as the GTK front end chose it.
    fn refresh_ports(&mut self) {
        let ports: Vec<String> = match gcodekit5_communication::list_ports() {
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

    /// Draws the section.
    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        tr: &Translator,
        status: &MachineStatus,
    ) -> Option<Action> {
        let network = !matches!(self.settings.connection_type, ConnectionType::Serial);
        ui.strong(tr.tr("Connection"));
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
            let state = if status.connected {
                tr.tr("Connected")
            } else if self.connecting {
                tr.tr("Connecting…")
            } else {
                tr.tr("Disconnected")
            };
            ui.label(state);
        });
        let editable = !status.connected && !self.connecting;
        ui.add_enabled_ui(editable, |ui| {
            if network {
                self.show_host(ui, tr);
            } else {
                self.show_ports(ui, tr);
            }
        });
        let (label, action) = if status.connected || self.connecting {
            (tr.tr("Disconnect"), Action::Disconnect)
        } else {
            (tr.tr("Connect"), Action::Connect)
        };
        let button = egui::Button::new(label).min_size(egui::vec2(ui.available_width(), 0.0));
        ui.add(button).clicked().then_some(action)
    }

    /// The host and TCP port of a network connection.
    fn show_host(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        ui.horizontal(|ui| {
            let host = ui.label(tr.tr("Host:"));
            ui.add(egui::TextEdit::singleline(&mut self.settings.port).desired_width(120.0))
                .labelled_by(host.id);
            let port = ui.label(tr.tr("Port:"));
            ui.add(egui::DragValue::new(&mut self.settings.tcp_port).range(1..=65535))
                .labelled_by(port.id);
        });
    }

    /// The serial port list and its refresh button.
    fn show_ports(&mut self, ui: &mut egui::Ui, tr: &Translator) {
        if self.ports.is_none() {
            self.refresh_ports();
        }
        ui.horizontal(|ui| {
            let ports = self.ports.as_deref().unwrap_or_default();
            let selected = if ports.is_empty() && self.settings.port.trim().is_empty() {
                tr.tr("No ports available").into_owned()
            } else {
                self.settings.port.clone()
            };
            let label = ui.label(tr.tr("Port:"));
            egui::ComboBox::from_id_salt("machine-control-port")
                .selected_text(selected)
                .width(160.0)
                .show_ui(ui, |ui| {
                    for port in ports {
                        ui.selectable_value(&mut self.settings.port, port.clone(), port);
                    }
                })
                .response
                .labelled_by(label.id);
            if ui.button(tr.tr("Refresh ports")).clicked() {
                self.refresh_ports();
            }
        });
    }
}
