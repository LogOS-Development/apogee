//! Magnetosphere models: geomagnetic field.
//!
//! The primary model is the International Geomagnetic Reference Field (IGRF)
//! evaluated via spherical harmonics. It is time-varying through secular
//! variation coefficients, analogous to how atmospheric models vary with solar
//! and geomagnetic indices.

pub(crate) mod data;
pub(crate) mod disturbance;
pub mod igrf;
pub(crate) mod legendre;

pub use igrf::Igrf;

/// Trait for geomagnetic field models.
///
/// Implementations must be deterministic and thread-safe.
pub trait MagneticFieldModel: Send + Sync {
    /// Magnetic flux density at an ECEF position and epoch (nT).
    fn field(
        &self,
        position_m: &nalgebra::Vector3<f64>,
        epoch: hifitime::Epoch,
    ) -> nalgebra::Vector3<f64>;
}

impl MagneticFieldModel for Igrf {
    fn field(
        &self,
        position_m: &nalgebra::Vector3<f64>,
        epoch: hifitime::Epoch,
    ) -> nalgebra::Vector3<f64> {
        self.field(position_m, epoch)
    }
}
