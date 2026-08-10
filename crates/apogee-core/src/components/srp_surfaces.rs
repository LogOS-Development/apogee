//! SRP surface components for per-part solar radiation pressure modeling.
//!
//! A spacecraft entity may carry an [`SrpSurfaces`] component containing
//! multiple [`SrpSurface`] entries — one per physical part that is
//! exposed to solar radiation (main body, solar panels, radiator, etc.).
//! The force aggregator evaluates the shared nonlinear state (Sun
//! direction, distance, eclipse) once, then sums the linear per-surface
//! contributions (A * (1 + reflectivity)). This is the linear-superposition
//! principle: total SRP = sum of per-surface SRP forces.
//!
//! Each surface has a body-frame normal direction (`normal_dir`). A zero
//! normal means cannonball (isotropic) — the full area always faces the
//! Sun. A non-zero normal means flat plate — the projected area is
//! A * |n_body · s_hat|, computed by rotating the normal into the inertial
//! frame via the body's attitude quaternion.

use apogee_common::units::{AccelerationVector, Area, Kilograms};
use apogee_common::Position;
use nalgebra::Vector3;

use apogee_common::constants::{AU, R_EARTH_EQ, SRP_1AU};

/// A single SRP-exposed surface on a spacecraft.
///
/// Each surface has its own area, reflectivity coefficient, body-frame
/// normal direction, and reference-point offset. The normal determines
/// the projected area for a flat-plate model; the reference point is used
/// for torque computation (future 6DOF SRP torque).
#[derive(Debug, Clone, Copy)]
pub struct SrpSurface {
    /// Physical surface area exposed to solar radiation (m^2).
    pub area: Area<f64>,
    /// Reflectivity coefficient (0.0 = fully absorbing, 1.0 = perfectly
    /// reflecting). The force model uses (1 + reflectivity) as the
    /// effective multiplier.
    pub reflectivity: f64,
    /// Body-frame outward normal direction. Zero vector = cannonball
    /// (isotropic, full area always projected). Non-zero = flat plate
    /// (projected area = A * |n_body · s_hat|).
    pub normal_dir: Vector3<f64>,
    /// Reference point of the surface in the body frame (m), relative to
    /// the body's center of mass. Used for SRP torque computation (future).
    pub reference_point: Vector3<f64>,
}

impl SrpSurface {
    /// Create a cannonball (isotropic) SRP surface with the given area
    /// and reflectivity. The normal is zero (full area always projected).
    pub fn new(area: Area<f64>, reflectivity: f64) -> Self {
        Self {
            area,
            reflectivity,
            normal_dir: Vector3::zeros(),
            reference_point: Vector3::zeros(),
        }
    }

    /// Create a flat-plate SRP surface with a body-frame normal and
    /// reference point.
    pub fn flat_plate(
        area: Area<f64>,
        reflectivity: f64,
        normal_dir: Vector3<f64>,
        reference_point: Vector3<f64>,
    ) -> Self {
        Self {
            area,
            reflectivity,
            normal_dir,
            reference_point,
        }
    }

    /// Effective SRP area factor: A * (1 + reflectivity) in m^2. This is
    /// the cannonball contribution; for a flat plate, the force model
    /// further scales by the projected-area factor.
    pub fn effective_area(&self) -> Area<f64> {
        Area::new(self.area.into_value() * (1.0 + self.reflectivity))
    }

    /// Is this a cannonball (isotropic) surface?
    pub fn is_cannonball(&self) -> bool {
        self.normal_dir == Vector3::zeros()
    }
}

impl Default for SrpSurface {
    fn default() -> Self {
        Self {
            area: Area::new(0.0),
            reflectivity: 0.0,
            normal_dir: Vector3::zeros(),
            reference_point: Vector3::zeros(),
        }
    }
}

/// ECS component: a collection of SRP-exposed surfaces on one entity.
///
/// Implements [`crate::systems::force_model::ForceModel`] so the force
/// aggregator picks it up automatically. Entities without this component
/// are skipped by SRP aggregation.
#[derive(Debug, Clone, Default)]
pub struct SrpSurfaces {
    /// The individual SRP surfaces.
    pub surfaces: Vec<SrpSurface>,
}

impl SrpSurfaces {
    /// Create an empty SRP-surface set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an SRP-surface set from a list of surfaces.
    pub fn from_surfaces(surfaces: Vec<SrpSurface>) -> Self {
        Self { surfaces }
    }

    /// Add an SRP surface.
    pub fn push(&mut self, surface: SrpSurface) {
        self.surfaces.push(surface)
    }

    /// Total effective SRP area (sum of A * (1 + reflectivity) over all
    /// surfaces), ignoring the projected-area factor. This is the
    /// cannonball equivalent.
    pub fn total_effective_area(&self) -> Area<f64> {
        Area::new(
            self.surfaces
                .iter()
                .map(|s| s.effective_area().into_value())
                .sum(),
        )
    }

