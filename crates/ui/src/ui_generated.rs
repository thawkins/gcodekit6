// Minimal generated UI shim used for CI and --all-features builds.
// This provides a tiny `MainWindow` type with `new()` and `run()` so the
// crate builds even when real Slint-generated modules are not present.
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

// Minimal MainWindow stub used by tests and CI. It stores a device model
// and a simple connect handler so tests can exercise model updates.
pub struct MainWindow {
    model: Option<std::sync::Arc<std::sync::Mutex<VecModel<Device>>>>,
    connect_handler: Option<Box<dyn Fn(&mut MainWindow, String) + 'static>>,
}

impl MainWindow {
    pub fn new() -> Self {
        Self { model: None, connect_handler: None }
    }

    pub fn run(&self) {
        tracing::info!("gcodekit-ui: fake MainWindow running (generated stub)");
    }

    #[allow(non_snake_case)]
    pub fn set_deviceModel(&mut self, model: VecModel<Device>) {
        self.model = Some(std::sync::Arc::new(std::sync::Mutex::new(model)));
    }

    #[allow(non_snake_case)]
    pub fn on_connectRequested<F>(&mut self, f: F)
    where
        F: Fn(&mut MainWindow, String) + 'static,
    {
        self.connect_handler = Some(Box::new(f));
    }

    pub fn invoke_connect(&mut self, id: String) {
        if let Some(h) = self.connect_handler.take() {
            (h)(self, id);
            self.connect_handler = Some(h);
        }
    }

    #[allow(dead_code)]
    pub fn model_len(&self) -> usize {
        if let Some(m) = &self.model {
            match m.lock() {
                Ok(guard) => {
                    #[cfg(feature = "with-slint")]
                    {
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

    #[allow(dead_code)]
    pub fn model_items(&self) -> Vec<Device> {
        if let Some(m) = &self.model {
            match m.lock() {
                Ok(guard) => {
                    let mut out = Vec::new();
                    #[cfg(feature = "with-slint")]
                    {
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
    fn default() -> Self { Self::new() }
}
