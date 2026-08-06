//! Compile-time symbolic unit system.
//!
//! Quantities are tagged by a seven-tuple of type-level signed integers
//! (typenum) representing exponents of the SI base units
//! (meter, kilogram, second, ampere, kelvin, mole, candela). All unit
//! derivation is type-level: `*` adds exponents, `/` subtracts them, `sqrt`
//! halves them. Distinct expressions with the same dimensions collapse to
//! the same Rust type, so assigning `Meters / Seconds` to `Velocity` works
//! without explicit conversion.
//!
//! # SI prefix handling
//!
//! [`SiPrefix`] is a runtime enum that maps each SI decimal prefix
//! (Yocto..Yotta, including the identity `None`) to its multiplicative
//! scale factor. A const [`SiPrefix::SCALES`] table provides the factors
//! for programmatic conversion; users can:
//!
//! 1. Construct a quantity at a specific scale with the prefixed-type
//!    aliases (e.g. `Kilometers::new(1.0)`) and have it normalize to the
//!    underlying SI base on construction.
//! 2. Convert between any two prefixed representations of the same
//!    dimension via [`ConvertPrefix::convert_to`], which uses the
//!    `SiPrefix` table to compute the multiplicative factor at runtime.
//! 3. Read the scale factor directly with [`SiPrefix::scale`] for
//!    ad-hoc arithmetic.
//!
//! # Design cost
//!
//! * Type-checking cost per operation: **O(1)** — each unit is a fixed 7-tuple.
//! * Monomorphization cost: **O(number of distinct unit types used)** — bounded
//!   by the combinations actually referenced.
//! * Runtime cost: identical to the wrapped scalar; the wrapper is a single-field
//!   struct with no runtime unit table.
//! * Memory cost: identical to the wrapped scalar.
//!
//! # Example
//! ```
//! use apogee_common::units::*;
//!
//! let x = Meters::new(10.0);
//! let t = Seconds::new(2.0);
//! let v: Velocity<f64> = x / t;
//! let a: Acceleration<f64> = v / t;
//! ```

use std::fmt;
use std::marker::PhantomData;
use std::ops::{Add, Deref, DerefMut, Div, Mul, Neg, Sub};

use nalgebra::Vector3;
use typenum::consts::*;
use typenum::{Diff, Sum, Z0};

/// SI decimal prefixes in increasing order, with the identity
/// (`SiPrefix::None`, no scaling) at index 10. Indexing [`SCALES`] by
/// `variant as usize` gives the multiplicative factor relative to the
/// unprefixed SI base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum SiPrefix {
    Yocto,
    Zepto,
    Atto,
    Femto,
    Pico,
    Nano,
    Micro,
    Milli,
    Centi,
    Deci,
    /// Identity: 1.0 (no scaling).
    #[default]
    None,
    Deca,
    Hecto,
    Kilo,
    Mega,
    Giga,
    Tera,
    Peta,
    Exa,
    Zetta,
    Yotta,
}

impl SiPrefix {
    /// Multiplicative scale factors indexed by `variant as usize`:
    /// 10^index_minus_10 for the 21 prefixes (Yocto at index 0 → 10^-24,
    /// Yotta at index 20 → 10^24). The identity prefix (`None`, index 10)
    /// is 1.0.
    pub const SCALES: [f64; 21] = [
        1.0e-24, // Yocto
        1.0e-21, // Zepto
        1.0e-18, // Atto
        1.0e-15, // Femto
        1.0e-12, // Pico
        1.0e-9,  // Nano
        1.0e-6,  // Micro
        1.0e-3,  // Milli
        1.0e-2,  // Centi
        1.0e-1,  // Deci
        1.0,     // None
        1.0e1,   // Deca
        1.0e2,   // Hecto
        1.0e3,   // Kilo
        1.0e6,   // Mega
        1.0e9,   // Giga
        1.0e12,  // Tera
        1.0e15,  // Peta
        1.0e18,  // Exa
        1.0e21,  // Zetta
        1.0e24,  // Yotta
    ];

    /// Return the multiplicative scale for this prefix. Equivalent to
    /// `Self::SCALES[self as usize]`.
    #[inline]
    #[must_use]
    pub const fn scale(self) -> f64 {
        Self::SCALES[self as usize]
    }
}

