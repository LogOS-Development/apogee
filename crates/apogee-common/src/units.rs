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
//! ```

use std::fmt;
use std::marker::PhantomData;
use std::ops::{Add, Div, Mul, Neg, Sub};

use typenum::consts::*;
use typenum::{Diff, Sum, Z0};

/// A unit is a 7-tuple of type-level signed integer exponents over the SI
/// base units, in order: `[m, kg, s, A, K, mol, cd]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Unit<T>(PhantomData<T>);

/// Convenience: base units.
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
pub type Pressure<T> = Quantity<T, Unit<(N1, P1, N2, Z0, Z0, Z0, Z0)>>;
pub type Energy<T> = Quantity<T, Unit<(P2, P1, N2, Z0, Z0, Z0, Z0)>>;
pub type Power<T> = Quantity<T, Unit<(P2, P1, N3, Z0, Z0, Z0, Z0)>>;
pub type Area<T> = Quantity<T, Unit<(P2, Z0, Z0, Z0, Z0, Z0, Z0)>>;
pub type Volume<T> = Quantity<T, Unit<(P3, Z0, Z0, Z0, Z0, Z0, Z0)>>;
pub type Density<T> = Quantity<T, Unit<(N3, P1, Z0, Z0, Z0, Z0, Z0)>>;
pub type Frequency<T> = Quantity<T, Unit<(Z0, Z0, N1, Z0, Z0, Z0, Z0)>>;
pub type ElectricCharge<T> = Quantity<T, Unit<(Z0, Z0, P1, P1, Z0, Z0, Z0)>>;
pub type Voltage<T> = Quantity<T, Unit<(P2, P1, N3, N1, Z0, Z0, Z0)>>;

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
}
