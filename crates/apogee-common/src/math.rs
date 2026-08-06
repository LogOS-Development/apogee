//! Shared math utilities for the workspace.

#[inline]
pub fn modulo(value: f64, scale: f64) -> f64 {
    let mut v = value % scale;
    if v < 0.0 {
        v += scale;
    }
    v
}