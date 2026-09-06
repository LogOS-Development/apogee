//! Terrain elevation data layer.
//!
//! Loads USGS 3DEP GeoTIFF tiles into a regular elevation grid.
//! Provides bilinear interpolation, slope, and aspect computation.
//! Source-agnostic — any GeoTIFF DEM works, 3DEP is the first provider.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TerrainError {
    #[error("GeoTIFF read error: {0}")]
    TiffRead(String),
    #[error("invalid geotransform: {0}")]
    BadGeoTransform(String),
    #[error("no elevation data")]
    NoData,
}

/// Affine geotransform mapping pixel (col, row) → world (x, y).
///
/// GT(0) x-coordinate of top-left pixel
/// GT(1) horizontal pixel size (meters for UTM, degrees for geographic)
/// GT(2) rotation term (0 for north-up)
/// GT(3) y-coordinate of top-left pixel
/// GT(4) rotation term (0 for north-up)
/// GT(5) vertical pixel size (negative for north-up)
#[derive(Debug, Clone, PartialEq)]
pub struct GeoTransform {
    pub origin_x: f64,
    pub pixel_width: f64,
    pub origin_y: f64,
    pub pixel_height: f64,
}

impl GeoTransform {
    /// World coordinates of pixel center (col, row).
    pub fn pixel_to_world(&self, col: usize, row: usize) -> (f64, f64) {
        let x = self.origin_x + (col as f64 + 0.5) * self.pixel_width;
        let y = self.origin_y + (row as f64 + 0.5) * self.pixel_height;
        (x, y)
    }

    /// Nearest pixel (col, row) for world coordinates (x, y).
    pub fn world_to_pixel(&self, x: f64, y: f64) -> (usize, usize) {
        let col = ((x - self.origin_x) / self.pixel_width - 0.5).round() as usize;
        let row = ((y - self.origin_y) / self.pixel_height - 0.5).round() as usize;
        (col, row)
    }
}

/// Regular grid of elevation samples.
///
/// Stores heights as f32 in row-major order (row 0 = north).
/// Missing/no-data values are stored as `f32::NAN`.
#[derive(Debug, Clone)]
pub struct ElevationGrid {
    pub width: usize,
    pub height: usize,
    pub geotransform: GeoTransform,
    /// Row-major: elevations[row * width + col]
    pub elevations: Vec<f32>,
}

impl ElevationGrid {
    /// Create from raw components.
    pub fn new(
        width: usize,
        height: usize,
        geotransform: GeoTransform,
        elevations: Vec<f32>,
    ) -> Result<Self, TerrainError> {
        if elevations.len() != width * height {
            return Err(TerrainError::BadGeoTransform(format!(
                "elevation count {} != {}x{}",
                elevations.len(),
                width,
                height
            )));
        }
        Ok(Self {
            width,
            height,
            geotransform,
            elevations,
        })
    }

    /// Elevation at pixel (col, row). Returns NaN for out-of-bounds.
    pub fn at(&self, col: usize, row: usize) -> f32 {
        if col < self.width && row < self.height {
            self.elevations[row * self.width + col]
        } else {
            f32::NAN
        }
    }

    /// Bilinear interpolation at world coordinates (x, y).
    ///
    /// Interpolates between the 4 nearest pixels. Returns NaN if
    /// any contributing pixel is NaN or the point is outside the grid.
    pub fn interpolate(&self, x: f64, y: f64) -> f32 {
        // Continuous pixel coordinates (0-based, pixel centers at 0.5)
        let px = (x - self.geotransform.origin_x) / self.geotransform.pixel_width - 0.5;
        let py = (y - self.geotransform.origin_y) / self.geotransform.pixel_height - 0.5;

        if px < 0.0 || py < 0.0 {
            return f32::NAN;
        }

        let col0 = px.floor() as usize;
        let row0 = py.floor() as usize;
        let col1 = col0 + 1;
        let row1 = row0 + 1;

        if col1 >= self.width || row1 >= self.height {
            return f32::NAN;
        }

        let fx = px - col0 as f64;
        let fy = py - row0 as f64;

        let z00 = self.at(col0, row0);
        let z10 = self.at(col1, row0);
        let z01 = self.at(col0, row1);
        let z11 = self.at(col1, row1);

        if z00.is_nan() || z10.is_nan() || z01.is_nan() || z11.is_nan() {
            return f32::NAN;
        }

        let z0 = z00 as f64 * (1.0 - fx) + z10 as f64 * fx;
        let z1 = z01 as f64 * (1.0 - fx) + z11 as f64 * fx;
        (z0 * (1.0 - fy) + z1 * fy) as f32
    }