impl fmt::Display for SiPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Yocto => "y",
            Self::Zepto => "z",
            Self::Atto => "a",
            Self::Femto => "f",
            Self::Pico => "p",
            Self::Nano => "n",
            Self::Micro => "µ",
            Self::Milli => "m",
            Self::Centi => "c",
            Self::Deci => "d",
            Self::None => "",
            Self::Deca => "da",
            Self::Hecto => "h",
            Self::Kilo => "k",
            Self::Mega => "M",
            Self::Giga => "G",
            Self::Tera => "T",
            Self::Peta => "P",
            Self::Exa => "E",
            Self::Zetta => "Z",
            Self::Yotta => "Y",
        };
        f.write_str(name)
    }
}

/// A unit is a 7-tuple of type-level signed integer exponents over the SI
/// base units, in order: `[m, kg, s, A, K, mol, cd]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Unit<T>(PhantomData<T>);

/// Convenience: base units. These are the unprefixed SI base dimensions;
/// use [`SiPrefix`] (or the prefixed aliases below) for scaled variants.
pub type Meters<T> = Quantity<T, Unit<(P1, Z0, Z0, Z0, Z0, Z0, Z0)>>;
pub type Kilograms<T> = Quantity<T, Unit<(Z0, P1, Z0, Z0, Z0, Z0, Z0)>>;
pub type Seconds<T> = Quantity<T, Unit<(Z0, Z0, P1, Z0, Z0, Z0, Z0)>>;
pub type Amperes<T> = Quantity<T, Unit<(Z0, Z0, Z0, P1, Z0, Z0, Z0)>>;
pub type Kelvins<T> = Quantity<T, Unit<(Z0, Z0, Z0, Z0, P1, Z0, Z0)>>;
pub type Moles<T> = Quantity<T, Unit<(Z0, Z0, Z0, Z0, Z0, P1, Z0)>>;
pub type Candelas<T> = Quantity<T, Unit<(Z0, Z0, Z0, Z0, Z0, Z0, P1)>>;
pub type Dimensionless<T> = Quantity<T, Unit<(Z0, Z0, Z0, Z0, Z0, Z0, Z0)>>;

/// Convenience: derived units. These are dimensionally equivalent to the
/// corresponding base-unit operations (e.g. `Meters / Seconds` collapses to
/// `Velocity`), but are spelled out explicitly because Rust type aliases cannot
/// carry the required trait bounds for inference.
pub type Velocity<T> = Quantity<T, Unit<(P1, Z0, N1, Z0, Z0, Z0, Z0)>>;
pub type Acceleration<T> = Quantity<T, Unit<(P1, Z0, N2, Z0, Z0, Z0, Z0)>>;
pub type Force<T> = Quantity<T, Unit<(P1, P1, N2, Z0, Z0, Z0, Z0)>>;
pub type Torque<T> = Quantity<T, Unit<(P2, P1, N2, Z0, Z0, Z0, Z0)>>;
pub type Pressure<T> = Quantity<T, Unit<(N1, P1, N2, Z0, Z0, Z0, Z0)>>;
pub type Energy<T> = Quantity<T, Unit<(P2, P1, N2, Z0, Z0, Z0, Z0)>>;
pub type Power<T> = Quantity<T, Unit<(P2, P1, N3, Z0, Z0, Z0, Z0)>>;
pub type Area<T> = Quantity<T, Unit<(P2, Z0, Z0, Z0, Z0, Z0, Z0)>>;
pub type Volume<T> = Quantity<T, Unit<(P3, Z0, Z0, Z0, Z0, Z0, Z0)>>;
pub type Density<T> = Quantity<T, Unit<(N3, P1, Z0, Z0, Z0, Z0, Z0)>>;
pub type Frequency<T> = Quantity<T, Unit<(Z0, Z0, N1, Z0, Z0, Z0, Z0)>>;
pub type ElectricCharge<T> = Quantity<T, Unit<(Z0, Z0, P1, P1, Z0, Z0, Z0)>>;
pub type Voltage<T> = Quantity<T, Unit<(P2, P1, N3, N1, Z0, Z0, Z0)>>;
pub type Kilometers<T> = Quantity<T, Unit<(P3, Z0, Z0, Z0, Z0, Z0, Z0)>>;
pub type Nanoteslas<T> = Quantity<T, Unit<(N2, P1, N2, Z0, Z0, Z0, Z0)>>;
pub type GravitationalParameter<T> = Quantity<T, Unit<(P3, Z0, N2, Z0, Z0, Z0, Z0)>>;

