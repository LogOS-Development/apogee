//! Dynamics quantity type aliases.
//!
//! Concrete instantiations of [`VectorQuantity`] and [`TensorQuantity`]
//! pinned to `f64` and the appropriate SI unit.  These exist purely for
//! ergonomics — a `PositionVector` is exactly a
//! `VectorQuantity<f64, 3, dim::Meter>` — but the descriptive name makes
//! APIs and struct fields self-documenting.
//!
//! Types use the full word "Vector" — no abbreviations.

use crate::units::{dim, Quantity, TensorQuantity, VectorQuantity};

use nalgebra::Vector3;

// ---------------------------------------------------------------------------
// 3-D vector aliases
// ---------------------------------------------------------------------------

/// Position vector in meters (m).
pub type PositionVector = VectorQuantity<f64, 3, dim::Meter>;

/// Velocity vector in meters per second (m/s).
pub type VelocityVector = VectorQuantity<f64, 3, dim::Velocity>;

/// Acceleration vector in meters per second squared (m/s²).
pub type AccelerationVector = VectorQuantity<f64, 3, dim::Acceleration>;

/// Force vector in newtons (N).
pub type ForceVector = VectorQuantity<f64, 3, dim::Force>;

/// Torque vector in newton-meters (N·m).
pub type TorqueVector = VectorQuantity<f64, 3, dim::Torque>;

/// Angular velocity vector in radians per second (rad/s).
pub type AngularVelocityVector = VectorQuantity<f64, 3, dim::AngularVelocity>;

/// Angular acceleration vector in radians per second squared (rad/s²).
pub type AngularAccelerationVector = VectorQuantity<f64, 3, dim::AngularAcceleration>;

/// Magnetic field vector in teslas (T).
pub type MagneticFieldVector = VectorQuantity<f64, 3, dim::MagneticFluxDensity>;

/// Dimensionless 3-D direction vector (unit vector).
pub type DirectionVector = VectorQuantity<f64, 3, dim::Dimensionless>;

/// Angle vector — Euler angles or similar, in radians (rad).
pub type AngleVector = VectorQuantity<f64, 3, dim::Angle>;

// ---------------------------------------------------------------------------
// 3×3 tensor aliases
// ---------------------------------------------------------------------------

/// Inertia tensor in kg·m².
pub type InertiaTensor = TensorQuantity<f64, 3, 3, dim::MomentOfInertia>;

/// Stress tensor in pascals (Pa = N/m²).
pub type StressTensor = TensorQuantity<f64, 3, 3, dim::Pressure>;

/// Strain tensor (dimensionless).
pub type StrainTensor = TensorQuantity<f64, 3, 3, dim::Dimensionless>;

// ---------------------------------------------------------------------------
// Scalar dynamics aliases
// ---------------------------------------------------------------------------

/// Mass in kilograms.
pub type Mass = Quantity<f64, dim::Kilogram>;

/// Gravitational parameter μ = GM in m³/s².
pub type Mu = Quantity<f64, dim::GravitationalParameter>;

/// Power in watts (W).
pub type PowerScalar = Quantity<f64, dim::Power>;

// ---------------------------------------------------------------------------
// Inherent helper methods on VectorQuantity aliases
// ---------------------------------------------------------------------------

impl PositionVector {
    /// Scalar (Euclidean) distance to another position.
    #[inline]
    #[must_use]
    pub fn distance_to(&self, other: &Self) -> f64 {
        (other.vector - self.vector).norm()
    }

    /// Displacement vector from self to `other`.
    #[inline]
    #[must_use]
    pub fn vector_to(&self, other: &Self) -> Self {
        Self::new(other.vector - self.vector)
    }

    /// Unit direction from self toward `other`.
    #[inline]
    #[must_use]
    pub fn direction_to(&self, other: &Self) -> DirectionVector {
        DirectionVector::new((other.vector - self.vector).normalize())
    }

    /// Construct from `[x, y, z]` in meters.
    #[inline]
    #[must_use]
    pub fn from_xyz(x: f64, y: f64, z: f64) -> Self {
        Self::new(Vector3::new(x, y, z))
    }
}

impl VelocityVector {
    /// Construct from `[vx, vy, vz]` in m/s.
    #[inline]
    #[must_use]
    pub fn from_xyz(x: f64, y: f64, z: f64) -> Self {
        Self::new(Vector3::new(x, y, z))
    }
}

impl AccelerationVector {
    /// Construct from `[ax, ay, az]` in m/s².
    #[inline]
    #[must_use]
    pub fn from_xyz(x: f64, y: f64, z: f64) -> Self {
        Self::new(Vector3::new(x, y, z))
    }
}

