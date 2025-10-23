// Synthetic Slint-generated shim used when real Slint codegen isn't available.
// This matches the expected API surface used by `main.rs` and the typed
// binding helpers: MainWindow with `new`, `run`, `set_deviceModel`, and
// `on_connectRequested`, plus the `Device` struct used by the model.

use std::sync::{Arc, Mutex};

// When using the real Slint crate, bring the Model trait into scope so
// methods like `len()` and `row_data()` are available on VecModel's guard.
#[cfg(feature = "with-slint")]
use slint::Model;

// Provide a small VecModel shim when Slint is not available so the UI crate
// and tests can operate without pulling in the Slint runtime. When Slint is
// enabled we re-export its VecModel type for compatibility.
#[cfg(feature = "with-slint")]
pub use slint::VecModel;

#[cfg(not(feature = "with-slint"))]
pub struct VecModel<T>(pub Vec<T>);

#[cfg(not(feature = "with-slint"))]
impl<T: Clone> VecModel<T> {
    pub fn from(v: Vec<T>) -> Self {
        VecModel(v)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn row_data(&self, i: usize) -> Option<T> {
        self.0.get(i).cloned()
    }
}

#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Default)]
pub struct Device {
    pub id: String,
    pub display: String,
    pub transport: String,
    pub product: String,
    pub manufacturer: String,
    pub vid: String,
    pub pid: String,
}

#[allow(non_camel_case_types)]
pub struct MainWindow {
    // store a device model so set_deviceModel() can accept it
    model: Option<Arc<Mutex<VecModel<Device>>>>,
    // simple connect handler storage (no Send/Sync requirement; UI callbacks
    // typically run on the UI thread). Handler is called with `&mut self` so
    // closures do not have to capture the `ui` binding from the environment.
    connect_handler: Option<Box<dyn Fn(&mut MainWindow, String) + 'static>>,
}

impl MainWindow {
    pub fn new() -> Self {
        Self { model: None, connect_handler: None }
    }

    pub fn run(&self) {
        tracing::info!("gcodekit-ui (synthetic): MainWindow running");
    }

    #[allow(non_snake_case)]
    pub fn set_deviceModel(&mut self, model: VecModel<Device>) {
        self.model = Some(Arc::new(Mutex::new(model)));
    }

    #[allow(non_snake_case)]
    pub fn on_connectRequested<F>(&mut self, f: F)
    where
        F: Fn(&mut MainWindow, String) + 'static,
    {
        self.connect_handler = Some(Box::new(f));
    }

    // Helper to invoke the stored handler (in a real Slint UI this would be
    // called by the UI runtime when the user clicks the Connect button).
    pub fn invoke_connect(&mut self, id: String) {
        if let Some(h) = self.connect_handler.take() {
            (h)(self, id);
            // put it back so subsequent calls still work
            self.connect_handler = Some(h);
        }
    }

    /// Test helper: return the number of items in the attached model (0 when none).
    #[allow(dead_code)]
    pub fn model_len(&self) -> usize {
        if let Some(m) = &self.model {
            match m.lock() {
                Ok(guard) => {
                    #[cfg(feature = "with-slint")]
                    {
                        // When using real Slint VecModel, ask the Model trait for
                        // the row count.
                        return slint::Model::row_count(&*guard);
                    }
                    #[cfg(not(feature = "with-slint"))]
                    {
                        return guard.len();
                    }
                }
                Err(_) => 0,
            }
        } else {
            0
        }
    }

    /// Test helper: clone model items into a Vec for inspection in tests.
    #[allow(dead_code)]
    pub fn model_items(&self) -> Vec<Device> {
        if let Some(m) = &self.model {
            match m.lock() {
                Ok(guard) => {
                    // VecModel supports iteration via Model trait when using
                    // the real Slint VecModel. Use conditional compilation to
                    // call the correct API depending on whether `with-slint`
                    // is enabled.
                    let mut out = Vec::new();
                    #[cfg(feature = "with-slint")]
                    {
                        // Model::row_count and Model::row_data are provided by
                        // the slint::Model trait (imported at top when
                        // with-slint is enabled).
                        let count = slint::Model::row_count(&*guard);
                        for i in 0..count {
                            if let Some(d) = slint::Model::row_data(&*guard, i) {
                                out.push(d.clone());
                            }
                        }
                    }
                    #[cfg(not(feature = "with-slint"))]
                    {
                        for i in 0..guard.len() {
                            if let Some(d) = guard.row_data(i) {
                                out.push(d.clone());
                            }
                        }
                    }
                    out
                }
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        }
    }
}

impl Default for MainWindow {
    fn default() -> Self {
        Self::new()
    }
}
