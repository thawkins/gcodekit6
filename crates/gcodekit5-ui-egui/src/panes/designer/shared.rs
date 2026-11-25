//! [`SharedDesigner`]: the Designer pane behind an `Rc<RefCell<..>>`, so the workspace draws
//! it while the app routes commands to it and reads its modified flag.
//!
//! Dependencies: `egui`, `gcodekit5-app` (translations), `gcodekit5-settings`.

use std::cell::RefCell;
use std::rc::Rc;

use gcodekit5_app::i18n::Translator;
use gcodekit5_settings::config::Config;

use super::DesignerPane;
use crate::commands::Command;
use crate::workspace::{Pane, PaneContext, PaneId};

/// The designer pane shared between the workspace, which draws it, and the app, which routes
/// commands to it and reads its modified flag (and, later, the layers, properties and file
/// commands).
#[derive(Debug, Clone)]
pub struct SharedDesigner(pub Rc<RefCell<DesignerPane>>);

impl Pane for SharedDesigner {
    fn id(&self) -> PaneId {
        PaneId::Designer
    }

    fn title(&self, tr: &Translator) -> String {
        tr.tr(PaneId::Designer.title_msgid()).into_owned()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut PaneContext) {
        match self.0.try_borrow_mut() {
            Ok(mut pane) => pane.ui(ui, ctx),
            Err(error) => tracing::warn!(%error, "designer busy; not drawn this frame"),
        }
    }

    fn command(&mut self, command: Command, ctx: &mut PaneContext) -> bool {
        match self.0.try_borrow_mut() {
            Ok(mut pane) => pane.command(command, ctx),
            Err(error) => {
                tracing::warn!(%error, ?command, "designer busy; command dropped");
                false
            }
        }
    }

    fn configure(&mut self, config: &Config) {
        match self.0.try_borrow_mut() {
            Ok(mut pane) => pane.configure(config),
            Err(error) => tracing::warn!(%error, "designer busy; settings not applied"),
        }
    }
}
