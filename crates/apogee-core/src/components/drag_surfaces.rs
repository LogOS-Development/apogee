//! Drag surface components for per-part aerodynamic drag modeling.
//!
//! A spacecraft entity may carry a [`DragSurfaces`] component containing
//! multiple [`DragSurface`] entries — one per physical part that produces
//! drag (main body, solar panels, antenna, etc.). The force aggregator
//! evaluates the shared nonlinear atmospheric state (density, relative
//! velocity) once, then sums the linear per-surface contributions
//! (Cd * A). This is the linear-superposition principle applied at the
//! component level: total drag = sum of per-surface drag forces.
//!
//! Each surface has a body-frame normal direction (`normal_dir`). A zero
//! normal means cannonball (isotropic) — the full area is always
//! projected, matching the legacy model. A non-zero normal means flat
//! plate — the projected area is A * |n_body · v_hat_rel|, computed by
//! rotating the normal into the inertial frame via the body's attitude
//! quaternion.

use apogee_common::units::{AccelerationVector, Area, Density, Kilograms};
use apogee_common::Position;
use nalgebra::Vector3;

use crate::aero::model::{AtmosphereInput, AtmosphereModel};
use crate::aero::nrlmsise00::Nrlmsise00;

/// A single drag-producing surface on a spacecraft.
///
/// Each surface has its own area, drag coefficient, body-frame normal
/// direction, and reference-point offset. The normal determines the
/// projected area for a flat-plate model; the reference point is used for
/// torque computation (future 6DOF drag torque).
#[derive(Debug, Clone, Copy)]
pub struct DragSurface {
    /// Physical surface area (m^2).
    pub area: Area<f64>,
    /// Drag coefficient (dimensionless).
    pub cd: f64,
    /// Body-frame outward normal direction. Zero vector = cannonball
    /// (isotropic, full area always projected). Non-zero = flat plate
    /// (projected area = A * |n_body · v_hat_rel|).
    pub normal_dir: Vector3<f64>,
    /// Reference point of the surface in the body frame (m), relative to
    /// the body's center of mass. Used for drag torque computation (future).
    pub reference_point: Vector3<f64>,
}

impl DragSurface {
    /// Create a cannonball (isotropic) drag surface with the given area
    /// and drag coefficient. The normal is zero (full area always
    /// projected).
    pub fn new(area: Area<f64>, cd: f64) -> Self {
        Self {
            area,
            cd,
            normal_dir: Vector3::zeros(),
            reference_point: Vector3::zeros(),
        }
    }

    /// Create a flat-plate drag surface with a body-frame normal and
    /// reference point.
    pub fn flat_plate(
        area: Area<f64>,
        cd: f64,
        normal_dir: Vector3<f64>,
        reference_point: Vector3<f64>,
    ) -> Self {
        Self {
            area,
            cd,
            normal_dir,
            reference_point,
        }
    }

    /// Effective drag area (Cd * A) in m^2. This is the cannonball
    /// contribution; for a flat plate, the force model further scales by
    /// the projected-area factor.
    pub fn drag_area(&self) -> Area<f64> {
        Area::new(self.cd * self.area.into_value())
    }

    /// Is this a cannonball (isotropic) surface?
    pub fn is_cannonball(&self) -> bool {
        self.normal_dir == Vector3::zeros()
    }
}

impl Default for DragSurface {
    fn default() -> Self {
        Self {
            area: Area::new(0.0),
            cd: 0.0,
            normal_dir: Vector3::zeros(),
            reference_point: Vector3::zeros(),
        }
    }
}

/// ECS component: a collection of drag-producing surfaces on one entity.
///
/// Implements [`crate::systems::force_model::ForceModel`] so the force
/// aggregator picks it up automatically. Entities without this component are
/// skipped by drag aggregation — no zero-filled fields, no special-casing.
#[derive(Debug, Clone, Default)]
pub struct DragSurfaces {
    /// The individual drag surfaces.
    pub surfaces: Vec<DragSurface>,
}

impl DragSurfaces {
    /// Create an empty drag-surface set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a drag-surface set from a list of surfaces.
    pub fn from_surfaces(surfaces: Vec<DragSurface>) -> Self {
        Self { surfaces }
    }

    /// Add a drag surface.
    pub fn push(&mut self, surface: DragSurface) {
        self.surfaces.push(surface);
    }

    /// Total effective drag area (sum of Cd * A over all surfaces), ignoring
    /// the projected-area factor. This is the cannonball equivalent.
    pub fn total_drag_area(&self) -> Area<f64> {
        Area::new(
            self.surfaces
                .iter()
                .map(|s| s.drag_area().into_value())
                .sum(),
        )
    }

    /// Number of drag surfaces.
    pub fn len(&self) -> usize {
        self.surfaces.len()
    }

    /// Is the surface set empty?
    pub fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }
}