    /// Slope (radians from horizontal) at pixel (col, row) via Horn's method.
    ///
    /// Uses the 3x3 neighborhood with dz/dx and dz/dy from Horn's
    /// finite difference formula. Returns NaN at grid edges or if
    /// any neighbor is NaN.
    pub fn slope(&self, col: usize, row: usize) -> f32 {
        if col == 0 || col >= self.width - 1 || row == 0 || row >= self.height - 1 {
            return f32::NAN;
        }

        let dx = self.geotransform.pixel_width.abs() as f32;
        let dy = self.geotransform.pixel_height.abs() as f32;

        // Horn's method: weighted 3x3 finite difference
        let z = |c: usize, r: usize| self.at(c, r);
        let dz_dx = ((z(col + 1, row - 1) + 2.0 * z(col + 1, row) + z(col + 1, row + 1))
            - (z(col - 1, row - 1) + 2.0 * z(col - 1, row) + z(col - 1, row + 1)))
            / (8.0 * dx);
        let dz_dy = ((z(col - 1, row + 1) + 2.0 * z(col, row + 1) + z(col + 1, row + 1))
            - (z(col - 1, row - 1) + 2.0 * z(col, row - 1) + z(col + 1, row - 1)))
            / (8.0 * dy);

        let slope = (dz_dx * dz_dx + dz_dy * dz_dy).sqrt().atan();
        if slope.is_nan() {
            f32::NAN
        } else {
            slope
        }
    }

    /// Aspect (radians, 0 = north, clockwise) at pixel (col, row).
    ///
    /// Computed from the same dz/dx, dz/dy as slope via Horn's method.
    /// Returns NaN at grid edges or if slope is zero (flat).
    pub fn aspect(&self, col: usize, row: usize) -> f32 {
        if col == 0 || col >= self.width - 1 || row == 0 || row >= self.height - 1 {
            return f32::NAN;
        }

        let dx = self.geotransform.pixel_width.abs() as f32;
        let dy = self.geotransform.pixel_height.abs() as f32;

        let z = |c: usize, r: usize| self.at(c, r);
        let dz_dx = ((z(col + 1, row - 1) + 2.0 * z(col + 1, row) + z(col + 1, row + 1))
            - (z(col - 1, row - 1) + 2.0 * z(col - 1, row) + z(col - 1, row + 1)))
            / (8.0 * dx);
        let dz_dy = ((z(col - 1, row + 1) + 2.0 * z(col, row + 1) + z(col + 1, row + 1))
            - (z(col - 1, row - 1) + 2.0 * z(col, row - 1) + z(col + 1, row - 1)))
            / (8.0 * dy);

        if dz_dx == 0.0 && dz_dy == 0.0 {
            return f32::NAN;
        }

        // atan2(dz_dx, -dz_dy) gives radians clockwise from north
        let aspect = dz_dx.atan2(-dz_dy);
        if aspect < 0.0 {
            aspect + 2.0 * std::f32::consts::PI
        } else {
            aspect
        }
    }

