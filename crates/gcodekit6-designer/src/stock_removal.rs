//! Stock Material Definition
//!
//! Defines the `StockMaterial` struct used by the 3D visualizer and
//! stock removal simulation for CNC machining operations.

/// Represents the stock material dimensions and position
#[derive(Debug, Clone)]
pub struct StockMaterial {
    /// Width in X dimension (mm)
    pub width: f64,
    /// Height in Y dimension (mm)
    pub height: f64,
    /// Thickness in Z dimension (mm)
    pub thickness: f64,
    /// Origin point (bottom-left corner) in world coordinates
    pub origin: (f64, f64, f64),
    /// Safe Z height for rapid moves (mm)
    pub safe_z: f64,
}

impl StockMaterial {
    /// Default clearance above stock top (mm) for safe Z moves.
    pub const DEFAULT_SAFE_Z_ABOVE_STOCK_MM: f64 = 5.0;
    /// Minimum clearance above stock top (mm) for safe Z moves.
    pub const MIN_SAFE_Z_ABOVE_STOCK_MM: f64 = 1.0;

    /// Returns the default safe Z for a given stock thickness.
    pub fn default_safe_z_for_thickness(thickness: f64) -> f64 {
        thickness.max(0.0) + Self::DEFAULT_SAFE_Z_ABOVE_STOCK_MM
    }

    /// Create a new stock material definition
    pub fn new(width: f64, height: f64, thickness: f64, origin: (f64, f64, f64)) -> Self {
        let default_safe_z = Self::default_safe_z_for_thickness(thickness);
        let mut stock = Self {
            width,
            height,
            thickness,
            origin,
            safe_z: default_safe_z,
        };
        stock.normalize_safe_z();
        stock
    }

    /// Returns the minimum safe Z required for a given stock thickness.
    pub fn min_safe_z_for_thickness(thickness: f64) -> f64 {
        thickness.max(0.0) + Self::MIN_SAFE_Z_ABOVE_STOCK_MM
    }

    /// Clamps stock thickness/safe-Z so safe-Z is always above stock top.
    pub fn normalize_safe_z(&mut self) {
        self.thickness = self.thickness.max(0.0);
        let min_safe_z = Self::min_safe_z_for_thickness(self.thickness);
        if self.safe_z < min_safe_z {
            self.safe_z = min_safe_z;
        }
    }

    /// Get the center point of the stock
    pub fn center(&self) -> (f64, f64, f64) {
        (
            self.origin.0 + self.width / 2.0,
            self.origin.1 + self.height / 2.0,
            self.origin.2 + self.thickness / 2.0,
        )
    }

    /// Get the top surface Z coordinate
    pub fn top_z(&self) -> f64 {
        self.origin.2 + self.thickness
    }

    /// Check if a point is within stock bounds
    pub fn contains(&self, x: f64, y: f64, z: f64) -> bool {
        x >= self.origin.0
            && x <= self.origin.0 + self.width
            && y >= self.origin.1
            && y <= self.origin.1 + self.height
            && z >= self.origin.2
            && z <= self.origin.2 + self.thickness
    }
}