    /// Number of SRP surfaces.
    pub fn len(&self) -> usize {
        self.surfaces.len()
    }

    /// Is the surface set empty?
    pub fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }
}

impl SrpSurfaces {
    /// Compute the total SRP acceleration from all surfaces.
    ///
    /// This is the linear-superposition core: the nonlinear SRP state
    /// (Sun direction, distance, eclipse) is evaluated once, then each
    /// surface contributes a force proportional to its (A * (1 + r))
    /// (cannonball) or (A * (1 + r) * |n . s_hat|) (flat plate). The total
    /// force is divided by the body mass to get acceleration.
    ///
    /// For cannonball surfaces (zero `normal_dir`), the full effective area
    /// is always projected. For flat-plate surfaces, the projected area is
    /// `A * |n_body . s_hat|` where `n_body` is rotated into the inertial
    /// frame by the body's attitude quaternion.
    pub(crate) fn srp_acceleration(
        &self,
        spacecraft_position: &Position,
        sun_position: &Position,
        attitude: &nalgebra::Quaternion<f64>,
        mass: Kilograms<f64>,
    ) -> AccelerationVector {
        let to_sun = sun_position - spacecraft_position;
        let r = to_sun.norm();
        if r == 0.0 {
            return AccelerationVector::new(Vector3::zeros());
        }

        if is_eclipsed(spacecraft_position, sun_position) {
            return AccelerationVector::new(Vector3::zeros());
        }

        let s_hat = to_sun / r;
        let flux_factor = AU * AU / (r * r);
        let pressure = SRP_1AU * flux_factor;

        // Sum per-surface SRP force. Each surface contributes:
        //   F_i = P * A_i * (1 + r_i) * proj_factor_i
        // where proj_factor is 1 for cannonball, |n . s_hat| for flat plate.
        // This sum is the linear superposition of per-surface SRP forces.
        let mut total_force_magnitude = 0.0_f64;
        for surface in &self.surfaces {
            let effective_area = surface.effective_area().into_value();
            if effective_area == 0.0 {
                continue;
            }
            let proj_factor = if surface.is_cannonball() {
                1.0
            } else {
                let rot = nalgebra::UnitQuaternion::from_quaternion(*attitude);
                let n_inertial = rot * surface.normal_dir;
                (n_inertial.dot(&s_hat)).abs()
            };
            total_force_magnitude += pressure * effective_area * proj_factor;
        }

        if total_force_magnitude == 0.0 {
            return AccelerationVector::new(Vector3::zeros());
        }

        let accel_magnitude = total_force_magnitude / mass.into_value();
        AccelerationVector::new(s_hat * accel_magnitude)
    }
}

impl crate::systems::force_model::ForceModel for SrpSurfaces {
    fn name(&self) -> &str {
        "srp surfaces"
    }

    fn acceleration(&self, ctx: &crate::systems::force_model::ForceContext) -> AccelerationVector {
        self.srp_acceleration(
            &ctx.kinematics.position,
            &ctx.sun_position,
            &ctx.kinematics.attitude,
            ctx.rigid_body.mass,
        )
    }
}

/// Simple cylindrical eclipse check: spacecraft is eclipsed if it is in
/// Earth's shadow cylinder opposite the Sun. Uses WGS84 equatorial radius
/// as a conservative approximation.
fn is_eclipsed(spacecraft_position: &Position, sun_position: &Position) -> bool {
    let sun_to_sc = spacecraft_position - sun_position;
    let sun_dir = sun_position / sun_position.norm();
    let projection = sun_to_sc.dot(&sun_dir);
    let closest_approach = sun_to_sc - sun_dir * projection;
    let distance = closest_approach.norm();

    if projection > 0.0 {
        return false;
    }

    distance < R_EARTH_EQ
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn srp_surface_effective_area() {
        let s = SrpSurface::new(Area::new(10.0), 0.3);
        // 10.0 * (1 + 0.3) = 13.0
        assert_relative_eq!(s.effective_area().value, 13.0);
        assert!(s.is_cannonball());
    }

    #[test]
    fn flat_plate_is_not_cannonball() {
        let s = SrpSurface::flat_plate(
            Area::new(10.0),
            0.3,
            Vector3::new(0.0, 0.0, 1.0),
            Vector3::zeros(),
        );
        assert!(!s.is_cannonball());
    }

    #[test]
    fn srp_surfaces_total() {
        let mut ss = SrpSurfaces::new();
        ss.push(SrpSurface::new(Area::new(2.0), 0.0)); // 2.0 * 1.0 = 2.0
        ss.push(SrpSurface::new(Area::new(4.0), 1.0)); // 4.0 * 2.0 = 8.0
        assert_relative_eq!(ss.total_effective_area().value, 10.0);
    }

    #[test]
    fn srp_surfaces_empty() {
        let ss = SrpSurfaces::new();
        assert!(ss.is_empty());
        assert_relative_eq!(ss.total_effective_area().value, 0.0);
    }
}
