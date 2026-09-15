//! `SolarSystemView` — Godot GDExtension class exposing a live
//! `StarSystem` to Godot for visualization.
//!
//! Wraps a `StarSystem` built from a preset (or config file) and
//! exposes body positions, names, and radii as Godot properties/arrays.
//! The simulation is stepped in real-time with a configurable time scale.
//!
//! Coordinate scaling: solar system distances (1e11 m) are divided by
//! a scale factor to produce manageable Godot world units. Each body's
//! radius is also scaled but with a different factor so planets remain
//! visible at system scale.

use apogee_common::units::Seconds;
use apogee_core::ephemeris::EphemerisService;
use apogee_core::star_system::{presets, StarSystem};
use godot::prelude::*;
use hifitime::Epoch;

/// Godot-exposed solar system simulator.
#[derive(GodotClass)]
#[class(base=Node)]
pub struct SolarSystemView {
    base: Base<Node>,

    /// Preset: 0 = earth-moon (DE441 compatible), 1 = inner solar system, 2 = earth-only.
    #[var]
    preset: i64,

    /// Simulation time scale (sim seconds per real second).
    #[var]
    time_scale: f64,

    /// Distance scale: Godot units per meter (1e-9 = 1 unit per Gm).
    #[var]
    distance_scale: f64,

    /// Radius exaggeration factor for visibility.
    #[var]
    radius_scale: f64,

    /// Current simulation epoch (UTC string).
    #[var]
    epoch_str: GString,

    /// Number of bodies.
    #[var]
    body_count: i64,

    system: Option<StarSystem>,
    body_names: Vec<String>,
    body_radii: Vec<f64>,
}

#[godot_api]
impl INode for SolarSystemView {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            preset: 0,
            time_scale: 86400.0,  // 1 day per second
            distance_scale: 1e-9, // 1 Godot unit = 1 Gm
            radius_scale: 1e-7,   // exaggerate radii for visibility
            epoch_str: GString::from("2026-01-01T00:00:00 UTC"),
            body_count: 0,
            system: None,
            body_names: Vec::new(),
            body_radii: Vec::new(),
        }
    }

    fn ready(&mut self) {
        self.init_system();
    }
}