    /// Elevation statistics over valid (non-NaN) pixels.
    pub fn stats(&self) -> ElevationStats {
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        let mut sum = 0.0_f64;
        let mut count = 0_usize;

        for &e in &self.elevations {
            if !e.is_nan() {
                min = min.min(e);
                max = max.max(e);
                sum += e as f64;
                count += 1;
            }
        }

        if count == 0 {
            ElevationStats {
                min: f32::NAN,
                max: f32::NAN,
                mean: f32::NAN,
                valid_pixels: 0,
            }
        } else {
            ElevationStats {
                min,
                max,
                mean: (sum / count as f64) as f32,
                valid_pixels: count,
            }
        }
    }

    /// Load from raw GeoTIFF bytes.
    ///
    /// Parses the TIFF structure and extracts elevation data from
    /// the first IFD. Supports 16-bit and 32-bit sample formats.
    /// The geotransform is derived from GeoTIFF ModelTiepointTag (33922)
    /// and ModelPixelScaleTag (33550) tags.
    pub fn from_geotiff_bytes(bytes: &[u8]) -> Result<Self, TerrainError> {
        Self::parse_geotiff(bytes)
    }

    /// Load from a GeoTIFF file.
    pub fn from_geotiff_file(path: &str) -> Result<Self, TerrainError> {
        let bytes =
            std::fs::read(path).map_err(|e| TerrainError::TiffRead(format!("file read: {e}")))?;
        Self::from_geotiff_bytes(&bytes)
    }

    /// Parse GeoTIFF: read TIFF IFD + extract GeoTIFF geotransform tags.
    fn parse_geotiff(bytes: &[u8]) -> Result<Self, TerrainError> {
        use tiff::decoder::{Decoder, DecodingResult};

        let mut decoder = Decoder::new(std::io::Cursor::new(bytes))
            .map_err(|e| TerrainError::TiffRead(format!("decoder init: {e}")))?;

        // Read GeoTIFF tags from the first IFD
        let tiepoint = decoder
            .get_tag_f64_vec(tiff::tags::Tag::ModelTiepointTag)
            .ok();
        let pixel_scale = decoder
            .get_tag_f64_vec(tiff::tags::Tag::ModelPixelScaleTag)
            .ok();

        let (width_u32, height_u32) = decoder
            .dimensions()
            .map_err(|e| TerrainError::TiffRead(format!("dimensions: {e}")))?;

        let width = width_u32 as usize;
        let height = height_u32 as usize;

        if width == 0 || height == 0 {
            return Err(TerrainError::NoData);
        }

        let result = decoder
            .read_image()
            .map_err(|e| TerrainError::TiffRead(format!("read_image: {e}")))?;

        let elevations = match result {
            DecodingResult::F32(v) => v,
            DecodingResult::F64(v) => v.iter().map(|&x| x as f32).collect(),
            DecodingResult::U16(v) => v.iter().map(|&x| x as f32).collect(),
            DecodingResult::U8(v) => v.iter().map(|&x| x as f32).collect(),
            DecodingResult::I16(v) => v.iter().map(|&x| x as f32).collect(),
            DecodingResult::U32(v) => v.iter().map(|&x| x as f32).collect(),
            DecodingResult::I32(v) => v.iter().map(|&x| x as f32).collect(),
            other => {
                return Err(TerrainError::TiffRead(format!(
                    "unsupported sample format: {other:?}"
                )))
            }
        };

        // Build geotransform from GeoTIFF tags
        let geotransform = match (&tiepoint, &pixel_scale) {
            (Some(tp), Some(ps)) if tp.len() >= 6 && ps.len() >= 2 => {
                // ModelTiepointTag: (i, j, k, x, y, z) — pixel (i,j) maps to world (x,y)
                // ModelPixelScaleTag: (scale_x, scale_y, scale_z)
                GeoTransform {
                    origin_x: tp[3] - tp[0] * ps[0],
                    pixel_width: ps[0],
                    origin_y: tp[4] - tp[1] * ps[1],
                    pixel_height: -ps[1], // TIFF rows go down, GeoTIFF y goes up
                }
            }
            _ => {
                // Fallback: assume 1-unit pixels with origin at (0, height)
                GeoTransform {
                    origin_x: 0.0,
                    pixel_width: 1.0,
                    origin_y: height as f64,
                    pixel_height: -1.0,
                }
            }
        };

        Self::new(width, height, geotransform, elevations)
    }
}