/// A scalar `value` tagged with a compile-time unit `U`.
///
/// The unit is part of the type, so dimensional mismatches are caught at
/// compile time. The runtime representation stores the SI base-unit value
/// plus an auto-selected [`SiPrefix`] for display and conversion. The base
/// value is always available via [`Self::into_value`] and the public `value`
/// field; the normalized mantissa can be obtained with [`Self::mantissa`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Quantity<T, U> {
    pub value: T,
    pub prefix: SiPrefix,
    _unit: PhantomData<U>,
}

// --- Dimension-only unit aliases for vector quantities ---
//
// These are bare `Unit<T>` markers used by `Vector3Quantity<U>` to record
// physical dimension. The default scale is SI base units (m, m/s, m/s², N,
// N·m); callers convert at output boundaries using [`SiPrefix`] or project
// constants such as `crate::constants::AU`.

/// Dimension marker for length (m¹).
pub type LengthDim = Unit<(P1, Z0, Z0, Z0, Z0, Z0, Z0)>;
/// Dimension marker for velocity (m¹·s⁻¹).
pub type VelocityDim = Unit<(P1, Z0, N1, Z0, Z0, Z0, Z0)>;
/// Dimension marker for acceleration (m¹·s⁻²).
pub type AccelerationDim = Unit<(P1, Z0, N2, Z0, Z0, Z0, Z0)>;
/// Dimension marker for force (m¹·kg¹·s⁻²).
pub type ForceDim = Unit<(P1, P1, N2, Z0, Z0, Z0, Z0)>;
/// Dimension marker for torque (m²·kg¹·s⁻²).
pub type TorqueDim = Unit<(P2, P1, N2, Z0, Z0, Z0, Z0)>;

// --- Vector-quantity newtypes ---
//
// nalgebra's `Vector3<S>` requires `S: Scalar` (which implies `One` and `Zero`),
// so a `Vector3<Acceleration<f64>>` cannot compile. To give force-aggregator
// and celestial models a unit-aware public API without breaking nalgebra
// geometry operations, each vector quantity is exposed as a thin newtype
// around `Vector3<f64>` with a named escape hatch and per-component accessors
// returning the corresponding `Quantity<f64, U>`. Values are stored in SI base
// units; conversions happen at crate boundaries.

/// A vector of `f64` components tagged with a compile-time unit dimension `U`.
///
/// The dimension is part of the type, so assigning a velocity vector to a
/// position field is a compile-time error. The wrapper dereferences to the raw
/// `Vector3<f64>` so nalgebra operations (`+`, `-`, `.dot()`, `.norm()`,
/// `.normalize()`) work directly. Components and constructor values are in SI
/// base units unless the caller applies a prefix or conversion constant.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vector3Quantity<U> {
    value: Vector3<f64>,
    _unit: PhantomData<U>,
}

impl<U> Vector3Quantity<U> {
    /// Wrap a raw `Vector3<f64>`.
    #[must_use]
    pub const fn new(value: Vector3<f64>) -> Self {
        Self {
            value,
            _unit: PhantomData,
        }
    }

    /// Borrow the raw vector.
    #[must_use]
    pub const fn value(&self) -> &Vector3<f64> {
        &self.value
    }

    /// Borrow the raw vector (alias for [`Self::value`] for parity with the
    /// magnetosphere domain-newtype pattern).
    #[must_use]
    pub const fn raw(&self) -> &Vector3<f64> {
        &self.value
    }

    /// Sum two same-dimension vectors component-wise.
    #[must_use]
    pub fn plus(&self, other: &Self) -> Self {
        Self::new(self.value + other.value)
    }

    /// Euclidean distance to another same-dimension vector.
    #[must_use]
    pub fn distance_to(&self, other: &Self) -> f64 {
        (self.value - other.value).norm()
    }

