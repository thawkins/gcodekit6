//! The CAM Tools pane as the workspace holds it: shared with the app, which takes the
//! pane's requests, and holding machine events that arrive while the pane is borrowed.
//!
//! Dependencies: `egui`, `gcodekit6-app` (events, translations), `gcodekit6-settings`.

use std::cell::RefCell;
use std::rc::Rc;

use gcodekit6_app::i18n::Translator;
use gcodekit6_app::machine::MachineEvent;
use gcodekit6_devicedb::DeviceProfile;
use gcodekit6_settings::config::Config;

use super::CamToolsPane;
use crate::workspace::{Pane, PaneContext, PaneId};

/// The CAM Tools pane shared between the workspace, which draws it, and the app, which takes
/// its editor requests.
///
/// Machine events that arrive while the pane is borrowed elsewhere wait here and are handed
/// over at the next chance (never dropped: a probe run must see every event in order).
#[derive(Debug, Clone)]
pub struct SharedCamTools {
    pane: Rc<RefCell<CamToolsPane>>,
    waiting: Vec<MachineEvent>,
    /// An axis count that arrived while the pane was borrowed elsewhere.
    waiting_axes: Option<u8>,
}

impl SharedCamTools {
    /// Shares `pane`.
    ///
    /// # Arguments
    /// * `pane` - The pane the app also holds.
    pub fn new(pane: Rc<RefCell<CamToolsPane>>) -> Self {
        Self {
            pane,
            waiting: Vec::new(),
            waiting_axes: None,
        }
    }

    /// How many machine events wait for the pane.
    #[doc(hidden)]
    pub fn waiting_events(&self) -> usize {
        self.waiting.len()
    }

    /// Hands the waiting machine events to `pane`.
    fn deliver_waiting(&mut self, pane: &mut CamToolsPane, tr: &Translator) {
        if let Some(axes) = self.waiting_axes.take() {
            pane.set_num_axes(axes);
        }
        if !self.waiting.is_empty() {
            let waiting = std::mem::take(&mut self.waiting);
            pane.probe_events(&waiting, tr);
        }
    }
}

impl Pane for SharedCamTools {
    fn id(&self) -> PaneId {
        PaneId::CamTools
    }

    fn title(&self, tr: &Translator) -> String {
        tr.tr(PaneId::CamTools.title_msgid()).into_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext) {
        let pane = Rc::clone(&self.pane);
        match pane.try_borrow_mut() {
            Ok(mut pane) => {
                self.deliver_waiting(&mut pane, ctx.tr);
                pane.ui(ui, ctx.tr, ctx.machine());
            }
            Err(error) => tracing::warn!(%error, "CAM tools busy; not drawn this frame"),
        }
    }

    fn machine_events(&mut self, events: &[MachineEvent], tr: &Translator) {
        let pane = Rc::clone(&self.pane);
        match pane.try_borrow_mut() {
            Ok(mut pane) => {
                self.deliver_waiting(&mut pane, tr);
                pane.probe_events(events, tr);
            }
            Err(error) => {
                tracing::debug!(%error, "CAM tools busy; machine events wait");
                self.waiting.extend_from_slice(events);
            }
        }
    }

    fn configure(&mut self, config: &Config) {
        match self.pane.try_borrow_mut() {
            Ok(mut pane) => pane.set_units(config.ui.measurement_system),
            Err(error) => tracing::warn!(%error, "CAM tools busy; settings not applied"),
        }
    }

    /// Device facts reach the CAM pages through this call, never through a process-wide
    /// static: the axis count comes from the active profile (3 without one). Laser mode
    /// comes from the controller's reported `$32` ([`crate::machine::MachineStatus`]).
    fn set_device(&mut self, profile: Option<&DeviceProfile>) {
        let axes = profile.map_or(3, |p| p.num_axes);
        match self.pane.try_borrow_mut() {
            Ok(mut pane) => pane.set_num_axes(axes),
            Err(error) => {
                tracing::debug!(%error, "CAM tools busy; device waits");
                self.waiting_axes = Some(axes);
            }
        }
    }
}
