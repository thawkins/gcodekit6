//! The CAM Tools pane as the workspace holds it: shared with the app, which takes the
//! pane's requests, and holding machine events that arrive while the pane is borrowed.
//!
//! Dependencies: `egui`, `gcodekit5-app` (events, translations), `gcodekit5-settings`.

use std::cell::RefCell;
use std::rc::Rc;

use gcodekit5_app::i18n::Translator;
use gcodekit5_app::machine::MachineEvent;
use gcodekit5_settings::config::Config;

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
        }
    }

    /// How many machine events wait for the pane.
    #[doc(hidden)]
    pub fn waiting_events(&self) -> usize {
        self.waiting.len()
    }

    /// Hands the waiting machine events to `pane`.
    fn deliver_waiting(&mut self, pane: &mut CamToolsPane, tr: &Translator) {
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
}