/// Summary statistics for an elevation grid.
#[derive(Debug, Clone, PartialEq)]
pub struct ElevationStats {
    pub min: f32,
    pub max: f32,
    pub mean: f32,
    pub valid_pixels: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_grid(width: usize, height: usize, value: f32) -> ElevationGrid {
        let gt = GeoTransform {
            origin_x: 0.0,
            pixel_width: 10.0,
            origin_y: (height as f64) * 10.0,
            pixel_height: -10.0,
        };
        ElevationGrid::new(width, height, gt, vec![value; width * height]).unwrap()
    }

    #[test]
    fn test_grid_construction_and_access() {
        let grid = flat_grid(4, 3, 100.0);
        assert_eq!(grid.width, 4);
        assert_eq!(grid.height, 3);
        assert_eq!(grid.at(0, 0), 100.0);
        assert_eq!(grid.at(3, 2), 100.0);
        assert!(grid.at(4, 0).is_nan()); // out of bounds
    }

    #[test]
    fn test_geotransform_pixel_to_world() {
        let gt = GeoTransform {
            origin_x: 500.0,
            pixel_width: 10.0,
            origin_y: 1000.0,
            pixel_height: -10.0,
        };
        let (x, y) = gt.pixel_to_world(0, 0);
        assert_eq!(x, 505.0); // origin + 0.5 * pixel_width
        assert_eq!(y, 995.0); // origin + 0.5 * pixel_height

        let (x, y) = gt.pixel_to_world(2, 3);
        assert_eq!(x, 525.0);
        assert_eq!(y, 965.0);
    }

    #[test]
    fn test_bilinear_interpolation_flat() {
        let grid = flat_grid(4, 4, 500.0);
        // Any point should return 500.0 on a flat grid
        let z = grid.interpolate(25.0, 20.0);
        assert!((z - 500.0).abs() < 0.01);
    }