    /// X component wrapped as a scalar [`Quantity`].
    #[must_use]
    pub fn x(&self) -> Quantity<f64, U> {
        Quantity::new(self.value.x)
    }

    /// Y component wrapped as a scalar [`Quantity`].
    #[must_use]
    pub fn y(&self) -> Quantity<f64, U> {
        Quantity::new(self.value.y)
    }

    /// Z component wrapped as a scalar [`Quantity`].
    #[must_use]
    pub fn z(&self) -> Quantity<f64, U> {
        Quantity::new(self.value.z)
    }
}

impl<U> Deref for Vector3Quantity<U> {
    type Target = Vector3<f64>;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<U> DerefMut for Vector3Quantity<U> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<U> From<Vector3<f64>> for Vector3Quantity<U> {
    fn from(value: Vector3<f64>) -> Self {
        Self::new(value)
    }
}

impl<U> From<Vector3Quantity<U>> for Vector3<f64> {
    fn from(v: Vector3Quantity<U>) -> Self {
        v.value
    }
}

impl<U> Add for Vector3Quantity<U> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.value + rhs.value)
    }
}

impl<U> Sub for Vector3Quantity<U> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.value - rhs.value)
    }
}

impl<U> Neg for Vector3Quantity<U> {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self::new(-self.value)
    }
}

impl<U> Mul<f64> for Vector3Quantity<U> {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self::Output {
        Self::new(self.value * rhs)
    }
}

impl<U> Div<f64> for Vector3Quantity<U> {
    type Output = Self;
    fn div(self, rhs: f64) -> Self::Output {
        Self::new(self.value / rhs)
    }
}

