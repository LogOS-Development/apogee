//! Shared math utilities for the workspace.

#[inline]
pub fn modulo(value: f64, scale: f64) -> f64 {
    let mut v = value % scale;
    if v < 0.0 {
        v += scale;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn modulo_positive() {
        assert_relative_eq!(modulo(10.0, 3.0), 1.0);
    }

    #[test]
    fn modulo_negative() {
        assert_relative_eq!(modulo(-1.0, 360.0), 359.0);
    }

    #[test]
    fn modulo_zero() {
        assert_relative_eq!(modulo(0.0, 360.0), 0.0);
    }

    #[test]
    fn modulo_exact_multiple() {
        assert_relative_eq!(modulo(360.0, 360.0), 0.0);
    }

    #[test]
    fn modulo_large_negative() {
        assert_relative_eq!(modulo(-720.5, 360.0), 359.5);
    }
}