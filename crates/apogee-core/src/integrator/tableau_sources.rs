//! Test-only helpers: parse Runge-Kutta coefficients from the reference
//! Fortran sources committed in the repo-root `sources/` directory, and
//! shared convergence-order machinery for the integrator unit tests.
//!
//! Provenance (see `sources/README.md`): `dopri5.f` and `dop853.f` are
//! E. Hairer & G. Wanner's official DOPRI5 / DOP853 implementations,
//! retrieved 2026-10-22 from https://www.unige.ch/~hairer/software.html.
//! The integrator unit tests parse them to cross-check every coefficient
//! constant against the primary source at test time.

use std::collections::HashMap;
use std::path::PathBuf;

use apogee_common::units::Seconds;

use super::{Integrator, StateDerivative, StateVector};

/// Harmonic-oscillator derivative: x'' = -x (analytic cos/sin).
pub(crate) fn harmonic_derivative(s: &StateVector) -> StateDerivative {
    StateDerivative {
        velocity: s.velocity,
        acceleration: apogee_common::Position::new(-s.position.x, 0.0, 0.0),
        attitude_derivative: nalgebra::Quaternion::new(0.0, 0.0, 0.0, 0.0),
        angular_acceleration: nalgebra::Vector3::zeros(),
    }
}

/// Integrate x(0) = 1, x'(0) = 0 for `total` seconds in fixed steps of `h`;
/// return the endpoint position error against cos(total).
pub(crate) fn harmonic_endpoint_error(integ: &mut dyn Integrator, h: f64, total: f64) -> f64 {
    let mut state = StateVector {
        position: apogee_common::Position::new(1.0, 0.0, 0.0),
        velocity: apogee_common::Velocity::new(0.0, 0.0, 0.0),
        attitude: nalgebra::Quaternion::identity(),
        angular_velocity: nalgebra::Vector3::zeros(),
    };
    let n = (total / h).round() as usize;
    for _ in 0..n {
        integ.step(&mut state, &harmonic_derivative, Seconds::new(h));
    }
    (state.position.x - total.cos()).abs()
}

/// Assert that halving the step shrinks the endpoint error by ~2^order.
pub(crate) fn assert_convergence_order(
    integ: impl Fn() -> Box<dyn Integrator>,
    h: f64,
    order: u32,
    total: f64,
) {
    let e_h = harmonic_endpoint_error(integ().as_mut(), h, total);
    let e_h2 = harmonic_endpoint_error(integ().as_mut(), h / 2.0, total);
    let ratio = e_h / e_h2;
    let lo = 2.0_f64.powf(order as f64 - 0.5);
    let hi = 2.0_f64.powf(order as f64 + 0.5);
    assert!(
        ratio > lo && ratio < hi,
        "convergence order degraded: err({h})/err({}/2) = {ratio}, expected ~2^{order} (window [{lo}, {hi}])",
        h
    );
}

/// Absolute path of a reference source file in the repo-root `sources/`.
pub(crate) fn source_path(file: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../sources")
        .join(file);
    assert!(
        path.exists(),
        "reference source missing: {} — see sources/README.md",
        path.display()
    );
    path
}

/// Parse all `name = number` (and `name = num / den`) entries from one of
/// Hairer's Fortran sources into a lowercase-name -> value map.
///
/// Handles the file's actual quirks: fixed-column comment lines (`C ...`),
/// `&` continuation prefixes, Fortran `D` exponents, padded single-digit
/// error names (`er 1 = ...`), and trailing `,` or `)` separators.
pub(crate) fn fortran_params(file: &str) -> HashMap<String, f64> {
    let text = std::fs::read_to_string(source_path(file)).expect("read reference source");
    // Hairer pads single-digit error-weight names with a space.
    let text = (1..=9).fold(text, |t, i| {
        t.replace(&format!("er {i}"), &format!("er{i}"))
    });

    // Join continuation lines and drop column-1 comment lines.
    let mut logical = String::new();
    for line in text.lines() {
        if line.starts_with(['C', 'c', '*', '!']) {
            continue;
        }
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('&') {
            logical.push_str(rest);
        } else {
            logical.push(' ');
            logical.push_str(trimmed);
        }
    }

    let mut map = HashMap::new();
    let mut tokens = logical.split_whitespace().peekable();
    // Spaced "NAME = VALUE" entries keep NAME and VALUE in separate
    // tokens; remember the last identifier seen before "=".
    let mut pending_name: Option<String> = None;
    while let Some(token) = tokens.next() {
        let Some((left, right)) = token.split_once('=') else {
            let t = token.trim();
            if t.starts_with(|c: char| c.is_ascii_alphabetic())
                && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                pending_name = Some(t.to_lowercase());
            }
            continue;
        };
        let name = {
            let l = left.trim();
            if l.is_empty() {
                pending_name.take().unwrap_or_default()
            } else {
                l.to_lowercase()
            }
        };
        let value = {
            let r = right.trim();
            if r.is_empty() {
                match tokens.next() {
                    Some(next) => next.trim(),
                    None => continue,
                }
            } else {
                r
            }
        };
        let value = value.trim_end_matches([',', ')']);
        let parsed = if let Some((num, den)) = value.split_once('/') {
            match (fortran_num(num), fortran_num(den)) {
                (Some(n), Some(d)) if d != 0.0 => Some(n / d),
                _ => None,
            }
        } else {
            fortran_num(value)
        };
        if let Some(v) = parsed {
            map.insert(name, v);
        }
    }
    map
}

/// Parse one Fortran real literal (`0.2D0`, `5.26...D-02`) into an f64.
fn fortran_num(literal: &str) -> Option<f64> {
    literal.trim().replace(['D', 'd'], "E").parse::<f64>().ok()
}
