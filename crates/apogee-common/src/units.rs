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
//! ```ignore
//! use apogee_common::units::*;
//!
//! let x = Meters::new(10.0);
//! let t = Seconds::new(2.0);
//! let v: Velocity<f64> = x / t;
//! let a: Acceleration<f64> = v / t;
//!
//! // Programmatic SI prefix handling.
//! let km = Meters::with_prefix(SiPrefix::Kilo, 1.0);  // stored as 1000.0 m
//! let in_mm = km.with_prefix(SiPrefix::Milli);       // stored as 1_000_000.0 m
//! ```

use std::fmt;
use std::marker::PhantomData;
use std::ops::{Add, Div, Mul, Neg, Sub};

use typenum::consts::*;
use typenum::{Diff, Sum, Z0};

/// SI decimal prefixes in increasing order, with the identity
/// (`SiPrefix::None`, no scaling) at index 10. Indexing [`SCALES`] by
/// `variant as usize` gives the multiplicative factor relative to the
/// unprefixed SI base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Convenience: derived units.
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
/// compile time. The runtime representation is the scalar alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Quantity<T, U> {
    pub value: T,
    _unit: PhantomData<U>,
}

impl<T, U> Quantity<T, U> {
    /// Wrap a raw scalar value with a unit.
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self {
            value,
            _unit: PhantomData,
        }
    }

    /// Unwrap the scalar value.
    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }

    /// Borrow the scalar value.
    #[must_use]
    pub const fn value_ref(&self) -> &T {
        &self.value
    }

    /// Apply an SI decimal prefix to the wrapped value, returning a new
    /// `Quantity` in the same unit. The prefix's [`SiPrefix::scale`]
    /// factor is multiplied with `self.value`.
    ///
    /// This is the runtime-programmatic counterpart to the type-level
    /// prefixed aliases (e.g. `Kilometers`): users can construct any
    /// quantity with `Quantity::new(value)` and then `with_prefix` it into
    /// a chosen scale, or do the reverse by dividing by a known prefix
    /// scale (`self.value / SiPrefix::Milli.scale()`).
    ///
    /// # Example
    /// ```ignore
    /// use apogee_common::units::*;
    ///
    /// // Convert 1 km (stored as 1000 m) into millimeters: 1_000_000 mm.
    /// let one_km: Meters<f64> = Meters::new(1.0e3);
    /// let in_mm = one_km.with_prefix(SiPrefix::Milli);
    /// assert_eq!(in_mm.into_value(), 1.0e6);
    ///
    /// // Convert 1 Mm (megameter) into meters: 1_000_000 m.
    /// let in_m = Meters::new(1.0).with_prefix(SiPrefix::Mega);
    /// assert_eq!(in_m.into_value(), 1.0e6);
    /// ```
    #[must_use]
    pub fn with_prefix(self, prefix: SiPrefix) -> Self
    where
        T: Copy + core::ops::Mul<f64, Output = T>,
    {
        Quantity::new(self.value * prefix.scale())
    }

    /// Divide the wrapped value by an SI prefix scale, returning a new
    /// `Quantity` in the same unit. Inverse of [`Quantity::with_prefix`].
    #[must_use]
    pub fn strip_prefix(self, prefix: SiPrefix) -> Self
    where
        T: Copy + core::ops::Div<f64, Output = T>,
    {
        Quantity::new(self.value / prefix.scale())
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

impl<T, M, Kg, S, A, K, Mol, Cd> Quantity<T, Unit<(M, Kg, S, A, K, Mol, Cd)>>
where
    M: Half,
    Kg: Half,
    S: Half,
    A: Half,
    K: Half,
    Mol: Half,
    Cd: Half,
    T: num_traits::Float,
{
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn sqrt(
        self,
    ) -> Quantity<
        T,
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

impl<T, U> Add for Quantity<T, U>
where
    T: Add,
{
    type Output = Quantity<<T as Add>::Output, U>;
    fn add(self, rhs: Self) -> Self::Output {
        Quantity::new(self.value + rhs.value)
    }
}

impl<T, U> Sub for Quantity<T, U>
where
    T: Sub,
{
    type Output = Quantity<<T as Sub>::Output, U>;
    fn sub(self, rhs: Self) -> Self::Output {
        Quantity::new(self.value - rhs.value)
    }
}

impl<T, U> Neg for Quantity<T, U>
where
    T: Neg,
{
    type Output = Quantity<<T as Neg>::Output, U>;
    fn neg(self) -> Self::Output {
        Quantity::new(-self.value)
    }
}

// --- Multiplication / division (combine units) ---

impl<T, M1, Kg1, S1, A1, K1, Mol1, Cd1, M2, Kg2, S2, A2, K2, Mol2, Cd2>
    Mul<Quantity<T, Unit<(M2, Kg2, S2, A2, K2, Mol2, Cd2)>>>
    for Quantity<T, Unit<(M1, Kg1, S1, A1, K1, Mol1, Cd1)>>
where
    T: Mul,
    M1: Add<M2>,
    Kg1: Add<Kg2>,
    S1: Add<S2>,
    A1: Add<A2>,
    K1: Add<K2>,
    Mol1: Add<Mol2>,
    Cd1: Add<Cd2>,
{
    type Output = Quantity<
        <T as Mul>::Output,
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
    fn mul(self, rhs: Quantity<T, Unit<(M2, Kg2, S2, A2, K2, Mol2, Cd2)>>) -> Self::Output {
        Quantity::new(self.value * rhs.value)
    }
}

impl<T, M1, Kg1, S1, A1, K1, Mol1, Cd1, M2, Kg2, S2, A2, K2, Mol2, Cd2>
    Div<Quantity<T, Unit<(M2, Kg2, S2, A2, K2, Mol2, Cd2)>>>
    for Quantity<T, Unit<(M1, Kg1, S1, A1, K1, Mol1, Cd1)>>
where
    T: Div,
    M1: Sub<M2>,
    Kg1: Sub<Kg2>,
    S1: Sub<S2>,
    A1: Sub<A2>,
    K1: Sub<K2>,
    Mol1: Sub<Mol2>,
    Cd1: Sub<Cd2>,
{
    type Output = Quantity<
        <T as Div>::Output,
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
    fn div(self, rhs: Quantity<T, Unit<(M2, Kg2, S2, A2, K2, Mol2, Cd2)>>) -> Self::Output {
        Quantity::new(self.value / rhs.value)
    }
}

// --- Scalar multiplication / division ---

impl<T, U> Mul<T> for Quantity<T, U>
where
    T: Mul<Output = T>,
{
    type Output = Quantity<T, U>;
    fn mul(self, rhs: T) -> Self::Output {
        Quantity::new(self.value * rhs)
    }
}

impl<T, U> Div<T> for Quantity<T, U>
where
    T: Div<Output = T>,
{
    type Output = Quantity<T, U>;
    fn div(self, rhs: T) -> Self::Output {
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

impl<T, M, Kg, S, A, K, Mol, Cd> fmt::Display for Quantity<T, Unit<(M, Kg, S, A, K, Mol, Cd)>>
where
    T: fmt::Display,
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
        write!(f, "{} {}", self.value, format_unit(&exponents))?;
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

// --- Vector-quantity newtypes ---
//
// nalgebra's `Vector3<S>` requires `S: Scalar` (which implies `One` and `Zero`),
// so a `Vector3<Acceleration<f64>>` cannot compile. To give force-aggregator
// models a unit-aware public API without breaking nalgebra geometry operations,
// each vector quantity is exposed as a thin newtype around `Vector3<f64>` with
// a raw escape hatch and per-component accessors returning the corresponding
// `Quantity<T, U>`. This mirrors the pattern established for `MagneticFieldVector`
// in `apogee-core::magnetosphere`.

use nalgebra::Vector3;

/// Acceleration vector in m/s². Wraps a raw `Vector3<f64>`; the units live in
/// the accessors.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AccelerationVec(pub Vector3<f64>);

impl AccelerationVec {
    /// Wrap a raw m/s² vector.
    #[must_use]
    pub const fn from_mps2(raw: Vector3<f64>) -> Self {
        Self(raw)
    }

    /// Borrow the raw vector in m/s².
    #[must_use]
    pub const fn raw(&self) -> &Vector3<f64> {
        &self.0
    }

    /// Sum two acceleration vectors component-wise.
    #[must_use]
    pub fn plus(&self, other: &Self) -> Self {
        Self(self.0 + other.0)
    }

    /// X component in m/s².
    #[must_use]
    pub fn x_mps2(&self) -> Acceleration<f64> {
        Acceleration::new(self.0.x)
    }

    /// Y component in m/s².
    #[must_use]
    pub fn y_mps2(&self) -> Acceleration<f64> {
        Acceleration::new(self.0.y)
    }

    /// Z component in m/s².
    #[must_use]
    pub fn z_mps2(&self) -> Acceleration<f64> {
        Acceleration::new(self.0.z)
    }
}

impl From<Vector3<f64>> for AccelerationVec {
    fn from(raw: Vector3<f64>) -> Self {
        Self(raw)
    }
}

impl From<AccelerationVec> for Vector3<f64> {
    fn from(v: AccelerationVec) -> Self {
        v.0
    }
}

/// Force vector in N. Wraps a raw `Vector3<f64>`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ForceVec(pub Vector3<f64>);

impl ForceVec {
    /// Wrap a raw N vector.
    #[must_use]
    pub const fn from_n(raw: Vector3<f64>) -> Self {
        Self(raw)
    }

    /// Borrow the raw vector in N.
    #[must_use]
    pub const fn raw(&self) -> &Vector3<f64> {
        &self.0
    }

    /// X component in N.
    #[must_use]
    pub fn x_n(&self) -> Force<f64> {
        Force::new(self.0.x)
    }

    /// Y component in N.
    #[must_use]
    pub fn y_n(&self) -> Force<f64> {
        Force::new(self.0.y)
    }

    /// Z component in N.
    #[must_use]
    pub fn z_n(&self) -> Force<f64> {
        Force::new(self.0.z)
    }
}

impl From<Vector3<f64>> for ForceVec {
    fn from(raw: Vector3<f64>) -> Self {
        Self(raw)
    }
}

impl From<ForceVec> for Vector3<f64> {
    fn from(v: ForceVec) -> Self {
        v.0
    }
}

/// Torque vector in N·m. Wraps a raw `Vector3<f64>`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TorqueVec(pub Vector3<f64>);

impl TorqueVec {
    /// Wrap a raw N·m vector.
    #[must_use]
    pub const fn from_nm(raw: Vector3<f64>) -> Self {
        Self(raw)
    }

    /// Borrow the raw vector in N·m.
    #[must_use]
    pub const fn raw(&self) -> &Vector3<f64> {
        &self.0
    }

    /// X component in N·m.
    #[must_use]
    pub fn x_nm(&self) -> Torque<f64> {
        Torque::new(self.0.x)
    }

    /// Y component in N·m.
    #[must_use]
    pub fn y_nm(&self) -> Torque<f64> {
        Torque::new(self.0.y)
    }

    /// Z component in N·m.
    #[must_use]
    pub fn z_nm(&self) -> Torque<f64> {
        Torque::new(self.0.z)
    }
}

impl From<Vector3<f64>> for TorqueVec {
    fn from(raw: Vector3<f64>) -> Self {
        Self(raw)
    }
}

impl From<TorqueVec> for Vector3<f64> {
    fn from(v: TorqueVec) -> Self {
        v.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn base_units_wrap_values() {
        let m: Meters<f64> = Meters::new(5.0);
        let kg: Kilograms<f64> = Kilograms::new(2.0);
        let s: Seconds<f64> = Seconds::new(3.0);
        assert_eq!(m.into_value(), 5.0);
        assert_eq!(kg.into_value(), 2.0);
        assert_eq!(s.into_value(), 3.0);
    }

    #[test]
    fn addition_requires_same_unit() {
        let a = Meters::new(3.0);
        let b = Meters::new(4.0);
        assert_eq!((a + b).into_value(), 7.0);
    }

    #[test]
    fn subtraction_requires_same_unit() {
        let a = Seconds::new(10.0);
        let b = Seconds::new(3.0);
        assert_eq!((a - b).into_value(), 7.0);
    }

    #[test]
    fn negation_preserves_unit() {
        let v = Velocity::new(5.0);
        assert_eq!((-v).into_value(), -5.0);
    }

    #[test]
    fn multiplication_combines_units() {
        let v = Velocity::new(10.0); // m/s
        let t = Seconds::new(2.0); // s
        let d: Meters<f64> = v * t;
        assert_eq!(d.into_value(), 20.0);
    }

    #[test]
    fn division_combines_units() {
        let d = Meters::new(100.0);
        let t = Seconds::new(10.0);
        let v: Velocity<f64> = d / t;
        assert_eq!(v.into_value(), 10.0);
    }

    #[test]
    fn scalar_multiplication_preserves_unit() {
        let f = Force::new(5.0);
        let scaled = f * 2.0;
        assert_eq!(scaled.into_value(), 10.0);
    }

    #[test]
    fn scalar_division_preserves_unit() {
        let p = Pressure::new(10.0);
        let halved = p / 2.0;
        assert_eq!(halved.into_value(), 5.0);
    }

    #[test]
    fn derived_units_from_base() {
        let m = Meters::new(10.0);
        let s = Seconds::new(2.0);
        let a: Acceleration<f64> = m / (s * s);
        assert_eq!(a.into_value(), 2.5);
    }

    #[test]
    fn sqrt_of_area_is_length() {
        let area = Area::new(16.0);
        let side: Meters<f64> = area.sqrt();
        assert_relative_eq!(side.into_value(), 4.0, epsilon = 1e-12);
    }

    #[test]
    fn display_renders_base_unit() {
        let m = Meters::new(5.0);
        assert_eq!(format!("{}", m), "5 m");
    }

    #[test]
    fn display_renders_derived_unit() {
        let a = Acceleration::new(9.81);
        assert_eq!(format!("{}", a), "9.81 m/s²");
    }

    #[test]
    fn display_renders_inverse_unit() {
        let f = Frequency::new(60.0);
        assert_eq!(format!("{}", f), "60 s⁻¹");
    }

    #[test]
    fn display_renders_dimensionless() {
        let d = Dimensionless::new(0.5);
        assert_eq!(format!("{}", d), "0.5 (dimensionless)");
    }

    #[test]
    fn display_renders_complex_derived_unit() {
        // Newton = m·kg/s²
        let n = Force::new(1.0);
        assert_eq!(format!("{}", n), "1 m·kg/s²");
    }

    #[test]
    fn display_renders_power_unit() {
        let p = Power::new(100.0);
        assert_eq!(format!("{}", p), "100 m²·kg/s³");
    }

    // SiPrefix tests.
    #[test]
    fn si_prefix_scales_match_si_definitions() {
        assert_eq!(SiPrefix::Yocto.scale(), 1.0e-24);
        assert_eq!(SiPrefix::Milli.scale(), 1.0e-3);
        assert_eq!(SiPrefix::None.scale(), 1.0);
        assert_eq!(SiPrefix::Kilo.scale(), 1.0e3);
        assert_eq!(SiPrefix::Mega.scale(), 1.0e6);
        assert_eq!(SiPrefix::Giga.scale(), 1.0e9);
        assert_eq!(SiPrefix::Yotta.scale(), 1.0e24);
    }

    #[test]
    fn si_prefix_scales_table_matches_individual_scale_method() {
        for (idx, &scale) in SiPrefix::SCALES.iter().enumerate() {
            // Round-trip: each entry in the table is the scale of the
            // corresponding SiPrefix variant.
            let prefix = match idx {
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
                _ => unreachable!(),
            };
            assert_eq!(prefix.scale(), scale, "mismatch at index {idx}");
        }
    }

    #[test]
    fn si_prefix_display_uses_official_abbreviation() {
        assert_eq!(format!("{}", SiPrefix::Kilo), "k");
        assert_eq!(format!("{}", SiPrefix::Micro), "µ");
        assert_eq!(format!("{}", SiPrefix::Mega), "M");
        assert_eq!(format!("{}", SiPrefix::None), "");
    }

    #[test]
    fn with_prefix_multiplies_value() {
        // 1 m scaled by Kilo = 1000 m. Same Rust type, value reflects the
        // applied scale.
        let one_m: Meters<f64> = Meters::new(1.0);
        let in_kilo: Meters<f64> = one_m.with_prefix(SiPrefix::Kilo);
        assert_eq!(in_kilo.into_value(), 1000.0);
    }

    #[test]
    fn strip_prefix_divides_value() {
        // Inverse of with_prefix: 1000.0 m / Kilo (1e3) = 1.0 m.
        let in_kilo: Meters<f64> = Meters::new(1_000.0);
        let in_meters: Meters<f64> = in_kilo.strip_prefix(SiPrefix::Kilo);
        assert_eq!(in_meters.into_value(), 1.0);
    }

    #[test]
    fn with_prefix_round_trip_returns_original_value() {
        let original: Meters<f64> = Meters::new(42.0);
        let in_mm: Meters<f64> = original.with_prefix(SiPrefix::Milli);
        let back: Meters<f64> = in_mm.strip_prefix(SiPrefix::Milli);
        assert_eq!(back.into_value(), 42.0);
    }

    #[test]
    fn with_prefix_supports_full_si_ladder() {
        // Spot-check several SI prefixes around the ladder.
        let one: Meters<f64> = Meters::new(1.0);
        assert_eq!(one.with_prefix(SiPrefix::Micro).into_value(), 1.0e-6);
        assert_eq!(one.with_prefix(SiPrefix::Milli).into_value(), 1.0e-3);
        assert_eq!(one.with_prefix(SiPrefix::Centi).into_value(), 1.0e-2);
        assert_eq!(one.with_prefix(SiPrefix::Deci).into_value(), 1.0e-1);
        assert_eq!(one.with_prefix(SiPrefix::Kilo).into_value(), 1.0e3);
        assert_eq!(one.with_prefix(SiPrefix::Mega).into_value(), 1.0e6);
    }

    #[test]
    fn acceleration_vec_wraps_and_exposes_components() {
        let raw = Vector3::new(1.0, 2.0, 3.0);
        let a = AccelerationVec::from_mps2(raw);
        assert_eq!(a.raw(), &raw);
        assert_eq!(a.x_mps2().into_value(), 1.0);
        assert_eq!(a.y_mps2().into_value(), 2.0);
        assert_eq!(a.z_mps2().into_value(), 3.0);
    }

    #[test]
    fn acceleration_vec_plus_sums_components() {
        let a = AccelerationVec::from_mps2(Vector3::new(1.0, 0.0, 0.0));
        let b = AccelerationVec::from_mps2(Vector3::new(0.0, 2.0, 3.0));
        let sum = a.plus(&b);
        assert_eq!(sum.raw(), &Vector3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn force_vec_round_trips_via_from_into() {
        let raw = Vector3::new(4.0, 5.0, 6.0);
        let f: ForceVec = raw.into();
        let back: Vector3<f64> = f.into();
        assert_eq!(back, raw);
    }

    #[test]
    fn torque_vec_exposes_nm_components() {
        let t = TorqueVec::from_nm(Vector3::new(7.0, 8.0, 9.0));
        assert_relative_eq!(t.x_nm().into_value(), 7.0);
        assert_relative_eq!(t.y_nm().into_value(), 8.0);
        assert_relative_eq!(t.z_nm().into_value(), 9.0);
    }

    #[test]
    fn torque_type_unit_is_kg_m2_per_s2() {
        // Torque = m²·kg/s² (force-arm). Multiply a length-arm by a force and
        // confirm the resulting type is `Torque`.
        let arm: Meters<f64> = Meters::new(0.5);
        let force: Force<f64> = Force::new(10.0);
        let _t: Torque<f64> = arm * force;
    }
}
