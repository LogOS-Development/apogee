use apogee_core::solar_system::StarSystem;
use godot::classes::Node3D;
use godot::prelude::*;
use hifitime::{Epoch, TimeScale, Unit};

/// Godot node that computes a star-system state at a given epoch and exposes
/// body positions/directions so the visualizer scene can place the Sun, Earth,
/// Moon, etc. as needed.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct StarSystemNode {
    base: Base<Node3D>,

    /// Inputs: day of year (1-366) and UTC seconds since midnight.
    #[var]
    day_of_year: i32,
    #[var]
    seconds_utc: f64,

    /// Cached Earth rotation and obliquity (degrees), exposed for convenience.
    #[var]
    earth_rotation_degrees: f64,
    #[var]
    earth_obliquity_degrees: f64,
}

#[godot_api]
impl INode3D for StarSystemNode {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            day_of_year: 1,
            seconds_utc: 43200.0,
            earth_rotation_degrees: 0.0,
            earth_obliquity_degrees: 23.44,
        }
    }
}

impl StarSystemNode {
    fn current_epoch(&self) -> Epoch {
        let days = (self.day_of_year - 1) as f64 + self.seconds_utc / 86400.0;
        Epoch::from_jde_in_time_scale(2451545.0, TimeScale::TT) + days * Unit::Day
    }

    fn current_system(&self) -> StarSystem {
        StarSystem::at_epoch(self.current_epoch())
    }
}

#[godot_api]
impl StarSystemNode {
    /// Recompute star-system geometry from the current DOY/UTC fields.
    #[func]
    fn recompute(&mut self) {
        let system = self.current_system();
        self.earth_rotation_degrees = system.earth_rotation_rad.to_degrees();
        self.earth_obliquity_degrees = system.earth_obliquity_rad.to_degrees();
    }

    /// Return the barycentric position of `body_name` as a Vector3 in AU.
    /// Returns Vector3.ZERO if the body is unknown.
    #[func]
    fn body_position_au(&self, body_name: GString) -> Vector3 {
        match self.current_system().body(&body_name.to_string()) {
            Some(body) => Vector3::new(
                body.position_au().x as f32,
                body.position_au().y as f32,
                body.position_au().z as f32,
            ),
            None => Vector3::ZERO,
        }
    }

    /// Return the displacement vector from `observer_name` to `target_name` in AU.
    /// Returns Vector3.ZERO if either body is unknown.
    #[func]
    fn vector_between(&self, observer_name: GString, target_name: GString) -> Vector3 {
        match self
            .current_system()
            .vector_between(&observer_name.to_string(), &target_name.to_string())
        {
            Some(v) => Vector3::new(v.x as f32, v.y as f32, v.z as f32),
            None => Vector3::ZERO,
        }
    }

    /// Return the unit direction from `observer_name` to `target_name`.
    /// Returns Vector3.ZERO if either body is unknown.
    #[func]
    fn direction_between(&self, observer_name: GString, target_name: GString) -> Vector3 {
        match self
            .current_system()
            .direction_between(&observer_name.to_string(), &target_name.to_string())
        {
            Some(dir) => Vector3::new(dir.x as f32, dir.y as f32, dir.z as f32),
            None => Vector3::ZERO,
        }
    }

    /// Return the distance from `observer_name` to `target_name` in AU.
    /// Returns 0.0 if either body is unknown.
    #[func]
    fn distance_between(&self, observer_name: GString, target_name: GString) -> f64 {
        self.current_system()
            .distance_between(&observer_name.to_string(), &target_name.to_string())
            .unwrap_or(0.0)
    }
}