impl ForceVector {
    /// Construct from `[fx, fy, fz]` in newtons.
    #[inline]
    #[must_use]
    pub fn from_xyz(x: f64, y: f64, z: f64) -> Self {
        Self::new(Vector3::new(x, y, z))
    }
}

impl TorqueVector {
    /// Construct from `[tx, ty, tz]` in N·m.
    #[inline]
    #[must_use]
    pub fn from_xyz(x: f64, y: f64, z: f64) -> Self {
        Self::new(Vector3::new(x, y, z))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn position_distance() {
        let a = PositionVector::from_xyz(0.0, 0.0, 0.0);
        let b = PositionVector::from_xyz(3.0, 4.0, 0.0);
        assert_relative_eq!(a.distance_to(&b), 5.0);
    }

    #[test]
    fn position_vector_to() {
        let a = PositionVector::from_xyz(1.0, 0.0, 0.0);
        let b = PositionVector::from_xyz(4.0, 0.0, 0.0);
        let d = a.vector_to(&b);
        assert_relative_eq!(d.vector.x, 3.0);
    }

    #[test]
    fn position_direction_to() {
        let a = PositionVector::from_xyz(0.0, 0.0, 0.0);
        let b = PositionVector::from_xyz(0.0, 5.0, 0.0);
        let dir = a.direction_to(&b);
        assert_relative_eq!(dir.vector.y, 1.0);
    }

    #[test]
    fn default_is_zero() {
        let p = PositionVector::default();
        assert_relative_eq!(p.vector.norm(), 0.0);
    }

    #[test]
    fn inertia_tensor_identity() {
        let i = InertiaTensor::identity();
        assert_relative_eq!(i.matrix[(0, 0)], 1.0);
        assert_relative_eq!(i.matrix[(1, 1)], 1.0);
        assert_relative_eq!(i.matrix[(2, 2)], 1.0);
    }

    #[test]
    fn velocity_vector_from_xyz() {
        let v = VelocityVector::from_xyz(1.0, 2.0, 3.0);
        assert_relative_eq!(v.vector.x, 1.0);
        assert_relative_eq!(v.vector.y, 2.0);
        assert_relative_eq!(v.vector.z, 3.0);
    }

    #[test]
    fn acceleration_vector_from_xyz() {
        let a = AccelerationVector::from_xyz(0.1, 0.2, 9.8);
        assert_relative_eq!(a.vector.z, 9.8);
    }

    #[test]
    fn force_vector_from_xyz() {
        let f = ForceVector::from_xyz(10.0, 0.0, 0.0);
        assert_relative_eq!(f.vector.x, 10.0);
    }

    #[test]
    fn torque_vector_from_xyz() {
        let t = TorqueVector::from_xyz(0.0, 5.0, 0.0);
        assert_relative_eq!(t.vector.y, 5.0);
    }

    #[test]
    fn force_vector_add() {
        let a = ForceVector::from_xyz(1.0, 2.0, 3.0);
        let b = ForceVector::from_xyz(4.0, 5.0, 6.0);
        let c = a + b;
        assert_relative_eq!(c.vector.x, 5.0);
        assert_relative_eq!(c.vector.y, 7.0);
        assert_relative_eq!(c.vector.z, 9.0);
    }

    #[test]
    fn torque_vector_sub() {
        let a = TorqueVector::from_xyz(5.0, 3.0, 1.0);
        let b = TorqueVector::from_xyz(1.0, 1.0, 1.0);
        let c = a - b;
        assert_relative_eq!(c.vector.x, 4.0);
        assert_relative_eq!(c.vector.y, 2.0);
    }

    #[test]
    fn angular_velocity_default() {
        let v = AngularVelocityVector::default();
        assert_relative_eq!(v.vector.norm(), 0.0);
    }

    #[test]
    fn stress_tensor_default() {
        let s = StressTensor::default();
        assert_relative_eq!(s.matrix[(0, 0)], 0.0);
    }

    #[test]
    fn strain_tensor_default() {
        let s = StrainTensor::default();
        assert_relative_eq!(s.matrix[(0, 0)], 0.0);
    }

    #[test]
    fn mass_mu_power_aliases() {
        let m = Mass::new(100.0);
        let mu = Mu::new(3.986e14);
        let p = PowerScalar::new(500.0);
        assert_relative_eq!(m.value, 100.0);
        assert_relative_eq!(mu.value, 3.986e14);
        assert_relative_eq!(p.value, 500.0);
    }
}