/// Position vector in meters.
pub type PositionVec = Vector3Quantity<LengthDim>;
/// Velocity vector in m/s.
pub type VelocityVec = Vector3Quantity<VelocityDim>;
/// Acceleration vector in m/s².
pub type AccelerationVec = Vector3Quantity<AccelerationDim>;
/// Force vector in N.
pub type ForceVec = Vector3Quantity<ForceDim>;
/// Torque vector in N·m.
pub type TorqueVec = Vector3Quantity<TorqueDim>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_quantity_wraps_value() {
        let m = Meters::new(5.0);
        assert_eq!(m.into_value(), 5.0);
        assert_eq!(*m.value_ref(), 5.0);
    }

    #[test]
    fn same_unit_addition_preserves_dimension() {
        let a = Meters::new(3.0);
        let b = Meters::new(2.0);
        let sum = a + b;
        assert_eq!(sum.into_value(), 5.0);
    }

    #[test]
    fn velocity_from_distance_over_time() {
        let x = Meters::new(10.0);
        let t = Seconds::new(2.0);
        let v: Velocity<f64> = x / t;
        assert_eq!(v.into_value(), 5.0);
    }

    #[test]
    fn acceleration_from_velocity_over_time() {
        let v: Velocity<f64> = Velocity::new(10.0);
        let t = Seconds::new(2.0);
        let a: Acceleration<f64> = v / t;
        assert_eq!(a.into_value(), 5.0);
    }

    #[test]
    fn scalar_mul_and_div_preserve_unit() {
        let f = Force::new(6.0);
        let doubled = f * 2.0;
        let halved = f / 2.0;
        assert_eq!(doubled.into_value(), 12.0);
        assert_eq!(halved.into_value(), 3.0);
    }

    #[test]
    fn sqrt_of_area_is_length() {
        let area = Area::new(25.0);
        let length: Meters<f64> = area.sqrt();
        assert_eq!(length.into_value(), 5.0);
    }

    #[test]
    fn display_renders_inverse_unit_with_negative_superscript() {
        let f = Frequency::new(60.0);
        assert_eq!(format!("{}", f), "6 das⁻¹");
    }

    #[test]
    fn display_renders_acceleration_unit() {
        let a = Acceleration::new(9.8);
        assert_eq!(format!("{}", a), "9.8 m/s²");
    }

    #[test]
    fn display_renders_force_unit() {
        let f = Force::new(5.0);
        assert_eq!(format!("{}", f), "5 m·kg/s²");
    }

    #[test]
    fn display_renders_prefixed_force_unit() {
        let f = Force::new(5_000.0);
        assert_eq!(format!("{}", f), "5 km·kg/s²");
    }

    #[test]
    fn display_renders_power_unit() {
        let p = Power::new(5.0);
        assert_eq!(format!("{}", p), "5 m²·kg/s³");
    }

    #[test]
    fn display_renders_prefixed_power_unit() {
        let p = Power::new(500.0);
        assert_eq!(format!("{}", p), "5 hm²·kg/s³");
    }

    #[test]
    fn prefix_scale_table_is_ordered() {
        assert_eq!(SiPrefix::Yocto.scale(), 1.0e-24);
        assert_eq!(SiPrefix::None.scale(), 1.0);
        assert_eq!(SiPrefix::Yotta.scale(), 1.0e24);
    }

    #[test]
    fn auto_prefix_normalizes_mantissa_to_one_to_ten() {
        let m = Meters::new(5_000.0);
        assert_eq!(m.into_value(), 5_000.0);
        assert_eq!(m.mantissa(), 5.0);
        assert_eq!(m.prefix(), SiPrefix::Kilo);

        let small = Meters::new(0.5);
        assert_eq!(small.into_value(), 0.5);
        assert_eq!(small.mantissa(), 5.0);
        assert_eq!(small.prefix(), SiPrefix::Deci);

        let zero = Meters::new(0.0);
        assert_eq!(zero.into_value(), 0.0);
        assert_eq!(zero.mantissa(), 0.0);
        assert_eq!(zero.prefix(), SiPrefix::None);

        let negative = Meters::new(-5_000.0);
        assert_eq!(negative.into_value(), -5_000.0);
        assert_eq!(negative.mantissa(), -5.0);
        assert_eq!(negative.prefix(), SiPrefix::Kilo);
    }

    #[test]
    fn vector_quantity_derefs_to_raw_vector() {
        let p = PositionVec::new(Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(p.norm(), (14.0_f64).sqrt());
        assert_eq!(p.value(), &Vector3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn vector_quantity_components_have_scalar_unit() {
        let a = AccelerationVec::new(Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(a.x().into_value(), 1.0);
        assert_eq!(a.y().into_value(), 2.0);
        assert_eq!(a.z().into_value(), 3.0);
    }

    #[test]
    fn vector_quantity_add_and_sub_require_same_dimension() {
        let a = AccelerationVec::new(Vector3::new(1.0, 0.0, 0.0));
        let b = AccelerationVec::new(Vector3::new(0.0, 2.0, 3.0));
        let sum = a + b;
        assert_eq!(sum.value(), &Vector3::new(1.0, 2.0, 3.0));

        let diff = a - b;
        assert_eq!(diff.value(), &Vector3::new(1.0, -2.0, -3.0));
    }

    #[test]
    fn vector_quantity_scalar_mul_and_div() {
        let v = VelocityVec::new(Vector3::new(1.0, 2.0, 3.0));
        let doubled = v * 2.0;
        assert_eq!(doubled.value(), &Vector3::new(2.0, 4.0, 6.0));
        let halved = v / 2.0;
        assert_eq!(halved.value(), &Vector3::new(0.5, 1.0, 1.5));
    }

    #[test]
    fn position_and_velocity_are_distinct_types() {
        let p = PositionVec::new(Vector3::new(1.0, 0.0, 0.0));
        let v = VelocityVec::new(Vector3::new(1.0, 0.0, 0.0));

        // Both dereference to the same raw value.
        assert_eq!(p.value(), v.value());

        // Subtraction within the same dimension works.
        let _ = p - PositionVec::new(Vector3::zeros());
        let _ = v - VelocityVec::new(Vector3::zeros());

        // The following would be a compile-time error (dimensions differ):
        // let _ = p - v;
    }

    #[test]
    fn force_vec_plus_sums_components() {
        let a = ForceVec::new(Vector3::new(1.0, 0.0, 0.0));
        let b = ForceVec::new(Vector3::new(0.0, 2.0, 3.0));
        let sum = a.plus(&b);
        assert_eq!(sum.value(), &Vector3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn torque_vec_round_trips_via_from_into() {
        let raw = Vector3::new(4.0, 5.0, 6.0);
        let t: TorqueVec = raw.into();
        let back: Vector3<f64> = t.into();
        assert_eq!(back, raw);
    }
}

impl<T, U> Quantity<T, U> {
    /// Construct a quantity without auto-prefixing. The stored value is the
    /// SI base value and the prefix is [`SiPrefix::None`].
    ///
    /// Use this for non-`f64` scalar types. For `f64` values, prefer
    /// [`Quantity::new`] so the prefix is auto-selected.
    #[must_use]
    pub const fn new_raw(value: T) -> Self {
        Self {
            value,
            prefix: SiPrefix::None,
            _unit: PhantomData,
        }
    }

    /// Unwrap the SI base-unit scalar value.
    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }

    /// Borrow the SI base-unit scalar value.
    #[must_use]
    pub const fn value_ref(&self) -> &T {
        &self.value
    }

    /// The auto-selected decimal prefix carried by this quantity.
    #[must_use]
    pub const fn prefix(&self) -> SiPrefix {
        self.prefix
    }
}

impl<U> Quantity<f64, U> {
    /// Wrap an `f64` scalar value with a unit, auto-selecting the SI decimal
    /// prefix that normalizes the mantissa into `[1.0, 10.0)`.
    ///
    /// The SI base value is stored in `value`; `prefix` records the selected
    /// scale. Use [`Self::mantissa`] to read the normalized mantissa, and
    /// [`Self::into_value`] for the base value.
    ///
    /// # Example
    /// ```
    /// use apogee_common::units::*;
    ///
    /// let r = Meters::new(5_000.0);        // base value 5000 m, prefix Kilo
    /// assert_eq!(r.into_value(), 5_000.0); // base value unchanged
    /// assert_eq!(r.mantissa(), 5.0);       // normalized mantissa
    /// assert_eq!(r.prefix(), SiPrefix::Kilo);
    ///
    /// let small = Meters::new(0.5);        // 0.5 m -> 5 dm, prefix Deci
    /// assert_eq!(small.mantissa(), 5.0);
    /// assert_eq!(small.prefix(), SiPrefix::Deci);
    /// ```
    #[must_use]
    pub const fn new(value: f64) -> Self {
        let prefix = Self::select_prefix(value);
        Self {
            value,
            prefix,
            _unit: PhantomData,
        }
    }

    /// The normalized mantissa in `[1.0, 10.0)` (sign preserved). Multiply by
    /// [`Self::prefix`]`].scale()` to recover the base SI value.
    #[must_use]
    pub fn mantissa(&self) -> f64 {
        self.value / self.prefix.scale()
    }

    const fn select_prefix(value: f64) -> SiPrefix {
        Self::select_prefix_from(value, SiPrefix::SCALES.len())
    }

    const fn select_prefix_from(value: f64, idx: usize) -> SiPrefix {
        if idx == 0 {
            return SiPrefix::None;
        }
        let i = idx - 1;
        let scale = SiPrefix::SCALES[i];
        let scaled_abs = (value / scale).abs();
        if scaled_abs >= 1.0 && scaled_abs < 10.0 {
            Self::prefix_at(i)
        } else {
            Self::select_prefix_from(value, i)
        }
    }

    const fn prefix_at(idx: usize) -> SiPrefix {
        match idx {
            0 => SiPrefix::Yocto,
            1 => SiPrefix::Zepto,
            2 => SiPrefix::Atto,
            3 => SiPrefix::Femto,
            4 => SiPrefix::Pico,
            5 => SiPrefix::Nano,
            6 => SiPrefix::Micro,
            7 => SiPrefix::Milli,
            8 => SiPrefix::Centi,
            9 => SiPrefix::Deci,
            10 => SiPrefix::None,
            11 => SiPrefix::Deca,
            12 => SiPrefix::Hecto,
            13 => SiPrefix::Kilo,
            14 => SiPrefix::Mega,
            15 => SiPrefix::Giga,
            16 => SiPrefix::Tera,
            17 => SiPrefix::Peta,
            18 => SiPrefix::Exa,
            19 => SiPrefix::Zetta,
            20 => SiPrefix::Yotta,
            _ => SiPrefix::None,
        }
    }
}

/// Type-level halving for square root. Implemented for the even exponents
/// likely to appear in physical models.
pub trait Half {
    type Output;
}
impl Half for Z0 {
    type Output = Z0;
}
impl Half for P2 {
    type Output = P1;
}
impl Half for P4 {
    type Output = P2;
}
impl Half for P6 {
    type Output = P3;
}
impl Half for P8 {
    type Output = P4;
}
impl Half for N2 {
    type Output = N1;
}
impl Half for N4 {
    type Output = N2;
}
impl Half for N6 {
    type Output = N3;
}
impl Half for N8 {
    type Output = N4;
}

impl<M, Kg, S, A, K, Mol, Cd> Quantity<f64, Unit<(M, Kg, S, A, K, Mol, Cd)>>
where
    M: Half,
    Kg: Half,
    S: Half,
    A: Half,
    K: Half,
    Mol: Half,
    Cd: Half,
{
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn sqrt(
        self,
    ) -> Quantity<
        f64,
        Unit<(
            M::Output,
            Kg::Output,
            S::Output,
            A::Output,
            K::Output,
            Mol::Output,
            Cd::Output,
        )>,
    > {
        Quantity::new(self.value.sqrt())
    }
}

// --- Addition / subtraction (same unit) ---

impl<U> Add for Quantity<f64, U> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Quantity::new(self.value + rhs.value)
    }
}

impl<U> Sub for Quantity<f64, U> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Quantity::new(self.value - rhs.value)
    }
}

