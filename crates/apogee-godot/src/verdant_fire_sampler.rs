//! `VerdantFireSampler` — Godot GDExtension class exposing the Verdant
//! fire spread model to the Godot engine for visualization.
//!
//! Wraps a `FireFront` on a flat grid and exposes:
//! - Grid dimensions and cell size
//! - `ignite(col, row)` to start a fire
//! - `step(dt)` to advance the simulation
//! - `get_lfn()` / `get_fuel_frac()` / `get_heat_flux()` as packed arrays
//! - Wind properties (u, v) as Godot properties

use godot::prelude::*;
use verdant::{FireFront, FuelCategory};

/// Godot-exposed fire spread simulator on a flat grid.
#[derive(GodotClass)]
#[class(base=Node)]
pub struct VerdantFireSampler {
    base: Base<Node>,

    /// Grid width in cells.
    #[var]
    grid_width: i64,

    /// Grid height in cells.
    #[var]
    grid_height: i64,

    /// Cell size in meters.
    #[var]
    cell_size: f64,

    /// Anderson fuel category (1-14).
    #[var]
    fuel_category: i64,

    /// Fuel moisture content (fraction, e.g. 0.08).
    #[var]
    fuel_moisture: f64,

    /// Wind U component (m/s, east-west).
    #[var]
    wind_u: f64,

    /// Wind V component (m/s, north-south).
    #[var]
    wind_v: f64,

    /// Current simulation time (s).
    #[var]
    sim_time: f64,

    /// Number of burning cells.
    #[var]
    burning_count: i64,

    /// Burned area (m^2).
    #[var]
    burned_area: f64,

    fire: Option<FireFront>,
}

#[godot_api]
impl INode for VerdantFireSampler {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            grid_width: 50,
            grid_height: 50,
            cell_size: 10.0,
            fuel_category: 1,
            fuel_moisture: 0.08,
            wind_u: 3.0,
            wind_v: 0.0,
            sim_time: 0.0,
            burning_count: 0,
            burned_area: 0.0,
            fire: None,
        }
    }

    fn ready(&mut self) {
        self.init_fire();
    }
}

#[godot_api]
impl VerdantFireSampler {
    /// Initialize (or re-initialize) the fire grid with current properties.
    #[func]
    fn init_fire(&mut self) {
        let fire = FireFront::new(
            self.grid_width as usize,
            self.grid_height as usize,
            self.cell_size as f32,
            self.cell_size as f32,
            self.fuel_category as u8,
            self.fuel_moisture as f32,
        );
        self.fire = Some(fire);
        self.sim_time = 0.0;
        self.burning_count = 0;
        self.burned_area = 0.0;
    }

    /// Ignite a point at grid cell (col, row).
    #[func]
    fn ignite_point(&mut self, col: i64, row: i64) {
        if let Some(fire) = &mut self.fire {
            fire.ignite_point(col as usize, row as usize);
            self.burning_count = fire.burning_count() as i64;
            self.burned_area = fire.burned_area() as f64;
        }
    }

    /// Ignite a circle at world position (x, y) with radius (meters).
    #[func]
    fn ignite_circle(&mut self, cx: f64, cy: f64, radius: f64) {
        if let Some(fire) = &mut self.fire {
            fire.ignite_circle(cx as f32, cy as f32, radius as f32);
            self.burning_count = fire.burning_count() as i64;
            self.burned_area = fire.burned_area() as f64;
        }
    }

    /// Advance the simulation by dt seconds.
    #[func]
    fn step(&mut self, dt: f64) {
        if let Some(fire) = &mut self.fire {
            let w = self.grid_width as usize;
            let h = self.grid_height as usize;
            let n = w * h;
            let wind_u = vec![self.wind_u as f32; n];
            let wind_v = vec![self.wind_v as f32; n];
            let dz_dx = vec![0.0_f32; n];
            let dz_dy = vec![0.0_f32; n];

            fire.step(dt as f32, &wind_u, &wind_v, &dz_dx, &dz_dy);
            self.sim_time = fire.time as f64;
            self.burning_count = fire.burning_count() as i64;
            self.burned_area = fire.burned_area() as f64;
        }
    }

    /// Get the level-set function as a packed float32 array.
    /// Negative = burned, zero = fire front, positive = unburned.
    #[func]
    fn get_lfn(&self) -> PackedFloat32Array {
        if let Some(fire) = &self.fire {
            PackedFloat32Array::from(&fire.lfn[..])
        } else {
            PackedFloat32Array::new()
        }
    }

    /// Get remaining fuel fraction (0-1) as a packed float32 array.
    #[func]
    fn get_fuel_frac(&self) -> PackedFloat32Array {
        if let Some(fire) = &self.fire {
            PackedFloat32Array::from(&fire.fuel_frac[..])
        } else {
            PackedFloat32Array::new()
        }
    }

    /// Get heat flux (W/m^2) as a packed float32 array.
    #[func]
    fn get_heat_flux(&self) -> PackedFloat32Array {
        if let Some(fire) = &self.fire {
            PackedFloat32Array::from(&fire.heat_flux[..])
        } else {
            PackedFloat32Array::new()
        }
    }

    /// Get ignition time (s) as a packed float32 array.
    /// NaN = not ignited.
    #[func]
    fn get_tign(&self) -> PackedFloat32Array {
        if let Some(fire) = &self.fire {
            PackedFloat32Array::from(&fire.tign[..])
        } else {
            PackedFloat32Array::new()
        }
    }

    /// Check if a cell is currently burning.
    #[func]
    fn is_burning(&self, col: i64, row: i64) -> bool {
        if let Some(fire) = &self.fire {
            fire.is_burning(col as usize, row as usize)
        } else {
            false
        }
    }

    /// Get the fuel category name as a string.
    #[func]
    fn fuel_category_name(&self) -> GString {
        let cat = FuelCategory::from_u8(self.fuel_category as u8);
        GString::from(&format!("{cat:?}")[..])
    }
}
