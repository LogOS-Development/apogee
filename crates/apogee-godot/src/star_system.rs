use apogee_common::constants::AU;
use apogee_core::star_system::StarSystem;
use godot::classes::Node3D;
use godot::prelude::*;
use hifitime::{Epoch, TimeScale, Unit};

/// Godot node that computes a star-system state at a given epoch and exposes
/// body positions/directions so the visualizer scene can place the Sun, Earth,
/// Moon, etc. as needed.
///
/// The Rust backend stores everything in SI units. This node converts to AU
/// on output because that is the natural scale for the Godot scene.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct StarSystemNode {
    base: Base<Node3D>,

    /// Inputs: day of year (1-366) and UTC seconds since midnight.
    #[var]
    day_of_year: i32,
    #[var]
    seconds_utc: f64,
}

#[godot_api]
impl INode3D for StarSystemNode {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            day_of_year: 1,
            seconds_utc: 43200.0,
        }
    }
}

impl StarSystemNode {
    fn current_epoch(&self) -> Epoch {
        let days = (self.day_of_year - 1) as f64 + self.seconds_utc / 86_400.0;
        Epoch::from_jde_in_time_scale(2451545.0, TimeScale::TT) + days * Unit::Day
    }

    fn current_system(&self) -> StarSystem {
        StarSystem::at_epoch(self.current_epoch())
    }
}

#[godot_api]
impl StarSystemNode {
    /// Return the barycentric position of `body_name` as a Vector3 in AU.
    /// Returns Vector3.ZERO if the body is unknown.
    #[func]
    fn body_position_au(&self, body_name: GString) -> Vector3 {
        match self.current_system().body(&body_name.to_string()) {
            Some(body) => Vector3::new(
                (body.position.x / AU) as f32,
                (body.position.y / AU) as f32,
                (body.position.z / AU) as f32,
            ),
            None => Vector3::ZERO,
        }
    }

    /// Return the body rotation axis of `body_name` as a unit Vector3.
    /// Returns Vector3.ZERO if the body is unknown.
    #[func]
    fn body_rotation_axis(&self, body_name: GString) -> Vector3 {
        match self.current_system().body(&body_name.to_string()) {
            Some(body) => Vector3::new(
                body.rotation_axis.x as f32,
                body.rotation_axis.y as f32,
                body.rotation_axis.z as f32,
            ),
            None => Vector3::ZERO,
        }
    }

    /// Return the body rotation angle of `body_name` in degrees.
    /// Returns 0.0 if the body is unknown.
    #[func]
    fn body_rotation_degrees(&self, body_name: GString) -> f64 {
        self.current_system()
            .body(&body_name.to_string())
            .map(|body| body.rotation_angle.to_degrees())
            .unwrap_or(0.0)
    }

    /// Return the displacement vector from `observer_name` to `target_name` in AU.
    /// Convention: vector points from target to observer.
    /// Returns Vector3.ZERO if either body is unknown.
    #[func]
    fn vector_between(&self, observer_name: GString, target_name: GString) -> Vector3 {
        match self
            .current_system()
            .vector_between(&observer_name.to_string(), &target_name.to_string())
        {
            Some(v) => Vector3::new((v.x / AU) as f32, (v.y / AU) as f32, (v.z / AU) as f32),
            None => Vector3::ZERO,
        }
    }

    /// Return the unit direction from `observer_name` to `target_name`.
    /// Convention: vector points from target to observer.
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
            .map(|d| d / AU)
            .unwrap_or(0.0)
    }
}