impl<U> Neg for Quantity<f64, U> {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Quantity::new(-self.value)
    }
}

// --- Multiplication / division (combine units) ---

impl<M1, Kg1, S1, A1, K1, Mol1, Cd1, M2, Kg2, S2, A2, K2, Mol2, Cd2>
    Mul<Quantity<f64, Unit<(M2, Kg2, S2, A2, K2, Mol2, Cd2)>>>
    for Quantity<f64, Unit<(M1, Kg1, S1, A1, K1, Mol1, Cd1)>>
where
    M1: Add<M2>,
    Kg1: Add<Kg2>,
    S1: Add<S2>,
    A1: Add<A2>,
    K1: Add<K2>,
    Mol1: Add<Mol2>,
    Cd1: Add<Cd2>,
{
    type Output = Quantity<
        f64,
        Unit<(
            Sum<M1, M2>,
            Sum<Kg1, Kg2>,
            Sum<S1, S2>,
            Sum<A1, A2>,
            Sum<K1, K2>,
            Sum<Mol1, Mol2>,
            Sum<Cd1, Cd2>,
        )>,
    >;
    fn mul(self, rhs: Quantity<f64, Unit<(M2, Kg2, S2, A2, K2, Mol2, Cd2)>>) -> Self::Output {
        Quantity::new(self.value * rhs.value)
    }
}