#[godot_api]
impl SolarSystemView {
    /// Initialize the solar system from the selected preset.
    #[func]
    fn init_system(&mut self) {
        let definition = match self.preset {
            1 => presets::inner_solar_system(),
            2 => presets::earth_only_j2(),
            _ => presets::earth_moon(),
        };

        let epoch = Epoch::from_gregorian_utc(2026, 1, 1, 0, 0, 0, 0);

        // Try to load DE441 ephemeris for real orbital positions.
        // Resolve the data directory from (1) APOGEE_DATA_DIR env var or
        // (2) $XDG_DATA_HOME/apogee or $HOME/.local/share/apogee (XDG default).
        let data_dir = std::env::var_os("APOGEE_DATA_DIR")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("XDG_DATA_HOME")
                    .map(std::path::PathBuf::from)
                    .or_else(|| {
                        std::env::var_os("HOME")
                            .map(|h| std::path::PathBuf::from(h).join(".local").join("share"))
                    })
                    .map(|base| base.join("apogee"))
            });
        let ephemeris_path = match &data_dir {
            Some(dir) => dir.join("ephemeris").join("de441.bsp"),
            None => std::path::PathBuf::from("de441.bsp"),
        };
        let ephemeris_path_str = ephemeris_path.to_string_lossy();
        let system = match EphemerisService::load(&ephemeris_path_str, 32) {
            Ok(eph) => {
                match StarSystem::builder(definition)
                    .with_ephemeris(eph)
                    .build_at(epoch)
                {
                    Ok(s) => s,
                    Err(e) => {
                        godot_print!("[solar] Ephemeris build failed ({e:?}), falling back to static positions");
                        // Rebuild definition (consumed by builder) — use fresh preset
                        let def2 = match self.preset {
                            1 => presets::inner_solar_system(),
                            2 => presets::earth_only_j2(),
                            _ => presets::earth_moon(),
                        };
                        StarSystem::builder(def2)
                            .build_at(epoch)
                            .unwrap_or_else(|e| {
                                godot_error!("Fallback StarSystem also failed: {e:?}");
                                StarSystem::builder(presets::inner_solar_system())
                                    .build_at(epoch)
                                    .expect("fallback inner solar system")
                            })
                    }
                }
            }
            Err(_) => {
                godot_print!(
                    "[solar] DE441 not found at {ephemeris_path_str}, using static positions"
                );
                StarSystem::builder(definition)
                    .build_at(epoch)
                    .unwrap_or_else(|e| {
                        godot_error!("Failed to build StarSystem: {e:?}");
                        StarSystem::builder(presets::inner_solar_system())
                            .build_at(epoch)
                            .expect("fallback inner solar system")
                    })
            }
        };

        self.body_names = system
            .definition
            .bodies
            .iter()
            .map(|b| b.name.clone())
            .collect();
        self.body_radii = system
            .definition
            .bodies
            .iter()
            .map(|b| b.radius.unwrap_or(1e7))
            .collect();
        self.body_count = self.body_names.len() as i64;
        self.system = Some(system);
    }

    /// Advance the simulation by dt seconds (real time).
    #[func]
    fn step(&mut self, dt: f64) {
        if let Some(system) = &mut self.system {
            let sim_dt = Seconds::new(dt * self.time_scale);
            if let Err(e) = system.step(sim_dt) {
                godot_error!("StarSystem step error: {e:?}");
            }
            self.epoch_str = GString::from(&format!("{}", system.world.epoch)[..]);
        }
    }

    /// Get body positions as a packed float32 array (3 floats per body: x, y, z).
    /// Positions are scaled by distance_scale.
    #[func]
    fn get_positions(&self) -> PackedVector3Array {
        if let Some(system) = &self.system {
            let world = &system.world;
            let mut positions = PackedVector3Array::new();
            for i in 0..self.body_names.len() {
                let name = &self.body_names[i];
                if let Some(body) = system.definition.body(name) {
                    // Get position from the world — find the entity
                    let naif_id = body.naif_id.unwrap_or(0);
                    if let Some(entity) = world.find_celestial(naif_id) {
                        if let Ok(mut q) = world
                            .ecs
                            .query_one::<&apogee_core::components::kinematics::Kinematics>(entity)
                        {
                            if let Some(kin) = q.get() {
                                let pos = kin.position;
                                let scaled = Vector3::new(
                                    (pos.x * self.distance_scale) as f32,
                                    (pos.z * self.distance_scale) as f32,
                                    (-pos.y * self.distance_scale) as f32,
                                );
                                positions.push(scaled);
                            } else {
                                positions.push(Vector3::ZERO);
                            }
                        } else {
                            positions.push(Vector3::ZERO);
                        }
                    } else {
                        // No entity found — use definition position
                        let pos = body.position;
                        let scaled = Vector3::new(
                            (pos[0] * self.distance_scale) as f32,
                            (pos[2] * self.distance_scale) as f32,
                            (-pos[1] * self.distance_scale) as f32,
                        );
                        positions.push(scaled);
                    }
                } else {
                    positions.push(Vector3::ZERO);
                }
            }
            positions
        } else {
            PackedVector3Array::new()
        }
    }

    /// Get body visual radii (scaled) as a packed float32 array.
    #[func]
    fn get_radii(&self) -> PackedFloat32Array {
        let mut radii = PackedFloat32Array::new();
        for &r in &self.body_radii {
            radii.push((r * self.radius_scale) as f32);
        }
        radii
    }

    /// Get body names as a packed string array.
    #[func]
    fn get_names(&self) -> PackedStringArray {
        let mut names = PackedStringArray::new();
        for name in &self.body_names {
            let gname = GString::from(&name[..]);
            names.push(&gname);
        }
        names
    }

    /// Get the index of a body by name. Returns -1 if not found.
    #[func]
    fn body_index(&self, name: GString) -> i64 {
        let name_str = name.to_string();
        self.body_names
            .iter()
            .position(|n| n == &name_str)
            .map(|i| i as i64)
            .unwrap_or(-1)
    }

    /// Get the unscaled distance between two bodies (meters).
    #[func]
    fn distance_between(&self, body_a: i64, body_b: i64) -> f64 {
        if let Some(system) = &self.system {
            let world = &system.world;
            let get_pos = |idx: i64| -> Option<[f64; 3]> {
                if idx < 0 || idx as usize >= self.body_names.len() {
                    return None;
                }
                let name = &self.body_names[idx as usize];
                let body = system.definition.body(name)?;
                let naif_id = body.naif_id.unwrap_or(0);
                let entity = world.find_celestial(naif_id)?;
                let mut q = world
                    .ecs
                    .query_one::<&apogee_core::components::kinematics::Kinematics>(entity)
                    .ok()?;
                let kin = q.get()?;
                Some([kin.position.x, kin.position.y, kin.position.z])
            };

            if let (Some(a), Some(b)) = (get_pos(body_a), get_pos(body_b)) {
                let dx = a[0] - b[0];
                let dy = a[1] - b[1];
                let dz = a[2] - b[2];
                return (dx * dx + dy * dy + dz * dz).sqrt();
            }
        }
        0.0
    }
}