    #[test]
    fn test_bilinear_interpolation_gradient() {
        // 2x2 grid with known values:
        // (0,0)=0  (1,0)=100
        // (0,1)=0  (1,1)=100
        let gt = GeoTransform {
            origin_x: 0.0,
            pixel_width: 10.0,
            origin_y: 20.0,
            pixel_height: -10.0,
        };
        let grid = ElevationGrid::new(2, 2, gt, vec![0.0, 100.0, 0.0, 100.0]).unwrap();

        // Center between the four pixels
        let z = grid.interpolate(10.0, 10.0);
        // At center: average of all four = 50.0
        assert!((z - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_bilinear_interpolation_out_of_bounds() {
        let grid = flat_grid(2, 2, 100.0);
        assert!(grid.interpolate(-1.0, 10.0).is_nan());
        assert!(grid.interpolate(100.0, 10.0).is_nan());
    }

    #[test]
    fn test_slope_flat() {
        let grid = flat_grid(5, 5, 100.0);
        let s = grid.slope(2, 2);
        assert!(s.abs() < 0.001); // flat → slope ≈ 0
    }

    #[test]
    fn test_slope_tilted() {
        // 5x5 grid with linear ramp in x: z = col * 10
        let gt = GeoTransform {
            origin_x: 0.0,
            pixel_width: 10.0,
            origin_y: 50.0,
            pixel_height: -10.0,
        };
        let elevations: Vec<f32> = (0..5)
            .flat_map(|_row| (0..5).map(|col| (col * 10) as f32))
            .collect();
        let grid = ElevationGrid::new(5, 5, gt, elevations).unwrap();

        let s = grid.slope(2, 2);
        // dz/dx = 1.0 (10m rise per 10m run), dz/dy = 0
        // slope = atan(1.0) = 45 degrees = pi/4
        assert!((s - std::f32::consts::FRAC_PI_4).abs() < 0.01);
    }

    #[test]
    fn test_aspect_north_facing() {
        // Ramp increasing to the south (positive y = south in geo coords)
        let gt = GeoTransform {
            origin_x: 0.0,
            pixel_width: 10.0,
            origin_y: 50.0,
            pixel_height: -10.0,
        };
        let elevations: Vec<f32> = (0..5)
            .flat_map(|row| (0..5).map(move |_col| (row * 10) as f32))
            .collect();
        let grid = ElevationGrid::new(5, 5, gt, elevations).unwrap();

        let a = grid.aspect(2, 2);
        // dz/dx = 0, dz/dy = 1.0 → aspect = atan2(0, -1) = pi (south)
        assert!((a - std::f32::consts::PI).abs() < 0.01);
    }

    #[test]
    fn test_stats() {
        let gt = GeoTransform {
            origin_x: 0.0,
            pixel_width: 1.0,
            origin_y: 3.0,
            pixel_height: -1.0,
        };
        let grid = ElevationGrid::new(3, 1, gt, vec![100.0, 200.0, 300.0]).unwrap();
        let s = grid.stats();
        assert_eq!(s.min, 100.0);
        assert_eq!(s.max, 300.0);
        assert!((s.mean - 200.0).abs() < 0.01);
        assert_eq!(s.valid_pixels, 3);
    }

    #[test]
    fn test_stats_with_nodata() {
        let gt = GeoTransform {
            origin_x: 0.0,
            pixel_width: 1.0,
            origin_y: 3.0,
            pixel_height: -1.0,
        };
        let grid = ElevationGrid::new(3, 1, gt, vec![100.0, f32::NAN, 300.0]).unwrap();
        let s = grid.stats();
        assert_eq!(s.min, 100.0);
        assert_eq!(s.max, 300.0);
        assert!((s.mean - 200.0).abs() < 0.01);
        assert_eq!(s.valid_pixels, 2);
    }

    #[test]
    fn test_elevation_count_mismatch() {
        let gt = GeoTransform {
            origin_x: 0.0,
            pixel_width: 1.0,
            origin_y: 2.0,
            pixel_height: -1.0,
        };
        let result = ElevationGrid::new(3, 2, gt, vec![1.0, 2.0, 3.0]);
        assert!(result.is_err());
    }

    #[test]
    fn test_geotiff_roundtrip() {
        use tiff::encoder::{colortype::Gray32Float, TiffEncoder};

        // 4x3 grid with a simple ramp: z = col + row * 10
        let width = 4usize;
        let height = 3usize;
        let elevations: Vec<f32> = (0..height)
            .flat_map(|row| (0..width).map(move |col| (col + row * 10) as f32))
            .collect();

        // Write a plain TIFF (no GeoTIFF tags — tests fallback geotransform)
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut buf).unwrap();
            encoder
                .write_image::<Gray32Float>(width as u32, height as u32, &elevations)
                .unwrap();
        }

        // Read it back
        let bytes = buf.into_inner();
        let grid = ElevationGrid::from_geotiff_bytes(&bytes).unwrap();

        assert_eq!(grid.width, width);
        assert_eq!(grid.height, height);

        // Fallback geotransform: 1m pixels, origin at (0, height)
        assert!((grid.geotransform.pixel_width - 1.0).abs() < 0.01);
        assert!((grid.geotransform.pixel_height - (-1.0)).abs() < 0.01);

        // Check elevation values survived the round-trip
        for row in 0..height {
            for col in 0..width {
                let expected = (col + row * 10) as f32;
                let actual = grid.at(col, row);
                assert!(
                    (actual - expected).abs() < 0.001,
                    "at ({col},{row}): expected {expected}, got {actual}"
                );
            }
        }
    }
}