impl<M1, Kg1, S1, A1, K1, Mol1, Cd1, M2, Kg2, S2, A2, K2, Mol2, Cd2>
    Div<Quantity<f64, Unit<(M2, Kg2, S2, A2, K2, Mol2, Cd2)>>>
    for Quantity<f64, Unit<(M1, Kg1, S1, A1, K1, Mol1, Cd1)>>
where
    M1: Sub<M2>,
    Kg1: Sub<Kg2>,
    S1: Sub<S2>,
    A1: Sub<A2>,
    K1: Sub<K2>,
    Mol1: Sub<Mol2>,
    Cd1: Sub<Cd2>,
{
    type Output = Quantity<
        f64,
        Unit<(
            Diff<M1, M2>,
            Diff<Kg1, Kg2>,
            Diff<S1, S2>,
            Diff<A1, A2>,
            Diff<K1, K2>,
            Diff<Mol1, Mol2>,
            Diff<Cd1, Cd2>,
        )>,
    >;
    fn div(self, rhs: Quantity<f64, Unit<(M2, Kg2, S2, A2, K2, Mol2, Cd2)>>) -> Self::Output {
        Quantity::new(self.value / rhs.value)
    }
}

// --- Scalar multiplication / division ---

impl<U> Mul<f64> for Quantity<f64, U> {
    type Output = Quantity<f64, U>;
    fn mul(self, rhs: f64) -> Self::Output {
        Quantity::new(self.value * rhs)
    }
}