impl DragSurfaces {
    /// Compute the total drag acceleration from all surfaces.
    ///
    /// This is the linear-superposition core: the nonlinear atmospheric
    /// state (density, relative velocity) is evaluated once, then each
    /// surface contributes a force proportional to its (Cd * A) (cannonball)
    /// or (Cd * A * |n . v_hat|) (flat plate). The total force is divided
    /// by the body mass to get acceleration.
    ///
    /// For cannonball surfaces (zero `normal_dir`), the full `Cd * A` is
    /// always projected. For flat-plate surfaces, the projected area is
    /// `A * |n_body . v_hat_rel|` where `n_body` is rotated into the
    /// inertial frame by the body's attitude quaternion.
    pub(crate) fn drag_acceleration(
        &self,
        spacecraft_position: &Position,
        spacecraft_velocity: &Vector3<f64>,
        attitude: &nalgebra::Quaternion<f64>,
        density: Density<f64>,
        mass: Kilograms<f64>,
    ) -> AccelerationVector {
        use apogee_common::constants::R_EARTH_EQ;

        // Approximate Earth rotation velocity at equator; inertial atmosphere
        // is assumed co-rotating for this simple model.
        let omega_earth = Vector3::new(0.0, 0.0, 7.2921159e-5);
        let r = spacecraft_position.norm();
        let altitude_m = r - R_EARTH_EQ;

        let vel_rel = spacecraft_velocity - omega_earth.cross(spacecraft_position);
        let v_rel = vel_rel.norm();
        let density_value = density.into_value();
        if v_rel == 0.0 || density_value <= 0.0 || altitude_m < 0.0 {
            return AccelerationVector::new(Vector3::zeros());
        }

        let v_hat = vel_rel / v_rel;

        // Sum per-surface drag force. Each surface contributes:
        //   F_i = 0.5 * rho * v^2 * (Cd * A_i * proj_factor_i)
        // where proj_factor is 1 for cannonball, |n . v_hat| for flat plate.
        // This sum is the linear superposition of per-surface drag forces.
        let mut total_force_magnitude = 0.0_f64;
        for surface in &self.surfaces {
            let cd_a = surface.drag_area().into_value();
            if cd_a == 0.0 {
                continue;
            }
            let proj_factor = if surface.is_cannonball() {
                1.0
            } else {
                // Rotate body-frame normal into inertial frame.
                let rot = nalgebra::UnitQuaternion::from_quaternion(*attitude);
                let n_inertial = rot * surface.normal_dir;
                (n_inertial.dot(&v_hat)).abs()
            };
            total_force_magnitude += 0.5 * density_value * v_rel * v_rel * cd_a * proj_factor;
        }

        if total_force_magnitude == 0.0 {
            return AccelerationVector::new(Vector3::zeros());
        }

        let accel_magnitude = total_force_magnitude / mass.into_value();
        AccelerationVector::new(-v_hat * accel_magnitude)
    }
}

impl crate::systems::force_model::ForceModel for DragSurfaces {
    fn name(&self) -> &str {
        "drag surfaces"
    }

    fn acceleration(&self, ctx: &crate::systems::force_model::ForceContext) -> AccelerationVector {
        // Evaluate the atmosphere model once (nonlinear state shared across
        // all surfaces), then sum the linear per-surface contributions.
        let doy_f64 = ctx.epoch.day_of_year();
        let day_of_year = doy_f64 as u16;
        let seconds_utc = (doy_f64 - doy_f64.floor()) * 86_400.0;

        let model = Nrlmsise00;
        let latlon =
            crate::systems::force_aggregator::ecef_lat_lon_from_inertial(&ctx.kinematics.position);
        let input = AtmosphereInput {
            altitude_m: latlon.altitude_m,
            latitude_rad: latlon.latitude_rad,
            longitude_rad: latlon.longitude_rad,
            day_of_year,
            seconds_utc,
            f107: ctx.sim_config.f107,
            f107a: ctx.sim_config.f107a,
            ap: ctx.sim_config.ap,
        };
        let output = model.evaluate(&input);

        self.drag_acceleration(
            &ctx.kinematics.position,
            &ctx.kinematics.velocity,
            &ctx.kinematics.attitude,
            output.density,
            ctx.rigid_body.mass,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn drag_surface_drag_area() {
        let s = DragSurface::new(Area::new(5.0), 2.2);
        assert_relative_eq!(s.drag_area().value, 11.0);
        assert!(s.is_cannonball());
    }

    #[test]
    fn flat_plate_is_not_cannonball() {
        let s = DragSurface::flat_plate(
            Area::new(5.0),
            2.2,
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::zeros(),
        );
        assert!(!s.is_cannonball());
    }

    #[test]
    fn drag_surfaces_total() {
        let mut ds = DragSurfaces::new();
        ds.push(DragSurface::new(Area::new(3.0), 2.0));
        ds.push(DragSurface::new(Area::new(5.0), 2.2));
        // 2.0*3.0 + 2.2*5.0 = 6.0 + 11.0 = 17.0
        assert_relative_eq!(ds.total_drag_area().value, 17.0);
    }

    #[test]
    fn drag_surfaces_empty() {
        let ds = DragSurfaces::new();
        assert!(ds.is_empty());
        assert_relative_eq!(ds.total_drag_area().value, 0.0);
    }
}