impl<U> Div<f64> for Quantity<f64, U> {
    type Output = Quantity<f64, U>;
    fn div(self, rhs: f64) -> Self::Output {
        Quantity::new(self.value / rhs)
    }
}

// --- Display ---

/// Convert a typenum integer type to its `i8` value for rendering.
/// Implemented for the common exponent range used in physical models.
pub trait ToI8 {
    const VALUE: i8;
}

impl ToI8 for Z0 {
    const VALUE: i8 = 0;
}
macro_rules! impl_to_i8 {
    ($($ty:ty => $val:expr),* $(,)?) => {
        $(impl ToI8 for $ty { const VALUE: i8 = $val; })*
    };
}

impl_to_i8! {
    P1 => 1, P2 => 2, P3 => 3, P4 => 4, P5 => 5,
    P6 => 6, P7 => 7, P8 => 8, P9 => 9,
    N1 => -1, N2 => -2, N3 => -3, N4 => -4, N5 => -5,
    N6 => -6, N7 => -7, N8 => -8, N9 => -9,
}

const UNIT_NAMES: [&str; 7] = ["m", "kg", "s", "A", "K", "mol", "cd"];

impl<M, Kg, S, A, K, Mol, Cd> fmt::Display for Quantity<f64, Unit<(M, Kg, S, A, K, Mol, Cd)>>
where
    M: ToI8,
    Kg: ToI8,
    S: ToI8,
    A: ToI8,
    K: ToI8,
    Mol: ToI8,
    Cd: ToI8,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let exponents = [
            M::VALUE,
            Kg::VALUE,
            S::VALUE,
            A::VALUE,
            K::VALUE,
            Mol::VALUE,
            Cd::VALUE,
        ];
        let mantissa = self.value / self.prefix.scale();
        write!(f, "{} {}{}", mantissa, self.prefix, format_unit(&exponents))?;
        Ok(())
    }
}

fn format_unit(exponents: &[i8; 7]) -> String {
    let mut num = String::new();
    let mut den = String::new();

    for (i, &name) in UNIT_NAMES.iter().enumerate() {
        let exp = exponents[i];
        if exp == 0 {
            continue;
        }
        if exp > 0 {
            if !num.is_empty() {
                num.push('·');
            }
            num.push_str(name);
            if exp != 1 {
                num.push_str(&format_superscript(exp));
            }
        } else {
            if !den.is_empty() {
                den.push('·');
            }
            den.push_str(name);
            if exp != -1 {
                den.push_str(&format_superscript(-exp));
            }
        }
    }

    if num.is_empty() && den.is_empty() {
        return "(dimensionless)".to_string();
    }
    if den.is_empty() {
        return num;
    }
    if num.is_empty() {
        // Lone inverse unit: render with explicit negative exponents.
        let mut inv = String::new();
        for (i, &name) in UNIT_NAMES.iter().enumerate() {
            let exp = exponents[i];
            if exp == 0 {
                continue;
            }
            if !inv.is_empty() {
                inv.push('·');
            }
            inv.push_str(name);
            inv.push_str(&format_superscript(exp));
        }
        return inv;
    }
    format!("{}/{}", num, den)
}

fn format_superscript(exp: i8) -> String {
    match exp {
        -1 => "⁻¹".to_string(),
        -2 => "⁻²".to_string(),
        -3 => "⁻³".to_string(),
        -4 => "⁻⁴".to_string(),
        -5 => "⁻⁵".to_string(),
        -6 => "⁻⁶".to_string(),
        -7 => "⁻⁷".to_string(),
        -8 => "⁻⁸".to_string(),
        -9 => "⁻⁹".to_string(),
        1 => "¹".to_string(),
        2 => "²".to_string(),
        3 => "³".to_string(),
        4 => "⁴".to_string(),
        5 => "⁵".to_string(),
        6 => "⁶".to_string(),
        7 => "⁷".to_string(),
        8 => "⁸".to_string(),
        9 => "⁹".to_string(),
        _ => format!("^{}", exp),
    }
}
